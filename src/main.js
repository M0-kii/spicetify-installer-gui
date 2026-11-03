import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { blockers, nextStep, parseProgress } from "./state.js";
import "./style.css";

const $ = (id) => document.getElementById(id);
const screens = ["check", "options", "install", "done"];
let system;
let step = 0;
let checking = false;
let installing = false;
let failed = false;
let logs = [];

function notice(message, kind = "") {
  $("notice").textContent = message;
  $("notice").className = `notice ${kind}`;
  $("notice").hidden = !message;
}

function controls() {
  $("refresh").disabled = checking;
  $("marketplace").disabled = installing;
  $("close").disabled = installing;
  $("close").title = installing ? "Wait for installation to finish" : "Close";
  $("back").disabled = checking || installing || step === 0;
  $("back").textContent = step === 3 ? "Close" : "Back";
  $("next").disabled =
    step === 0 ? checking || blockers(system).length > 0 : installing;
  $("next").textContent = [
    checking ? "Checking…" : "Next",
    "Install",
    failed ? "Try again" : "Installing…",
    "Open Spotify",
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
        "Browser preview. Open the desktop app to check your system and install Spicetify.",
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
    $("cli-status").textContent = system.cliPath ? "Installed" : "Will install";
    $("cli-option-detail").textContent = system.cliPath
      ? "Use your existing installation"
      : "The latest official release";
    notice(blockers(system).join(" "), "error");
  } catch (error) {
    system = undefined;
    $("spotify-detail").textContent = "System check failed";
    $("spotify-detail").removeAttribute("title");
    $("spotify-status").textContent = "Unavailable";
    $("spotify-status").className = "status";
    $("cli-status").textContent = "Unavailable";
    notice(String(error), "error");
  } finally {
    checking = false;
    controls();
  }
}

async function install() {
  if (installing) return;
  installing = true;
  failed = false;
  logs = [];
  $("log").textContent = "Starting installation…";
  $("log-details").open = false;
  $("progress").value = 0;
  $("progress-title").textContent = "Checking your system…";
  $("progress-count").textContent = "1 / 4";
  $("install-heading").textContent = "Installing Spicetify";
  notice("");
  showStep(2);
  const onEvent = new Channel();
  onEvent.onmessage = log;
  try {
    await invoke("install_spicetify", {
      marketplace: $("marketplace").checked,
      onEvent,
    });
    $("progress").value = 4;
    $("done-description").textContent = $("marketplace").checked
      ? "Find Marketplace in the Spotify sidebar."
      : "Open Spotify to get started.";
    showStep(3);
  } catch (error) {
    failed = true;
    log(String(error));
    notice(String(error), "error");
    $("install-heading").textContent = "Installation stopped";
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
$("close").addEventListener("click", closeWindow);
$("refresh").addEventListener("click", inspect);
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
    try {
      await invoke("open_spotify");
    } catch (error) {
      notice(String(error), "error");
    }
  } else if (step === 2 && failed) await install();
  else {
    const next = nextStep(step, system, false);
    if (next === 2) await install();
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

inspect();
