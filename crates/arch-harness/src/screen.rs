//! Watches the raw terminal output for the one prompt `PromptDetector` does
//! not report (yay's `==> Packages to exclude:` menu) and for stalls. It never
//! answers anything else: any other bare `==>` menu fails the scenario.

use std::time::Duration;

use crate::prompts::{EXCLUDE_AUR_UPGRADES, strip_ansi};

/// Silence after which the screen is inspected.
pub const SETTLE: Duration = Duration::from_secs(2);
/// No output for this long and nothing recognised: stuck on an unknown prompt.
pub const IDLE_LIMIT: Duration = Duration::from_secs(120);
const TAIL_BYTES: usize = 4096;

/// What to do after a quiet moment.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Nothing to do yet.
    Nothing,
    /// Type this line (plus Enter).
    Answer(&'static str),
    /// Fail the scenario with this reason.
    Fail(String),
}

/// Screen tail plus the bookkeeping that keeps an answer from being repeated.
#[derive(Default)]
pub struct Screen {
    tail: Vec<u8>,
    total: usize,
    /// `total` at the time something already dealt with the current screen.
    handled_at: Option<usize>,
    /// The exclude menu was answered for the running command.
    exclude_answered: bool,
}

impl Screen {
    /// A new command started: its exclude menu may be answered once.
    pub fn command_started(&mut self) {
        self.exclude_answered = false;
        self.tail.clear();
        self.handled_at = None;
    }

    /// Raw bytes from the PTY.
    pub fn output(&mut self, bytes: &[u8]) {
        self.total += bytes.len();
        self.tail.extend_from_slice(bytes);
        if self.tail.len() > TAIL_BYTES {
            let cut = self.tail.len() - TAIL_BYTES;
            self.tail.drain(..cut);
        }
    }

    /// The detector reported a prompt for the current screen (and the caller
    /// answers or fails it), so this screen needs no further attention.
    pub fn prompt_reported(&mut self) {
        self.handled_at = Some(self.total);
    }

    /// Called when no event arrived for `idle`.
    pub fn poll(&mut self, idle: Duration) -> Decision {
        if idle < SETTLE || self.handled_at == Some(self.total) {
            return Decision::Nothing;
        }
        let screen = strip_ansi(&self.tail);
        let lines: Vec<&str> = screen
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if lines.last() == Some(&"==>") {
            let n = lines.len();
            let is_exclude = n >= 3
                && lines[n - 3].starts_with("==> Packages to exclude:")
                && lines[n - 2].starts_with("-> Excluding packages");
            if is_exclude && !self.exclude_answered {
                self.exclude_answered = true;
                // The echo of our answer must not be mistaken for a prompt.
                self.tail.clear();
                self.handled_at = Some(self.total);
                return Decision::Answer(EXCLUDE_AUR_UPGRADES);
            }
            let shown = lines[n.saturating_sub(4)..].join(" | ");
            return Decision::Fail(if is_exclude {
                format!("the exclude menu appeared twice for one command: {shown:?}")
            } else {
                format!("unrecognised yay menu, not answering: {shown:?}")
            });
        }
        if idle >= IDLE_LIMIT {
            let shown = lines[lines.len().saturating_sub(3)..].join(" | ");
            return Decision::Fail(format!(
                "no output for {} s; probably an unrecognised prompt. Screen ends with: {shown:?}",
                idle.as_secs()
            ));
        }
        Decision::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXCLUDE: &str = "\x1b[1m1  aur/x  1 -> 2\x1b[0m\n==> Packages to exclude: (eg: \"1 2 3\", \"1-3\", \"^4\" or repo name)\n -> Excluding packages may cause partial upgrades and break systems\n==> ";
    const QUIET: Duration = Duration::from_secs(3);

    fn screen(text: &str) -> Screen {
        let mut s = Screen::default();
        s.command_started();
        s.output(text.as_bytes());
        s
    }

    #[test]
    fn answers_the_exclude_menu_once_and_excludes_aur() {
        let mut s = screen(EXCLUDE);
        assert_eq!(s.poll(Duration::from_secs(1)), Decision::Nothing);
        assert_eq!(s.poll(QUIET), Decision::Answer("aur"));
    }

    #[test]
    fn does_not_answer_again_after_the_echo() {
        let mut s = screen(EXCLUDE);
        assert_eq!(s.poll(QUIET), Decision::Answer("aur"));
        // yay echoes the typed answer, then goes quiet (for example in sudo).
        s.output(b"aur\r\n");
        assert_eq!(s.poll(QUIET), Decision::Nothing);
        s.output(b"[sudo] password for dev: ");
        assert_eq!(s.poll(QUIET), Decision::Nothing);
    }

    #[test]
    fn a_stale_exclude_line_does_not_unlock_a_later_menu() {
        let mut s = screen(EXCLUDE);
        assert_eq!(s.poll(QUIET), Decision::Answer("aur"));
        s.output(b"aur\r\n:: Downloaded PKGBUILD\n==> Diffs to show?\n==> [N]one [A]ll\n==> ");
        assert!(matches!(s.poll(QUIET), Decision::Fail(m) if m.contains("unrecognised")));
    }

    #[test]
    fn a_second_exclude_menu_in_the_same_command_fails() {
        let mut s = screen(EXCLUDE);
        assert_eq!(s.poll(QUIET), Decision::Answer("aur"));
        s.output(EXCLUDE.as_bytes());
        assert!(matches!(s.poll(QUIET), Decision::Fail(m) if m.contains("twice")));
        // a new command may be asked again
        s.command_started();
        s.output(EXCLUDE.as_bytes());
        assert_eq!(s.poll(QUIET), Decision::Answer("aur"));
    }

    #[test]
    fn other_bare_menus_fail() {
        let mut s = screen("==> Diffs to show?\n==> [N]one [A]ll\n==> ");
        assert!(matches!(s.poll(QUIET), Decision::Fail(_)));
    }

    #[test]
    fn menus_the_detector_reported_are_left_alone() {
        let mut s = screen("==> Packages to cleanBuild?\n==> [N]one [A]ll\n==> ");
        s.prompt_reported();
        assert_eq!(s.poll(QUIET), Decision::Nothing);
        s.output(b"N\r\n");
        assert_eq!(s.poll(QUIET), Decision::Nothing);
    }

    #[test]
    fn exclude_needle_mid_screen_is_not_the_menu() {
        let mut s = screen("==> Packages to exclude: x\n -> Excluding packages may\nbuilding...\n");
        assert_eq!(s.poll(QUIET), Decision::Nothing);
    }

    #[test]
    fn long_silence_fails() {
        let mut s = screen("compiling things\n");
        assert_eq!(s.poll(QUIET), Decision::Nothing);
        assert!(matches!(s.poll(IDLE_LIMIT), Decision::Fail(_)));
    }
}
