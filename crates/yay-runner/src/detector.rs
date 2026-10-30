//! Incremental prompt/error detector for yay, pacman, makepkg and sudo/doas output.

use std::collections::VecDeque;

/// Category of a recognised failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// `unable to lock database`: another pacman holds (or left) `db.lck`.
    DbLock,
    /// `target not found: pkg`.
    TargetNotFound,
    /// `failed to commit transaction`.
    CommitFailed,
    /// `conflicting files` (reported instead of `CommitFailed` on the same line).
    ConflictingFiles,
    /// makepkg `A failure occurred in build()` / `Error making:`.
    BuildFailed,
    /// Wrong sudo password (`Sorry, try again.`).
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
            if let Some(ev) = self.match_prompt(&text) {
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
                    // Two-byte escapes (e.g. ESC ( B charset select): treat the next byte as final.
                    _ => Esc::None,
                };
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
            && let Some(ev) = self.match_prompt(&text).or_else(|| classify_error(&text))
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
        for (suffix, default_yes) in [("[Y/n]", true), ("[y/N]", false)] {
            if let Some(q) = t.strip_suffix(suffix) {
                let question = q.trim().trim_start_matches(':').trim().to_owned();
                return Some(PromptEvent::YesNo {
                    question,
                    default_yes,
                });
            }
        }
        if t == "==>" {
            return self.menu_text();
        }
        None
    }

    /// A bare `==> ` is only a menu when the previous line was the options line,
    /// otherwise a chunk split right after `==> ` would be misreported.
    fn menu_text(&self) -> Option<PromptEvent> {
        let last = self.recent.back()?;
        if !(last.contains("[N]one") || last.contains("1 2 3")) {
            return None;
        }
        let mut parts = Vec::new();
        if self.recent.len() >= 2 {
            let head = &self.recent[self.recent.len() - 2];
            if head.trim_start().starts_with("==>") {
                parts.push(head.trim().to_owned());
            }
        }
        parts.push(last.trim().to_owned());
        Some(PromptEvent::Menu {
            text: parts.join("\n"),
        })
    }
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
    let kind =
        if text.contains("unable to lock database") || text.contains("could not lock database") {
            ErrorKind::DbLock
        } else if text.contains("conflicting files") {
            ErrorKind::ConflictingFiles
        } else if text.contains("target not found") {
            ErrorKind::TargetNotFound
        } else if text.contains("failed to commit transaction") {
            ErrorKind::CommitFailed
        } else if text.contains("A failure occurred in")
            || text.trim_start().starts_with("Error making:")
            || text.contains("Error making:")
        {
            ErrorKind::BuildFailed
        } else if text.trim() == "Sorry, try again." {
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
