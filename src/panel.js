import { invoke, onState, praiser } from "./common.js";
import { mountFretboard } from "./fretboard.js";

const $ = (id) => document.getElementById(id);
const renderBoard = mountFretboard($("board"));
const praise = praiser();

onState((s) => {
  const cheer = praise(s);
  $("panel").classList.toggle("overdue", s.overdue);
  $("panel").classList.toggle("solved", s.solved);
  $("category").textContent = cheer ? `✓ ${cheer}` : s.category;
  $("short").textContent = s.short;
  $("progress").style.width = s.solved ? "100%" : `${(100 * s.remaining_secs) / s.total_secs}%`;
  $("held").textContent = s.held.length ? `Playing: ${s.held.join(" ")}` : "";
  $("pause").textContent = s.user_paused ? "Resume" : "Pause";
  $("skip").disabled = s.overdue;
  renderBoard(s);
});

$("pause").onclick = () => invoke("toggle_pause");
$("skip").onclick = () => invoke("skip");
$("settings").onclick = () => invoke("open_settings");
$("quit").onclick = () => invoke("quit");

// Size the window to its content, so hiding the staff doesn't leave a gap.
const { getCurrentWindow, LogicalSize } = window.__TAURI__.window;
new ResizeObserver(() => {
  getCurrentWindow().setSize(new LogicalSize(window.innerWidth, Math.ceil($("panel").scrollHeight)));
}).observe($("panel"));
