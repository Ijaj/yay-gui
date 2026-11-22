# yay Lua hook API (as observed, yay v13.1.0)

Written for P0.2b. Source citations are file and line at tag `v13.1.0` of
<https://github.com/Jguer/yay> (commit `0938a75e98e63648ebf6e22ece6b4a30ec11a907`), the version
installed from the AUR `yay-bin` package in the test container (`yay v13.1.0 - libalpm v16.0.1`,
pacman 7.1.0, sudo 1.9.17p2). Every answer was also checked by an experiment in a disposable
`archlinux:base-devel` container as a non-root user (commands at the end). yay's own docs are
`doc/lua.md` and `meta/yay.d.lua` in the same tree.

## 1. Version

- The AUR RPC lists `yay` and `yay-bin` at `13.1.0-1` (checked Nov 22, 2026). yay is not in the
  official repos, so "the version in Arch today" means the AUR package. v13.1.0 was tagged 2026-10-28.
- Lua support was added by commit `ecae3e6` ("Add Lua configuration support (#2845)", 2026-06-09),
  first contained in `v13.0.0` (tagged 2026-06-17). `doc/lua.md` marks the AUR hooks "Available from
  yay v13.0.0". `require()` of sibling modules is "Available from yay v13.0.1". v12.6.0 has no Lua.

## 2. Where `init.lua` is loaded from

- `pkg/settings/dirs.go:35-57` (`GetLuaConfigPath`): candidates in order are `./init.lua` (only with
  `--debug`), `$XDG_CONFIG_HOME/yay/init.lua`, `$HOME/.config/yay/init.lua`. The first one that exists
  is used. Only one file is ever loaded. `main.go:92-102` loads it at startup, for every yay command.
- So `XDG_CONFIG_HOME` does redirect it. Experiment: with `XDG_CONFIG_HOME=/home/alice/exp/X` the
  hooks of `exp/X/yay/init.lua` ran; with an XDG directory that has no `init.lua` and a user file at
  `~/.config/yay/init.lua`, the user file ran.
- Two side effects an overlay must handle (both confirmed by experiment):
  1. `XDG_CONFIG_HOME` also moves yay's `config.json` (`pkg/settings/dirs.go:16-32`). With an empty
     overlay directory, `yay -Pg` printed `"editor": ""` instead of the `"editor"` saved in
     `~/.config/yay/config.json`. The user's saved yay settings are silently ignored unless the
     overlay copies or links `config.json` into its `yay/` directory.
  2. `require()` resolves against the directory of the file being loaded
     (`pkg/settings/lua/lua.go:108-118`, `SetSearchDir`, called from `load.go:118`). A user module
     next to the user's own `init.lua` is not found from the overlay (`require "usermod"` failed with
     `package.path` limited to the overlay directory). The overlay must prepend the user's config
     directory to `package.path` before it `dofile`s the user's init.

## 3. Registration call and hook names

- Not `yay.hook`. The global `yay` table has `opt`, `abort`, `create_autocmd`, `log`
  (`pkg/settings/lua/lua.go:39-44`). Hooks are registered with:

  ```lua
  yay.create_autocmd("AURPreInstall", {
    desc = "optional text",
    callback = function(event) ... end,
  })
  ```

  (`pkg/settings/lua/autocmd.go:144-178`: arg 1 event name, arg 2 table with a required `callback`
  function and an optional string `desc`; any other event name raises "unsupported event".)
- Event names (`autocmd.go:10-18`): `AURPreInstall`, `AURPostDownload`, `UpgradeSelect`,
  `PostInstall`, `SearchFilter`, `RenderAUR`, `RenderSync`. Several callbacks per event run in
  registration order (`autocmd.go:197`).
- Experiment: `yay.hook("AURPreInstall", fn)` (what `overlay_init.lua.tmpl` calls today) fails with
  `init.lua:2: attempt to call a non-function object`.

## 4. Fields of `AURPreInstall` and `AURPostDownload`

Both events carry the same table (`autocmd.go:263-279`, built in
`pkg/sync/workdir/aur_preinstall.go:93-127`). Observed keys by dumping the table in a hook:

```
event keys: data,event,match
data keys:  base,dir,installed,last_modified,packages,pkgbuild,pkgbuild_path,srcinfo,srcinfo_path,version
event = "AURPreInstall" | "AURPostDownload";  match = <pkgbase>
data.base, data.dir (= <build_dir>/<pkgbase>, i.e. ~/.cache/yay/<pkgbase>),
data.pkgbuild_path, data.srcinfo_path, data.pkgbuild (file contents), data.version,
data.last_modified, data.installed, data.packages[] {name,version,local_version,reason,upgrade,devel},
data.srcinfo {pkgbase,pkgver,pkgrel,epoch,version,pkgdesc,url,arch,license,depends,...}
```

- **pkgbase**: yes, `event.match` and `event.data.base`.
- **Fetched git commit**: no field for it. It is derivable: `data.dir` is the clone yay fetched into
  (`pkg/sync/workdir/preparer.go:200`, `filepath.Join(BuildDir, pkgBase)`; the AUR repo is a normal
  git working tree). Hook code runs in gopher-lua with the standard `io` and `os` libraries
  (`doc/lua.md`, "Hooks can still edit files through Lua's normal `io` and `os` libraries"). Experiment,
  all inside the hook:
  - `io.open(dir.."/.git/HEAD")` gave `ref: refs/heads/master`, and the loose ref file gave
    `17c603cf293c3c0b3002e44111e4b255fea38ff6`, equal to `git rev-parse HEAD` of that clone.
  - `io.popen("git -C '<dir>' rev-parse HEAD")` also works and returned the same hash, and so does
    `os.execute`. Do not use them for the pin: they build a shell string from `dir`, which contains the
    user-configurable `BuildDir`. Read the files directly (section 7).
  - A pure-Lua read must handle a detached HEAD (`.git/HEAD` holds the hash), a loose ref file, and
    `packed-refs` (`<hash> <ref>` lines).
- Timing (`pkg/sync/workdir/preparer.go:189-243`, `aur_preinstall.go:21-40`):
  `AURPreInstall` runs once per pkgbase, sorted, after the clone/merge and BEFORE yay's clean, diff
  and edit menus and before sources are downloaded. `AURPostDownload` runs after the menus and after
  `makepkg --verifysource` (`preparer.go:233-240`, `aur_post_download.go:11-30`), so it sees edits
  made in yay's edit menu. When the clone is already up to date yay skips the download
  (`preparer.go:245-260`, "PKGBUILD up to date, skipping download") and the hook sees the cached
  clone's HEAD (experiment: same hash on the second run).

## 5. Can a hook abort the build?

Yes, in `AURPreInstall` and `AURPostDownload` (not in `PostInstall`, which runs after install).

- `yay.abort("message")` (`pkg/settings/lua/abort.go`) or a plain `error("message")`. Any error from a
  callback is returned up `PrepareWorkspace` (`preparer.go:228-240`), so yay stops before any build
  and before any sudo or pacman step. Nothing is installed (by source reading; the experiments used `-w`, see the end of section 7).
- Experiment, `AURPreInstall` hook for `pfetch-git`, run as `yay -S -w ... -- pfetch-git`. A
  rejecting hook also runs before the clean/diff menus appear:
  - `yay.abort(...)`: exit status 1, output ` -> AURPreInstall pfetch-git: aurdeck test: refusing pfetch-git via yay.abort()`
  - `error(...)`: exit status 1, same line plus Lua file, line and a `stack traceback:`
    (`init.lua:2: aurdeck test: ... via error()`).
  - `error(...)` in `AURPostDownload`: exit 1, ` -> AURPostDownload pfetch-git: .../init.lua:2: ...`,
    printed after the source download.
- The message goes to the terminal as an ` -> ` line, with the event name and pkgbase as prefix. Use
  `yay.abort` for a clean one-line message; a marker such as `aurdeck:` inside the message lets the
  app recognise it.

## 6. When `init.lua` itself fails

All of these were run with a one-line init and a `-S -w` of one AUR package. Each ended with
exit status 1 and nothing was built or downloaded (yay exits during startup, `main.go:93-99`):

| `init.lua` | yay prints |
|---|---|
| `yay.hook(...)` (unknown function; the current template) | ` -> .../init.lua:2: attempt to call a non-function object` plus traceback |
| syntax error | ` -> .../init.lua line:1(column:7) near 'is':   parse error` |
| `error("boom at load time")` | ` -> .../init.lua:1: boom at load time` plus traceback |
| `yay.opt.no_such_option = true` | ` -> init.lua: unknown yay.opt key: no_such_option` |

So a broken overlay does not degrade to "no hook": it makes EVERY yay command fail, including repo
installs and removals. The current template's unguarded `yay.hook(...)` call is therefore fatal.
Guard with `type(yay.create_autocmd) == "function"` and pcall the user's init
(`pcall(dofile, user_init)` as the template already does), but fail closed for the pinning hook.

## 7. Verdict for P0.5

Yes, the overlay can pin a reviewed commit and abort on mismatch, with yay v13.1.0 as it is. No
upstream change is required. The template needs these corrections:

1. Register with `yay.create_autocmd("AURPreInstall", {callback = ...})`, and the same callback on
   `AURPostDownload`. Pin in both: `AURPostDownload` runs after yay's edit menu, so it should see edits.
2. Read the fields as `event.match` / `event.data.base` for the pkgbase and `event.data.dir` for
   the clone. There is no `event.commit`. Derive the commit in pure Lua with `io.open`: read
   `<dir>/.git/HEAD`; if it is `ref: <ref>` read `<dir>/.git/<ref>`, and if that file is missing look
   the ref up in `<dir>/.git/packed-refs`; otherwise HEAD holds the hash. No shell, no `io.popen`
   (that would put the user-configurable `BuildDir` into a shell string).
3. Abort with `yay.abort("aurdeck: ...")`. Fail closed: no approved entry for a base or an
   unreadable HEAD aborts.
   Order matters, so that a user's own init or module cannot subvert the pin: (a) first capture every
   global the pin uses as locals (`local io_open, abort, create_autocmd, pcall, dofile =
   io.open, yay.abort, yay.create_autocmd, pcall, dofile`, and the string functions); (b) then set
   `package.path` and load the user's init with `pcall(dofile, user_init)`; (c) register the pin
   LAST, through the captured `create_autocmd`, so it runs after every user callback (callbacks run
   in registration order, section 3) and only uses the captured functions. Also capture the
   file-handle methods (`read`, `close`) and call them as functions. This raises the bar but is
   not a sandbox: the user's init runs in the same Lua state with the user's full privileges (it
   can patch shared metatables or the cached clone itself). The pin protects against a changed AUR
   clone, not against a malicious user config.
4. Overlay directory must also carry the user's `config.json` (copy or symlink) and put the user's
   config directory on `package.path` before loading the user's `init.lua` (section 2). The user's
   `init.lua` is found only through `$XDG_CONFIG_HOME` or `$HOME/.config/yay`, so the overlay must
   resolve the real path before it redirects `XDG_CONFIG_HOME`.
5. yay still prints the clean-build and diff menus with `--answerdiff None --answeredit None` (see
   `tests/fixtures/recorded/aur_install_app.bin`); the diff menu is answered by the flag.
6. Clean-tree check (owner decision, not done by the pin above). A commit match alone does not prove
   the built tree is the reviewed one: local edits and untracked files ride along. A real check needs
   `git status --porcelain --ignored` (without `--ignored`, gitignored files ride along), which needs a
   shell or a process spawn from Lua, i.e. the `io.popen` risk above, or reimplementing it. Also,
   for `-git` packages `pkgver()` rewrites `PKGBUILD` in the cached clone during the build (seen
   in the experiments: `Updated version: pfetch-git r432...`), so a clean tree can only be required
   at `AURPreInstall` / `AURPostDownload` time, before `pkgver()` runs on this build, and a cached
   clone can be dirty from the previous build.

Nice-to-have upstream asks, none blocking: a `commit` (and `tree_clean`) field on the AUR events so
hooks need not read `.git` themselves; a documented statement that `io`/`os` are available to hooks.
`io.popen` and `os.execute` work in hooks, so an `init.lua` has the full privileges of the user running
yay. That is the same trust level as the user's own yay config.

### What is experiment and what is only source reading

All experiments ran `yay -S -w` (download only) on one AUR package. Verified by experiment: the
registration call and event fields, reading HEAD with `io.open`, abort messages and exit codes of
`AURPreInstall`/`AURPostDownload`, hooks running before the menus on `-S`, init.lua failure behaviour,
`XDG_CONFIG_HOME` redirect, config.json hiding, `require` path. Resting on source citations only,
not run: hooks firing on `-Syu` AUR upgrades and devel (`--devel`) builds, an abort happening before
sudo in a real (non `-w`) install, `AURPostDownload` seeing an edit made in yay's edit menu, and the
split-package case (several packages per base).

## Experiment commands

Throwaway container `aurdeck-p02b` (`archlinux:base-devel` plus `git python sudo`, user `alice`,
sudo needs a password, yay from `yay-bin`). Each experiment ran:
`XDG_CONFIG_HOME=/home/alice/exp/<name> yay -S -w --noconfirm --answerclean None
--answerdiff None --answeredit None -- pfetch-git`. `-w` (download only) skips sudo and install. Each
`init.lua` used in the experiments is a few lines and is quoted above. The dump hook printed
`event`/`data` keys with `pairs`, then the `.git/HEAD`, `io.popen` and `os.execute` results quoted in
section 4.

## Implemented in P0.5a

`crates/yay-runner/src/overlay_init.lua.tmpl` now follows section 7: captures first, user init second, the pin registered last on `AURPreInstall` and `AURPostDownload`, commit read from `.git` in pure Lua, abort with `aurdeck:` messages. It checks the commit and the PKGBUILD text (`event.data.pkgbuild`) of every base. Proven in the Arch container by `pin-commit-mismatch`, `pin-unapproved-base` and `pin-unreviewed-upgrade` (`tests/arch/README.md`); in the last one a full upgrade with an unreviewed pending AUR update aborted on that base before anything was built. Observed: with `-Syu`, yay printed the exclude menu and fetched PKGBUILDs before the hook fired; `AURPreInstall` runs per base in sorted order, so the first unapproved base aborts the run.

The commit-only `Overlay::create` (used by the desktop app until P0.5b) does NOT compare the PKGBUILD text: its entries carry `pkgbuild_unchecked = true`. Only `Overlay::create_pinned` pins the text too. When `.git` is a gitfile or HEAD/refs cannot be parsed (for example reftable), the pin aborts with a distinct `aurdeck: ... cannot read the fetched commit ...: <reason>` message.

Known gaps, not covered by the pin (owner decision, section 7 item 6): no clean-work-tree check, so local edits and untracked files in the clone ride along; gitignored files in the clone are not covered by the commit pin at all. The user's own `init.lua` runs with the user's full privileges.

`-git` (devel) package, run twice through yay in a throwaway container (`pfetch-git`, `--devel`, second run forced without `--needed`): `pkgver()` did NOT rewrite `PKGBUILD` in the cached clone. `event.data.pkgbuild` and its md5 were identical on both runs (`pkgver=r340...`), only untracked files appeared (`pfetch/`, the built package), so the PKGBUILD-text pin did not fail closed there. Not tested: a `-git` package whose `pkgver()` edits tracked files, and the commit of a devel update (the commit changes with every upstream push of the AUR repo only).
