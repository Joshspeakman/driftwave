//! A live check of driftwave-platform with a real event loop: the menu bar icon and its menu, Now
//! Playing, single instance and notifications. Logs what happens; quits after `--seconds N` (default 20).
//!
//! `cargo run --example demo -p driftwave-platform -- [--media] [--seconds N]`
//! `--media` registers Now Playing and the media keys: only when nothing else is playing (CLAUDE.md).

use driftwave_platform::media::{Media, NowPlaying};
use driftwave_platform::notify::Notifier;
use driftwave_platform::single::{self, Instance};
use driftwave_platform::tray::{Tray, TrayState};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

slint::slint! {
    #[edition = "1.19"]
    export component Demo inherits Window {
        title: "driftwave-platform demo";
        width: 260px; height: 80px;
        Text { text: "driftwave-platform demo"; }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seconds = args.iter().position(|a| a == "--seconds").and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(20u64);
    let with_media = args.iter().any(|a| a == "--media");
    let Instance::Primary(listener) = single::claim("io.github.joshspeakman.Driftwave.PlatformDemo", "show") else {
        println!("instance: another demo is running; handed it \"show\"");
        return;
    };
    println!("instance: primary");
    println!("notify: available={}", Notifier::new().available());
    let _ = slint::BackendSelector::new().backend_name("winit".into()).renderer_name("software".into()).select();
    let ui = Demo::new().expect("window");
    let state: Rc<RefCell<(Option<Tray>, Option<Media>)>> = Rc::new(RefCell::new((None, None)));
    {
        let state = state.clone();
        // Created once the event loop runs, on the main thread (both need that on macOS).
        slint::Timer::single_shot(Duration::from_millis(300), move || {
            let (logo, w, h) = (vec![255u8; 32 * 32 * 4], 32, 32);
            match Tray::new(logo, w, h, "Driftwave") {
                Ok(mut t) => {
                    t.update(&TrayState {
                        station: Some("SomaFM Groove Salad".into()),
                        track: Some("Kalabi – Diaphanous".into()),
                        playing: true,
                        stations: vec!["SomaFM Groove Salad".into(), "Nightride FM".into()],
                        current: Some(0),
                    });
                    println!("tray: created");
                    state.borrow_mut().0 = Some(t);
                }
                Err(e) => println!("tray: failed: {e}"),
            }
            if with_media {
                match Media::new("Driftwave", "driftwave", None) {
                    Ok(mut m) => {
                        m.update(&NowPlaying { title: "Diaphanous", artist: Some("Kalabi"), station: "SomaFM Groove Salad", cover_url: None, playing: true });
                        println!("media: now playing set");
                        state.borrow_mut().1 = Some(m);
                    }
                    Err(e) => println!("media: failed: {e}"),
                }
            }
        });
    }
    let timer = slint::Timer::default();
    {
        let state = state.clone();
        timer.start(slint::TimerMode::Repeated, Duration::from_millis(100), move || {
            let s = state.borrow();
            if let Some(t) = &s.0 {
                for a in t.actions() {
                    println!("tray action: {a:?}");
                }
            }
            if let Some(m) = &s.1 {
                for c in m.commands() {
                    println!("media command: {c:?}");
                }
            }
            for msg in listener.messages.try_iter() {
                println!("instance message: {msg}");
            }
        });
    }
    slint::Timer::single_shot(Duration::from_secs(seconds), || {
        let _ = slint::quit_event_loop();
    });
    ui.show().expect("show");
    slint::run_event_loop_until_quit().expect("event loop");
    println!("done");
}
