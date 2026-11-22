use tauri::ipc::Channel;
use zeroize::Zeroizing;

use crate::plan::QueueItemDto;
use crate::state::AppState;
use crate::tx::{TxEventDto, TxRecord};

#[tauri::command]
pub async fn start_transaction(
    state: tauri::State<'_, AppState>,
    items: Vec<QueueItemDto>,
    skip_review: bool,
    rows: u16,
    cols: u16,
    on_event: Channel<TxEventDto>,
) -> Result<(), String> {
    state
        .transactions
        .start(items, skip_review, (rows, cols), on_event)
        .await
}

#[tauri::command]
pub fn answer_password(state: tauri::State<'_, AppState>, password: String) -> Result<(), String> {
    state.transactions.answer_password(Zeroizing::new(password))
}

#[tauri::command]
pub fn answer_yes_no(state: tauri::State<'_, AppState>, yes: bool) -> Result<(), String> {
    state.transactions.answer_yes_no(yes)
}

#[tauri::command]
pub fn send_input(state: tauri::State<'_, AppState>, text: String) -> Result<(), String> {
    state.transactions.send_input(&text)
}

#[tauri::command]
pub fn resize_terminal(
    state: tauri::State<'_, AppState>,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    state.transactions.resize(rows, cols)
}

#[tauri::command]
pub fn cancel_transaction(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.transactions.cancel()
}

#[tauri::command]
pub fn last_transaction(state: tauri::State<'_, AppState>) -> Option<TxRecord> {
    state.transactions.last_transaction()
}
