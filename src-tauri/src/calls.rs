//! "Am I in a call?" heuristic: any camera is in use, or a mic is in use while a meeting app is running.
//! (Mic use alone is ignored: music apps listen too. Our own guitar listening never counts.)

pub fn in_call() -> bool {
    #[cfg(target_os = "macos")]
    return macos::camera_in_use() || (macos::mic_in_use() && macos::meeting_app_running());
    #[cfg(not(target_os = "macos"))]
    false
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;
    use std::ptr;

    /// Process names (as reported by `proc_name`, truncated to 32 bytes) of native meeting apps.
    const MEETING_APPS: &[&str] = &["zoom.us", "MSTeams", "Microsoft Teams", "Webex", "FaceTime"];

    const fn fourcc(s: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*s)
    }

    const SYSTEM_OBJECT: u32 = 1;
    const SCOPE_GLOBAL: u32 = fourcc(b"glob");
    const SCOPE_INPUT: u32 = fourcc(b"inpt");
    const ELEMENT_MAIN: u32 = 0;
    const DEVICES: u32 = fourcc(b"dev#");
    const STREAMS: u32 = fourcc(b"stm#");
    const IS_RUNNING_SOMEWHERE: u32 = fourcc(b"gone");
    const PROCESSES: u32 = fourcc(b"prs#");
    const PROCESS_PID: u32 = fourcc(b"ppid");
    const PROCESS_IS_RUNNING_INPUT: u32 = fourcc(b"piri");

    /// Shared layout of `CMIOObjectPropertyAddress` and `AudioObjectPropertyAddress`.
    #[repr(C)]
    struct Address {
        selector: u32,
        scope: u32,
        element: u32,
    }

    type GetSize = unsafe extern "C" fn(u32, *const Address, u32, *const c_void, *mut u32) -> i32;
    type GetData = unsafe extern "C" fn(u32, *const Address, u32, *const c_void, u32, *mut u32, *mut c_void) -> i32;

    #[link(name = "CoreMediaIO", kind = "framework")]
    extern "C" {
        fn CMIOObjectGetPropertyDataSize(id: u32, a: *const Address, qs: u32, q: *const c_void, size: *mut u32) -> i32;
        fn CMIOObjectGetPropertyData(id: u32, a: *const Address, qs: u32, q: *const c_void, size: u32, used: *mut u32, out: *mut c_void) -> i32;
    }

    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        fn AudioObjectGetPropertyDataSize(id: u32, a: *const Address, qs: u32, q: *const c_void, size: *mut u32) -> i32;
        fn AudioObjectGetPropertyData(id: u32, a: *const Address, qs: u32, q: *const c_void, size: *mut u32, out: *mut c_void) -> i32;
    }

    /// Adapts CoreAudio's in/out size parameter to CoreMediaIO's (size, used) shape.
    unsafe extern "C" fn audio_get_data(id: u32, a: *const Address, qs: u32, q: *const c_void, size: u32, used: *mut u32, out: *mut c_void) -> i32 {
        *used = size;
        AudioObjectGetPropertyData(id, a, qs, q, used, out)
    }

    extern "C" {
        fn proc_listallpids(buf: *mut c_void, size: i32) -> i32;
        fn proc_name(pid: i32, buf: *mut c_void, size: u32) -> i32;
    }

    fn addr(selector: u32, scope: u32) -> Address {
        Address { selector, scope, element: ELEMENT_MAIN }
    }

    fn size_of(get_size: GetSize, id: u32, a: &Address) -> Option<u32> {
        let mut size = 0u32;
        (unsafe { get_size(id, a, 0, ptr::null(), &mut size) } == 0).then_some(size)
    }

    /// A system-wide list of object IDs (devices, processes...), or `None` if unsupported.
    fn objects(get_size: GetSize, get_data: GetData, selector: u32) -> Option<Vec<u32>> {
        let a = addr(selector, SCOPE_GLOBAL);
        let size = size_of(get_size, SYSTEM_OBJECT, &a)?;
        let mut ids = vec![0u32; size as usize / 4];
        let mut used = 0u32;
        if unsafe { get_data(SYSTEM_OBJECT, &a, 0, ptr::null(), size, &mut used, ids.as_mut_ptr() as *mut c_void) } != 0 {
            return None;
        }
        ids.truncate(used as usize / 4);
        Some(ids)
    }

    fn devices(get_size: GetSize, get_data: GetData) -> Vec<u32> {
        objects(get_size, get_data, DEVICES).unwrap_or_default()
    }

    fn u32_prop(get_data: GetData, id: u32, selector: u32) -> Option<u32> {
        let mut value = 0u32;
        let mut used = 0u32;
        let ok = unsafe { get_data(id, &addr(selector, SCOPE_GLOBAL), 0, ptr::null(), 4, &mut used, &mut value as *mut u32 as *mut c_void) };
        (ok == 0).then_some(value)
    }

    fn running_somewhere(get_data: GetData, dev: u32) -> bool {
        u32_prop(get_data, dev, IS_RUNNING_SOMEWHERE).is_some_and(|v| v != 0)
    }

    pub fn camera_in_use() -> bool {
        devices(CMIOObjectGetPropertyDataSize, CMIOObjectGetPropertyData)
            .into_iter()
            .any(|dev| running_somewhere(CMIOObjectGetPropertyData, dev))
    }

    /// Mic in use by some process other than this one.
    pub fn mic_in_use() -> bool {
        other_process_recording().unwrap_or_else(any_input_running)
    }

    /// Per-process input state (macOS 14+), so our own listening can be told apart.
    pub(super) fn other_process_recording() -> Option<bool> {
        let me = std::process::id();
        let processes = objects(AudioObjectGetPropertyDataSize, audio_get_data, PROCESSES)?;
        Some(processes.into_iter().any(|p| {
            u32_prop(audio_get_data, p, PROCESS_IS_RUNNING_INPUT).is_some_and(|v| v != 0)
                && u32_prop(audio_get_data, p, PROCESS_PID) != Some(me)
        }))
    }

    /// Older systems: any input device running, which includes our own listening.
    pub(super) fn any_input_running() -> bool {
        devices(AudioObjectGetPropertyDataSize, audio_get_data).into_iter().any(|dev| {
            let has_input = size_of(AudioObjectGetPropertyDataSize, dev, &addr(STREAMS, SCOPE_INPUT)).is_some_and(|s| s > 0);
            has_input && running_somewhere(audio_get_data, dev)
        })
    }

    pub fn meeting_app_running() -> bool {
        let count = unsafe { proc_listallpids(ptr::null_mut(), 0) };
        if count <= 0 {
            return false;
        }
        let mut pids = vec![0i32; count as usize + 64];
        let n = unsafe { proc_listallpids(pids.as_mut_ptr() as *mut c_void, (pids.len() * 4) as i32) };
        pids.truncate(n.max(0) as usize);
        let mut buf = [0u8; 64];
        pids.into_iter().any(|pid| {
            let len = unsafe { proc_name(pid, buf.as_mut_ptr() as *mut c_void, buf.len() as u32) };
            len > 0 && std::str::from_utf8(&buf[..len as usize]).is_ok_and(|name| MEETING_APPS.contains(&name))
        })
    }
}

#[cfg(test)]
mod tests {
    /// Smoke test against the real system; prints rather than asserts since it depends on what's running.
    #[test]
    fn probe_runs() {
        #[cfg(target_os = "macos")]
        println!(
            "camera: {}, mic (other processes): {:?}, mic (any): {}, meeting app: {}",
            super::macos::camera_in_use(),
            super::macos::other_process_recording(),
            super::macos::any_input_running(),
            super::macos::meeting_app_running()
        );
        println!("in call: {}", super::in_call());
    }
}
