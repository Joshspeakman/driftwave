//! One running copy per flavour. The first copy listens on a local socket; a second copy hands its
//! message ("show", "quit", or a URL to play) to the first and exits. `driftwave --quit` uses this.
//!
//! macOS and Linux: a Unix socket in the temporary directory. Windows: a named pipe for this user (as the C# app
//! did), created with FILE_FLAG_FIRST_PIPE_INSTANCE so only one copy can own it.

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

#[cfg(windows)]
pub fn claim(name: &str, message: &str) -> Instance {
    use std::io::{BufRead, BufReader, Write};
    use std::os::windows::io::{FromRawHandle, RawHandle};
    use std::sync::mpsc;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_NO_DATA, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND};
    use windows_sys::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT};

    // Pipes are machine-wide: one per user.
    let user: String = std::env::var("USERNAME").unwrap_or_default().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let path = format!(r"\\.\pipe\{name}-{user}");
    // Someone's listening: hand them the message.
    let hand_over = || -> bool {
        match std::fs::OpenOptions::new().write(true).open(&path) {
            Ok(mut pipe) => pipe.write_all(format!("{message}\n").as_bytes()).is_ok(),
            Err(_) => false,
        }
    };
    if hand_over() {
        return Instance::Secondary;
    }
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let create = |first: bool| {
        let flags = PIPE_ACCESS_INBOUND | if first { FILE_FLAG_FIRST_PIPE_INSTANCE } else { 0 };
        // SAFETY: `wide` is a NUL-terminated UTF-16 name; no security attributes (the default: this user).
        unsafe { CreateNamedPipeW(wide.as_ptr(), flags, PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT, PIPE_UNLIMITED_INSTANCES, 0, 4096, 0, std::ptr::null()) }
    };
    let (tx, rx) = mpsc::channel();
    let first = create(true);
    if first == INVALID_HANDLE_VALUE {
        // Another copy owns the pipe and is between callers (busy): try again for a moment.
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if hand_over() {
                return Instance::Secondary;
            }
        }
        // It never answered: run alone.
        drop(tx);
        return Instance::Primary(Listener { messages: rx });
    }
    let first = first as usize;
    let wide_for_thread = wide.clone();
    let _ = std::thread::Builder::new().name("driftwave-instance".into()).spawn(move || {
        let mut handle = first as RawHandle;
        loop {
            // SAFETY: `handle` is a pipe instance we created and still own.
            // A copy that wrote and left before we got here still left its message in the pipe (ERROR_NO_DATA).
            let connected = unsafe { ConnectNamedPipe(handle as _, std::ptr::null_mut()) } != 0 || matches!(unsafe { GetLastError() }, ERROR_PIPE_CONNECTED | ERROR_NO_DATA);
            // The next copy's instance is ready before this one's message is read, so there's always one waiting.
            // SAFETY: as above.
            let next = unsafe {
                CreateNamedPipeW(wide_for_thread.as_ptr(), PIPE_ACCESS_INBOUND, PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT, PIPE_UNLIMITED_INSTANCES, 0, 4096, 0, std::ptr::null())
            };
            if connected {
                // SAFETY: the File takes over the handle and closes it when dropped.
                let file = unsafe { std::fs::File::from_raw_handle(handle) };
                for line in BufReader::new(file).lines().map_while(Result::ok) {
                    if !line.trim().is_empty() && tx.send(line.trim().to_string()).is_err() {
                        return;
                    }
                }
            } else {
                // SAFETY: closing the instance we created.
                unsafe { CloseHandle(handle as _) };
            }
            if next == INVALID_HANDLE_VALUE {
                return;
            }
            handle = next as RawHandle;
        }
    });
    Instance::Primary(Listener { messages: rx })
}

#[cfg(not(any(unix, windows)))]
pub fn claim(_name: &str, _message: &str) -> Instance {
    let (_tx, rx) = std::sync::mpsc::channel();
    Instance::Primary(Listener { messages: rx })
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn second_copy_hands_its_message_to_the_first() {
        let name = format!("driftwave-test-{}", std::process::id());
        let Instance::Primary(first) = claim(&name, "show") else { panic!("first copy must be primary") };
        assert!(matches!(claim(&name, "quit"), Instance::Secondary));
        assert_eq!(first.messages.recv_timeout(Duration::from_secs(2)).as_deref(), Ok("quit"));
        // And again: the listener makes a fresh instance for each copy.
        assert!(matches!(claim(&name, "https://example.com/stream"), Instance::Secondary));
        assert_eq!(first.messages.recv_timeout(Duration::from_secs(2)).as_deref(), Ok("https://example.com/stream"));
    }
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
