# Spicetify Installer

A very small and simple portable Windows app to install or uninstall **Spicetify** for non-tech people :D

No setup, no installation: download `spicetify-installer.exe` and run it.

## Getting started

1. Get [Spotify for Windows](https://www.spotify.com/download/windows/), open it, and sign in once.
2. Quit Spotify completely, including the tray icon.
3. Run the app, leave **Install** selected, pick whether you want Marketplace, and hit Install.

The app pulls the CLI and Marketplace from their official GitHub releases, checks the downloads, and backs up Spotify before applying Spicetify. If Spicetify is already installed, the app uses that copy.

## Uninstalling

Run the app, switch to **Uninstall**, and hit Uninstall. Spotify is restored from its backup first; then the CLI, Spicetify settings, the PATH entry this app added, and Marketplace files are removed. Quit Spotify before you start.

For a manual uninstall, see [Spicetify’s uninstall guide](https://spicetify.app/docs/advanced-usage/uninstallation).

## If you wanna Build it

You’ll need pnpm, Node.js 22.12+, Rust, and the [Tauri Windows setup](https://v2.tauri.app/start/prerequisites/).

```sh
pnpm install
pnpm tauri dev
```

Run `pnpm test` for the checks or `pnpm tauri build` for the portable `.exe`. It lands in `src-tauri/target/release/spicetify-installer.exe`.

This is an unofficial installer. The flame logo comes from [Spicetify](https://spicetify.app/) and its [website repo](https://github.com/spicetify/docs). See the [upstream license](licenses/spicetify-docs.txt).
