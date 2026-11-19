//! One running copy per flavour. The first copy listens on a local socket; a second copy hands its
//! message ("show", "quit", or a URL to play) to the first and exits. `driftwave --quit` uses this.
//!
//! macOS and Linux: a Unix socket in the temporary directory. Windows: not yet (the C# app used a
//! named pipe; `claim` always says Primary there until it's ported).

use std::sync::mpsc::Receiver;

/// What `claim` found.
pub enum Instance {
    /// This is the only copy. Messages from later copies arrive on the receiver, one per line.
    Primary(Listener),
    /// Another copy is running and has been handed the message: exit.
    Secondary,
}

pub struct Listener {
    pub messages: Receiver<String>,
    #[cfg(unix)]
    path: std::path::PathBuf,
}

impl Drop for Listener {
    fn drop(&mut self) {
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Claims the single-instance slot named `name` (e.g. "io.github.joshspeakman.Driftwave"), or hands
/// `message` to the copy that holds it.
#[cfg(unix)]
pub fn claim(name: &str, message: &str) -> Instance {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::sync::mpsc;
    use std::time::Duration;

    let path = std::env::temp_dir().join(format!("{name}.sock"));
    if let Ok(mut s) = UnixStream::connect(&path) {
        let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
        if s.write_all(format!("{message}\n").as_bytes()).is_ok() {
            return Instance::Secondary;
        }
    }
    // Nobody answered: a stale socket from a copy that crashed.
    let _ = std::fs::remove_file(&path);
    let (tx, rx) = mpsc::channel();
    match UnixListener::bind(&path) {
        Ok(listener) => {
            std::thread::Builder::new()
                .name("driftwave-instance".into())
                .spawn(move || {
                    for stream in listener.incoming().flatten() {
                        for line in BufReader::new(stream).lines().map_while(Result::ok) {
                            if !line.trim().is_empty() && tx.send(line.trim().to_string()).is_err() {
                                return;
                            }
                        }
                    }
                })
                .expect("instance thread");
        }
        Err(_) => drop(tx), // can't listen (e.g. read-only temp dir): run anyway, alone
    }
    Instance::Primary(Listener { messages: rx, path })
}

#[cfg(not(unix))]
pub fn claim(_name: &str, _message: &str) -> Instance {
    let (_tx, rx) = std::sync::mpsc::channel();
    Instance::Primary(Listener { messages: rx })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn second_copy_hands_its_message_to_the_first() {
        let name = format!("driftwave-test-{}", std::process::id());
        let Instance::Primary(first) = claim(&name, "show") else { panic!("first copy must be primary") };
        assert!(matches!(claim(&name, "quit"), Instance::Secondary));
        assert_eq!(first.messages.recv_timeout(Duration::from_secs(2)).as_deref(), Ok("quit"));
        drop(first);
        // Once the first copy is gone, the slot is free again.
        let Instance::Primary(_again) = claim(&name, "show") else { panic!("slot should be free") };
    }

    #[test]
    fn a_stale_socket_is_taken_over() {
        let name = format!("driftwave-stale-{}", std::process::id());
        let path = std::env::temp_dir().join(format!("{name}.sock"));
        std::fs::write(&path, b"").unwrap(); // left behind by a crash
        assert!(matches!(claim(&name, "show"), Instance::Primary(_)));
    }
}
