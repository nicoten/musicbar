export const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

export const fmt = (secs) => `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, "0")}`;

/** Calls `render` with the current state now and on every change. */
export async function onState(render) {
  await listen("state", (e) => render(e.payload));
  render(await invoke("get_state"));
}

const PRAISE = ["Nailed it!", "Nice one!", "Spot on!", "Well played!", "Bravo!", "Clean!", "That's it!"];

/** A congratulation that stays the same while one solved challenge is on screen. */
export function praiser() {
  let current = null;
  return (s) => {
    if (!s.solved) return (current = null);
    current ??= PRAISE[Math.floor(Math.random() * PRAISE.length)];
    return current;
  };
}
