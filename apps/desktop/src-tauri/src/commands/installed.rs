use crate::installed::InstalledResponse;
use crate::state::AppState;

#[tauri::command]
pub async fn list_installed(
    state: tauri::State<'_, AppState>,
    refresh: bool,
) -> Result<InstalledResponse, String> {
    Ok(state.installed.installed(refresh).await)
}
