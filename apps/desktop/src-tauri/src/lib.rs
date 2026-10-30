//! YayGUI desktop backend (Tauri 2).

mod plan;
mod review;
mod search;
mod state;
mod tx;

use state::AppState;
use tauri::ipc::Channel;
use yaygui_core::ensure_not_root;
use zeroize::Zeroizing;

/// Refuse to run with root privileges (real or effective uid).
fn check_not_root() -> Result<(), yaygui_core::SafetyError> {
    ensure_not_root(rustix::process::getuid().as_raw())?;
    ensure_not_root(rustix::process::geteuid().as_raw())
}

#[tauri::command]
async fn search_packages(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<search::SearchResponse, String> {
    Ok(state.search(&query).await)
}

#[tauri::command]
async fn plan_transaction(
    state: tauri::State<'_, AppState>,
    items: Vec<plan::QueueItemDto>,
    skip_review: bool,
) -> Result<plan::PlanResponse, String> {
    Ok(state.plan(items, skip_review).await.response)
}

#[tauri::command]
async fn get_review(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<review::ReviewBundle, String> {
    state.review(name).await
}

#[tauri::command]
fn approve_review(
    state: tauri::State<'_, AppState>,
    pkgbase: String,
    commit: String,
) -> Result<(), String> {
    state.approve(&pkgbase, &commit)
}

#[tauri::command]
async fn start_transaction(
    state: tauri::State<'_, AppState>,
    items: Vec<plan::QueueItemDto>,
    skip_review: bool,
    rows: u16,
    cols: u16,
    on_event: Channel<tx::TxEventDto>,
) -> Result<(), String> {
    state
        .start(items, skip_review, (rows, cols), on_event)
        .await
}

#[tauri::command]
fn answer_password(state: tauri::State<'_, AppState>, password: String) -> Result<(), String> {
    state.answer_password(Zeroizing::new(password))
}

#[tauri::command]
fn answer_yes_no(state: tauri::State<'_, AppState>, yes: bool) -> Result<(), String> {
    state.answer_yes_no(yes)
}

#[tauri::command]
fn send_input(state: tauri::State<'_, AppState>, text: String) -> Result<(), String> {
    state.send_input(&text)
}

#[tauri::command]
fn resize_terminal(state: tauri::State<'_, AppState>, rows: u16, cols: u16) -> Result<(), String> {
    state.resize(rows, cols)
}

#[tauri::command]
fn cancel_transaction(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.cancel()
}

#[tauri::command]
fn last_transaction(state: tauri::State<'_, AppState>) -> Option<tx::TxRecord> {
    state.last_transaction()
}

pub fn run() {
    if let Err(e) = check_not_root() {
        eprintln!("yaygui: refusing to start: {e}");
        std::process::exit(1);
    }

    let state = match AppState::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("yaygui: failed to initialise: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            search_packages,
            plan_transaction,
            get_review,
            approve_review,
            start_transaction,
            answer_password,
            answer_yes_no,
            send_input,
            resize_terminal,
            cancel_transaction,
            last_transaction,
        ])
        .run(tauri::generate_context!())
    {
        eprintln!("yaygui: error while running application: {e}");
        std::process::exit(1);
    }
}
