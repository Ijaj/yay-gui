use crate::plan::{PlanResponse, QueueItemDto};
use crate::state::AppState;

#[tauri::command]
pub async fn plan_transaction(
    state: tauri::State<'_, AppState>,
    items: Vec<QueueItemDto>,
    skip_review: bool,
) -> Result<PlanResponse, String> {
    Ok(state.plan.plan(items, skip_review).await.response)
}
