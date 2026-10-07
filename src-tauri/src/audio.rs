use crate::pitch::{Analyzer, AudioEvent};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, SampleFormat, SizedSample, Stream, StreamConfig, StreamError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn name(device: &Device) -> Option<String> {
    device.description().ok().map(|d| d.name().to_string())
}

pub fn list_inputs() -> Vec<String> {
    let Ok(devices) = cpal::default_host().input_devices() else { return vec![] };
    devices.filter_map(|d| name(&d)).collect()
}

/// `None` = the system default input.
fn find(wanted: Option<&str>) -> Option<Device> {
    let host = cpal::default_host();
    match wanted {
        None => host.default_input_device(),
        Some(wanted) => host.input_devices().ok()?.find(|d| name(d).as_deref() == Some(wanted)),
    }
}

struct Listening {
    device: String,
    _stream: Stream,
    failed: Arc<AtomicBool>,
}

fn on_error(failed: &Arc<AtomicBool>) -> impl FnMut(StreamError) + Send + 'static {
    let failed = failed.clone();
    move |_| failed.store(true, Ordering::Relaxed)
}

/// Mixes the channels down to mono and hands blocks to the analysis thread.
fn build<T>(device: &Device, config: &StreamConfig, tx: SyncSender<Vec<f32>>, failed: &Arc<AtomicBool>) -> Option<Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    let on_data = move |data: &[T], _: &cpal::InputCallbackInfo| {
        let mono = data.chunks(channels).map(|frame| frame.iter().map(|&s| s.to_sample::<f32>()).sum()).collect();
        // Drop audio rather than ever block the audio thread.
        let _ = tx.try_send(mono);
    };
    device.build_input_stream(config, on_data, on_error(failed), None).ok()
}

fn listen(device: Device, on_event: Arc<dyn Fn(AudioEvent) + Send + Sync>) -> Option<Listening> {
    let supported = device.default_input_config().ok()?;
    let config = supported.config();
    let (tx, rx) = sync_channel::<Vec<f32>>(64);
    let failed = Arc::new(AtomicBool::new(false));
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, &config, tx, &failed),
        SampleFormat::I16 => build::<i16>(&device, &config, tx, &failed),
        SampleFormat::I32 => build::<i32>(&device, &config, tx, &failed),
        SampleFormat::U16 => build::<u16>(&device, &config, tx, &failed),
        _ => None,
    }?;
    stream.play().ok()?;
    let sample_rate = config.sample_rate;
    // Ends when the stream (and with it the sender) is dropped.
    thread::spawn(move || {
        let mut analyzer = Analyzer::new(sample_rate);
        let mut emit = |ev| on_event(ev);
        for block in rx {
            analyzer.push(&block, &mut emit);
        }
        analyzer.flush(&mut emit);
    });
    Some(Listening { device: name(&device)?, _stream: stream, failed })
}

/// Keeps listening to the configured input, reopening it when it changes, fails or comes back.
/// `wanted` returns `None` to not listen at all, `Some(None)` for the system default input,
/// or `Some(Some(name))` for a specific one.
pub fn spawn(
    wanted: impl Fn() -> Option<Option<String>> + Send + 'static,
    on_event: impl Fn(AudioEvent) + Send + Sync + 'static,
) {
    let on_event: Arc<dyn Fn(AudioEvent) + Send + Sync> = Arc::new(on_event);
    thread::spawn(move || {
        let mut current: Option<(Option<String>, Listening)> = None;
        loop {
            let want = wanted();
            let device = want.as_ref().and_then(|w| find(w.as_deref()));
            let device_name = device.as_ref().and_then(name);
            let stale = current.as_ref().is_none_or(|(w, l)| {
                Some(w) != want.as_ref() || l.failed.load(Ordering::Relaxed) || Some(&l.device) != device_name.as_ref()
            });
            if stale {
                current = None;
                if let (Some(want), Some(device)) = (want, device) {
                    current = listen(device, on_event.clone()).map(|l| (want, l));
                }
            }
            thread::sleep(Duration::from_secs(2));
        }
    });
}
