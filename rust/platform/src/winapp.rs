//! Windows: Driftwave's AppUserModelID, and the Start-menu shortcut that carries it.
//!
//! Windows shows an app's notifications, and names it in the media overlay and on the taskbar, by its
//! AppUserModelID. A desktop app gets one through a Start-menu shortcut with that id; `Install.ps1` makes the
//! shortcut by running `Driftwave.exe --create-shortcut <path>`. An installed copy (its folder has `uninstall.ps1`)
//! sets the id on itself at launch, so its windows and notifications are matched to that shortcut.

use std::path::{Path, PathBuf};

pub const AUMID: &str = "Joshspeakman.Driftwave";

/// The folder this copy runs from.
pub fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf))
}

/// An installed copy: `Install.ps1` put it (and `uninstall.ps1` beside it) in its own folder.
pub fn installed() -> bool {
    exe_dir().is_some_and(|d| d.join("uninstall.ps1").exists())
}

/// The Start-menu shortcut that carries the id.
pub fn shortcut_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs\Driftwave.lnk"))
}

/// This process is Driftwave (before any window is made).
pub fn set_app_id() {
    // SAFETY: a plain call with a valid string; it only affects this process.
    unsafe {
        let _ = windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(&windows::core::HSTRING::from(AUMID));
    }
}

/// A shortcut at `lnk` to `exe`, carrying the app id (and `icon`'s picture, if given).
pub fn create_shortcut(lnk: &Path, exe: &Path, icon: Option<&Path>) -> Result<(), String> {
    use std::mem::ManuallyDrop;
    use windows::core::{Interface, HSTRING, PWSTR};
    use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
    use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemAlloc, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
    use windows::Win32::System::Variant::VT_LPWSTR;
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    let err = |what: &str, e: windows::core::Error| format!("{what}: {e}");
    // SAFETY: COM calls on this thread. The id's string is COM memory, freed by PropVariantClear.
    let done = unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let made = (|| -> Result<(), String> {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|e| err("shell link", e))?;
        link.SetPath(&HSTRING::from(exe.as_os_str())).map_err(|e| err("path", e))?;
        if let Some(dir) = exe.parent() {
            link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str())).map_err(|e| err("folder", e))?;
        }
        if let Some(i) = icon {
            link.SetIconLocation(&HSTRING::from(i.as_os_str()), 0).map_err(|e| err("icon", e))?;
        }
        link.SetDescription(&HSTRING::from("Internet radio")).map_err(|e| err("description", e))?;
        let store: IPropertyStore = link.cast().map_err(|e| err("properties", e))?;
        let wide: Vec<u16> = AUMID.encode_utf16().chain(Some(0)).collect();
        let text = CoTaskMemAlloc(wide.len() * 2) as *mut u16;
        if text.is_null() {
            return Err("out of memory".into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), text, wide.len());
        let mut value = PROPVARIANT {
            Anonymous: PROPVARIANT_0 {
                Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                    vt: VT_LPWSTR,
                    wReserved1: 0,
                    wReserved2: 0,
                    wReserved3: 0,
                    Anonymous: PROPVARIANT_0_0_0 { pwszVal: PWSTR(text) },
                }),
            },
        };
        let set = store.SetValue(&PKEY_AppUserModel_ID, &value);
        let _ = PropVariantClear(&mut value);
        set.map_err(|e| err("app id", e))?;
        store.Commit().map_err(|e| err("commit", e))?;
        let file: IPersistFile = link.cast().map_err(|e| err("file", e))?;
        if let Some(dir) = lnk.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        file.Save(&HSTRING::from(lnk.as_os_str()), true).map_err(|e| err("save", e))?;
        Ok(())
        })();
        CoUninitialize();
        made
    };
    done
}

/// A notification under the app's name (replacing the last one with the same tag), without a sound.
pub fn toast(title: &str, lines: &[&str], tag: &str) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    let mut texts = format!("<text>{}</text>", esc(title));
    for l in lines.iter().filter(|l| !l.is_empty()) {
        texts.push_str(&format!("<text>{}</text>", esc(l)));
    }
    let xml = format!(r#"<toast><visual><binding template="ToastGeneric">{texts}</binding></visual><audio silent="true"/></toast>"#);
    let run = || -> windows::core::Result<()> {
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(xml.as_str()))?;
        let t = ToastNotification::CreateToastNotification(&doc)?;
        t.SetTag(&HSTRING::from(tag))?;
        t.SetGroup(&HSTRING::from("driftwave"))?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(AUMID))?.Show(&t)
    };
    run().map_err(|e| e.to_string())
}
