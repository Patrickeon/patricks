// commands/document.rs — Document 관련 Tauri commands
// DS-60 §3.4: list_documents, read_document, write_latest_document

use tauri::{Emitter, State};

use crate::{
    app_state::AppState,
    file_document::{DocumentChanged, DocumentContent, DocumentTree, DocumentWriteResult, FileDocumentService},
    models::error::AppError,
};

/// workspace 문서 트리를 조회한다 (DS-60 §3.4)
#[tauri::command]
pub async fn list_documents(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<DocumentTree, AppError> {
    let workspace_path = state.get_workspace_path(&workspace_id)?;
    let svc = FileDocumentService::new(&workspace_path);
    svc.list_documents(&workspace_id)
}

/// workspace root 하위 문서를 읽는다 (DS-60 §3.4)
#[tauri::command]
pub async fn read_document(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<DocumentContent, AppError> {
    let workspace_path = state.get_workspace_path(&workspace_id)?;
    let svc = FileDocumentService::new(&workspace_path);
    svc.read_document(&path)
}

/// `_archive` 백업 후 `.latest.md` 를 갱신한다 (DS-60 §3.4)
#[tauri::command]
pub async fn write_latest_document(
    workspace_id: String,
    path: String,
    content: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<DocumentWriteResult, AppError> {
    let workspace_path = state.get_workspace_path(&workspace_id)?;
    let svc = FileDocumentService::new(&workspace_path);
    let result = svc.write_latest_document(&path, &content)?;

    // document:changed event emit (DS-60 §4.1)
    let event_payload = DocumentChanged {
        path: path.clone(),
        version: result.version_hint.clone(),
        last_updated: result.version_hint.clone(),
    };
    let _ = app.emit("document:changed", event_payload);

    Ok(result)
}
