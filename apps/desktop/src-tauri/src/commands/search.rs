use crate::search::SearchResponse;
use crate::state::AppState;

#[tauri::command]
pub async fn search_packages(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<SearchResponse, String> {
    Ok(state.search.search(&query).await)
}
