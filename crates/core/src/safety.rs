//! Process-level safety rules.

/// A violated safety rule.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SafetyError {
    /// The app was started as root (UID 0).
    #[error("YayGUI must not run as root; start it as a regular user")]
    RunningAsRoot,
}

/// Refuses to continue when `uid` is 0. The caller supplies the real uid.
pub fn ensure_not_root(uid: u32) -> Result<(), SafetyError> {
    if uid == 0 {
        Err(SafetyError::RunningAsRoot)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_rejected() {
        assert_eq!(ensure_not_root(0), Err(SafetyError::RunningAsRoot));
    }

    #[test]
    fn user_allowed() {
        assert!(ensure_not_root(1000).is_ok());
        assert!(ensure_not_root(1).is_ok());
    }
}
