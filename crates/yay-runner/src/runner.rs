use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use zeroize::Zeroizing;

/// Errors from spawning or driving a PTY process.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// Opening the PTY or spawning the child failed.
    #[error("pty error: {0}")]
    Pty(String),
    /// Reading from or writing to the PTY failed.
    #[error("pty i/o error: {0}")]
    Io(#[from] std::io::Error),
}

impl RunError {
    fn pty(e: impl std::fmt::Display) -> Self {
        Self::Pty(e.to_string())
    }
}

/// What to run. `program` + `args` are passed as an argument array, never via a shell.
#[derive(Debug, Clone)]
pub struct RunSpec {
    /// Executable name or path.
    pub program: String,
    /// Arguments, one element per argv entry.
    pub args: Vec<String>,
    /// Environment variables to set (on top of the inherited environment).
    pub env: Vec<(String, String)>,
    /// Environment variables to remove.
    pub env_remove: Vec<String>,
    /// Working directory; inherited when `None`.
    pub cwd: Option<PathBuf>,
    /// Initial terminal size as (rows, cols).
    pub size: (u16, u16),
}

impl RunSpec {
    /// A spec with an 80x24 terminal, no env changes and the current directory.
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
            env: Vec::new(),
            env_remove: Vec::new(),
            cwd: None,
            size: (24, 80),
        }
    }
}

/// How the child ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitStatus {
    /// Exit code (non-zero when killed by a signal, depending on platform).
    pub code: u32,
    /// Signal name if the child was terminated by one.
    pub signal: Option<String>,
}

impl ExitStatus {
    /// True for exit code 0 without a signal.
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == 0 && self.signal.is_none()
    }
}

/// Spawns programs inside a PTY.
pub struct Runner;

impl Runner {
    /// Spawns `spec` in a PTY. Every chunk the child writes is passed unchanged to
    /// `on_output` from a background reader thread (feed it to xterm.js and a
    /// [`PromptDetector`](crate::PromptDetector)).
    ///
    /// # Errors
    /// Fails if the PTY cannot be opened or the program cannot be started.
    pub fn spawn(
        spec: RunSpec,
        mut on_output: impl FnMut(&[u8]) + Send + 'static,
    ) -> Result<RunHandle, RunError> {
        let pair = native_pty_system()
            .openpty(size(spec.size))
            .map_err(RunError::pty)?;

        let mut cmd = CommandBuilder::new(&spec.program);
        cmd.args(&spec.args);
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }
        for k in &spec.env_remove {
            cmd.env_remove(OsString::from(k));
        }
        if let Some(cwd) = &spec.cwd {
            cmd.cwd(cwd);
        }

        let child = pair.slave.spawn_command(cmd).map_err(RunError::pty)?;
        // Drop our slave end so the master sees EOF when the child exits.
        drop(pair.slave);
        let killer = child.clone_killer();
        let mut reader = pair.master.try_clone_reader().map_err(RunError::pty)?;
        let writer = pair.master.take_writer().map_err(RunError::pty)?;

        let reader_thread = std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            // EIO on Linux (or Ok(0)) means the child side closed.
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                on_output(&buf[..n]);
            }
        });

        Ok(RunHandle {
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            killer: Mutex::new(killer),
            child: Mutex::new(child),
            reader: Mutex::new(Some(reader_thread)),
        })
    }
}

fn size((rows, cols): (u16, u16)) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// Handle to a running PTY child. All methods take `&self`, so it can be shared via `Arc`.
pub struct RunHandle {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    child: Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
    reader: Mutex<Option<JoinHandle<()>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl RunHandle {
    /// Writes raw bytes (keystrokes) to the child's terminal.
    ///
    /// # Errors
    /// Fails if the PTY is closed.
    pub fn write_input(&self, data: &[u8]) -> Result<(), RunError> {
        let mut w = lock(&self.writer);
        w.write_all(data)?;
        w.flush()?;
        Ok(())
    }

    /// Writes `password` + `"\n"` to the PTY and zeroizes every buffer used.
    ///
    /// # Errors
    /// Fails if the PTY is closed.
    pub fn answer_password(&self, password: Zeroizing<String>) -> Result<(), RunError> {
        let mut bytes = Zeroizing::new(Vec::with_capacity(password.len() + 1));
        bytes.extend_from_slice(password.as_bytes());
        bytes.push(b'\n');
        drop(password);
        self.write_input(&bytes)
    }

    /// Answers a yes/no prompt with `y` or `n` plus newline.
    ///
    /// # Errors
    /// Fails if the PTY is closed.
    pub fn answer_yes_no(&self, yes: bool) -> Result<(), RunError> {
        self.write_input(if yes { b"y\n" } else { b"n\n" })
    }

    /// Resizes the terminal (rows, cols).
    ///
    /// # Errors
    /// Fails if the PTY rejects the new size.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), RunError> {
        lock(&self.master)
            .resize(size((rows, cols)))
            .map_err(RunError::pty)
    }

    /// Graceful cancel: sends Ctrl-C through the terminal so the foreground
    /// process group (yay, makepkg, ...) receives SIGINT.
    ///
    /// # Errors
    /// Fails if the PTY is closed.
    pub fn interrupt(&self) -> Result<(), RunError> {
        self.write_input(&[0x03])
    }

    /// Forcefully kills the direct child. Its descendants may survive; prefer
    /// [`interrupt`](Self::interrupt) first.
    ///
    /// # Errors
    /// Fails if the kill request fails (e.g. the child already exited).
    pub fn kill(&self) -> Result<(), RunError> {
        lock(&self.killer).kill().map_err(RunError::Io)
    }

    /// Non-blocking exit check.
    ///
    /// # Errors
    /// Fails if the child state cannot be queried.
    pub fn try_wait(&self) -> Result<Option<ExitStatus>, RunError> {
        Ok(lock(&self.child).try_wait()?.map(convert))
    }

    /// Blocks until the child exits, then drains the output reader (up to 2 s).
    ///
    /// # Errors
    /// Fails if waiting on the child fails.
    pub fn wait(&self) -> Result<ExitStatus, RunError> {
        // Poll instead of blocking inside the lock so other methods stay usable.
        let status = loop {
            if let Some(s) = self.try_wait()? {
                break s;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let handle = lock(&self.reader).take();
        if let Some(h) = handle {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !h.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            if h.is_finished() {
                let _ = h.join();
            }
        }
        Ok(status)
    }
}

fn convert(s: portable_pty::ExitStatus) -> ExitStatus {
    ExitStatus {
        code: s.exit_code(),
        signal: s.signal().map(str::to_owned),
    }
}
