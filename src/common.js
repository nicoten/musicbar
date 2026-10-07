export const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

export const fmt = (secs) => `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, "0")}`;

/** Calls `render` with the current state now and on every change. */
export async function onState(render) {
  await listen("state", (e) => render(e.payload));
  render(await invoke("get_state"));
}
