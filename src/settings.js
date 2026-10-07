import { invoke } from "./common.js";
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
const GROUPS = { chords: "Chords", intervals: "Intervals", scales: "Scales" };

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
    grid.append(label);
  }
  fs.append(legend, grid);
}

async function load() {
  const [options, settings, ports, audioInputs] = await Promise.all([
    invoke("get_options"),
    invoke("get_settings"),
    invoke("list_midi_ports"),
    invoke("list_audio_inputs"),
  ]);
  for (const key of Object.keys(GROUPS)) renderGroup(key, options[key], settings[key]);
  $("minutes").value = settings.minutes;
  $("sound").checked = settings.sound;
  $("pause_in_calls").checked = settings.pause_in_calls;
  $("launch_at_login").checked = settings.launch_at_login;
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
    chords: checked("chords"),
    intervals: checked("intervals"),
    scales: checked("scales"),
    minutes: Number($("minutes").value),
    midi_port: $("midi").value || null,
    audio_enabled: $("audio_enabled").checked,
    audio_device: $("audio_device").value || null,
    sound: $("sound").checked,
    pause_in_calls: $("pause_in_calls").checked,
    launch_at_login: $("launch_at_login").checked,
  };
  try {
    await invoke("save_settings", { settings });
    setStatus("Saved. New challenge loaded.");
  } catch (err) {
    setStatus(String(err), true);
  }
};

$("audio_enabled").onchange = syncAudio;
listen("reload-settings", load);
load();
