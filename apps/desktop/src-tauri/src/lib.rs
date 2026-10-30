//! YayGUI desktop backend (Tauri 2).

mod search;
mod state;

use state::AppState;
use yaygui_core::ensure_not_root;

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
        .invoke_handler(tauri::generate_handler![search_packages])
        .run(tauri::generate_context!())
    {
        eprintln!("yaygui: error while running application: {e}");
        std::process::exit(1);
    }
}
