//! Domain types, transaction planner and safety rules for YayGUI.
//!
//! This crate is pure: no IO, no Tauri. It turns a queue of user actions into
//! argument arrays for `yay`, enforcing the project's hard safety rules.

pub mod name;
pub mod plan;
pub mod safety;
pub mod state;
pub mod types;

pub use name::{InvalidPackageName, PackageName};
pub use plan::{PlanContext, PlanError, PlannedCommand, plan};
pub use safety::{SafetyError, ensure_not_root};
pub use state::{InvalidTransition, TransactionEvent, TransactionState};
pub use types::{PackageRef, QueueItem, RemoveMode, ReviewMap, ReviewStatus, Source};
