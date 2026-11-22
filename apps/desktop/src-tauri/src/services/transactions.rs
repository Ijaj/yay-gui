//! The running transaction: start, prompt answers, cancel, last record.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use aurdeck_core::TransactionState;
use aurdeck_yay_runner::{Overlay, TxConfig, TxEvent, TxHandle, start_transaction};
use tauri::ipc::Channel;
use zeroize::Zeroizing;

use super::plan::PlanService;
use super::{lock, now};
use crate::plan::QueueItemDto;
use crate::tx::{self, TxEventDto, TxRecord, Utf8Decoder};

pub struct TransactionService {
    pub(crate) plan: Arc<PlanService>,
    pub(crate) tx: Mutex<Option<Arc<TxHandle>>>,
    pub(crate) starting: tokio::sync::Mutex<()>,
    pub(crate) config_dir: PathBuf,
    pub(crate) record_path: PathBuf,
    /// Debug builds only: run this instead of `yay` (`AURDECK_YAY`).
    pub(crate) yay_override: Option<String>,
}

impl TransactionService {
    fn current_tx(&self) -> Result<Arc<TxHandle>, String> {
        lock(&self.tx)
            .clone()
            .filter(|t| !t.is_finished())
            .ok_or_else(|| "no transaction is running".to_owned())
    }

    /// Re-plans the queue (the frontend's plan is never trusted) and runs it.
    pub async fn start(
        &self,
        items: Vec<QueueItemDto>,
        skip_review: bool,
        size: (u16, u16),
        channel: Channel<TxEventDto>,
    ) -> Result<(), String> {
        let _guard = self.starting.lock().await;
        if self.current_tx().is_ok() {
            return Err("a transaction is already running".into());
        }
        let outcome = self.plan.plan(items, skip_review).await;
        if !outcome.response.can_run {
            return Err(outcome.response.error.map_or_else(
                || {
                    if outcome.response.lock_held {
                        "the pacman database is locked by another program".to_owned()
                    } else {
                        "nothing to do".to_owned()
                    }
                },
                |e| e.message,
            ));
        }

        // The overlay lives inside the event sink, so it is removed when the run ends.
        let overlay = if outcome.approved.is_empty() {
            None
        } else {
            Some(Overlay::create(&self.config_dir, &outcome.approved).map_err(|e| e.to_string())?)
        };
        let config = TxConfig {
            program_override: self.yay_override.clone(),
            env: overlay.as_ref().map(Overlay::env).unwrap_or_default(),
            size,
        };

        let mut record = TxRecord {
            state: TransactionState::Approved,
            commands: outcome.response.commands.iter().map(Into::into).collect(),
            started_at: now(),
            updated_at: now(),
            interrupted: false,
        };
        let record_path = self.record_path.clone();
        let mut decoder = Utf8Decoder::default();
        let handle = start_transaction(outcome.commands, config, move |ev| {
            let _keep_alive = &overlay;
            if let TxEvent::State(s) = &ev {
                record.state = s.clone();
                record.updated_at = now();
                if let Err(e) = tx::save_record(&record_path, &record) {
                    eprintln!("aurdeck: could not save transaction record: {e}");
                }
            }
            if let Some(dto) = tx::to_dto(ev, &mut decoder) {
                let _ = channel.send(dto);
            }
        });
        *lock(&self.tx) = Some(Arc::new(handle));
        Ok(())
    }

    pub fn answer_password(&self, password: Zeroizing<String>) -> Result<(), String> {
        self.current_tx()?
            .answer_password(password)
            .map_err(|e| e.to_string())
    }

    pub fn answer_yes_no(&self, yes: bool) -> Result<(), String> {
        self.current_tx()?
            .answer_yes_no(yes)
            .map_err(|e| e.to_string())
    }

    pub fn send_input(&self, text: &str) -> Result<(), String> {
        self.current_tx()?
            .write_input(text.as_bytes())
            .map_err(|e| e.to_string())
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), String> {
        match self.current_tx() {
            Ok(t) => t.resize(rows, cols).map_err(|e| e.to_string()),
            Err(_) => Ok(()),
        }
    }

    pub fn cancel(&self) -> Result<(), String> {
        self.current_tx()?.cancel();
        Ok(())
    }

    pub fn last_transaction(&self) -> Option<TxRecord> {
        tx::load_record(&self.record_path, self.current_tx().is_ok())
    }
}
