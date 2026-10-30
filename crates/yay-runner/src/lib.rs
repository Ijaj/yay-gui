//! `yaygui-yay-runner`: runs yay (or any program) in a real PTY, forwards the
//! raw byte stream unchanged, and recognises interactive prompts and common
//! failures so the UI can show native dialogs.
//!
//! - [`Runner::spawn`] starts a program from an argument array (never a shell
//!   string) and returns a [`RunHandle`].
//! - [`PromptDetector`] is an incremental parser producing [`PromptEvent`]s.
//! - [`pacman_lock_held`] checks for a stale/active pacman db lock.
//! - [`Overlay`] builds a private `XDG_CONFIG_HOME` that pins reviewed commits.
//! - [`start_transaction`] runs a whole plan, command by command, as one
//!   stream of [`TxEvent`]s driven by the core state machine.

mod detector;
mod lock;
mod overlay;
mod runner;
mod transaction;

pub use detector::{ErrorKind, PromptDetector, PromptEvent};
pub use lock::pacman_lock_held;
pub use overlay::{Overlay, OverlayError};
pub use runner::{ExitStatus, RunError, RunHandle, RunSpec, Runner};
pub use transaction::{CANCEL_GRACE, TxConfig, TxEvent, TxHandle, start_transaction};
