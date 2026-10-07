import { onState } from "./common.js";
import { mountFretboard } from "./fretboard.js";

const $ = (id) => document.getElementById(id);
const renderBoard = mountFretboard($("board"));

onState((s) => {
  $("short").textContent = s.short;
  $("long").textContent = s.long;
  $("held").textContent = s.held.length ? s.held.join("  ") : "";
  $("board").hidden = !s.fretboard_on_alarm;
  renderBoard(s);
});
