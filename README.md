# Spicetify Installer

A small Windows app that sets up Spicetify without opening a terminal. Marketplace is one checkbox away.

## Getting started

1. Get [Spotify for Windows](https://www.spotify.com/download/windows/), open it, and sign in once.
2. Quit Spotify completely, including the tray icon.
3. Open the installer normally, pick whether you want Marketplace, and hit Install.

The app pulls the CLI and Marketplace from their official GitHub releases, checks the downloads, and backs up Spotify before applying Spicetify. If Spicetify is already installed, the app uses that copy.

Need to undo it? See [Spicetify’s uninstall guide](https://spicetify.app/docs/advanced-usage/uninstallation).

## Build it

You’ll need pnpm, Node.js 22.12+, Rust, and the [Tauri Windows setup](https://v2.tauri.app/start/prerequisites/).

```sh
pnpm install
pnpm tauri dev
```

Run `pnpm test` for the checks or `pnpm tauri build` for a Windows setup `.exe`. The setup file lands in `src-tauri/target/release/bundle/nsis/`.

This is an unofficial installer. The flame logo comes from [Spicetify](https://spicetify.app/) and its [website repo](https://github.com/spicetify/docs). See the [upstream license](licenses/spicetify-docs.txt).
