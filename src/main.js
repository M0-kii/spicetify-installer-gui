import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  blockers,
  friendlyError,
  nextStep,
  parseProgress,
  uninstallBlockers,
} from "./state.js";
import "./style.css";

const $ = (id) => document.getElementById(id);
const screens = ["check", "options", "install", "done"];
let mode = "install";
let system;
let step = 0;
let checking = false;
let installing = false;
let failed = false;
let logs = [];

const removing = () => mode === "uninstall";
const activeBlockers = () => (removing() ? uninstallBlockers : blockers)(system);

function applyMode() {
  const uninstall = removing();
  $("mode-install").setAttribute("aria-pressed", String(!uninstall));
  $("mode-uninstall").setAttribute("aria-pressed", String(uninstall));
  $("options-heading-text").textContent = uninstall
    ? "Uninstall options"
    : "Installation options";
  $("marketplace-title").textContent = uninstall ? "Remove Marketplace" : "Marketplace";
  $("marketplace-subtitle").textContent = uninstall
    ? "Delete Marketplace files from Spotify"
    : "Browse themes and extensions in Spotify";
  $("mode-note").textContent = uninstall
    ? "Spotify is restored first, then the CLI, settings, PATH entry and Marketplace are removed."
    : "Spotify is backed up before changes are applied.";
  $("done-heading-text").textContent = uninstall
    ? "Uninstall complete"
    : "Installation complete";
}

function notice(message, kind = "") {
  $("notice").textContent = message;
  $("notice").className = `notice ${kind}`;
  $("notice").hidden = !message;
}

function controls() {
  $("refresh").disabled = checking;
  $("mode-install").disabled = checking || installing;
  $("mode-uninstall").disabled = checking || installing;
  $("marketplace").disabled = installing;
  $("close").disabled = installing;
  $("close").title = installing
    ? `Wait for ${removing() ? "uninstallation" : "installation"} to finish`
    : "Close";
  $("back").disabled = checking || installing || step === 0;
  $("back").textContent = step === 3 ? "Close" : "Back";
  $("next").disabled =
    step === 0 ? checking || activeBlockers().length > 0 : installing;
  $("next").textContent = [
    checking ? "Checking…" : "Next",
    removing() ? "Uninstall" : "Install",
    failed ? "Try again" : removing() ? "Uninstalling…" : "Installing…",
    removing() ? "Close" : "Open Spotify",
  ][step];
  $("step-count").textContent = `Step ${step + 1} of 4`;
}

function showStep(value) {
  step = value;
  for (const [index, name] of screens.entries())
    $(`${name}-screen`).hidden = index !== step;
  $("log-details").hidden = step < 2;
  if (step === 1) notice("");
  document.querySelector("main").scrollTop = 0;
  $(`${screens[step]}-title`).focus();
  controls();
}

function log(line) {
  // Spicetify emits ANSI formatting even when its output is redirected.
  const clean = line.replace(/\x1b\[[0-9;?]*[a-zA-Z]/g, "").trim();
  if (!clean) return;
  logs.push(clean);
  if (logs.length > 1000) logs.shift();
  $("log").textContent = logs.join("\n");
  $("log").scrollTop = $("log").scrollHeight;
  const progress = parseProgress(clean);
  if (progress) {
    $("progress-title").textContent = progress.title;
    $("progress-count").textContent = `${progress.step} / 4`;
    $("progress").value = progress.step - 1;
  }
}

async function inspect() {
  checking = true;
  controls();
  notice("Checking your system…");
  try {
    if (!isTauri())
      throw new Error(
        "Browser preview. Open the desktop app to check your system and manage Spicetify.",
      );
    system = await invoke("inspect_system");
    const ready = Boolean(
      system.spotifyPath && system.prefsFound && !system.spotifyRunning,
    );
    $("spotify-detail").textContent =
      system.spotifyPath || "Desktop app required";
    $("spotify-detail").title = system.spotifyPath || "";
    $("spotify-status").textContent = !system.spotifyPath
      ? "Not found"
      : !system.prefsFound
        ? "Open once"
        : system.spotifyRunning
          ? "Quit Spotify"
          : "Ready";
    $("spotify-status").className = `status ${ready ? "good" : "bad"}`;
    $("cli-status").textContent = system.cliPath
      ? "Installed"
      : removing()
        ? "Not found"
        : "Will install";
    $("cli-option-detail").textContent = removing()
      ? system.cliPath
        ? "The CLI that will be removed"
        : "Nothing to remove"
      : system.cliPath
        ? "Use your existing installation"
        : "The latest official release";
    notice(activeBlockers().join(" "), "error");
  } catch (error) {
    system = undefined;
    $("spotify-detail").textContent = "System check failed";
    $("spotify-detail").removeAttribute("title");
    $("spotify-status").textContent = "Unavailable";
    $("spotify-status").className = "status";
    $("cli-status").textContent = "Unavailable";
    notice(friendlyError(error), "error");
  } finally {
    checking = false;
    controls();
  }
}

async function run() {
  if (installing) return;
  const uninstall = removing();
  installing = true;
  failed = false;
  logs = [];
  $("log").textContent = uninstall ? "Starting uninstall…" : "Starting installation…";
  $("recovery").hidden = true;
  $("copy-restore").textContent = "Copy command";
  $("log-details").open = false;
  $("progress").value = 0;
  $("progress-title").textContent = "Checking your system…";
  $("progress-count").textContent = "1 / 4";
  $("install-heading").textContent = uninstall
    ? "Uninstalling Spicetify"
    : "Installing Spicetify";
  notice("");
  showStep(2);
  const onEvent = new Channel();
  onEvent.onmessage = log;
  try {
    await invoke(uninstall ? "uninstall_spicetify" : "install_spicetify", {
      marketplace: $("marketplace").checked,
      onEvent,
    });
    $("progress").value = 4;
    $("done-description").textContent = uninstall
      ? "Spotify has been restored to its original state."
      : $("marketplace").checked
        ? "Find Marketplace in the Spotify sidebar."
        : "Open Spotify to get started.";
    showStep(3);
  } catch (error) {
    failed = true;
    log(String(error));
    notice(friendlyError(error), "error");
    $("recovery").hidden = uninstall;
    $("install-heading").textContent = uninstall
      ? "Uninstall stopped"
      : "Installation stopped";
    $("log-details").open = true;
  } finally {
    installing = false;
    controls();
  }
}

async function closeWindow() {
  if (installing || !isTauri()) return;
  try {
    await getCurrentWindow().close();
  } catch (error) {
    notice(`Could not close the window: ${error}`, "error");
  }
}

$("minimize").addEventListener("click", async () => {
  if (!isTauri()) return;
  try {
    await getCurrentWindow().minimize();
  } catch (error) {
    notice(`Could not minimize the window: ${error}`, "error");
  }
});
for (const [id, value] of [
  ["mode-install", "install"],
  ["mode-uninstall", "uninstall"],
])
  $(id).addEventListener("click", () => {
    if (mode === value || checking || installing) return;
    mode = value;
    applyMode();
    showStep(0);
    inspect();
  });
$("close").addEventListener("click", closeWindow);
$("refresh").addEventListener("click", inspect);
$("copy-restore").addEventListener("click", async () => {
  const command = $("restore-command").textContent;
  try {
    await navigator.clipboard.writeText(command);
    $("copy-restore").textContent = "Copied";
  } catch {
    // Clipboard access can be denied; selecting the text still lets Ctrl+C work.
    const range = document.createRange();
    range.selectNodeContents($("restore-command"));
    const selection = getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    $("copy-restore").textContent = "Press Ctrl+C";
  }
});
$("back").addEventListener("click", () => {
  if (installing || checking) return;
  if (step === 3) closeWindow();
  else if (step === 2) showStep(1);
  else if (step === 1) {
    showStep(0);
    inspect();
  }
});
$("next").addEventListener("click", async () => {
  if (checking || installing) return;
  if (step === 3) {
    if (removing()) return closeWindow();
    try {
      await invoke("open_spotify");
    } catch (error) {
      notice(String(error), "error");
    }
  } else if (step === 2 && failed) await run();
  else {
    const next = nextStep(step, system, false, activeBlockers);
    if (next === 2) await run();
    else if (next !== step) showStep(next);
  }
});

document.querySelectorAll('a[target="_blank"]').forEach((link) => {
  link.addEventListener("click", async (event) => {
    if (!isTauri()) return;
    event.preventDefault();
    try {
      await openUrl(link.href);
    } catch (error) {
      notice(`Could not open the link: ${error}`, "error");
    }
  });
});

applyMode();
inspect();
