//! Transaction events for the frontend and the persisted transaction record.

use std::path::Path;

use serde::{Deserialize, Serialize};
use yaygui_core::TransactionState;
use yaygui_yay_runner::{ErrorKind, PromptEvent, TxEvent};

use crate::plan::CommandDto;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum PromptDto {
    SudoPassword {
        user: String,
    },
    YesNo {
        question: String,
        default_yes: bool,
    },
    Menu {
        text: String,
    },
    Error {
        error_kind: &'static str,
        line: String,
    },
}

fn error_kind(k: ErrorKind) -> &'static str {
    match k {
        ErrorKind::DbLock => "db_lock",
        ErrorKind::TargetNotFound => "target_not_found",
        ErrorKind::CommitFailed => "commit_failed",
        ErrorKind::ConflictingFiles => "conflicting_files",
        ErrorKind::BuildFailed => "build_failed",
        ErrorKind::AuthFailed => "auth_failed",
    }
}

impl From<PromptEvent> for PromptDto {
    fn from(p: PromptEvent) -> Self {
        match p {
            PromptEvent::SudoPassword { user } => Self::SudoPassword { user },
            PromptEvent::YesNo {
                question,
                default_yes,
            } => Self::YesNo {
                question,
                default_yes,
            },
            PromptEvent::Menu { text } => Self::Menu { text },
            PromptEvent::Error { kind, line } => Self::Error {
                error_kind: error_kind(kind),
                line,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum TxEventDto {
    /// Flattened: `{"type":"state","state":"failed","reason":"..."}`.
    State {
        #[serde(flatten)]
        state: TransactionState,
    },
    CommandStarted {
        index: usize,
        total: usize,
        description: String,
        argv: Vec<String>,
    },
    Output {
        text: String,
    },
    Prompt {
        prompt: PromptDto,
    },
    CommandFinished {
        index: usize,
        code: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        signal: Option<String>,
    },
}

/// Turns PTY byte chunks into strings without splitting UTF-8 sequences.
#[derive(Debug, Default)]
pub struct Utf8Decoder {
    pending: Vec<u8>,
}

impl Utf8Decoder {
    /// Decodes as much as possible; an incomplete trailing sequence is kept
    /// for the next call and invalid bytes become U+FFFD.
    pub fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut out = String::new();
        let mut rest: &[u8] = &self.pending;
        loop {
            match std::str::from_utf8(rest) {
                Ok(s) => {
                    out.push_str(s);
                    rest = &[];
                    break;
                }
                Err(e) => {
                    let (good, bad) = rest.split_at(e.valid_up_to());
                    // valid_up_to() bytes are valid UTF-8 by definition.
                    out.push_str(std::str::from_utf8(good).unwrap_or_default());
                    match e.error_len() {
                        Some(n) => {
                            out.push('\u{FFFD}');
                            rest = &bad[n..];
                        }
                        None => {
                            rest = bad;
                            break;
                        }
                    }
                }
            }
        }
        self.pending = rest.to_vec();
        out
    }
}

/// Converts a runner event, or `None` when there is nothing to send.
pub fn to_dto(ev: TxEvent, decoder: &mut Utf8Decoder) -> Option<TxEventDto> {
    Some(match ev {
        TxEvent::State(state) => TxEventDto::State { state },
        TxEvent::CommandStarted {
            index,
            total,
            description,
            argv,
        } => TxEventDto::CommandStarted {
            index,
            total,
            description,
            argv,
        },
        TxEvent::Output(bytes) => {
            let text = decoder.push(&bytes);
            if text.is_empty() {
                return None;
            }
            TxEventDto::Output { text }
        }
        TxEvent::Prompt(p) => TxEventDto::Prompt { prompt: p.into() },
        TxEvent::CommandFinished { index, status } => TxEventDto::CommandFinished {
            index,
            code: status.code,
            signal: status.signal,
        },
    })
}

/// The last transaction, persisted so a crash mid-run is visible on the next launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxRecord {
    pub state: TransactionState,
    pub commands: Vec<CommandRecord>,
    pub started_at: i64,
    pub updated_at: i64,
    /// Set when read back: the record says "running" but no transaction is.
    #[serde(default)]
    pub interrupted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandRecord {
    pub description: String,
    pub argv: Vec<String>,
}

impl From<&CommandDto> for CommandRecord {
    fn from(c: &CommandDto) -> Self {
        Self {
            description: c.description.clone(),
            argv: c.argv.clone(),
        }
    }
}

pub fn save_record(path: &Path, record: &TxRecord) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(record).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(tmp, path)
}

/// Reads the record; `running` marks it interrupted when nothing runs now.
pub fn load_record(path: &Path, running: bool) -> Option<TxRecord> {
    let bytes = std::fs::read(path).ok()?;
    let mut r: TxRecord = serde_json::from_slice(&bytes).ok()?;
    r.interrupted = !running && !r.state.is_terminal();
    Some(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use yaygui_yay_runner::ExitStatus;

    #[test]
    fn utf8_split_across_chunks() {
        let mut d = Utf8Decoder::default();
        let s = "héllo ✓".as_bytes();
        let mut out = String::new();
        for b in s {
            out.push_str(&d.push(std::slice::from_ref(b)));
        }
        assert_eq!(out, "héllo ✓");
        assert_eq!(d.push(b"a\xffb"), "a\u{FFFD}b");
        assert_eq!(d.push(b"\xe2\x9c"), "");
        assert_eq!(d.push(b"\x93!"), "✓!");
    }

    #[test]
    fn event_shapes_match_ts_contract() {
        let mut d = Utf8Decoder::default();
        let j = |e| serde_json::to_value(to_dto(e, &mut Utf8Decoder::default())).unwrap();
        assert_eq!(
            j(TxEvent::State(TransactionState::Failed {
                reason: "x".into()
            })),
            json!({"type": "state", "state": "failed", "reason": "x"})
        );
        assert_eq!(
            j(TxEvent::State(TransactionState::Running)),
            json!({"type": "state", "state": "running"})
        );
        assert_eq!(
            j(TxEvent::Prompt(PromptEvent::YesNo {
                question: "Proceed?".into(),
                default_yes: true
            })),
            json!({"type": "prompt", "prompt": {"kind": "yes_no", "question": "Proceed?", "defaultYes": true}})
        );
        assert_eq!(
            j(TxEvent::Prompt(PromptEvent::Error {
                kind: ErrorKind::DbLock,
                line: "l".into()
            })),
            json!({"type": "prompt", "prompt": {"kind": "error", "errorKind": "db_lock", "line": "l"}})
        );
        assert_eq!(
            j(TxEvent::CommandFinished {
                index: 0,
                status: ExitStatus {
                    code: 1,
                    signal: None
                }
            }),
            json!({"type": "command_finished", "index": 0, "code": 1})
        );
        assert_eq!(to_dto(TxEvent::Output(vec![0xe2]), &mut d), None);
    }

    #[test]
    fn record_roundtrip_and_interrupted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/last-transaction.json");
        let mut r = TxRecord {
            state: TransactionState::Running,
            commands: vec![],
            started_at: 1,
            updated_at: 2,
            interrupted: false,
        };
        save_record(&path, &r).unwrap();
        assert!(load_record(&path, false).unwrap().interrupted);
        assert!(!load_record(&path, true).unwrap().interrupted);
        r.state = TransactionState::Succeeded;
        save_record(&path, &r).unwrap();
        assert!(!load_record(&path, false).unwrap().interrupted);
    }
}
