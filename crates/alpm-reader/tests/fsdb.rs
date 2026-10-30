use std::io::Write;
use std::path::Path;
use yaygui_alpm_reader::*;

fn write_local(dbpath: &Path, name: &str, ver: &str, extra: &str) {
    let d = dbpath.join("local").join(format!("{name}-{ver}"));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("desc"),
        format!("%NAME%\n{name}\n\n%VERSION%\n{ver}\n\n%DESC%\nlocal {name}\n\n{extra}"),
    )
    .unwrap();
}

fn sync_tar(pkgs: &[(&str, &str, &str, &str)]) -> Vec<u8> {
    let mut b = tar::Builder::new(Vec::new());
    for (name, ver, desc, extra) in pkgs {
        let text = format!("%NAME%\n{name}\n\n%VERSION%\n{ver}\n\n%DESC%\n{desc}\n\n{extra}");
        let mut h = tar::Header::new_gnu();
        h.set_size(text.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, format!("{name}-{ver}/desc"), text.as_bytes())
            .unwrap();
    }
    b.into_inner().unwrap()
}

fn gz(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn fixture() -> (tempfile::TempDir, FsDb) {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path();
    std::fs::create_dir_all(p.join("sync")).unwrap();
    let core = sync_tar(&[
        (
            "glibc",
            "2.40-1",
            "GNU C Library",
            "%ISIZE%\n1000\n\n%CSIZE%\n400\n\n%LICENSE%\nGPL\nLGPL\n\n",
        ),
        (
            "vim",
            "9.1-1",
            "Vi Improved",
            "%OPTDEPENDS%\npython: python support\nruby\n\n%DEPENDS%\nglibc>=2.0\n\n%BUILDDATE%\n1700000000\n\n",
        ),
    ]);
    let extra = sync_tar(&[(
        "ripgrep",
        "14.0-1",
        "Fast grep, a Vim-friendly tool",
        "%GROUPS%\ntools\n\n",
    )]);
    // core as gzip, extra as zstd
    std::fs::write(p.join("sync/core.db"), gz(&core)).unwrap();
    std::fs::write(
        p.join("sync/extra.db"),
        zstd::encode_all(extra.as_slice(), 3).unwrap(),
    )
    .unwrap();

    write_local(
        p,
        "glibc",
        "2.40-1",
        "%REASON%\n1\n\n%INSTALLDATE%\n1700000100\n\n%SIZE%\n1000\n\n",
    );
    write_local(
        p,
        "vim",
        "9.1-1",
        "%DEPENDS%\nglibc\nlibfoo.so=1-64\n\n%OPTDEPENDS%\nripgrep: search\n\n",
    );
    write_local(p, "ripgrep", "14.0-1", "%REASON%\n1\n\n");
    write_local(
        p,
        "libfoo",
        "1.0-1",
        "%REASON%\n1\n\n%PROVIDES%\nlibfoo.so=1-64\n\n",
    );
    write_local(p, "lonely", "1.0-1", "%REASON%\n1\n\n");
    write_local(p, "aurpkg", "3-1", "");

    let db = FsDb::open(&FsConfig {
        dbpath: p.to_path_buf(),
        repos: vec!["core".into(), "extra".into(), "multilib".into()],
    })
    .unwrap();
    (tmp, db)
}

fn names(v: Vec<LocalPackage>) -> Vec<String> {
    v.into_iter().map(|p| p.name).collect()
}

#[test]
fn search_and_lookup() {
    let (_t, db) = fixture();
    let r = db.search("vim");
    // name match (vim) + description match (ripgrep), repo order
    assert_eq!(
        r.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ["vim", "ripgrep"]
    );
    assert_eq!(db.search("GNU").len(), 1);
    assert_eq!(db.search("fast  vim").len(), 1); // AND of terms
    assert_eq!(db.search("^gl.bc$").len(), 1);
    assert_eq!(db.search("(unbalanced").len(), 0); // invalid regex falls back to literal
    let vim = db.sync_package("vim").unwrap();
    assert_eq!(vim.repo, "core");
    assert_eq!(vim.build_date, 1_700_000_000);
    assert_eq!(vim.depends, ["glibc>=2.0"]);
    assert_eq!(
        vim.optdepends[0],
        OptDepend {
            name: "python".into(),
            reason: Some("python support".into())
        }
    );
    assert_eq!(vim.optdepends[1].reason, None);
    let g = db.sync_package("glibc").unwrap();
    assert_eq!(
        (g.installed_size, g.download_size, g.licenses.len()),
        (1000, 400, 2)
    );
    assert_eq!(db.sync_package("ripgrep").unwrap().repo, "extra");
    assert!(db.sync_package("nope").is_none());
}

#[test]
fn installed_filters() {
    let (_t, db) = fixture();
    assert_eq!(db.installed(InstalledFilter::All).len(), 6);
    assert_eq!(
        names(db.installed(InstalledFilter::Explicit)),
        ["aurpkg", "vim"]
    );
    assert_eq!(
        names(db.installed(InstalledFilter::Foreign)),
        ["aurpkg", "libfoo", "lonely"]
    );
    assert_eq!(
        names(db.installed(InstalledFilter::Deps)),
        ["glibc", "libfoo", "lonely", "ripgrep"]
    );
    // ripgrep is only optionally required (not orphan); libfoo is required via provides
    assert_eq!(names(db.installed(InstalledFilter::Orphans)), ["lonely"]);
}

#[test]
fn reverse_deps_and_details() {
    let (_t, db) = fixture();
    assert_eq!(db.required_by("glibc"), ["vim"]);
    assert_eq!(db.required_by("libfoo"), ["vim"]);
    assert!(db.required_by("vim").is_empty());
    assert!(db.required_by("missing").is_empty());
    let rg = db.local_package("ripgrep").unwrap();
    assert_eq!(rg.optional_for, ["vim"]);
    assert_eq!(rg.reason, Reason::Dependency);
    let g = db.local_package("glibc").unwrap();
    assert_eq!(
        (g.install_date, g.installed_size, g.foreign),
        (Some(1_700_000_100), 1000, false)
    );
    assert!(db.local_package("aurpkg").unwrap().foreign);
}

#[test]
fn missing_dirs_and_misc() {
    let tmp = tempfile::tempdir().unwrap();
    let db = FsDb::open(&FsConfig {
        dbpath: tmp.path().into(),
        repos: vec!["core".into()],
    })
    .unwrap();
    assert!(db.installed(InstalledFilter::All).is_empty());
    assert_eq!(db.vercmp("1.0-1", "1.0-2"), std::cmp::Ordering::Less);
    assert_eq!(FsConfig::default().repos, ["core", "extra", "multilib"]);
    let conf = "[options]\nHoldPkg = pacman\n# c\n[core]\nInclude = x\n[extra] # e\n\n[custom]\nServer = y\n";
    assert_eq!(repos_from_pacman_conf(conf), ["core", "extra", "custom"]);
}
