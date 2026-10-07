import { invoke, fmt, onState } from "./common.js";
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);

onState((s) => {
  $("panel").classList.toggle("overdue", s.overdue);
  $("category").textContent = s.category;
  $("short").textContent = s.short;
  $("long").textContent = s.long;
  $("timer").textContent = s.in_call
    ? `Paused, you're in a call · ${fmt(s.remaining_secs)}`
    : s.overdue
      ? "Time's up. Play it!"
      : s.paused
        ? `Paused · ${fmt(s.remaining_secs)}`
        : fmt(s.remaining_secs);
  $("progress").style.width = `${(100 * s.remaining_secs) / s.total_secs}%`;
  $("held").textContent = s.held.length ? `Playing: ${s.held.join(" ")}` : "";
  $("pause").textContent = s.user_paused ? "Resume" : "Pause";
  $("skip").disabled = s.overdue;
});

$("pause").onclick = () => invoke("toggle_pause");
$("skip").onclick = () => invoke("skip");
$("settings").onclick = () => invoke("open_settings");
$("quit").onclick = () => invoke("quit");

function renderUpdate(u) {
  const text = {
    idle: `MusicBar v${u.current}`,
    checking: "Checking for updates…",
    up_to_date: `You're up to date (v${u.current})`,
    installing: `Downloading v${u.version}…`,
    ready: `v${u.version} installed`,
    failed: "Couldn't check for updates",
  }[u.state];
  $("update-status").textContent = text;
  $("update-status").title = u.state === "failed" ? u.error : "";
  const button = $("check-updates");
  button.textContent = u.state === "ready" ? "Restart now" : "Check for updates";
  button.disabled = u.state === "checking" || u.state === "installing";
}

$("check-updates").onclick = () => invoke("check_for_updates");
listen("update", (e) => renderUpdate(e.payload));
invoke("get_update_info").then(renderUpdate);
