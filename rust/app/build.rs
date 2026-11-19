fn main() {
    slint_build::compile("ui/app.slint").expect("compile the Classic layout");
    windows_resources();
}

/// Windows: the exe's icon and version (Explorer, the Start-menu shortcut and Settings > Apps show them), compiled
/// with the Windows SDK's resource compiler. Without the SDK the exe is built without them.
fn windows_resources() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") || !cfg!(windows) {
        return;
    }
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon = manifest.join("../../assets/icons/driftwave.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    let Some(rc) = find_rc() else {
        println!("cargo:warning=No Windows SDK resource compiler (rc.exe): the exe has no icon or version.");
        return;
    };
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let mut n: Vec<u32> = version.split(['.', '-']).take(3).map(|p| p.parse().unwrap_or(0)).collect();
    n.resize(4, 0);
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let script = out.join("driftwave.rc");
    let icon_path = icon.canonicalize().unwrap_or(icon).display().to_string().replace("\\\\?\\", "").replace('\\', "\\\\");
    let text = format!(
        r#"#pragma code_page(65001)
1 ICON "{icon_path}"
1 VERSIONINFO
FILEVERSION {a},{b},{c},{d}
PRODUCTVERSION {a},{b},{c},{d}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "Josh Speakman"
      VALUE "FileDescription", "Driftwave"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "Driftwave"
      VALUE "OriginalFilename", "Driftwave.exe"
      VALUE "ProductName", "Driftwave"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        a = n[0],
        b = n[1],
        c = n[2],
        d = n[3],
    );
    std::fs::write(&script, text).unwrap();
    let res = out.join("driftwave.res");
    let ok = std::process::Command::new(&rc).arg("/nologo").arg("/fo").arg(&res).arg(&script).status().map(|s| s.success()).unwrap_or(false);
    if ok {
        println!("cargo:rustc-link-arg-bins={}", res.display());
    } else {
        println!("cargo:warning=The resource compiler failed: the exe has no icon or version.");
    }
}

/// The newest x64 rc.exe in the Windows 10/11 SDK.
fn find_rc() -> Option<std::path::PathBuf> {
    let base = std::path::Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut found: Vec<std::path::PathBuf> = std::fs::read_dir(base).ok()?.flatten().map(|e| e.path().join("x64").join("rc.exe")).filter(|p| p.exists()).collect();
    found.sort();
    found.pop()
}
