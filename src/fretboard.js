import { invoke } from "./common.js";

/** Standard tuning, high E first (top row, like tab). */
const STRINGS = [64, 59, 55, 50, 45, 40];
const FRETS = 15;
const MARKERS = new Set([3, 5, 7, 9, 15]);
const NAMES_KEY = "fretboard-names";
const ACCIDENTALS = { "-2": "𝄫", "-1": "♭", 1: "♯", 2: "𝄪" };
const SVG = "http://www.w3.org/2000/svg";
// Staff geometry, in diatonic steps (C0 = 0). Guitar reads treble clef an octave up, so E2..G5
// sounding is written E3..G6.
const STEP = 5;
const BOTTOM_LINE = 30; // E4
const TOP_LINE = 38; // F5
const LOWEST = 23;
const HIGHEST = 46;
const y = (step) => 8 + (HIGHEST - step) * STEP;
const NATURAL_PCS = [0, 2, 4, 5, 7, 9, 11];
const midi = (n) => (n.octave + 1) * 12 + NATURAL_PCS[n.letter] + n.alter;

/** Builds a clickable fretboard in `el`; returns a function that renders the app state onto it. */
export function mountFretboard(el) {
  el.classList.add("fretboard-wrap");
  /** Held notes -> the button they were clicked on, so only that spot lights up. */
  const picked = new Map();
  let trail = [];
  /** Buttons clicked during this challenge, oldest first. */
  let clicks = [];
  let challenge = null;
  /** Pitch-class names for the current challenge, from the app state (D♯ in B major, E♭ in A♭). */
  let names = [];
  const noteName = (note) => {
    const { name, alter } = names[note % 12];
    return `${name}${Math.floor((note - alter) / 12) - 1}`;
  };
  const board = document.createElement("div");
  board.className = "fretboard";
  for (const [s, open] of STRINGS.entries()) {
    for (let fret = 0; fret <= FRETS; fret++) {
      const note = open + fret;
      const b = document.createElement("button");
      b.type = "button";
      b.className = fret === 0 ? "fret open" : "fret";
      b.dataset.note = note;
      b.innerHTML = "<span></span>";
      b.style.gridRow = s + 1;
      b.style.gridColumn = fret + 1;
      b.onclick = () => {
        picked.set(note, b);
        trail = [...trail.slice(-7), noteName(note)];
        clicks = [...clicks.slice(-15), b];
        b.classList.remove("flash");
        void b.offsetWidth;
        b.classList.add("flash");
        invoke("fret_click", { string: s, note });
      };
      board.append(b);
    }
  }
  // Inlay dots sit between the strings: one in the middle, two at the 12th.
  const inlay = (fret, rows) => {
    const dot = document.createElement("div");
    dot.className = "inlay";
    dot.style.gridColumn = fret + 1;
    dot.style.gridRow = rows;
    board.prepend(dot);
  };
  for (const fret of MARKERS) inlay(fret, "3 / 5");
  inlay(12, "2 / 4");
  inlay(12, "4 / 6");

  const tools = document.createElement("div");
  tools.className = "fret-tools";
  const hint = document.createElement("span");
  const played = document.createElement("span");
  played.className = "fret-trail";
  const showNames = document.createElement("label");
  showNames.innerHTML = `<input type="checkbox" /> Note names`;
  const box = showNames.querySelector("input");
  box.checked = load(NAMES_KEY) === "1";
  el.classList.toggle("show-names", box.checked);
  box.onchange = () => {
    el.classList.toggle("show-names", box.checked);
    save(NAMES_KEY, box.checked ? "1" : "0");
  };
  const clear = document.createElement("button");
  clear.type = "button";
  clear.textContent = "Clear";
  clear.onclick = () => invoke("fret_clear");
  tools.append(hint, played, showNames, clear);
  const staff = document.createElementNS(SVG, "svg");
  staff.classList.add("staff");
  staff.setAttribute("viewBox", `0 0 600 ${y(LOWEST) + 10}`);
  el.append(board, tools, staff);

  return (s) => {
    if (s.short !== challenge) {
      challenge = s.short;
      trail = [];
      clicks = [];
    }
    names = s.names;
    const held = new Set(s.held_notes);
    for (const note of picked.keys()) if (!held.has(note)) picked.delete(note);
    // Once solved, keep the clicked run (scale or interval) lit where it was played.
    const answer = s.solved && s.category !== "Chord" ? s.progress.map(midi) : [];
    const run = clicks.slice(-answer.length);
    const shown = answer.length && run.every((b, i) => Number(b.dataset.note) === answer[i]) ? new Set(run) : new Set();
    for (const b of board.querySelectorAll(".fret")) {
      const note = Number(b.dataset.note);
      b.classList.toggle("on", (held.has(note) && (picked.get(note) ?? b) === b) || shown.has(b));
      b.firstChild.textContent = names[note % 12].name;
    }
    played.textContent = trail.length && s.category !== "Scale" ? `Clicked: ${trail.join(" ")}` : "";
    drawStaff(staff, s.progress, s.category === "Chord");
    const chord = s.category === "Chord";
    hint.textContent = chord
      ? "Click each chord tone to hold it, one per string; click again to let go."
      : s.category === "Scale"
        ? "Click the notes in order, root to octave."
        : "Click the root, then the note above.";
    clear.hidden = !chord;
  };
}

/** Draws the notes played correctly so far, in a row or (chords) stacked; nothing that's still to come. */
function drawStaff(svg, notes, stacked) {
  const key = JSON.stringify([notes, stacked]);
  if (svg.dataset.key === key) return;
  const before = JSON.parse(svg.dataset.key || "[[]]")[0].map((n) => JSON.stringify(n));
  svg.dataset.key = key;
  svg.replaceChildren();
  const add = (tag, attrs, text) => {
    const node = document.createElementNS(SVG, tag);
    for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, v);
    if (text) node.textContent = text;
    svg.append(node);
    return node;
  };
  for (let step = BOTTOM_LINE; step <= TOP_LINE; step += 2) add("line", { x1: 4, x2: 596, y1: y(step), y2: y(step), class: "staff-line" });
  add("text", { x: 6, y: y(BOTTOM_LINE) + 10, class: "clef" }, "𝄞");
  add("text", { x: 20, y: y(BOTTOM_LINE) + 24, class: "clef-8" }, "8");
  let prevStep = null;
  let seconds = 0;
  notes.forEach((n, i) => {
    const step = (n.octave + 1) * 7 + n.letter;
    let x = stacked ? 150 : 90 + i * 66;
    // In a chord, a note a step above the one below it sits on the other side of the stem.
    if (stacked && prevStep !== null && step - prevStep === 1 && seconds++ % 2 === 0) x += 13;
    else if (stacked) seconds = 0;
    prevStep = step;
    const g = document.createElementNS(SVG, "g");
    if (!before.includes(JSON.stringify(n))) g.classList.add("new");
    svg.append(g);
    const into = (tag, attrs, text) => g.append(add(tag, attrs, text));
    for (let l = BOTTOM_LINE - 2; l >= step; l -= 2) into("line", { x1: x - 11, x2: x + 11, y1: y(l), y2: y(l), class: "staff-line" });
    for (let l = TOP_LINE + 2; l <= step; l += 2) into("line", { x1: x - 11, x2: x + 11, y1: y(l), y2: y(l), class: "staff-line" });
    into("ellipse", { cx: x, cy: y(step), rx: 6.5, ry: 4.6, transform: `rotate(-20 ${x} ${y(step)})`, class: "note-head" });
    // Stagger stacked accidentals so they don't print on top of each other.
    const accX = stacked ? 150 - 11 - (i % 2) * 11 : x - 11;
    if (n.alter) into("text", { x: accX, y: y(step) + 5, class: "accidental" }, ACCIDENTALS[n.alter]);
  });
}

function load(key) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function save(key, value) {
  try {
    localStorage.setItem(key, value);
  } catch {}
}
