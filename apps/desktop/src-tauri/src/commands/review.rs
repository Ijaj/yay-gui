use crate::review::ReviewBundle;
use crate::state::AppState;

#[tauri::command]
pub async fn get_review(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<ReviewBundle, String> {
    state.reviews.review(name).await
}

#[tauri::command]
pub fn approve_review(
    state: tauri::State<'_, AppState>,
    pkgbase: String,
    commit: String,
) -> Result<(), String> {
    state.reviews.approve(&pkgbase, &commit)
}
