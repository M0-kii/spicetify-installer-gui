export function blockers(system) {
  if (!system) return ["System checks have not finished."];
  const issues = [];
  if (system.admin)
    issues.push(
      "Close this app and open it normally, without administrator privileges.",
    );
  if (!system.spotifyPath)
    issues.push(
      "Install Spotify from spotify.com, open it and sign in once, then check again.",
    );
  else if (!system.prefsFound)
    issues.push(
      "Open Spotify and sign in once, then quit Spotify and check again.",
    );
  if (system.spotifyRunning)
    issues.push(
      "Quit Spotify, including its system tray icon, then check again.",
    );
  return issues;
}

export function uninstallBlockers(system) {
  if (!system) return ["System checks have not finished."];
  const issues = [];
  if (system.admin)
    issues.push(
      "Close this app and open it normally, without administrator privileges.",
    );
  if (!system.cliPath) issues.push("Spicetify CLI was not found. Nothing to uninstall.");
  if (system.spotifyRunning)
    issues.push(
      "Quit Spotify, including its system tray icon, then check again.",
    );
  return issues;
}

export function parseProgress(line) {
  const match = /^\[step:([1-4])\] (.+)$/.exec(line);
  return match ? { step: Number(match[1]), title: match[2] } : null;
}

export function nextStep(step, system, busy, check = blockers) {
  if (busy) return step;
  if (step === 0 && check(system).length === 0) return 1;
  if (step === 1) return 2;
  return step;
}

export function friendlyError(error) {
  const text = String(error);
  if (/curl\.exe failed|Could not start curl\.exe/i.test(text))
    return "The download failed. Check your internet connection and try again.";
  if (/tar\.exe failed|Could not start tar\.exe/i.test(text))
    return "Could not extract the downloaded files. Try again.";
  return text;
}
