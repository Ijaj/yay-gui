//! `AurGit` against local repositories standing in for the AUR.

use std::path::Path;
use std::process::Command;

use yaygui_review::{AurGit, ReviewError};

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "init.defaultBranch=master",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

/// Creates `<root>/<name>.git` (non-bare, so tests can commit) with one commit.
fn make_pkg(root: &Path, name: &str, pkgbuild: &str) -> std::path::PathBuf {
    let dir = root.join(format!("{name}.git"));
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "--quiet"]);
    std::fs::write(dir.join("PKGBUILD"), pkgbuild).unwrap();
    std::fs::write(dir.join(".SRCINFO"), "pkgbase = x\n").unwrap();
    std::fs::write(dir.join("foo.install"), "post_install() { :; }\n").unwrap();
    std::fs::write(dir.join("icon.png"), [0x89, b'P', b'N', b'G', 0, 1, 2]).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("/etc/passwd", dir.join("evil")).unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "--quiet", "-m", "init"]);
    dir
}

fn name(s: &str) -> yaygui_core::PackageName {
    s.parse().unwrap()
}

#[test]
fn clone_list_update_and_diff() {
    let remote = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let src = make_pkg(remote.path(), "foo", "pkgname=foo\npkgver=1\n");
    let c1 = git(&src, &["rev-parse", "HEAD"]);

    let aur = AurGit::new(cache.path()).with_base_url(remote.path().to_string_lossy());
    let f = aur.fetch(&name("foo")).unwrap();
    assert_eq!(f.commit, c1);
    let paths: Vec<&str> = f.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        ["PKGBUILD", "foo.install", "evil", "icon.png", ".SRCINFO"]
    );
    assert_eq!(
        f.files[0].content.as_deref(),
        Some("pkgname=foo\npkgver=1\n")
    );
    let evil = &f.files[2];
    assert!(evil.symlink);
    assert_eq!(
        evil.content.as_deref(),
        Some("/etc/passwd"),
        "link target, not the file"
    );
    assert!(f.files[3].content.is_none(), "binary files have no content");
    assert!(
        !cache.path().join("foo.git/PKGBUILD").exists(),
        "no work tree"
    );

    std::fs::write(src.join("PKGBUILD"), "pkgname=foo\npkgver=2\n").unwrap();
    git(&src, &["commit", "--quiet", "-am", "bump"]);
    let c2 = git(&src, &["rev-parse", "HEAD"]);
    let f2 = aur.fetch(&name("foo")).unwrap();
    assert_eq!(f2.commit, c2);

    let d = aur.diff(&name("foo"), &c1, &c2).unwrap().unwrap();
    assert!(d.contains("-pkgver=1") && d.contains("+pkgver=2"), "{d}");
    let unknown = "0".repeat(40);
    assert_eq!(aur.diff(&name("foo"), &unknown, &c2).unwrap(), None);
    assert!(matches!(
        aur.diff(&name("foo"), "--output=/tmp/x", &c2),
        Err(ReviewError::BadCommit(_))
    ));
}

#[test]
fn empty_repo_is_not_found() {
    let remote = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let dir = remote.path().join("ghost.git");
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "--quiet", "--bare"]);
    let aur = AurGit::new(cache.path()).with_base_url(remote.path().to_string_lossy());
    assert!(matches!(
        aur.fetch(&name("ghost")),
        Err(ReviewError::NotFound(_))
    ));
}

#[test]
fn missing_remote_is_a_git_error() {
    let remote = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let aur = AurGit::new(cache.path()).with_base_url(remote.path().to_string_lossy());
    assert!(matches!(
        aur.fetch(&name("nope")),
        Err(ReviewError::Git {
            action: "clone",
            ..
        })
    ));
}
