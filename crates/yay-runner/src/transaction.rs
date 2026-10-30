//! Runs a planned transaction: one yay command after another in a PTY, with
//! the state machine from `yaygui-core` and a single event stream for the UI.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use yaygui_core::{PlannedCommand, TransactionEvent, TransactionState};
use zeroize::Zeroizing;

use crate::{ExitStatus, PromptDetector, PromptEvent, RunError, RunHandle, RunSpec, Runner};

/// How long [`TxHandle::cancel`] waits after Ctrl-C before killing the child.
pub const CANCEL_GRACE: Duration = Duration::from_secs(5);

/// Something that happened during a transaction, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxEvent {
    /// The transaction state changed.
    State(TransactionState),
    /// Command `index` (0-based, of `total`) started.
    CommandStarted {
        /// Position in the plan.
        index: usize,
        /// Number of commands in the plan.
        total: usize,
        /// Planner description.
        description: String,
        /// Program plus arguments as actually spawned.
        argv: Vec<String>,
    },
    /// Raw terminal output of the running command.
    Output(Vec<u8>),
    /// A recognised prompt or error line.
    Prompt(PromptEvent),
    /// Command `index` exited.
    CommandFinished {
        /// Position in the plan.
        index: usize,
        /// Exit status.
        status: ExitStatus,
    },
}

/// Settings shared by every command of a transaction.
#[derive(Debug, Clone)]
pub struct TxConfig {
    /// Program to run instead of `PlannedCommand::program` (tests use a fake yay).
    pub program_override: Option<String>,
    /// Extra environment (for example the review overlay's `XDG_CONFIG_HOME`).
    pub env: Vec<(String, String)>,
    /// Initial terminal size (rows, cols).
    pub size: (u16, u16),
}

impl Default for TxConfig {
    fn default() -> Self {
        Self {
            program_override: None,
            env: Vec::new(),
            size: (24, 80),
        }
    }
}

struct Shared {
    current: Mutex<Option<Arc<RunHandle>>>,
    cancelled: AtomicBool,
    size: Mutex<(u16, u16)>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Controls a running transaction. Cheap to share behind an `Arc`.
pub struct TxHandle {
    shared: Arc<Shared>,
    thread: Mutex<Option<JoinHandle<TransactionState>>>,
}

/// Starts `commands` on a background thread. `sink` receives every
/// [`TxEvent`]; the last one is always a terminal [`TxEvent::State`].
///
/// The transaction begins in `Approved` (the review happened in the UI) and
/// stops at the first command that fails or when cancelled.
pub fn start_transaction(
    commands: Vec<PlannedCommand>,
    config: TxConfig,
    sink: impl FnMut(TxEvent) + Send + 'static,
) -> TxHandle {
    let shared = Arc::new(Shared {
        current: Mutex::new(None),
        cancelled: AtomicBool::new(false),
        size: Mutex::new(config.size),
    });
    let s = Arc::clone(&shared);
    let thread = std::thread::spawn(move || run_all(&commands, &config, &s, sink));
    TxHandle {
        shared,
        thread: Mutex::new(Some(thread)),
    }
}

fn run_all(
    commands: &[PlannedCommand],
    config: &TxConfig,
    shared: &Shared,
    sink: impl FnMut(TxEvent) + Send + 'static,
) -> TransactionState {
    let sink = Arc::new(Mutex::new(sink));
    let emit = |e: TxEvent| (lock(&sink))(e);

    let mut state = TransactionState::Approved;
    let step = |state: &mut TransactionState, ev: TransactionEvent| {
        // Every transition below is valid by construction; keep the old state otherwise.
        if let Ok(next) = state.transition(ev) {
            *state = next;
            emit(TxEvent::State(state.clone()));
        }
    };
    step(&mut state, TransactionEvent::Start);

    for (index, cmd) in commands.iter().enumerate() {
        if shared.cancelled.load(Ordering::SeqCst) {
            step(&mut state, TransactionEvent::Cancel);
            return state;
        }
        let program = config
            .program_override
            .clone()
            .unwrap_or_else(|| cmd.program.clone());
        let mut argv = vec![program.clone()];
        argv.extend(cmd.args.iter().cloned());
        emit(TxEvent::CommandStarted {
            index,
            total: commands.len(),
            description: cmd.description.clone(),
            argv,
        });

        let mut spec = RunSpec::new(program, cmd.args.clone());
        spec.env.clone_from(&config.env);
        spec.size = *lock(&shared.size);
        let out_sink = Arc::clone(&sink);
        let mut detector = PromptDetector::new();
        let spawned = Runner::spawn(spec, move |bytes| {
            let mut s = lock(&out_sink);
            s(TxEvent::Output(bytes.to_vec()));
            for p in detector.feed(bytes) {
                s(TxEvent::Prompt(p));
            }
        });
        let handle = match spawned {
            Ok(h) => Arc::new(h),
            Err(e) => {
                step(
                    &mut state,
                    TransactionEvent::Fail {
                        reason: format!("could not start {}: {e}", cmd.description),
                    },
                );
                return state;
            }
        };
        *lock(&shared.current) = Some(Arc::clone(&handle));
        // A cancel that raced with the spawn must still reach this child.
        if shared.cancelled.load(Ordering::SeqCst) {
            let _ = handle.interrupt();
        }
        let status = handle.wait();
        *lock(&shared.current) = None;

        let status = match status {
            Ok(s) => s,
            Err(e) => {
                step(
                    &mut state,
                    TransactionEvent::Fail {
                        reason: format!("lost track of {}: {e}", cmd.description),
                    },
                );
                return state;
            }
        };
        emit(TxEvent::CommandFinished {
            index,
            status: status.clone(),
        });
        if shared.cancelled.load(Ordering::SeqCst) {
            step(&mut state, TransactionEvent::Cancel);
            return state;
        }
        if !status.success() {
            let how = status
                .signal
                .as_ref()
                .map_or_else(|| format!("exit code {}", status.code), |s| s.clone());
            step(
                &mut state,
                TransactionEvent::Fail {
                    reason: format!("{} failed ({how})", cmd.description),
                },
            );
            return state;
        }
    }
    step(&mut state, TransactionEvent::Finish);
    state
}

impl TxHandle {
    fn with_current<T>(
        &self,
        f: impl FnOnce(&RunHandle) -> Result<T, RunError>,
    ) -> Result<T, RunError> {
        let cur = lock(&self.shared.current).clone();
        match cur {
            Some(h) => f(&h),
            None => Err(RunError::Pty("no command is running".into())),
        }
    }

    /// Sends a password to the running command (zeroized afterwards).
    ///
    /// # Errors
    /// Fails when no command is running or the PTY is closed.
    pub fn answer_password(&self, password: Zeroizing<String>) -> Result<(), RunError> {
        self.with_current(|h| h.answer_password(password))
    }

    /// Answers a `[Y/n]` question.
    ///
    /// # Errors
    /// Fails when no command is running or the PTY is closed.
    pub fn answer_yes_no(&self, yes: bool) -> Result<(), RunError> {
        self.with_current(|h| h.answer_yes_no(yes))
    }

    /// Writes raw keystrokes to the running command.
    ///
    /// # Errors
    /// Fails when no command is running or the PTY is closed.
    pub fn write_input(&self, data: &[u8]) -> Result<(), RunError> {
        self.with_current(|h| h.write_input(data))
    }

    /// Resizes the current and all later commands' terminals.
    ///
    /// # Errors
    /// Fails if the running PTY rejects the size.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), RunError> {
        *lock(&self.shared.size) = (rows, cols);
        match lock(&self.shared.current).clone() {
            Some(h) => h.resize(rows, cols),
            None => Ok(()),
        }
    }

    /// Cancels: no further commands start, the running one gets Ctrl-C and is
    /// killed if it is still alive after [`CANCEL_GRACE`]. Returns immediately.
    pub fn cancel(&self) {
        self.shared.cancelled.store(true, Ordering::SeqCst);
        let Some(h) = lock(&self.shared.current).clone() else {
            return;
        };
        let _ = h.interrupt();
        std::thread::spawn(move || {
            let deadline = Instant::now() + CANCEL_GRACE;
            while Instant::now() < deadline {
                if matches!(h.try_wait(), Ok(Some(_)) | Err(_)) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = h.kill();
        });
    }

    /// True once the background thread has finished.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        lock(&self.thread)
            .as_ref()
            .is_none_or(JoinHandle::is_finished)
    }

    /// Blocks until the transaction ends and returns its final state.
    /// Returns `None` if called twice or the worker panicked.
    pub fn wait(&self) -> Option<TransactionState> {
        let t = lock(&self.thread).take()?;
        t.join().ok()
    }
}
