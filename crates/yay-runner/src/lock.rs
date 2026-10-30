use std::path::Path;

/// Returns true if `<root>/var/lib/pacman/db.lck` exists (use `/` for the real system).
///
/// The file may be stale; callers should check no pacman process runs before removing it.
#[must_use]
pub fn pacman_lock_held(root: &Path) -> bool {
    root.join("var/lib/pacman/db.lck").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_lock_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!pacman_lock_held(dir.path()));
        let db = dir.path().join("var/lib/pacman");
        std::fs::create_dir_all(&db).unwrap();
        assert!(!pacman_lock_held(dir.path()));
        std::fs::write(db.join("db.lck"), "").unwrap();
        assert!(pacman_lock_held(dir.path()));
    }
}
