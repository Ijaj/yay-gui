//! `yaygui-yay-runner`: runs yay (or any program) in a real PTY, forwards the
//! raw byte stream unchanged, and recognises interactive prompts and common
//! failures so the UI can show native dialogs.
//!
//! - [`Runner::spawn`] starts a program from an argument array (never a shell
//!   string) and returns a [`RunHandle`].
//! - [`PromptDetector`] is an incremental parser producing [`PromptEvent`]s.
//! - [`pacman_lock_held`] checks for a stale/active pacman db lock.
//! - [`Overlay`] builds a private `XDG_CONFIG_HOME` that pins reviewed commits.

mod detector;
mod lock;
mod overlay;
mod runner;

pub use detector::{ErrorKind, PromptDetector, PromptEvent};
pub use lock::pacman_lock_held;
pub use overlay::{Overlay, OverlayError};
pub use runner::{ExitStatus, RunError, RunHandle, RunSpec, Runner};
