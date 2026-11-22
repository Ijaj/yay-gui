//! Incremental prompt/error detector for yay, pacman, makepkg and sudo/doas output.

use std::collections::VecDeque;

/// Category of a recognised failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// `unable to lock database` (pacman) or yay's `There may be another Pacman
    /// instance running. Waiting...`: another pacman holds (or left) `db.lck`.
    DbLock,
    /// `target not found: pkg` (pacman) or `No AUR package found for pkg` (yay).
    TargetNotFound,
    /// `failed to commit transaction`.
    CommitFailed,
    /// `conflicting files` (reported instead of `CommitFailed` on the same line).
    ConflictingFiles,
    /// makepkg `A failure occurred in build()` / `Error making:`.
    BuildFailed,
    /// Wrong sudo password (`Sorry, try again.`, `sudo: 3 incorrect password attempts`).
    AuthFailed,
}

/// Something the UI should react to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptEvent {
    /// A `sudo`/`doas` password prompt; answer with the password plus newline.
    SudoPassword {
        /// User named in the prompt.
        user: String,
    },
    /// A pacman/yay `[Y/n]` or `[y/N]` question.
    YesNo {
        /// Question text without the `::` marker and the `[Y/n]` suffix.
        question: String,
        /// True for `[Y/n]`.
        default_yes: bool,
    },
    /// A yay `==> ` selection menu awaiting a free-form answer.
    Menu {
        /// Heading and options lines that preceded the prompt.
        text: String,
    },
    /// A classified error line.
    Error {
        /// Classification.
        kind: ErrorKind,
        /// The full (ANSI-stripped) line.
        line: String,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Esc {
    None,
    Escape,
    Csi,
    Osc,
    OscTerm,
    /// `ESC (` / `ESC )` etc.: one more byte (the charset designator) follows.
    Charset,
}

const MAX_LINE: usize = 8192;

/// Incremental parser. Feed raw PTY chunks (any split) to [`feed`](Self::feed);
/// ANSI escapes are stripped for matching only, the caller keeps the raw bytes.
pub struct PromptDetector {
    esc: Esc,
    line: Vec<u8>,
    cr: bool,
    prompt_emitted: bool,
    recent: VecDeque<String>,
}

impl Default for PromptDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptDetector {
    /// Creates an empty detector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            esc: Esc::None,
            line: Vec::new(),
            cr: false,
            prompt_emitted: false,
            recent: VecDeque::new(),
        }
    }

    /// Consumes a chunk and returns the events it completed.
    pub fn feed(&mut self, data: &[u8]) -> Vec<PromptEvent> {
        let mut out = Vec::new();
        for &b in data {
            if self.strip_escape(b) {
                continue;
            }
            match b {
                b'\n' => {
                    self.cr = false;
                    self.finish_line(&mut out);
                }
                b'\r' => self.cr = true,
                0x00..=0x08 | 0x0b..=0x1f | 0x7f => {}
                _ => {
                    if self.cr {
                        // Carriage return without newline: line is being overwritten.
                        self.line.clear();
                        self.prompt_emitted = false;
                        self.cr = false;
                    }
                    if self.line.len() < MAX_LINE {
                        self.line.push(b);
                    }
                }
            }
        }
        if !self.prompt_emitted {
            let text = String::from_utf8_lossy(&self.line).into_owned();
            // yay prints its lock notice and then waits on the same line, so
            // the lock error is also reported before the newline arrives.
            if let Some(ev) = self.match_prompt(&text).or_else(|| {
                classify_error(&text).filter(|e| {
                    matches!(
                        e,
                        PromptEvent::Error {
                            kind: ErrorKind::DbLock,
                            ..
                        }
                    )
                })
            }) {
                self.prompt_emitted = true;
                out.push(ev);
            }
        }
        out
    }

    /// Returns true when the byte belongs to an escape sequence (and updates state).
    fn strip_escape(&mut self, b: u8) -> bool {
        match self.esc {
            Esc::None => {
                if b == 0x1b {
                    self.esc = Esc::Escape;
                    true
                } else {
                    false
                }
            }
            Esc::Escape => {
                self.esc = match b {
                    b'[' => Esc::Csi,
                    b']' => Esc::Osc,
                    // Charset select (makepkg emits `ESC ( B`): three bytes in total.
                    b'(' | b')' | b'*' | b'+' | b'#' => Esc::Charset,
                    // Other two-byte escapes: the byte just consumed is final.
                    _ => Esc::None,
                };
                true
            }
            Esc::Charset => {
                self.esc = Esc::None;
                true
            }
            Esc::Csi => {
                if (0x40..=0x7e).contains(&b) {
                    self.esc = Esc::None;
                }
                true
            }
            Esc::Osc => {
                self.esc = match b {
                    0x07 => Esc::None,
                    0x1b => Esc::OscTerm,
                    _ => Esc::Osc,
                };
                true
            }
            Esc::OscTerm => {
                self.esc = if b == b'\\' { Esc::None } else { Esc::Osc };
                true
            }
        }
    }

    fn finish_line(&mut self, out: &mut Vec<PromptEvent>) {
        let text = String::from_utf8_lossy(&self.line).into_owned();
        self.line.clear();
        if !self.prompt_emitted
            && let Some(ev) = self
                .match_prompt(&text)
                .or_else(|| self.match_answered_prompt(&text))
                .or_else(|| classify_error(&text))
        {
            out.push(ev);
        }
        self.prompt_emitted = false;
        if self.recent.len() >= 4 {
            self.recent.pop_front();
        }
        self.recent.push_back(text);
    }

    fn match_prompt(&self, text: &str) -> Option<PromptEvent> {
        let t = text.trim_end();
        if let Some(user) = sudo_user(t) {
            return Some(PromptEvent::SudoPassword { user });
        }
        for (suffix, default_yes) in YES_NO {
            if let Some(q) = t.strip_suffix(suffix) {
                return Some(yes_no(q, default_yes));
            }
        }
        if t == "==>" {
            return self.menu_text();
        }
        None
    }

    /// A prompt that was already answered on the same line: the terminal echo
    /// of the reply (or yay's own `--answerdiff None`) follows the prompt, as in
    /// `:: Proceed with install? [Y/n] n` or `==> None`. This is what a whole
    /// recording shows for every prompt that was not the last thing printed.
    fn match_answered_prompt(&self, text: &str) -> Option<PromptEvent> {
        let t = text.trim();
        for (marker, default_yes) in YES_NO {
            if let Some((q, answer)) = t.rsplit_once(marker)
                && is_echoed_answer(answer)
            {
                return Some(yes_no(q, default_yes));
            }
        }
        if let Some(answer) = t.strip_prefix("==>")
            && is_echoed_answer(answer)
            && !is_menu_line(t)
        {
            return self.menu_text();
        }
        None
    }

    /// A bare `==> ` is only a menu when the contiguous block of menu lines
    /// right above it (heading, `[N]one` options line, `->` warnings) carries a
    /// menu marker: the options line, or the `"1 2 3"` hint of yay's
    /// `Packages to exclude:` menu. Otherwise a chunk split right after the
    /// `==> ` of `==> Making package:` would be misreported.
    fn menu_text(&self) -> Option<PromptEvent> {
        let mut block: Vec<&str> = Vec::new();
        for line in self.recent.iter().rev() {
            let t = line.trim();
            if !is_menu_line(t) {
                break;
            }
            block.push(t);
        }
        block.reverse();
        let text = block.join("\n");
        if text.contains("[N]one") || text.contains("1 2 3") {
            Some(PromptEvent::Menu { text })
        } else {
            None
        }
    }
}

const YES_NO: [(&str, bool); 2] = [("[Y/n]", true), ("[y/N]", false)];

fn yes_no(question: &str, default_yes: bool) -> PromptEvent {
    PromptEvent::YesNo {
        question: question.trim().trim_start_matches(':').trim().to_owned(),
        default_yes,
    }
}

/// A short single token: what a user (or `--answer*`) typed after a prompt.
fn is_echoed_answer(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty() && s.len() <= 16 && !s.contains(char::is_whitespace)
}

/// Heading, options line or `->` note of a yay `==>` selection menu.
fn is_menu_line(t: &str) -> bool {
    (t.starts_with("==>") && (t.ends_with('?') || t.contains("[N]one") || t.contains("1 2 3")))
        || t.starts_with("->")
}

fn sudo_user(t: &str) -> Option<String> {
    // sudo: "[sudo] password for alice:"
    if let Some(rest) = t.strip_prefix("[sudo] password for ") {
        return rest.strip_suffix(':').map(|u| u.trim().to_owned());
    }
    // doas: "doas (alice@host) password:"
    if let Some(rest) = t.strip_prefix("doas (") {
        let (who, tail) = rest.split_once(')')?;
        if tail.trim() == "password:" {
            return Some(who.split('@').next().unwrap_or(who).to_owned());
        }
    }
    None
}

fn classify_error(text: &str) -> Option<PromptEvent> {
    let kind = if text.contains("unable to lock database")
        || text.contains("could not lock database")
        || text.contains("There may be another Pacman instance running")
    {
        ErrorKind::DbLock
    } else if text.contains("conflicting files") {
        ErrorKind::ConflictingFiles
    } else if text.contains("target not found") || text.contains("No AUR package found for") {
        ErrorKind::TargetNotFound
    } else if text.contains("failed to commit transaction") {
        ErrorKind::CommitFailed
    } else if text.contains("A failure occurred in")
        || text.trim_start().starts_with("Error making:")
        || text.contains("Error making:")
    {
        ErrorKind::BuildFailed
    } else if text.trim() == "Sorry, try again." || text.contains("incorrect password attempt") {
        ErrorKind::AuthFailed
    } else {
        return None;
    };
    Some(PromptEvent::Error {
        kind,
        line: text.trim().to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(data: &[u8], chunk: usize) -> Vec<PromptEvent> {
        let mut d = PromptDetector::new();
        data.chunks(chunk).flat_map(|c| d.feed(c)).collect()
    }

    fn golden(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    fn err(kind: ErrorKind) -> impl Fn(&PromptEvent) -> bool {
        move |e| matches!(e, PromptEvent::Error { kind: k, .. } if *k == kind)
    }

    #[test]
    fn install_session_all_chunk_sizes() {
        let data = golden("install_session.txt");
        for chunk in [1, 2, 3, 7, 64, 100_000] {
            let ev = run(&data, chunk);
            assert_eq!(
                ev,
                vec![
                    PromptEvent::YesNo {
                        question: "Replace foo-old with extra/foo?".into(),
                        default_yes: false
                    },
                    PromptEvent::SudoPassword {
                        user: "alice".into()
                    },
                    PromptEvent::Error {
                        kind: ErrorKind::AuthFailed,
                        line: "Sorry, try again.".into()
                    },
                    PromptEvent::SudoPassword {
                        user: "alice".into()
                    },
                    PromptEvent::YesNo {
                        question: "Proceed with installation?".into(),
                        default_yes: true
                    },
                ],
                "chunk size {chunk}"
            );
        }
    }

    #[test]
    fn menus_golden() {
        let data = golden("menus.txt");
        for chunk in [1, 5, 1000] {
            let ev = run(&data, chunk);
            assert_eq!(ev.len(), 2, "chunk {chunk}: {ev:?}");
            match &ev[0] {
                PromptEvent::Menu { text } => {
                    assert!(text.starts_with("==> Packages to cleanBuild?"));
                    assert!(text.contains("[N]one"));
                }
                other => panic!("{other:?}"),
            }
            match &ev[1] {
                PromptEvent::Menu { text } => assert!(text.starts_with("==> Diffs to show?")),
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn errors_golden() {
        let data = golden("errors.txt");
        let ev = run(&data, 13);
        assert!(ev.iter().any(err(ErrorKind::DbLock)));
        assert!(ev.iter().any(err(ErrorKind::TargetNotFound)));
        assert!(ev.iter().any(err(ErrorKind::ConflictingFiles)));
        assert!(ev.iter().any(err(ErrorKind::BuildFailed)));
        assert!(!ev.iter().any(err(ErrorKind::CommitFailed)));
        assert_eq!(
            ev.iter().filter(|e| err(ErrorKind::BuildFailed)(e)).count(),
            2
        );
    }

    // ---- Recordings of real yay v13.1.0 sessions (tests/fixtures/recorded/README.md) ----

    fn recorded(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/tests/fixtures/recorded/{name}.bin",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    /// One short token per event: `sudo:alice`, `yn:Question?:Y`, `menu:<heading>`, `err:Kind`.
    fn shape(ev: &PromptEvent) -> String {
        match ev {
            PromptEvent::SudoPassword { user } => format!("sudo:{user}"),
            PromptEvent::YesNo {
                question,
                default_yes,
            } => format!("yn:{question}:{}", if *default_yes { "Y" } else { "n" }),
            PromptEvent::Menu { text } => {
                format!("menu:{}", text.lines().next().unwrap_or_default())
            }
            PromptEvent::Error { kind, .. } => format!("err:{kind:?}"),
        }
    }

    /// Every real prompt (and classified error) of the recording is reported,
    /// in order and exactly once, at chunk sizes 1, 7 and the whole file, and
    /// nothing else fires on the build, download and progress output.
    fn assert_recording(name: &str, expected: &[&str]) {
        let data = recorded(name);
        for chunk in [1, 7, data.len()] {
            let got: Vec<String> = run(&data, chunk).iter().map(shape).collect();
            assert_eq!(got, expected, "{name}, chunk size {chunk}");
        }
    }

    const CLEAN: &str = "menu:==> Packages to cleanBuild?";
    const DIFFS: &str = "menu:==> Diffs to show?";
    const PROCEED: &str = "yn:Proceed with installation?:Y";
    const EXCLUDE: &str =
        "menu:==> Packages to exclude: (eg: \"1 2 3\", \"1-3\", \"^4\" or repo name)";

    #[test]
    fn recorded_repo_install() {
        assert_recording("repo_install", &["sudo:alice", PROCEED]);
        assert_recording("repo_install_color", &["sudo:alice", PROCEED]);
    }

    #[test]
    fn recorded_repo_install_with_sudo_retry() {
        assert_recording(
            "wrong_password",
            &["sudo:alice", "err:AuthFailed", "sudo:alice", PROCEED],
        );
    }

    #[test]
    fn recorded_three_wrong_passwords() {
        assert_recording(
            "wrong_password_3x",
            &[
                "sudo:alice",
                "err:AuthFailed",
                "sudo:alice",
                "err:AuthFailed",
                "sudo:alice",
                "err:AuthFailed",
            ],
        );
    }

    #[test]
    fn recorded_aur_install_with_menus() {
        // Diff accepted with Y, edit menu answered N, then sudo and pacman's confirmation.
        assert_recording(
            "aur_install_menus",
            &[
                CLEAN,
                DIFFS,
                "yn:Proceed with install?:Y",
                "menu:==> PKGBUILDs to edit?",
                "sudo:alice",
                PROCEED,
            ],
        );
    }

    #[test]
    fn recorded_aur_install_as_the_app_runs_it() {
        // `--answerdiff None --answeredit None`: yay still prints the clean and diff menus
        // (the diff one is answered by the flag, echoed as `==> None`) and has no edit menu.
        assert_recording("aur_install_app", &[CLEAN, DIFFS, "sudo:alice", PROCEED]);
    }

    #[test]
    fn recorded_syu_exclude_menu() {
        // The menu `yay -Syu` shows before the transaction: a `->` warning line sits
        // between its heading and the bare `==> ` prompt.
        for chunk in [1, 7, 1_000_000] {
            let ev = run(&recorded("syu_repo"), chunk);
            assert_eq!(
                ev[1],
                PromptEvent::Menu {
                    text:
                        "==> Packages to exclude: (eg: \"1 2 3\", \"1-3\", \"^4\" or repo name)\n\
                           -> Excluding packages may cause partial upgrades and break systems"
                            .into()
                },
                "chunk size {chunk}"
            );
        }
        assert_recording("syu_repo", &["sudo:alice", EXCLUDE, PROCEED]);
        assert_recording("syu_aur", &["sudo:alice", EXCLUDE, CLEAN, DIFFS, PROCEED]);
    }

    #[test]
    fn recorded_removal() {
        assert_recording(
            "remove",
            &["sudo:alice", "yn:Do you want to remove these packages?:Y"],
        );
        assert_recording("remove_not_found", &["sudo:alice", "err:TargetNotFound"]);
    }

    #[test]
    fn recorded_package_not_found() {
        assert_recording("target_not_found", &["err:TargetNotFound"]);
    }

    #[test]
    fn recorded_db_lock_held_by_another_pacman() {
        // yay prints the notice and keeps waiting on that line: no newline ever follows.
        assert_recording("db_lock", &["err:DbLock"]);
    }

    #[test]
    fn charset_select_escape_is_not_text() {
        // makepkg colours end with `ESC ( B ESC [ m`; the `B` must not leak into the line.
        let data = b"\x1b[1m\x1b[32m==>\x1b(B\x1b[m\x1b[1m Making package: foo\x1b(B\x1b[m\r\n";
        let mut d = PromptDetector::new();
        assert!(d.feed(data).is_empty());
        assert_eq!(
            d.recent.back().map(String::as_str),
            Some("==> Making package: foo")
        );
    }

    #[test]
    fn answered_prompts_are_still_reported() {
        let data =
            b":: Proceed with install? [Y/n] n\r\n==> [N]one [A]ll or (1 2 3)\r\n==> None\r\n";
        for chunk in [1, 5, 1000] {
            let got: Vec<String> = run(data, chunk).iter().map(shape).collect();
            assert_eq!(
                got,
                [
                    "yn:Proceed with install?:Y",
                    "menu:==> [N]one [A]ll or (1 2 3)"
                ],
                "chunk {chunk}"
            );
        }
    }

    #[test]
    fn commit_failed_alone() {
        let ev = run(
            b"error: failed to commit transaction (invalid or corrupted package)\n",
            4,
        );
        assert!(ev.iter().any(err(ErrorKind::CommitFailed)));
    }

    #[test]
    fn ansi_is_stripped_even_when_split() {
        let data = b"\x1b[1;34m::\x1b[0m\x1b[1m Proceed with installation? [Y/n] \x1b[0m";
        for chunk in [1, 2, 100] {
            assert_eq!(
                run(data, chunk),
                vec![PromptEvent::YesNo {
                    question: "Proceed with installation?".into(),
                    default_yes: true
                }]
            );
        }
        let osc = b"\x1b]0;title\x07[sudo] password for bob: ";
        assert_eq!(
            run(osc, 1),
            vec![PromptEvent::SudoPassword { user: "bob".into() }]
        );
    }

    #[test]
    fn doas_prompt() {
        assert_eq!(
            run(b"\rdoas (carol@arch) password: ", 3),
            vec![PromptEvent::SudoPassword {
                user: "carol".into()
            }]
        );
    }

    #[test]
    fn carriage_return_overwrites_progress() {
        let ev = run(b"downloading [Y/n\rdone\n", 100);
        assert!(ev.is_empty());
    }

    #[test]
    fn bare_arrow_is_not_a_menu() {
        assert!(run(b"==> ", 1).is_empty());
        assert!(run(b"==> Making package: foo 1.0\n", 3).is_empty());
    }

    #[test]
    fn prompt_not_repeated_across_chunks() {
        let mut d = PromptDetector::new();
        assert_eq!(d.feed(b"[sudo] password for x: ").len(), 1);
        assert!(d.feed(b"").is_empty());
        assert!(d.feed(b"\n").is_empty());
    }
}
