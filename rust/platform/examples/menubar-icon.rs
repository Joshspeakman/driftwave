//! Writes the menu bar template icon as a PNG-ready raw file for checking: `cargo run --example menubar-icon <out.rgba>`.
fn main() {
    let (px, s) = driftwave_platform::tray::menu_bar_template(44);
    let out = std::env::args().nth(1).unwrap_or_else(|| "menubar.rgba".into());
    std::fs::write(&out, &px).unwrap();
    println!("{s}x{s} RGBA written to {out}");
}
