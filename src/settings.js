import { invoke } from "./common.js";
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
const GROUPS = {
  chords: "Chords",
  intervals: "Intervals",
  scales: "Scales",
  arpeggios: "Arpeggios",
  arpeggio_orders: "Arpeggio orders",
  fret_modes: "Fretboard modes (each challenge uses one of these)",
};

/** Action mode's mappings, as edited (saved with the rest). */
let mappings = [];
/** The pad/knob/button Learn picked up for the next mapping. */
let learned = null;
let chordLabels = {};
let appNames = {};

const CONTROL = "control";

function controlLabel(c) {
  const ch = `ch ${c.channel + 1}`;
  if (c.type === "cc") return `Control ${c.number} · ${ch}`;
  if (c.type === "program") return `Program ${c.number + 1} · ${ch}`;
  return `Pad ${c.note} · ${ch}`;
}

const triggerLabel = (t) => (t.type === "chord" ? chordLabels[t.chord] : controlLabel(t.control));
const appName = (path) => appNames[path] ?? path.split("/").pop().replace(/\.app$/, "");
const sameTrigger = (a, b) => JSON.stringify(a) === JSON.stringify(b);

function renderMappings() {
  const list = $("mappings");
  list.innerHTML = "";
  if (!mappings.length) {
    const li = document.createElement("li");
    li.className = "empty";
    li.textContent = "No actions yet. Add one below.";
    list.append(li);
  }
  mappings.forEach((m, i) => {
    const li = document.createElement("li");
    const text = document.createElement("span");
    text.textContent = `${triggerLabel(m.trigger)} → Open ${appName(m.action.path)}`;
    const remove = document.createElement("button");
    remove.type = "button";
    remove.textContent = "Remove";
    remove.onclick = () => {
      mappings.splice(i, 1);
      renderMappings();
      setStatus("Not saved yet.");
    };
    li.append(text, remove);
    list.append(li);
  });
}

function setLearning(on) {
  const b = $("learn");
  b.classList.toggle("listening", on);
  b.textContent = on ? "Touch a pad, knob or button…" : learned ? controlLabel(learned) : "Learn";
  invoke(on ? "start_learning" : "stop_learning");
}

function syncTrigger() {
  const control = $("trigger").value === CONTROL;
  $("learn").hidden = !control;
  if (!control && $("learn").classList.contains("listening")) setLearning(false);
}

function syncMode() {
  const quiz = document.querySelector("input[name=mode]:checked")?.value === "quiz";
  $("quiz-settings").hidden = !quiz;
  $("action-settings").hidden = quiz;
  document.querySelectorAll(".quiz-only").forEach((el) => (el.hidden = !quiz));
}

function renderGroup(key, choices, selected) {
  const fs = $(key);
  fs.innerHTML = "";
  const legend = document.createElement("legend");
  legend.textContent = GROUPS[key];
  for (const [text, on] of [["all", true], ["none", false]]) {
    const b = document.createElement("button");
    b.type = "button";
    b.textContent = text;
    b.onclick = () => fs.querySelectorAll("input").forEach((i) => (i.checked = on));
    legend.append(b);
  }
  const grid = document.createElement("div");
  grid.className = "checks";
  for (const c of choices) {
    const label = document.createElement("label");
    label.innerHTML = `<input type="checkbox" value="${c.id}" ${selected.includes(c.id) ? "checked" : ""}/> `;
    label.append(c.label);
    // Double-click solos this option within its group.
    label.ondblclick = () =>
      fs.querySelectorAll("input").forEach((i) => (i.checked = i.value === c.id));
    grid.append(label);
  }
  fs.append(legend, grid);
}

async function load() {
  const [options, settings, ports, audioInputs, apps] = await Promise.all([
    invoke("get_options"),
    invoke("get_settings"),
    invoke("list_midi_ports"),
    invoke("list_audio_inputs"),
    invoke("list_apps"),
  ]);
  for (const r of document.querySelectorAll("input[name=mode]")) r.checked = r.value === settings.mode;
  syncMode();
  chordLabels = Object.fromEntries(options.trigger_chords.map((c) => [c.id, c.label]));
  appNames = Object.fromEntries(apps.map((a) => [a.path, a.name]));
  const trigger = $("trigger");
  trigger.innerHTML = "";
  for (const c of options.trigger_chords) trigger.add(new Option(c.label, c.id));
  trigger.add(new Option("Pad, knob or button…", CONTROL));
  const app = $("app");
  app.innerHTML = "";
  for (const a of apps) app.add(new Option(a.name, a.path));
  mappings = settings.mappings;
  learned = null;
  setLearning(false);
  syncTrigger();
  renderMappings();
  for (const key of Object.keys(GROUPS)) renderGroup(key, options[key], settings[key]);
  $("minutes").value = settings.minutes;
  $("sound").checked = settings.sound;
  $("pause_in_calls").checked = settings.pause_in_calls;
  $("mute_in_calls").checked = settings.mute_in_calls;
  $("launch_at_login").checked = settings.launch_at_login;
  $("fretboard_on_alarm").checked = settings.fretboard_on_alarm;
  const midi = $("midi");
  midi.innerHTML = `<option value="">All inputs</option>`;
  const names = new Set(ports);
  if (settings.midi_port) names.add(settings.midi_port);
  for (const name of names) midi.add(new Option(name, name, false, name === settings.midi_port));
  $("audio_enabled").checked = settings.audio_enabled;
  const audio = $("audio_device");
  audio.innerHTML = `<option value="">System default</option>`;
  const inputs = new Set(audioInputs);
  if (settings.audio_device) inputs.add(settings.audio_device);
  for (const name of inputs) audio.add(new Option(name, name, false, name === settings.audio_device));
  syncAudio();
  setStatus(`${ports.length} MIDI input(s), ${audioInputs.length} audio input(s) found`);
}

function syncAudio() {
  $("audio_device").disabled = !$("audio_enabled").checked;
}

function setStatus(text, error = false) {
  $("status").textContent = text;
  $("status").classList.toggle("error", error);
}

$("form").onsubmit = async (e) => {
  e.preventDefault();
  const checked = (key) => [...$(key).querySelectorAll("input:checked")].map((i) => i.value);
  const settings = {
    mode: document.querySelector("input[name=mode]:checked").value,
    mappings,
    chords: checked("chords"),
    intervals: checked("intervals"),
    scales: checked("scales"),
    arpeggios: checked("arpeggios"),
    arpeggio_orders: checked("arpeggio_orders"),
    fret_modes: checked("fret_modes"),
    minutes: Number($("minutes").value),
    midi_port: $("midi").value || null,
    audio_enabled: $("audio_enabled").checked,
    audio_device: $("audio_device").value || null,
    sound: $("sound").checked,
    pause_in_calls: $("pause_in_calls").checked,
    mute_in_calls: $("mute_in_calls").checked,
    launch_at_login: $("launch_at_login").checked,
    fretboard_on_alarm: $("fretboard_on_alarm").checked,
  };
  try {
    await invoke("save_settings", { settings });
    setStatus(settings.mode === "quiz" ? "Saved. New challenge loaded." : "Saved.");
  } catch (err) {
    setStatus(String(err), true);
  }
};

$("audio_enabled").onchange = syncAudio;
document.querySelectorAll("input[name=mode]").forEach((r) => (r.onchange = syncMode));
$("trigger").onchange = syncTrigger;
$("learn").onclick = () => setLearning(!$("learn").classList.contains("listening"));
listen("learned", (e) => {
  learned = e.payload.control;
  setLearning(false);
});
$("add").onclick = () => {
  let trigger;
  if ($("trigger").value === CONTROL) {
    if (!learned) return setStatus("Click Learn and touch a pad, knob or button first.", true);
    trigger = { type: "control", control: learned };
  } else {
    trigger = { type: "chord", chord: $("trigger").value };
  }
  const path = $("app").value;
  if (!path) return setStatus("Pick an app.", true);
  if (mappings.some((m) => sameTrigger(m.trigger, trigger) && m.action.path === path)) {
    return setStatus("That one's already there.", true);
  }
  mappings.push({ trigger, action: { type: "open_app", path } });
  learned = null;
  setLearning(false);
  renderMappings();
  setStatus("Not saved yet.");
};

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
listen("reload-settings", load);
load();
