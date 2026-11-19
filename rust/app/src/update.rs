//! Updates (the C# app's Updater): newer releases on github.com/Joshspeakman/driftwave, tagged `v1.0.N`.
//!
//! On Windows, an installed copy downloads the release's zip, checks its SHA-256 against the `.sha256` beside it,
//! unpacks it, and hands over to a small script that waits for Driftwave to quit, swaps the files in (keeping a
//! backup it restores on failure) and starts the new version. The checksum catches a broken download; it doesn't
//! prove who published the release, so only that repo's release downloads, over HTTPS, are fetched. Elsewhere (a
//! copy that isn't installed, or a Mac), a newer release is offered as its page.
//!
//! The check runs at launch at most once every 20 hours (Settings can turn it off). Test runs never contact GitHub:
//! they check only an `-updateFeed` (a file or URL with the same JSON) when given one.

use super::{finding, log, App, Upd, APP};
use driftwave_core::radio_browser::USER_AGENT;
use serde_json::{json, Value};
use slint::ComponentHandle;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

pub const REPO: &str = "Joshspeakman/driftwave";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Release {
    pub tag: String,
    pub version: [u32; 3],
    pub title: String,
    pub zip: Option<String>,
    pub size: u64,
    pub sha: Option<String>,
    pub page: String,
}

#[derive(Default)]
pub(crate) struct Updates {
    latest: Option<Release>,
    busy: bool,
}

/// "1.0.41" (or "1.0.41-beta.1": the number decides; the label is only shown).
pub(crate) fn parse_version(s: &str) -> Option<[u32; 3]> {
    let s = s.split(['-', '+']).next()?;
    let mut parts = s.split('.').map(|p| p.parse::<u32>().ok());
    let v = [parts.next()??, parts.next().unwrap_or(Some(0))?, parts.next().unwrap_or(Some(0))?];
    Some(v)
}

pub(crate) fn current() -> [u32; 3] {
    parse_version(env!("CARGO_PKG_VERSION")).unwrap_or([0, 0, 0])
}

fn shown(v: [u32; 3]) -> String {
    format!("{}.{}.{}", v[0], v[1], v[2])
}

/// The release zip for this computer.
const ASSET: &str = "-win-x64.zip";

/// Real runs download only this repo's release assets over HTTPS; test feeds may point anywhere.
fn allowed(url: &str, test: bool) -> bool {
    test || url.starts_with(&format!("https://github.com/{REPO}/releases/download/"))
}

/// Releases from GitHub's releases JSON: tagged `v…`, not drafts, with their Windows zip and checksum if they have one.
pub(crate) fn parse_releases(json: &str, test: bool) -> Vec<Release> {
    let Ok(Value::Array(list)) = serde_json::from_str::<Value>(json) else { return Vec::new() };
    let mut out = Vec::new();
    for r in list {
        if r.get("draft").and_then(Value::as_bool).unwrap_or(false) {
            continue;
        }
        let tag = r.get("tag_name").and_then(Value::as_str).unwrap_or("").to_string();
        let Some(version) = tag.strip_prefix('v').and_then(parse_version) else { continue };
        let (mut zip, mut sha, mut size) = (None, None, 0);
        for a in r.get("assets").and_then(Value::as_array).into_iter().flatten() {
            let name = a.get("name").and_then(Value::as_str).unwrap_or("");
            let url = a.get("browser_download_url").and_then(Value::as_str).unwrap_or("").to_string();
            if !name.starts_with("Driftwave-") || !allowed(&url, test) {
                continue;
            }
            if name.ends_with(ASSET) {
                zip = Some(url);
                size = a.get("size").and_then(Value::as_u64).unwrap_or(0);
            } else if name.ends_with(&format!("{ASSET}.sha256")) {
                sha = Some(url);
            }
        }
        let page = r.get("html_url").and_then(Value::as_str).filter(|p| p.starts_with(&format!("https://github.com/{REPO}/")) || test);
        out.push(Release {
            title: r.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()).unwrap_or(&tag).to_string(),
            page: page.map(String::from).unwrap_or_else(|| format!("https://github.com/{REPO}/releases/tag/{tag}")),
            tag,
            version,
            zip,
            size,
            sha,
        });
    }
    out
}

/// Releases from github.com's Atom feed (when the API is rate-limited): tags and titles; the zip and checksum are at
/// their usual download addresses.
pub(crate) fn parse_atom(xml: &str) -> Vec<Release> {
    let between = |s: &str, a: &str, b: &str| -> Option<String> {
        let i = s.find(a)? + a.len();
        let j = s[i..].find(b)? + i;
        Some(s[i..j].to_string())
    };
    xml.split("<entry>")
        .skip(1)
        .filter_map(|e| {
            let id = between(e, "<id>", "</id>")?;
            let tag = id.rsplit('/').next()?.to_string();
            let version = tag.strip_prefix('v').and_then(parse_version)?;
            let v = tag.trim_start_matches('v').split('-').next()?.to_string();
            let zip = format!("https://github.com/{REPO}/releases/download/{tag}/Driftwave-{v}{ASSET}");
            Some(Release {
                title: between(e, "<title>", "</title>").unwrap_or_else(|| tag.clone()),
                page: format!("https://github.com/{REPO}/releases/tag/{tag}"),
                sha: Some(format!("{zip}.sha256")),
                zip: Some(zip),
                size: 0,
                tag,
                version,
            })
        })
        .collect()
}

fn get(url: &str) -> Result<ureq::Response, ureq::Error> {
    ureq::get(url).set("User-Agent", USER_AGENT).set("Accept", "application/vnd.github+json").timeout(Duration::from_secs(30)).call()
}

fn is_url(s: &str) -> bool {
    s.starts_with("https://") || s.starts_with("http://")
}

fn read_text(src: &str) -> Result<String, String> {
    if is_url(src) {
        get(src).map_err(short)?.into_string().map_err(|e| e.to_string())
    } else {
        // A file written by Windows tools may start with a byte-order mark.
        std::fs::read_to_string(src).map(|t| t.trim_start_matches('\u{feff}').to_string()).map_err(|e| e.to_string())
    }
}

fn short(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(403 | 429, _) => "GitHub is limiting requests from your network right now. Try again in an hour.".into(),
        ureq::Error::Status(code, _) => format!("GitHub answered {code}."),
        ureq::Error::Transport(_) => "GitHub couldn't be reached. Check your internet connection.".into(),
    }
}

fn fetch_releases(feed: &str, test: bool) -> Result<Vec<Release>, String> {
    if feed.ends_with(".atom") {
        return Ok(parse_atom(&read_text(feed)?));
    }
    if !is_url(feed) {
        return Ok(parse_releases(&read_text(feed)?, test));
    }
    match get(feed) {
        Ok(r) => Ok(parse_releases(&r.into_string().map_err(|e| e.to_string())?, test)),
        Err(ureq::Error::Status(403 | 429, _)) if !test => {
            log("update check: rate-limited, using the releases feed");
            Ok(parse_atom(&read_text(&format!("https://github.com/{REPO}/releases.atom"))?))
        }
        Err(e) => Err(short(e)),
    }
}

/// Runs `f` with the app on the UI thread (from a worker).
fn post(f: impl FnOnce(&mut App) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || {
        APP.with(|a| {
            if let Some(app) = a.borrow().as_ref().and_then(|w| w.upgrade()) {
                f(&mut app.borrow_mut());
            }
        });
    });
}

/// The SHA-256 of a file, by Windows' own certutil (no hashing code to ship).
#[cfg(windows)]
fn sha256(file: &Path) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    let out = std::process::Command::new("certutil").arg("-hashfile").arg(file).arg("SHA256").creation_flags(0x0800_0000).output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .map(|l| l.replace(' ', "").to_ascii_lowercase())
        .find(|l| l.len() == 64 && l.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(|| "couldn't check the download".to_string())
}

/// Runs after Driftwave quits: back up, copy the new files in, restore on failure, start it again.
const APPLY_SCRIPT: &str = r#"param([int]$WaitPid, [string]$Source, [string]$Dest, [string]$Exe, [string]$Version, [string]$RegistryKey, [string]$Log)
function Write-Log([string]$m) { Add-Content -LiteralPath $Log -Value ("{0:u} {1}" -f (Get-Date).ToUniversalTime(), $m) }
Write-Log "update to ${Version}: waiting for ${WaitPid}"
try { Wait-Process -Id $WaitPid -Timeout 30 -ErrorAction SilentlyContinue } catch { }
# Only ever touch an installed copy, and only with a complete new one.
if (-not (Test-Path -LiteralPath (Join-Path $Dest 'uninstall.ps1')) -or -not (Test-Path -LiteralPath (Join-Path $Dest $Exe))) { Write-Log "not an install folder: $Dest"; exit 1 }
if (-not (Test-Path -LiteralPath (Join-Path $Source 'uninstall.ps1')) -or -not (Test-Path -LiteralPath (Join-Path $Source $Exe))) { Write-Log "incomplete download: $Source"; exit 1 }
$inside = $Dest.TrimEnd('\') + '\'
Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($inside, [StringComparison]::OrdinalIgnoreCase) } | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500
$backup = "$Dest.backup"
$ok = $false
if (Test-Path -LiteralPath $backup) { Remove-Item -LiteralPath $backup -Recurse -Force }
robocopy $Dest $backup /E /R:2 /W:1 /NFL /NDL /NJH /NJS /NP | Out-Null
if ($LASTEXITCODE -ge 8) { Write-Log "couldn't back up the current version (robocopy $LASTEXITCODE); leaving it as it is" }
else {
    try {
        robocopy $Source $Dest /MIR /XF Install.ps1 README.txt /R:3 /W:1 /NFL /NDL /NJH /NJS /NP | Out-Null
        if ($LASTEXITCODE -ge 8) { throw "copy failed (robocopy $LASTEXITCODE)" }
        if (-not (Test-Path -LiteralPath (Join-Path $Dest $Exe))) { throw "$Exe missing after the copy" }
        if ($RegistryKey -and (Test-Path $RegistryKey)) { Set-ItemProperty -Path $RegistryKey -Name DisplayVersion -Value $Version }
        $ok = $true
        Write-Log "updated to $Version"
    }
    catch {
        Write-Log "update failed: $_ (restoring the previous version)"
        robocopy $backup $Dest /MIR /R:3 /W:1 /NFL /NDL /NJH /NJS /NP | Out-Null
        Write-Log "restore: robocopy $LASTEXITCODE"
    }
    if ($ok) { Remove-Item -LiteralPath $backup -Recurse -Force -ErrorAction SilentlyContinue }
}
Start-Process -FilePath (Join-Path $Dest $Exe)
Write-Log "started $Exe"
Remove-Item -LiteralPath $Source -Recurse -Force -ErrorAction SilentlyContinue
"#;

impl App {
    fn upd_feed(&self) -> Option<String> {
        if self.args.test_run() {
            self.args.get("updateFeed").map(String::from)
        } else {
            Some(format!("https://api.github.com/repos/{REPO}/releases?per_page=30"))
        }
    }

    /// An installed Windows copy can replace itself.
    fn can_install(&self) -> bool {
        #[cfg(windows)]
        return driftwave_platform::winapp::installed();
        #[cfg(not(windows))]
        false
    }

    fn upd_show(&self, state: &str, message: &str) {
        let Some(ui) = self.ui.upgrade() else { return };
        let u = ui.global::<Upd>();
        u.set_state(state.into());
        u.set_message(message.into());
        u.set_can_install(self.can_install());
        u.set_auto(self.settings.get("checkForUpdates").and_then(Value::as_bool).unwrap_or(true));
    }

    pub(crate) fn show_updates(&self) {
        self.upd_show("idle", &format!("Driftwave {} looks for newer versions on GitHub.", shown(current())));
    }

    /// The launch check: at most once every 20 hours, unless turned off.
    pub(crate) fn check_updates_at_launch(&mut self) {
        let auto = self.settings.get("checkForUpdates").and_then(Value::as_bool).unwrap_or(true);
        if !auto || self.upd_feed().is_none() {
            return;
        }
        let recent = self
            .setting_str("lastUpdateCheck")
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok())
            .is_some_and(|t| chrono::Utc::now().signed_duration_since(t) < chrono::Duration::hours(20));
        if recent && !self.args.test_run() {
            return;
        }
        self.check_updates();
    }

    pub(crate) fn check_updates(&mut self) {
        if self.updates.busy {
            return;
        }
        let Some(feed) = self.upd_feed() else {
            self.upd_show("failed", "Test runs don't check for updates (give them -updateFeed to try it).");
            return;
        };
        self.updates.busy = true;
        self.upd_show("checking", "Checking for updates…");
        let test = self.args.test_run();
        let windows = cfg!(windows);
        finding::background(
            move || fetch_releases(&feed, test),
            move |app, result| {
                app.updates.busy = false;
                match result {
                    Ok(list) => {
                        if !app.args.test_run() {
                            app.settings["lastUpdateCheck"] = json!(chrono::Utc::now().to_rfc3339());
                            app.save_settings();
                        }
                        // On Windows only a release with its zip counts; a Mac is offered the page.
                        let best = list.into_iter().filter(|r| r.version > current() && (!windows || r.zip.is_some())).max_by_key(|r| r.version);
                        log(&format!("update check: current {}, latest {}", shown(current()), best.as_ref().map(|b| shown(b.version)).unwrap_or("none newer".into())));
                        match &best {
                            Some(b) => {
                                let name = if b.title.starts_with("Driftwave") { b.title.clone() } else { format!("Driftwave {}", shown(b.version)) };
                                app.upd_show("available", &format!("{name} is available (you have {}).", shown(current())));
                            }
                            None => app.upd_show("uptodate", &format!("You're up to date: Driftwave {}.", shown(current()))),
                        }
                        let found = best.is_some();
                        app.updates.latest = best;
                        // Test runs: -installUpdate goes straight on to install what was found.
                        if found && app.args.flag("installUpdate") {
                            app.install_update();
                        }
                    }
                    Err(e) => {
                        log(&format!("update check failed: {e}"));
                        app.upd_show("failed", &format!("Couldn't check for updates: {e}"));
                    }
                }
            },
        );
    }

    pub(crate) fn open_release_notes(&self) {
        let page = self.updates.latest.as_ref().map(|r| r.page.clone()).unwrap_or_else(|| format!("https://github.com/{REPO}/releases"));
        finding::open_url(&page);
    }

    /// Download and install (Windows, an installed copy); elsewhere, the release's page.
    pub(crate) fn install_update(&mut self) {
        let Some(release) = self.updates.latest.clone() else { return };
        if !self.can_install() {
            finding::open_url(&release.page);
            return;
        }
        if self.updates.busy {
            return;
        }
        #[cfg(windows)]
        self.download_and_install(release);
    }

    #[cfg(windows)]
    fn download_and_install(&mut self, release: Release) {
        let (Some(zip), Some(sha)) = (release.zip.clone(), release.sha.clone()) else {
            self.upd_show("failed", "Couldn't update: the release has no checksum file.");
            return;
        };
        let Ok(exe) = std::env::current_exe() else { return };
        let Some(dest) = exe.parent().map(Path::to_path_buf) else { return };
        let dir = self.paths.updates();
        let registry = if self.args.test_run() { String::new() } else { format!(r"HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{}", driftwave_platform::winapp::AUMID) };
        self.updates.busy = true;
        self.upd_show("downloading", &format!("Downloading Driftwave {}…", shown(release.version)));
        let version = shown(release.version);
        let _ = std::thread::Builder::new().name("driftwave-update".into()).spawn(move || {
            let work = || -> Result<(), String> {
                let _ = std::fs::remove_dir_all(&dir);
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let file = dir.join(format!("Driftwave-{version}.zip"));
                download(&zip, &file, release.size)?;
                let expected = read_text(&sha)?.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
                if sha256(&file)? != expected {
                    return Err("the download is damaged (its checksum doesn't match). Try again.".into());
                }
                post(|app| app.upd_show("installing", "Installing…"));
                let staged = dir.join("new");
                std::fs::create_dir_all(&staged).map_err(|e| e.to_string())?;
                // Windows' own tar unpacks zips.
                use std::os::windows::process::CommandExt;
                let ok = std::process::Command::new("tar").arg("-xf").arg(&file).arg("-C").arg(&staged).creation_flags(0x0800_0000).status().map_err(|e| e.to_string())?;
                let exe_name = exe.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Driftwave.exe".into());
                if !ok.success() || !staged.join(&exe_name).exists() {
                    return Err(format!("the download doesn't contain {exe_name}."));
                }
                let script = dir.join("apply-update.ps1");
                std::fs::write(&script, APPLY_SCRIPT).map_err(|e| e.to_string())?;
                std::process::Command::new("powershell.exe")
                    .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File"])
                    .arg(&script)
                    .arg("-WaitPid")
                    .arg(std::process::id().to_string())
                    .arg("-Source")
                    .arg(&staged)
                    .arg("-Dest")
                    .arg(&dest)
                    .arg("-Exe")
                    .arg(&exe_name)
                    .arg("-Version")
                    .arg(&version)
                    .arg("-RegistryKey")
                    .arg(&registry)
                    .arg("-Log")
                    .arg(dir.join("update.log"))
                    .creation_flags(0x0800_0000)
                    .spawn()
                    .map_err(|e| e.to_string())?;
                Ok(())
            };
            match work() {
                Ok(()) => {
                    log("update: installing, quitting");
                    let _ = slint::invoke_from_event_loop(|| {
                        APP.with(|a| {
                            if let Some(app) = a.borrow().as_ref().and_then(|w| w.upgrade()) {
                                crate::played::quit_app(&app);
                            }
                        });
                    });
                }
                Err(e) => post(move |app| {
                    app.updates.busy = false;
                    log(&format!("update failed: {e}"));
                    app.upd_show("failed", &format!("Couldn't update: {e}"));
                }),
            }
        });
    }

    pub(crate) fn set_auto_updates(&mut self, on: bool) {
        self.settings["checkForUpdates"] = json!(on);
        self.save_settings();
        if let Some(ui) = self.ui.upgrade() {
            ui.global::<Upd>().set_auto(on);
        }
    }
}

/// Downloads `url` (or copies a file, for test feeds) to `to`, posting the progress.
#[cfg(windows)]
fn download(url: &str, to: &Path, expected: u64) -> Result<(), String> {
    if !is_url(url) {
        std::fs::copy(url, to).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let resp = get(url).map_err(short)?;
    let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok()).unwrap_or(expected);
    let mut reader = resp.into_reader();
    let mut out = std::fs::File::create(to).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 64 * 1024];
    let (mut done, mut shown_pc) = (0u64, 0u64);
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if total > 0 {
            let pc = done * 100 / total;
            if pc > shown_pc {
                shown_pc = pc;
                post(move |app| {
                    if let Some(ui) = app.ui.upgrade() {
                        ui.global::<Upd>().set_progress(pc as f32 / 100.0);
                    }
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert_eq!(parse_version("1.0.41"), Some([1, 0, 41]));
        assert_eq!(parse_version("1.0.41-beta.1"), Some([1, 0, 41]));
        assert_eq!(parse_version("2"), Some([2, 0, 0]));
        assert!(parse_version("1.0.41").unwrap() > parse_version("1.0.9").unwrap());
        assert_eq!(parse_version("nightly"), None);
    }

    #[test]
    fn releases_need_their_own_zip_from_the_repo() {
        let json = r#"[
          {"tag_name":"v1.2.0","name":"Driftwave 1.2","draft":false,"html_url":"https://github.com/Joshspeakman/driftwave/releases/tag/v1.2.0",
           "assets":[{"name":"Driftwave-1.2.0-win-x64.zip","size":10,"browser_download_url":"https://github.com/Joshspeakman/driftwave/releases/download/v1.2.0/Driftwave-1.2.0-win-x64.zip"},
                     {"name":"Driftwave-1.2.0-win-x64.zip.sha256","size":1,"browser_download_url":"https://github.com/Joshspeakman/driftwave/releases/download/v1.2.0/Driftwave-1.2.0-win-x64.zip.sha256"}]},
          {"tag_name":"v1.3.0","draft":true,"assets":[]},
          {"tag_name":"v1.1.0","draft":false,"assets":[{"name":"Driftwave-1.1.0-win-x64.zip","size":10,"browser_download_url":"https://evil.example/Driftwave-1.1.0-win-x64.zip"}]},
          {"tag_name":"nightly-1.0.5","draft":false,"assets":[]}
        ]"#;
        let r = parse_releases(json, false);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].version, [1, 2, 0]);
        assert!(r[0].zip.is_some() && r[0].sha.is_some());
        assert_eq!(r[0].title, "Driftwave 1.2");
        // Another host's zip is never taken.
        assert_eq!(r[1].zip, None);
    }

    #[test]
    fn the_atom_feed_gives_tags_and_download_addresses() {
        let xml = r#"<feed><entry><id>tag:github.com,2008:Repository/1/v1.4.2</id><title>Driftwave 1.4.2</title></entry>
            <entry><id>tag:github.com,2008:Repository/1/notes</id><title>x</title></entry></feed>"#;
        let r = parse_atom(xml);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].version, [1, 4, 2]);
        assert_eq!(r[0].zip.as_deref(), Some("https://github.com/Joshspeakman/driftwave/releases/download/v1.4.2/Driftwave-1.4.2-win-x64.zip"));
    }
}
