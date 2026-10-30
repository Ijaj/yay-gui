//! Persistable transaction state machine.

use serde::{Deserialize, Serialize};

/// Lifecycle state of a transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TransactionState {
    /// Planned, nothing shown yet.
    Planned,
    /// User is reviewing.
    Reviewing,
    /// User approved; ready to run.
    Approved,
    /// yay is running.
    Running,
    /// Finished successfully.
    Succeeded,
    /// Failed with a reason.
    Failed {
        /// Human-readable failure reason.
        reason: String,
    },
    /// Cancelled by the user.
    Cancelled,
}

/// Events that drive [`TransactionState`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TransactionEvent {
    /// Begin review (Planned → Reviewing).
    StartReview,
    /// Approve (Reviewing → Approved).
    Approve,
    /// Start running (Approved → Running).
    Start,
    /// Completed OK (Running → Succeeded).
    Finish,
    /// Failed (Running → Failed).
    Fail {
        /// Failure reason.
        reason: String,
    },
    /// Cancel (any non-terminal state → Cancelled).
    Cancel,
}

/// An event that is not valid in the current state.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid transition: {event:?} in state {from:?}")]
pub struct InvalidTransition {
    /// State the event was applied to.
    pub from: TransactionState,
    /// The rejected event.
    pub event: TransactionEvent,
}

impl TransactionState {
    /// True for Succeeded, Failed and Cancelled.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed { .. } | Self::Cancelled
        )
    }

    /// Applies `event`, returning the new state or an error.
    pub fn transition(&self, event: TransactionEvent) -> Result<Self, InvalidTransition> {
        use TransactionEvent as E;
        use TransactionState as S;
        match (self, &event) {
            (S::Planned, E::StartReview) => Ok(S::Reviewing),
            (S::Reviewing, E::Approve) => Ok(S::Approved),
            (S::Approved, E::Start) => Ok(S::Running),
            (S::Running, E::Finish) => Ok(S::Succeeded),
            (S::Running, E::Fail { reason }) => Ok(S::Failed {
                reason: reason.clone(),
            }),
            (s, E::Cancel) if !s.is_terminal() => Ok(S::Cancelled),
            _ => Err(InvalidTransition {
                from: self.clone(),
                event,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TransactionEvent as E;
    use TransactionState as S;

    #[test]
    fn happy_path() {
        let s = S::Planned
            .transition(E::StartReview)
            .and_then(|s| s.transition(E::Approve))
            .and_then(|s| s.transition(E::Start))
            .and_then(|s| s.transition(E::Finish))
            .unwrap();
        assert_eq!(s, S::Succeeded);
    }

    #[test]
    fn failure_path() {
        let s = S::Running
            .transition(E::Fail {
                reason: "boom".into(),
            })
            .unwrap();
        assert_eq!(
            s,
            S::Failed {
                reason: "boom".into()
            }
        );
    }

    #[test]
    fn cancel_from_non_terminal() {
        for s in [S::Planned, S::Reviewing, S::Approved, S::Running] {
            assert_eq!(s.transition(E::Cancel), Ok(S::Cancelled));
        }
    }

    #[test]
    fn invalid_transitions() {
        assert!(S::Planned.transition(E::Approve).is_err());
        assert!(S::Planned.transition(E::Start).is_err());
        assert!(S::Reviewing.transition(E::Start).is_err());
        assert!(S::Approved.transition(E::Finish).is_err());
        assert!(
            S::Approved
                .transition(E::Fail { reason: "x".into() })
                .is_err()
        );
        for t in [S::Succeeded, S::Cancelled, S::Failed { reason: "x".into() }] {
            assert!(t.is_terminal());
            for e in [E::StartReview, E::Approve, E::Start, E::Finish, E::Cancel] {
                assert!(t.transition(e).is_err());
            }
        }
    }

    #[test]
    fn serde_roundtrip() {
        let s = S::Failed { reason: "x".into() };
        let j = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<S>(&j).unwrap(), s);
        let j = serde_json::to_string(&S::Running).unwrap();
        assert_eq!(j, r#"{"state":"running"}"#);
    }
}
