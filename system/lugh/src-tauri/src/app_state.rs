// app_state.rs — Tauri AppState (workspace, session service 레지스트리)
// DS-20 §3.2

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tauri::AppHandle;

use crate::{
    agent_session::AgentSessionService,
    models::{
        error::{AppError, AppResult},
        workspace::AgiteamConfig,
    },
};

/// 앱 전역 공유 상태
pub struct AppState {
    /// workspace_id → workspace_path
    workspaces: Arc<RwLock<HashMap<String, String>>>,
    /// workspace_id → agiteam.json 파싱 결과
    configs: Arc<RwLock<HashMap<String, AgiteamConfig>>>,
    /// workspace_id → AgentSessionService
    sessions: Arc<RwLock<HashMap<String, Arc<AgentSessionService>>>>,
    /// session_id → workspace_id (역방향 조회용)
    session_workspace_map: Arc<RwLock<HashMap<String, String>>>,
    app_handle: AppHandle,
}

impl AppState {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            workspaces: Arc::new(RwLock::new(HashMap::new())),
            configs: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_workspace_map: Arc::new(RwLock::new(HashMap::new())),
            app_handle,
        }
    }

    /// workspace를 등록한다.
    pub fn set_workspace(&self, workspace_id: String, path: String) {
        let mut workspaces = self.workspaces.write().unwrap();
        workspaces.insert(workspace_id.clone(), path.clone());

        // agiteam.json 자동 로드
        let config_path = std::path::PathBuf::from(&path).join("agiteam.json");
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(config) = serde_json::from_str::<AgiteamConfig>(&content) {
                    let mut configs = self.configs.write().unwrap();
                    configs.insert(workspace_id.clone(), config);
                }
            }
        }

        // AgentSessionService 생성
        let svc = AgentSessionService::new(self.app_handle.clone(), path);
        let mut sessions = self.sessions.write().unwrap();
        sessions.insert(workspace_id, Arc::new(svc));
    }

    /// workspace_id → 경로 조회
    pub fn get_workspace_path(&self, workspace_id: &str) -> AppResult<String> {
        self.workspaces
            .read()
            .unwrap()
            .get(workspace_id)
            .cloned()
            .ok_or_else(|| AppError::workspace_not_found(workspace_id))
    }

    /// workspace_id → AgiteamConfig 조회
    pub fn get_agiteam_config(&self, workspace_id: &str) -> AppResult<AgiteamConfig> {
        self.configs
            .read()
            .unwrap()
            .get(workspace_id)
            .cloned()
            .ok_or_else(|| AppError::new("CONFIG_NOT_LOADED", format!("workspace {} config 미로드", workspace_id)))
    }

    /// workspace_id → AgentSessionService 조회
    pub fn session_service(&self, workspace_id: &str) -> AppResult<Arc<AgentSessionService>> {
        self.sessions
            .read()
            .unwrap()
            .get(workspace_id)
            .cloned()
            .ok_or_else(|| AppError::new("SESSION_SERVICE_NOT_FOUND", format!("workspace {} 세션 서비스 없음", workspace_id)))
    }

    /// session_id → workspace_id 역방향 조회
    pub fn get_workspace_id_for_session(&self, session_id: &str) -> AppResult<String> {
        // 모든 session service에서 세션 검색
        let sessions = self.sessions.read().unwrap();
        for (workspace_id, svc) in sessions.iter() {
            if svc.get_session(session_id).is_ok() {
                return Ok(workspace_id.clone());
            }
        }
        Err(AppError::new("SESSION_NOT_FOUND", format!("세션 {} 없음", session_id)))
    }

    /// session_id → workspace_id 역방향 맵에 등록
    pub fn register_session(&self, session_id: String, workspace_id: String) {
        let mut map = self.session_workspace_map.write().unwrap();
        map.insert(session_id, workspace_id);
    }
}
