import { onState, praiser } from "./common.js";
import { mountFretboard } from "./fretboard.js";

const $ = (id) => document.getElementById(id);
const renderBoard = mountFretboard($("board"));
const praise = praiser();

onState((s) => {
  const cheer = praise(s);
  document.body.classList.toggle("solved", s.solved);
  $("eyebrow").textContent = cheer ? `✓ ${cheer}` : "Time's up. Play this to make me go away:";
  $("short").textContent = s.short;
  $("long").textContent = s.long;
  $("held").textContent = s.held.length ? s.held.join("  ") : "";
  $("board").hidden = !s.fretboard_on_alarm;
  renderBoard(s);
});
