use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    env, fs, io,
    os::windows::{
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use windows_sys::Win32::{
    Foundation::{ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE},
    System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    },
    UI::{
        Shell::IsUserAnAdmin,
        WindowsAndMessaging::{
            SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
        },
    },
};
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
    types::ToRegValue,
    RegKey,
};

struct System {
    spotify: Option<PathBuf>,
    prefs: PathBuf,
    cli: Option<PathBuf>,
    running: bool,
    admin: bool,
}

fn directory(name: &str) -> Result<PathBuf, String> {
    env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| format!("Windows did not provide a valid {name} folder."))
}

fn spotify_running() -> io::Result<bool> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = unsafe { Process32FirstW(snapshot.as_raw_handle(), &mut entry) };
    while found != 0 {
        let end = entry
            .szExeFile
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(entry.szExeFile.len());
        if String::from_utf16_lossy(&entry.szExeFile[..end]).eq_ignore_ascii_case("Spotify.exe") {
            return Ok(true);
        }
        found = unsafe { Process32NextW(snapshot.as_raw_handle(), &mut entry) };
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
        Ok(false)
    } else {
        Err(error)
    }
}

fn store_package(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("SpotifyAB.SpotifyMusic_"))
}

fn spotify_paths(roaming: &Path, local: &Path) -> Vec<PathBuf> {
    let mut paths = vec![roaming.join("Spotify"), local.join("Spotify")];
    for name in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(root) = directory(name) {
            paths.push(root.join("Spotify"));
            // Store builds live in a versioned package folder, one level under WindowsApps.
            if let Ok(entries) = fs::read_dir(root.join("WindowsApps")) {
                paths.extend(
                    entries
                        .filter_map(Result::ok)
                        .map(|entry| entry.path())
                        .filter(|path| store_package(path)),
                );
            }
        }
    }
    paths
}

fn system() -> Result<System, String> {
    let roaming = directory("APPDATA")?;
    let local = directory("LOCALAPPDATA")?;
    let spotify = spotify_paths(&roaming, &local)
        .into_iter()
        .find(|path| path.join("Spotify.exe").is_file() && path.join("Apps").is_dir());
    let cli = [
        Some(local.join("spicetify")),
        directory("USERPROFILE")
            .ok()
            .map(|path| path.join("spicetify-cli")),
    ]
    .into_iter()
    .flatten()
    .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default()))
    .filter(|path| path.is_absolute())
    .map(|path| path.join("spicetify.exe"))
    .find(|path| path.is_file());
    Ok(System {
        spotify,
        prefs: roaming.join(r"Spotify\prefs"),
        cli,
        running: spotify_running()
            .map_err(|e| format!("Could not check whether Spotify is running: {e}"))?,
        admin: unsafe { IsUserAnAdmin() } != 0,
    })
}

pub fn inspect() -> Result<Value, String> {
    let status = system()?;
    Ok(json!({
        "spotifyPath": status.spotify,
        "prefsFound": status.prefs.is_file(),
        "spotifyRunning": status.running,
        "cliPath": status.cli,
        "admin": status.admin,
    }))
}

fn ready(status: &System) -> Result<&Path, String> {
    if status.admin {
        return Err("Open this app without administrator privileges.".into());
    }
    let spotify = status
        .spotify
        .as_deref()
        .ok_or("Install Spotify desktop from spotify.com.")?;
    if !status.prefs.is_file() {
        return Err("Open Spotify and sign in once, then quit Spotify and try again.".into());
    }
    if status.running {
        return Err("Quit Spotify, including its system tray icon, then try again.".into());
    }
    Ok(spotify)
}

pub fn open_spotify() -> Result<(), String> {
    let folder = system()?
        .spotify
        .ok_or("Spotify desktop could not be found.")?;
    Command::new(folder.join("Spotify.exe"))
        .current_dir(folder)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open Spotify: {e}"))
}

fn system_tool(name: &str) -> Result<Command, String> {
    let path = directory("SystemRoot")?.join("System32").join(name);
    let mut command = Command::new(path);
    command.creation_flags(0x08000000).stdin(Stdio::null());
    Ok(command)
}

fn run(command: &mut Command) -> Result<String, String> {
    let name = Path::new(command.get_program())
        .file_name()
        .unwrap_or(command.get_program())
        .to_string_lossy()
        .into_owned();
    let output = command
        .output()
        .map_err(|e| format!("Could not start {name}: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(format!(
            "{name} failed ({}). {}",
            output.status,
            [stdout.trim(), stderr.trim()].join(" ").trim()
        ));
    }
    Ok(stdout.trim().into())
}

fn curl() -> Result<Command, String> {
    let mut command = system_tool("curl.exe")?;
    command.args([
        "-q",
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--connect-timeout",
        "20",
        "--user-agent",
        "Spicetify-Installer-GUI",
    ]);
    Ok(command)
}

fn release(repo: &str) -> Result<Value, String> {
    let body = run(curl()?.args([
        "--max-time",
        "60",
        &format!("https://api.github.com/repos/spicetify/{repo}/releases/latest"),
    ]))?;
    serde_json::from_str(&body)
        .map_err(|e| format!("GitHub returned invalid release metadata: {e}"))
}

fn asset<'a>(release: &'a Value, repo: &str, name: &str) -> Result<(&'a str, &'a str), String> {
    let tag = release["tag_name"]
        .as_str()
        .ok_or("The release has no tag.")?;
    if tag.is_empty()
        || matches!(tag, "." | "..")
        || !tag
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err("GitHub returned an invalid release tag.".into());
    }
    let entry = release["assets"]
        .as_array()
        .and_then(|assets| {
            assets
                .iter()
                .find(|entry| entry["name"].as_str() == Some(name))
        })
        .ok_or_else(|| format!("The official release does not include {name}."))?;
    let url = entry["browser_download_url"]
        .as_str()
        .filter(|url| {
            *url == format!("https://github.com/spicetify/{repo}/releases/download/{tag}/{name}")
        })
        .ok_or("GitHub returned an unexpected download address.")?;
    let digest = entry["digest"]
        .as_str()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|c| c.is_ascii_hexdigit()))
        .ok_or("GitHub did not publish a valid SHA-256 digest for this archive.")?;
    Ok((url, digest))
}

fn verify(bytes: &[u8], expected: &str) -> Result<(), String> {
    if format!("{:x}", Sha256::digest(bytes)).eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err("Download verification failed. Try again.".into())
    }
}

fn unpack(release: &Value, repo: &str, name: &str, destination: &Path) -> Result<(), String> {
    let (url, digest) = asset(release, repo, name)?;
    let archive = destination.with_extension("zip");
    run(curl()?
        .args(["--max-time", "180", "--output"])
        .arg(&archive)
        .arg(url))?;
    verify(
        &fs::read(&archive).map_err(|e| format!("Could not read the downloaded archive: {e}"))?,
        digest,
    )?;
    fs::create_dir(destination)
        .map_err(|e| format!("Could not prepare the extraction folder: {e}"))?;
    run(system_tool("tar.exe")?
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(destination))?;
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unexpected archive entry",
            ));
        }
    }
    Ok(())
}

fn notify_environment_change() {
    let environment: Vec<u16> = "Environment\0".encode_utf16().collect();
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            environment.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            1000,
            std::ptr::null_mut(),
        );
    }
}

fn add_to_path(folder: &Path) -> io::Result<()> {
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)?;
    let mut value = match key.get_value::<String, _>("Path") {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let folder = folder.to_string_lossy();
    if value
        .split(';')
        .any(|path| path.trim_end_matches('\\').eq_ignore_ascii_case(&folder))
    {
        return Ok(());
    }
    if !value.is_empty() && !value.ends_with(';') {
        value.push(';');
    }
    value.push_str(&folder);
    let mut updated = value.to_reg_value();
    if let Ok(previous) = key.get_raw_value("Path") {
        updated.vtype = previous.vtype;
    }
    key.set_raw_value("Path", &updated)?;
    notify_environment_change();
    Ok(())
}

fn without_path_entry(value: &str, folder: &str) -> String {
    value
        .split(';')
        .filter(|path| !path.trim_end_matches('\\').eq_ignore_ascii_case(folder))
        .collect::<Vec<_>>()
        .join(";")
}

fn remove_from_path(folder: &Path) -> io::Result<()> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)?;
    let value = match key.get_value::<String, _>("Path") {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let folder = folder.to_string_lossy();
    let trimmed = without_path_entry(&value, &folder);
    if trimmed == value {
        return Ok(());
    }
    let mut updated = trimmed.to_reg_value();
    if let Ok(previous) = key.get_raw_value("Path") {
        updated.vtype = previous.vtype;
    }
    key.set_raw_value("Path", &updated)?;
    notify_environment_change();
    Ok(())
}

fn remove_dir_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn install(marketplace: bool, log: impl Fn(&str)) -> Result<(), String> {
    let status = system()?;
    let spotify = ready(&status)?;
    let staging =
        tempfile::tempdir().map_err(|e| format!("Could not create a temporary folder: {e}"))?;
    let local_cli = directory("LOCALAPPDATA")?.join("spicetify");

    log("[step:1] Preparing Spicetify");
    let cli = if let Some(cli) = &status.cli {
        log("Using your existing Spicetify CLI.");
        cli.clone()
    } else {
        let release = release("cli")?;
        let version = release["tag_name"]
            .as_str()
            .and_then(|tag| tag.strip_prefix('v'))
            .filter(|version| {
                version.split('.').count() == 3
                    && version
                        .split('.')
                        .all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()))
            })
            .ok_or("The latest release has an unexpected version number.")?;
        let architecture = env::var("PROCESSOR_ARCHITEW6432")
            .or_else(|_| env::var("PROCESSOR_ARCHITECTURE"))
            .unwrap_or_default();
        let arch = match architecture.as_str() {
            "AMD64" => "x64",
            "ARM64" => "arm64",
            "x86" => "x32",
            _ => return Err("This Windows architecture is unsupported.".into()),
        };
        log(&format!("Downloading Spicetify {version} ({arch})."));
        let source = staging.path().join("cli");
        unpack(
            &release,
            "cli",
            &format!("spicetify-{version}-windows-{arch}.zip"),
            &source,
        )?;
        if !source.join("spicetify.exe").is_file() {
            return Err("The release archive does not contain spicetify.exe.".into());
        }
        // Publish the executable last so a failed copy can be retried.
        let executable = staging.path().join("spicetify.exe");
        fs::rename(source.join("spicetify.exe"), &executable)
            .and_then(|_| copy_tree(&source, &local_cli))
            .map_err(|e| format!("Could not install the CLI files: {e}"))?;
        let pending = tempfile::NamedTempFile::new_in(&local_cli)
            .map_err(|e| format!("Could not prepare the CLI executable: {e}"))?;
        io::copy(
            &mut fs::File::open(executable).map_err(|e| e.to_string())?,
            &mut pending.as_file(),
        )
        .map_err(|e| format!("Could not copy the CLI executable: {e}"))?;
        pending
            .persist_noclobber(local_cli.join("spicetify.exe"))
            .map_err(|e| format!("Could not publish the CLI executable: {e}"))?;
        local_cli.join("spicetify.exe")
    };
    if cli == local_cli.join("spicetify.exe") {
        add_to_path(&local_cli).map_err(|e| format!("Could not update the user PATH: {e}"))?;
    }
    let spice = |args: &[&str], path: Option<&Path>| {
        let mut cmd = Command::new(&cli);
        cmd.creation_flags(0x08000000)
            .stdin(Stdio::null())
            .args(args);
        if let Some(path) = path {
            cmd.arg(path);
        }
        run(&mut cmd)
    };
    log(&spice(&["--version"], None)?);
    log(&spice(&["config", "spotify_path"], Some(spotify))?);
    log(&spice(&["config", "prefs_path"], Some(&status.prefs))?);

    log("[step:2] Setting installation options");
    if marketplace {
        let userdata = PathBuf::from(spice(&["path", "userdata"], None)?);
        if !userdata.is_absolute() || !userdata.is_dir() {
            return Err("Spicetify returned an invalid configuration folder.".into());
        }
        let custom_apps = userdata.join("CustomApps");
        let market = custom_apps.join("marketplace");
        if market.exists() {
            if !market.join("index.js").is_file() {
                return Err("Marketplace is incomplete. Uncheck Marketplace to install the CLI alone, or fix the existing folder.".into());
            }
            log("Using your existing Marketplace files.");
        } else {
            let release = release("marketplace")?;
            let source = staging.path().join("marketplace");
            log("Downloading Marketplace.");
            unpack(&release, "marketplace", "marketplace.zip", &source)?;
            let contents = source.join("marketplace-dist");
            if !contents.join("index.js").is_file() {
                return Err("The Marketplace archive has an unexpected structure.".into());
            }
            fs::create_dir_all(&custom_apps)
                .map_err(|e| format!("Could not prepare the custom apps folder: {e}"))?;
            let pending = tempfile::tempdir_in(&custom_apps)
                .map_err(|e| format!("Could not prepare Marketplace: {e}"))?;
            copy_tree(&contents, pending.path())
                .and_then(|_| fs::rename(pending.path(), &market))
                .map_err(|e| format!("Could not install Marketplace: {e}"))?;
        }
        log(&spice(&["config", "custom_apps", "marketplace"], None)?);
        log(&spice(
            &["config", "inject_css", "1", "replace_colors", "1"],
            None,
        )?);
    }

    log("[step:3] Backing up and applying");
    ready(&system()?)?;
    log(&spice(&["backup", "apply", "--no-restart"], None)?);
    log("[step:4] Verifying installation");
    log(&spice(&["--version"], None)?);
    Ok(())
}

pub fn uninstall(marketplace: bool, log: impl Fn(&str)) -> Result<(), String> {
    let status = system()?;
    if status.admin {
        return Err("Open this app without administrator privileges.".into());
    }
    if status.running {
        return Err("Quit Spotify, including its system tray icon, then try again.".into());
    }
    let cli = status.cli.clone().ok_or("Spicetify CLI could not be found.")?;
    let local_cli = directory("LOCALAPPDATA")?.join("spicetify");
    let config = directory("APPDATA")?.join("spicetify");

    let spice = |args: &[&str]| {
        let mut cmd = Command::new(&cli);
        cmd.creation_flags(0x08000000)
            .stdin(Stdio::null())
            .args(args);
        run(&mut cmd)
    };

    log("[step:1] Restoring Spotify");
    log(&spice(&["restore"])?);

    log("[step:2] Removing Marketplace");
    if marketplace {
        let userdata = PathBuf::from(spice(&["path", "userdata"])?);
        if !userdata.is_absolute() || !userdata.is_dir() {
            return Err("Spicetify returned an invalid configuration folder.".into());
        }
        remove_dir_if_exists(&userdata.join("CustomApps").join("marketplace"))
            .map_err(|e| format!("Could not remove Marketplace: {e}"))?;
    }

    log("[step:3] Removing CLI and PATH entry");
    // Only delete the CLI we manage; a CLI the user installed elsewhere is left alone.
    let managed = cli == local_cli.join("spicetify.exe");
    if managed {
        remove_dir_if_exists(&local_cli)
            .map_err(|e| format!("Could not remove the CLI files: {e}"))?;
    }
    remove_from_path(&local_cli).map_err(|e| format!("Could not update the user PATH: {e}"))?;
    remove_dir_if_exists(&config)
        .map_err(|e| format!("Could not remove Spicetify settings: {e}"))?;

    log("[step:4] Verifying");
    if managed && cli.is_file() {
        return Err("The CLI files could not be removed.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_require_a_normal_user_and_closed_signed_in_spotify() {
        let folder = tempfile::tempdir().unwrap();
        let mut status = System {
            spotify: Some(folder.path().to_owned()),
            prefs: folder.path().join("prefs"),
            cli: None,
            running: false,
            admin: false,
        };
        assert!(ready(&status).is_err());
        fs::write(&status.prefs, b"").unwrap();
        assert!(ready(&status).is_ok());
        status.running = true;
        assert!(ready(&status).is_err());
        status.running = false;
        status.admin = true;
        assert!(ready(&status).is_err());
        status.admin = false;
        status.spotify = None;
        assert!(ready(&status).is_err());
    }

    #[test]
    fn only_verified_official_release_archives_are_accepted() {
        let digest = format!("{:x}", Sha256::digest(b"archive"));
        let mut metadata = json!({
            "tag_name": "v1",
            "assets": [{
                "name": "marketplace.zip",
                "browser_download_url": "https://github.com/spicetify/marketplace/releases/download/v1/marketplace.zip",
                "digest": format!("sha256:{digest}")
            }]
        });
        let (_, expected) = asset(&metadata, "marketplace", "marketplace.zip").unwrap();
        assert!(verify(b"archive", expected).is_ok());
        assert!(verify(b"corrupt", expected).is_err());
        metadata["assets"][0]["digest"] = Value::Null;
        assert!(asset(&metadata, "marketplace", "marketplace.zip").is_err());
        metadata["assets"][0]["digest"] = json!(format!("sha256:{digest}"));
        metadata["assets"][0]["browser_download_url"] =
            json!("https://github.com.evil.test/marketplace.zip");
        assert!(asset(&metadata, "marketplace", "marketplace.zip").is_err());
    }

    #[test]
    fn finds_store_spotify_packages_without_matching_unrelated_folders() {
        assert!(store_package(Path::new(
            r"C:\Program Files\WindowsApps\SpotifyAB.SpotifyMusic_1.250.0.0_x64__zpdnekdrzrea0"
        )));
        assert!(!store_package(Path::new(
            r"C:\Program Files\WindowsApps\SpotifyAB.SpotifyMusic"
        )));
        assert!(!store_package(Path::new(r"C:\Spotify")));
    }

    #[test]
    fn removes_only_the_managed_path_entry() {
        assert_eq!(
            without_path_entry(
                r"C:\Tools;C:\Users\Me\AppData\Local\spicetify;D:\Bin",
                r"C:\Users\Me\AppData\Local\spicetify"
            ),
            r"C:\Tools;D:\Bin"
        );
        assert_eq!(
            without_path_entry(
                r"C:\Tools;C:\Users\Me\AppData\Local\spicetify\",
                r"c:\users\me\appdata\local\spicetify"
            ),
            "C:\\Tools"
        );
        assert_eq!(
            without_path_entry(r"C:\Tools;D:\Bin", r"C:\Other"),
            r"C:\Tools;D:\Bin"
        );
    }

    #[test]
    fn copies_nested_files_into_the_destination() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        fs::create_dir(source.path().join("nested")).unwrap();
        fs::write(source.path().join("root.txt"), b"root").unwrap();
        fs::write(source.path().join("nested/child.txt"), b"child").unwrap();
        copy_tree(source.path(), &destination.path().join("out")).unwrap();
        assert_eq!(
            fs::read_to_string(destination.path().join("out/root.txt")).unwrap(),
            "root"
        );
        assert_eq!(
            fs::read_to_string(destination.path().join("out/nested/child.txt")).unwrap(),
            "child"
        );
    }
}
