//! Finding the folders the app ships with (`themes`, `theme-art`).
//!
//! In order:
//! - next to the program (the Windows zip, Linux);
//! - `Contents/Resources` of the macOS bundle;
//! - up the tree from the program (a build inside the repository);
//! - the repository the program was built from (a development build whose target folder is elsewhere,
//!   e.g. the Mac's `CARGO_TARGET_DIR` in ~/ProjectBuilds).

use std::path::{Path, PathBuf};

/// The folder `name` that contains `marker`.
pub fn find(name: &str, marker: &str) -> Option<PathBuf> {
    let ok = |p: PathBuf| p.join(marker).exists().then_some(p);
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    if let Some(p) = ok(exe_dir.join(name)) {
        return Some(p);
    }
    if let Some(p) = ok(exe_dir.join("../Resources").join(name)) {
        return Some(p);
    }
    let mut dir = Some(exe_dir.to_path_buf());
    while let Some(d) = dir {
        for p in [d.join(name), d.join("assets").join(name)] {
            if let Some(p) = ok(p) {
                return Some(p);
            }
        }
        dir = d.parent().map(Path::to_path_buf);
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    [repo.join(name), repo.join("assets").join(name)].into_iter().find_map(ok)
}
