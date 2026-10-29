const { listen } = window.__TAURI__.event;

const NAMES = ["C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"];
/** Within this many cents counts as in tune. */
const IN_TUNE = 5;
/** Keep showing the last reading this long after the note stops. */
const HOLD_MS = 1500;
/** How much each new reading moves the smoothed pitch (0–1). */
const SMOOTH = 0.35;

/** Shows the note being played and how many cents sharp or flat it is. */
export function mountTuner(el) {
  const note = el.querySelector(".tuner-note");
  const needle = el.querySelector("#tuner-needle");
  const cents = el.querySelector(".tuner-cents");
  let midi = null;
  let idleTimer = null;

  const idle = () => {
    midi = null;
    el.classList.add("idle");
    el.classList.remove("in-tune");
    note.textContent = "–";
    cents.textContent = "";
  };
  idle();

  listen("tuner", ({ payload: hz }) => {
    if (hz == null) {
      clearTimeout(idleTimer);
      idleTimer = setTimeout(idle, HOLD_MS);
      return;
    }
    clearTimeout(idleTimer);
    const m = 69 + 12 * Math.log2(hz / 440);
    // Snap to a new note right away; only smooth wobble around the same one.
    midi = midi != null && Math.abs(m - midi) < 0.5 ? midi + SMOOTH * (m - midi) : m;
    const nearest = Math.round(midi);
    const off = Math.round((midi - nearest) * 100);
    el.classList.remove("idle");
    el.classList.toggle("in-tune", Math.abs(off) <= IN_TUNE);
    note.innerHTML = `${NAMES[((nearest % 12) + 12) % 12]}<sub>${Math.floor(nearest / 12) - 1}</sub>`;
    cents.textContent = off === 0 ? "0¢" : `${off > 0 ? "+" : "−"}${Math.abs(off)}¢`;
    needle.style.left = `${50 + off}%`;
  });
}
