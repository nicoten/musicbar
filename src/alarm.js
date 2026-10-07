import { onState } from "./common.js";

const $ = (id) => document.getElementById(id);

onState((s) => {
  $("short").textContent = s.short;
  $("long").textContent = s.long;
  $("held").textContent = s.held.length ? s.held.join("  ") : "";
});
