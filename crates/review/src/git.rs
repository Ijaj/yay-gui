//! Fetching AUR package repositories with the `git` command.
//!
//! Repositories are kept as *bare* clones under the cache directory, one per
//! pkgbase. Nothing is ever checked out: file contents are read from git
//! objects, so a hostile repository cannot plant symlinks that make the app
//! read files outside the cache.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::Serialize;
use yaygui_core::PackageName;

/// Default AUR host.
pub const AUR_BASE_URL: &str = "https://aur.archlinux.org";

/// Files larger than this are listed without content.
pub const MAX_FILE_BYTES: u64 = 512 * 1024;

/// At most this many files are listed per repository.
pub const MAX_FILES: usize = 200;

/// Errors from fetching or reading a package repository.
#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    /// `git` could not be started.
    #[error("could not run git: {0}")]
    Spawn(#[from] std::io::Error),
    /// A git command failed.
    #[error("git {action} failed: {stderr}")]
    Git {
        /// What was being done.
        action: &'static str,
        /// git's error output.
        stderr: String,
    },
    /// The repository exists but has no commits: there is no such AUR package.
    #[error("AUR package base {0} does not exist")]
    NotFound(String),
    /// A commit argument was not a full lowercase hex hash.
    #[error("invalid commit hash {0:?}")]
    BadCommit(String),
}

/// One file of the package repository at the fetched commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoFile {
    /// Path inside the repository.
    pub path: String,
    /// Size in bytes.
    pub size: u64,
    /// Text content; `None` when binary or larger than [`MAX_FILE_BYTES`].
    pub content: Option<String>,
    /// True for a symlink (the content is the link target).
    pub symlink: bool,
}

/// Result of [`AurGit::fetch`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fetched {
    /// Package base.
    pub pkgbase: String,
    /// Commit the default branch points to.
    pub commit: String,
    /// Files at that commit, `PKGBUILD` first, `.SRCINFO` last.
    pub files: Vec<RepoFile>,
    /// True when more than [`MAX_FILES`] files exist.
    pub truncated: bool,
}

/// Fetches AUR package repositories into a private cache.
#[derive(Debug, Clone)]
pub struct AurGit {
    base_url: String,
    cache_dir: PathBuf,
    git: PathBuf,
}

/// True for a full SHA-1 or SHA-256 lowercase hex hash.
#[must_use]
pub fn is_commit_hash(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn check_commit(s: &str) -> Result<(), ReviewError> {
    if is_commit_hash(s) {
        Ok(())
    } else {
        Err(ReviewError::BadCommit(s.to_owned()))
    }
}

impl AurGit {
    /// Uses `cache_dir` for the bare clones and the public AUR as remote.
    #[must_use]
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_url: AUR_BASE_URL.to_owned(),
            cache_dir: cache_dir.into(),
            git: PathBuf::from("git"),
        }
    }

    /// Overrides the remote base (`<base>/<pkgbase>.git`); used by tests.
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_owned();
        self
    }

    fn repo_dir(&self, pkgbase: &PackageName) -> PathBuf {
        // A validated name has no '/' and cannot start with '.', so it stays inside the cache.
        self.cache_dir.join(format!("{}.git", pkgbase.as_str()))
    }

    fn git(&self, git_dir: Option<&Path>) -> Command {
        let mut c = Command::new(&self.git);
        // Ignore the user's and the system's git config (url rewrites, hooks,
        // credential helpers) and never prompt.
        c.env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            .args(["-c", "core.hooksPath=/dev/null"]);
        if let Some(d) = git_dir {
            c.arg("--git-dir").arg(d);
        }
        c
    }

    fn run(cmd: &mut Command, action: &'static str) -> Result<Output, ReviewError> {
        let out = cmd.output()?;
        if out.status.success() {
            Ok(out)
        } else {
            Err(ReviewError::Git {
                action,
                stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            })
        }
    }

    /// Clones (first time) or fetches the package repository, then lists its
    /// files at the tip of the default branch.
    ///
    /// # Errors
    /// Fails when git fails, or [`ReviewError::NotFound`] for an unknown package.
    pub fn fetch(&self, pkgbase: &PackageName) -> Result<Fetched, ReviewError> {
        let dir = self.repo_dir(pkgbase);
        let url = format!("{}/{}.git", self.base_url, pkgbase.as_str());
        if dir.join("HEAD").is_file() {
            Self::run(
                self.git(Some(&dir)).args([
                    "fetch",
                    "--quiet",
                    "--prune",
                    "--",
                    &url,
                    "+refs/heads/*:refs/heads/*",
                ]),
                "fetch",
            )?;
        } else {
            std::fs::create_dir_all(&self.cache_dir)?;
            // A half-finished clone from an earlier crash would make clone refuse.
            if dir.exists() {
                std::fs::remove_dir_all(&dir)?;
            }
            Self::run(
                self.git(None)
                    .args(["clone", "--bare", "--quiet", "--template=", "--"])
                    .arg(&url)
                    .arg(&dir),
                "clone",
            )?;
        }

        let head = self
            .git(Some(&dir))
            .args(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])
            .output()?;
        let commit = String::from_utf8_lossy(&head.stdout).trim().to_owned();
        if !head.status.success() || !is_commit_hash(&commit) {
            return Err(ReviewError::NotFound(pkgbase.as_str().to_owned()));
        }
        let (files, truncated) = self.list_files(&dir, &commit)?;
        Ok(Fetched {
            pkgbase: pkgbase.as_str().to_owned(),
            commit,
            files,
            truncated,
        })
    }

    fn list_files(&self, dir: &Path, commit: &str) -> Result<(Vec<RepoFile>, bool), ReviewError> {
        let out = Self::run(
            self.git(Some(dir))
                .args(["ls-tree", "-r", "-l", "-z", "--full-tree", commit]),
            "ls-tree",
        )?;
        let mut files = Vec::new();
        let mut truncated = false;
        for entry in out.stdout.split(|&b| b == 0).filter(|e| !e.is_empty()) {
            // "<mode> <type> <object> <size>\t<path>"
            let Some(tab) = entry.iter().position(|&b| b == b'\t') else {
                continue;
            };
            let meta = String::from_utf8_lossy(&entry[..tab]);
            let path = String::from_utf8_lossy(&entry[tab + 1..]).into_owned();
            let mut it = meta.split_whitespace();
            let (Some(mode), Some(kind), Some(oid), Some(size)) =
                (it.next(), it.next(), it.next(), it.next())
            else {
                continue;
            };
            if kind != "blob" {
                continue; // submodules
            }
            if files.len() == MAX_FILES {
                truncated = true;
                break;
            }
            let size: u64 = size.parse().unwrap_or(0);
            let content = if size <= MAX_FILE_BYTES {
                let blob = Self::run(
                    self.git(Some(dir)).args(["cat-file", "blob", oid]),
                    "cat-file",
                )?;
                String::from_utf8(blob.stdout)
                    .ok()
                    .filter(|s| !s.contains('\0'))
            } else {
                None
            };
            files.push(RepoFile {
                path,
                size,
                content,
                symlink: mode == "120000",
            });
        }
        files.sort_by_key(|f| {
            let rank = match f.path.as_str() {
                "PKGBUILD" => 0,
                p if p.ends_with(".install") => 1,
                ".SRCINFO" => 3,
                _ => 2,
            };
            (rank, f.path.clone())
        });
        Ok((files, truncated))
    }

    /// Unified diff between two fetched commits, or `None` when `from` is no
    /// longer in the repository (for example after a force push).
    ///
    /// # Errors
    /// Fails on malformed hashes or git errors.
    pub fn diff(
        &self,
        pkgbase: &PackageName,
        from: &str,
        to: &str,
    ) -> Result<Option<String>, ReviewError> {
        check_commit(from)?;
        check_commit(to)?;
        let dir = self.repo_dir(pkgbase);
        let exists = self
            .git(Some(&dir))
            .args(["cat-file", "-e", &format!("{from}^{{commit}}")])
            .output()?;
        if !exists.status.success() {
            return Ok(None);
        }
        let out = Self::run(
            self.git(Some(&dir)).args([
                "diff",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                from,
                to,
                "--",
            ]),
            "diff",
        )?;
        Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_hash_validation() {
        assert!(is_commit_hash(&"a".repeat(40)));
        assert!(is_commit_hash(&"0".repeat(64)));
        assert!(!is_commit_hash(&"A".repeat(40)));
        assert!(!is_commit_hash("--output=/tmp/x"));
        assert!(!is_commit_hash(&"a".repeat(41)));
        assert!(matches!(
            check_commit("HEAD"),
            Err(ReviewError::BadCommit(_))
        ));
    }
}
