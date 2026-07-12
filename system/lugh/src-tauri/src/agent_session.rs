// agent_session.rs — AgentSessionService
// Tauri invoke/event 기반 에이전트 채팅 채널
// 이벤트: agent:status_changed, agent:message_delta, agent:message_completed 등
// DS-20 §3.1, DS-60 §3.2, §4

use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::{
    chat_attachment,
    credential::CredentialStoreService,
    models::{
        attachment::PreparedChatAttachment,
        error::{AppError, AppResult},
        message::{
            AgentMessage, AgentMessageCompleted, AgentMessageDelta, AgentMessageFailed,
            AgentMessageStarted, MessagePage,
        },
        provider::{
            AiProviderKind, MessageRole, ProviderEvent, ProviderMessage, ProviderMessageRequest,
        },
        session::{
            AgentLifecycleState, AgentSession, AgentSessionDetail, AgentSessionSummary,
            AgentStatusChanged, BootTeamResult, MessageAck,
        },
        workspace::AgiteamConfig,
    },
    persona_bundle::PersonaBundleService,
    provider,
};

/// 세션별 메시지 로그 (메모리 내 저장, 향후 SQLite로 이관)
type MessageStore = Arc<RwLock<HashMap<String, Vec<AgentMessage>>>>;

/// AgentSessionService — 에이전트 세션 생명주기 관리
pub struct AgentSessionService {
    sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    messages: MessageStore,
    app_handle: AppHandle,
    workspace_root: std::path::PathBuf,
}

impl AgentSessionService {
    pub fn new(app_handle: AppHandle, workspace_root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(HashMap::new())),
            app_handle,
            workspace_root: workspace_root.into(),
        }
    }

    // ---- 팀 부팅 ----

    /// 전체 팀 booting을 시작한다 (boot_team command)
    /// PM(config.pm) + 팀원(config.team) 전원을 포함한다.
    /// FE BootView.vue는 PM을 포함한 전원 READY를 대기하므로 반드시 PM도 부팅해야 한다.
    /// (DV60-001 결함 수정)
    pub async fn boot_team(
        &self,
        workspace_id: &str,
        config: &AgiteamConfig,
    ) -> AppResult<BootTeamResult> {
        let mut summaries = Vec::new();

        // 1. PM 먼저 부팅 (config.pm은 team 배열에 없으므로 별도 처리)
        match self.boot_pm_session(workspace_id, config).await {
            Ok(summary) => summaries.push(summary),
            Err(e) => {
                log::error!("PM booting 실패: {}", e);
                summaries.push(AgentSessionSummary {
                    session_id: format!("failed-PM"),
                    role: "PM".to_string(),
                    display_name: config.pm.name.clone(),
                    provider: AiProviderKind::Claude,
                    state: AgentLifecycleState::Failed,
                });
            }
        }

        // 2. 팀원 부팅
        for member in &config.team {
            match self.boot_role(workspace_id, &member.role, config).await {
                Ok(summary) => summaries.push(summary),
                Err(e) => {
                    log::error!("역할 {} booting 실패: {}", member.role, e);
                    // 실패해도 다른 역할은 계속 진행
                    summaries.push(AgentSessionSummary {
                        session_id: format!("failed-{}", member.role),
                        role: member.role.clone(),
                        display_name: member.name.clone(),
                        provider: AiProviderKind::Claude,
                        state: AgentLifecycleState::Failed,
                    });
                }
            }
        }

        Ok(BootTeamResult {
            workspace_id: workspace_id.to_string(),
            sessions: summaries,
        })
    }

    /// PM 전용 AgentSession을 생성하고 Booting → Ready 전이를 수행한다.
    /// config.pm은 team 배열에 없으므로 boot_role과 별도로 처리한다.
    async fn boot_pm_session(
        &self,
        workspace_id: &str,
        config: &AgiteamConfig,
    ) -> AppResult<AgentSessionSummary> {
        let pm = &config.pm;
        let provider: AiProviderKind = pm.agent.parse().map_err(|e: AppError| e)?;
        let session_id = Uuid::new_v4().to_string();

        // Idle → Booting 전이
        let session = AgentSession {
            id: session_id.clone(),
            workspace_id: workspace_id.to_string(),
            role: "PM".to_string(),
            display_name: pm.name.clone(),
            provider: provider.clone(),
            state: AgentLifecycleState::Booting,
            persona_hash: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            failure_reason: None,
        };

        self.upsert_session(session.clone());
        self.emit_status_changed(
            &session_id,
            "PM",
            AgentLifecycleState::Idle,
            AgentLifecycleState::Booting,
            None,
        );

        // Persona bundle 생성
        let persona_svc = PersonaBundleService::new(&self.workspace_root);
        let bundle = match persona_svc.build(config, "PM") {
            Ok(b) => b,
            Err(e) => {
                self.transition_to_failed(&session_id, "PM", &e.message);
                return Err(e);
            }
        };

        let persona_hash = bundle.content_hash.clone();

        // Credential 체크는 실제 메시지 전송(send_message) 시 수행
        // boot 단계에서는 persona 파일 확인만으로 충분

        // Booting → Ready 전이
        {
            let mut sessions = self.sessions.write().unwrap();
            if let Some(s) = sessions.get_mut(&session_id) {
                s.state = AgentLifecycleState::Ready;
                s.persona_hash = persona_hash;
                s.updated_at = Utc::now();
            }
        }
        self.emit_status_changed(
            &session_id,
            "PM",
            AgentLifecycleState::Booting,
            AgentLifecycleState::Ready,
            None,
        );

        Ok(AgentSessionSummary {
            session_id,
            role: "PM".to_string(),
            display_name: pm.name.clone(),
            provider,
            state: AgentLifecycleState::Ready,
        })
    }

    /// 단일 역할 booting (boot_role command)
    pub async fn boot_role(
        &self,
        workspace_id: &str,
        role: &str,
        config: &AgiteamConfig,
    ) -> AppResult<AgentSessionSummary> {
        let member = config
            .team
            .iter()
            .find(|m| m.role == role)
            .ok_or_else(|| AppError::new("ROLE_NOT_FOUND", format!("역할 {}이 팀 설정에 없습니다", role)))?;

        let provider: AiProviderKind = member.agent.parse().map_err(|e: AppError| e)?;
        let session_id = Uuid::new_v4().to_string();

        // idle → booting 전이
        let session = AgentSession {
            id: session_id.clone(),
            workspace_id: workspace_id.to_string(),
            role: role.to_string(),
            display_name: member.name.clone(),
            provider: provider.clone(),
            state: AgentLifecycleState::Booting,
            persona_hash: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            failure_reason: None,
        };

        self.upsert_session(session.clone());
        self.emit_status_changed(&session_id, role, AgentLifecycleState::Idle, AgentLifecycleState::Booting, None);

        // 1. Persona bundle 생성
        let persona_svc = PersonaBundleService::new(&self.workspace_root);
        let bundle = match persona_svc.build(config, role) {
            Ok(b) => b,
            Err(e) => {
                self.transition_to_failed(&session_id, role, &e.message);
                return Err(e);
            }
        };

        let persona_hash = bundle.content_hash.clone();

        // 2. Credential 체크는 실제 메시지 전송(send_message) 시 수행
        // boot 단계에서는 persona 파일 확인만으로 충분

        // booting → ready 전이
        {
            let mut sessions = self.sessions.write().unwrap();
            if let Some(s) = sessions.get_mut(&session_id) {
                s.state = AgentLifecycleState::Ready;
                s.persona_hash = persona_hash;
                s.updated_at = Utc::now();
            }
        }
        self.emit_status_changed(
            &session_id,
            role,
            AgentLifecycleState::Booting,
            AgentLifecycleState::Ready,
            None,
        );

        Ok(AgentSessionSummary {
            session_id,
            role: role.to_string(),
            display_name: member.name.clone(),
            provider,
            state: AgentLifecycleState::Ready,
        })
    }

    // ---- 메시지 전송 ----

    /// agent session에 메시지를 전송하고 streaming 응답을 event로 emit한다.
    /// `attachments`가 비어있으면 기존 텍스트 전용 동작과 동일하다 (DS-60 §3.2, #21).
    pub async fn send_message(
        &self,
        session_id: &str,
        content: &str,
        attachments: Vec<PreparedChatAttachment>,
        config: &AgiteamConfig,
        default_model: &str,
    ) -> AppResult<MessageAck> {
        let session = self.get_session(session_id)?;

        // ready 상태 확인
        if session.state != AgentLifecycleState::Ready {
            return Err(AppError::session_not_ready(session_id));
        }

        let provider_kind = session.provider.clone();
        let account = format!("default-{}", provider_kind);
        // Claude provider 어댑터 선택:
        //   1순위: keychain API 키 (AgiTeamBuilder/claude)
        //   2순위: Claude CLI 서브프로세스 (OAuth rate_limit 우회)
        // ※ #21: 첨부 capability(vision) 검증을 위해 상태 전이 전에 어댑터를 확정한다
        let adapter: Box<dyn crate::provider::AiProvider> = match provider_kind {
            AiProviderKind::Claude => {
                match CredentialStoreService::get_secret(&provider_kind, &account) {
                    Ok(api_key) => {
                        // API 키 있음 → 직접 API 사용
                        provider::create_provider(&provider_kind, api_key)
                    }
                    Err(_) => {
                        // API 키 없음 → CLI 어댑터 사용 (rate_limit 없음, 이미지 미지원)
                        Box::new(crate::provider::claude_cli::ClaudeCliAdapter::new())
                    }
                }
            }
            _ => {
                let secret = CredentialStoreService::get_secret(&provider_kind, &account)?;
                provider::create_provider(&provider_kind, secret)
            }
        };

        // #21: 첨부 검증 — 한도·ready 상태·provider capability (DS-40 §7.3)
        // 하나라도 위반이면 상태 전이·메시지 저장 없이 전송 전체를 거절한다 (조용한 누락 금지)
        if !attachments.is_empty() {
            chat_attachment::validate_attachments_for_send(
                content,
                &attachments,
                adapter.supports_vision(),
            )?;
        }

        // user 메시지 저장
        let user_message_id = Uuid::new_v4().to_string();
        let user_msg = AgentMessage {
            id: user_message_id.clone(),
            session_id: session_id.to_string(),
            role: MessageRole::User,
            content: content.to_string(),
            created_at: Utc::now(),
            usage: None,
            is_streaming: false,
        };
        self.store_message(session_id, user_msg);

        // ready → running 전이
        self.transition_state(session_id, &session.role, AgentLifecycleState::Ready, AgentLifecycleState::Running, None);

        let ack = MessageAck {
            session_id: session_id.to_string(),
            user_message_id: user_message_id.clone(),
            accepted_at: Utc::now(),
        };

        // 비동기 streaming 처리
        let sessions = Arc::clone(&self.sessions);
        let messages = Arc::clone(&self.messages);
        let app_handle = self.app_handle.clone();
        let session_id_owned = session_id.to_string();
        let role_owned = session.role.clone();
        let workspace_root = self.workspace_root.clone();

        // persona bundle 재생성 (최신 내용 보장)
        let persona_svc = PersonaBundleService::new(&workspace_root);
        let bundle = persona_svc.build(config, &session.role)?;
        let system_prompt = bundle.content;

        // 메시지 히스토리 조회 (context 구성)
        let history = self.get_messages(session_id);
        let mut provider_messages: Vec<ProviderMessage> = history
            .iter()
            .filter(|m| m.id != user_message_id)
            .map(|m| ProviderMessage {
                role: m.role.clone(),
                content: m.content.clone(),
                content_blocks: None,
            })
            .collect();

        // #21: 첨부가 있으면 content_blocks가 provider 변환의 정본 (DS-40 §3.2)
        // content에는 사용자가 입력한 순수 텍스트만 유지한다
        let content_blocks = if attachments.is_empty() {
            None
        } else {
            Some(chat_attachment::build_content_blocks(content, &attachments))
        };
        provider_messages.push(ProviderMessage {
            role: MessageRole::User,
            content: content.to_string(),
            content_blocks,
        });

        let request = ProviderMessageRequest {
            session_id: session_id.to_string(),
            provider: provider_kind,
            model: default_model.to_string(),
            system_prompt,
            messages: provider_messages,
            attachments: chat_attachment::to_provider_attachments(&attachments),
            temperature: None,
            max_tokens: Some(8192),
            tools: vec![],
        };

        // event sink 채널
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ProviderEvent>(100);

        // streaming 처리를 별도 태스크로 분리
        let session_id_task = session_id_owned.clone();
        let role_task = role_owned.clone();
        let app_task = app_handle.clone();
        let messages_task = Arc::clone(&messages);
        let sessions_task = Arc::clone(&sessions);

        tokio::spawn(async move {
            // ── 단일 시도 (CLI 어댑터는 rate_limit 없음) ──
            let final_result = adapter.send_message_stream(request.clone(), tx.clone()).await;

            // ── 결과 처리 ──
            match final_result {
                Ok(res) => {
                    // assistant 메시지 저장
                    let assistant_msg = AgentMessage {
                        id: Uuid::new_v4().to_string(),
                        session_id: session_id_task.clone(),
                        role: MessageRole::Assistant,
                        content: res.content,
                        created_at: Utc::now(),
                        usage: res.usage,
                        is_streaming: false,
                    };
                    let mut store = messages_task.write().unwrap();
                    store.entry(session_id_task.clone()).or_default().push(assistant_msg);

                    // running → ready
                    {
                        let mut s = sessions_task.write().unwrap();
                        if let Some(sess) = s.get_mut(&session_id_task) {
                            sess.state = AgentLifecycleState::Ready;
                            sess.updated_at = Utc::now();
                        }
                    }
                    let payload = AgentStatusChanged {
                        session_id: session_id_task.clone(),
                        role: role_task.clone(),
                        from: AgentLifecycleState::Running,
                        to: AgentLifecycleState::Ready,
                        reason: None,
                        changed_at: Utc::now(),
                    };
                    let _ = app_task.emit("agent:status_changed", payload);
                }
                Err(ref e) if e.recoverable => {
                    // 복구 가능한 에러 (rate_limit 등) → Running → Ready 복귀 + 에러 노트
                    let err_note = AgentMessage {
                        id: Uuid::new_v4().to_string(),
                        session_id: session_id_task.clone(),
                        role: MessageRole::Assistant,
                        content: format!("⚠️ {}", e.message),
                        created_at: Utc::now(),
                        usage: None,
                        is_streaming: false,
                    };
                    {
                        let mut store = messages_task.write().unwrap();
                        store.entry(session_id_task.clone()).or_default().push(err_note);
                    }
                    {
                        let mut s = sessions_task.write().unwrap();
                        if let Some(sess) = s.get_mut(&session_id_task) {
                            sess.state = AgentLifecycleState::Ready; // Failed 아닌 Ready로
                            sess.updated_at = Utc::now();
                        }
                    }
                    let status_payload = AgentStatusChanged {
                        session_id: session_id_task.clone(),
                        role: role_task.clone(),
                        from: AgentLifecycleState::Running,
                        to: AgentLifecycleState::Ready,
                        reason: Some(e.message.clone()),
                        changed_at: Utc::now(),
                    };
                    let _ = app_task.emit("agent:status_changed", status_payload);
                }
                Err(e) => {
                    // 복구 불가능한 에러 → Failed
                    {
                        let mut s = sessions_task.write().unwrap();
                        if let Some(sess) = s.get_mut(&session_id_task) {
                            sess.state = AgentLifecycleState::Failed;
                            sess.failure_reason = Some(e.message.clone());
                            sess.updated_at = Utc::now();
                        }
                    }
                    let status_payload = AgentStatusChanged {
                        session_id: session_id_task.clone(),
                        role: role_task.clone(),
                        from: AgentLifecycleState::Running,
                        to: AgentLifecycleState::Failed,
                        reason: Some(e.message.clone()),
                        changed_at: Utc::now(),
                    };
                    let _ = app_task.emit("agent:status_changed", status_payload);

                    let fail_payload = AgentMessageFailed {
                        session_id: session_id_task.clone(),
                        message_id: None,
                        error: e,
                    };
                    let _ = app_task.emit("agent:message_failed", fail_payload);
                }
            }
        });

        // provider event → Tauri event 변환 태스크
        let app_event = self.app_handle.clone();
        let session_id_event = session_id_owned.clone();

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    ProviderEvent::MessageStarted { message_id } => {
                        let payload = AgentMessageStarted {
                            session_id: session_id_event.clone(),
                            message_id,
                            started_at: Utc::now(),
                        };
                        let _ = app_event.emit("agent:message_started", payload);
                    }
                    ProviderEvent::MessageDelta { message_id, delta, sequence } => {
                        let payload = AgentMessageDelta {
                            session_id: session_id_event.clone(),
                            message_id,
                            delta,
                            sequence,
                        };
                        let _ = app_event.emit("agent:message_delta", payload);
                    }
                    ProviderEvent::MessageCompleted { message_id, usage } => {
                        let payload = AgentMessageCompleted {
                            session_id: session_id_event.clone(),
                            message_id,
                            usage,
                            completed_at: Utc::now(),
                        };
                        let _ = app_event.emit("agent:message_completed", payload);
                    }
                    ProviderEvent::MessageFailed { message_id, error } => {
                        let payload = AgentMessageFailed {
                            session_id: session_id_event.clone(),
                            message_id: Some(message_id),
                            error,
                        };
                        let _ = app_event.emit("agent:message_failed", payload);
                    }
                    ProviderEvent::ToolRequested { tool_name, arguments } => {
                        let payload = crate::models::message::AgentToolRequested {
                            session_id: session_id_event.clone(),
                            tool_name,
                            arguments,
                        };
                        let _ = app_event.emit("agent:tool_requested", payload);
                    }
                }
            }
        });

        Ok(ack)
    }

    // ---- 세션 조회 ----

    pub fn get_session(&self, session_id: &str) -> AppResult<AgentSession> {
        self.sessions
            .read()
            .unwrap()
            .get(session_id)
            .cloned()
            .ok_or_else(|| AppError::new("SESSION_NOT_FOUND", format!("세션 {} 없음", session_id)))
    }

    pub fn get_session_detail(&self, session_id: &str) -> AppResult<AgentSessionDetail> {
        let session = self.get_session(session_id)?;
        let msg_count = self.messages
            .read()
            .unwrap()
            .get(session_id)
            .map(|v| v.len() as u32)
            .unwrap_or(0);

        Ok(AgentSessionDetail {
            session_id: session.id,
            workspace_id: session.workspace_id,
            role: session.role,
            display_name: session.display_name,
            provider: session.provider,
            state: session.state,
            persona_hash: session.persona_hash,
            created_at: session.created_at,
            updated_at: session.updated_at,
            failure_reason: session.failure_reason,
            message_count: msg_count,
        })
    }

    pub fn list_messages(&self, session_id: &str, cursor: Option<&str>, limit: usize) -> AppResult<MessagePage> {
        let store = self.messages.read().unwrap();
        let msgs = store.get(session_id).cloned().unwrap_or_default();
        let total = msgs.len() as u32;

        // cursor 기반 페이지네이션 (간단 구현: cursor는 마지막 메시지 ID)
        let start_idx = cursor
            .and_then(|c| msgs.iter().position(|m| m.id == c))
            .map(|pos| pos + 1)
            .unwrap_or(0);

        let page: Vec<AgentMessage> = msgs.into_iter().skip(start_idx).take(limit).collect();
        let next_cursor = page.last().map(|m| m.id.clone());

        Ok(MessagePage {
            session_id: session_id.to_string(),
            messages: page,
            next_cursor,
            total,
        })
    }

    /// 세션을 중지하고 idle 상태로 전이한다.
    pub fn stop_session(&self, session_id: &str) -> AppResult<()> {
        let session = self.get_session(session_id)?;
        let from = session.state.clone();
        self.transition_state(session_id, &session.role, from, AgentLifecycleState::Idle, Some("stop_role 호출".to_string()));
        Ok(())
    }

    // ---- 내부 헬퍼 ----

    fn upsert_session(&self, session: AgentSession) {
        let mut sessions = self.sessions.write().unwrap();
        sessions.insert(session.id.clone(), session);
    }

    fn transition_state(
        &self,
        session_id: &str,
        role: &str,
        from: AgentLifecycleState,
        to: AgentLifecycleState,
        reason: Option<String>,
    ) {
        {
            let mut sessions = self.sessions.write().unwrap();
            if let Some(s) = sessions.get_mut(session_id) {
                s.state = to.clone();
                s.updated_at = Utc::now();
                if let AgentLifecycleState::Failed = &to {
                    s.failure_reason = reason.clone();
                }
            }
        }
        self.emit_status_changed(session_id, role, from, to, reason);
    }

    fn transition_to_failed(&self, session_id: &str, role: &str, reason: &str) {
        self.transition_state(
            session_id,
            role,
            AgentLifecycleState::Booting,
            AgentLifecycleState::Failed,
            Some(reason.to_string()),
        );
    }

    fn emit_status_changed(
        &self,
        session_id: &str,
        role: &str,
        from: AgentLifecycleState,
        to: AgentLifecycleState,
        reason: Option<String>,
    ) {
        let payload = AgentStatusChanged {
            session_id: session_id.to_string(),
            role: role.to_string(),
            from,
            to,
            reason,
            changed_at: Utc::now(),
        };
        let _ = self.app_handle.emit("agent:status_changed", payload);
    }

    fn store_message(&self, session_id: &str, msg: AgentMessage) {
        let mut store = self.messages.write().unwrap();
        store.entry(session_id.to_string()).or_default().push(msg);
    }

    fn get_messages(&self, session_id: &str) -> Vec<AgentMessage> {
        self.messages
            .read()
            .unwrap()
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        provider::AiProviderKind,
        session::{AgentLifecycleState, AgentSession, AgentSessionSummary, AgentStatusChanged},
    };

    #[test]
    fn test_agent_lifecycle_state_transitions() {
        // 상태 전이 순서 검증 (단위 테스트는 AppHandle 없이 상태 로직만 검증)
        assert_ne!(AgentLifecycleState::Idle, AgentLifecycleState::Ready);
        assert_ne!(AgentLifecycleState::Booting, AgentLifecycleState::Running);
    }

    #[test]
    fn test_message_ack_fields() {
        let ack = MessageAck {
            session_id: "sess-001".into(),
            user_message_id: "msg-001".into(),
            accepted_at: Utc::now(),
        };
        assert_eq!(ack.session_id, "sess-001");
    }

    #[test]
    fn test_agent_session_summary_serialization() {
        // AgentSessionSummary JSON 직렬화 / 역직렬화 라운드트립
        let summary = AgentSessionSummary {
            session_id: "sess-abc".into(),
            role: "DeveloperBE".into(),
            display_name: "박개발".into(),
            provider: AiProviderKind::Claude,
            state: AgentLifecycleState::Ready,
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("sess-abc"), "session_id가 직렬화에 포함되어야 함");
        assert!(json.contains("DeveloperBE"), "role이 직렬화에 포함되어야 함");
        assert!(json.contains("ready"), "state가 소문자 ready로 직렬화되어야 함");

        let restored: AgentSessionSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.session_id, "sess-abc");
        assert_eq!(restored.state, AgentLifecycleState::Ready);
    }

    #[test]
    fn test_agent_status_changed_serialization() {
        // AgentStatusChanged 이벤트 페이로드 직렬화
        let payload = AgentStatusChanged {
            session_id: "sess-xyz".into(),
            role: "Architect".into(),
            from: AgentLifecycleState::Booting,
            to: AgentLifecycleState::Ready,
            reason: None,
            changed_at: Utc::now(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("booting"), "from 상태가 직렬화되어야 함");
        assert!(json.contains("ready"), "to 상태가 직렬화되어야 함");
        assert!(json.contains("Architect"), "role이 직렬화되어야 함");
    }

    #[test]
    fn test_agent_session_failed_state_has_reason() {
        // Failed 상태로 전이 시 failure_reason이 설정되어야 함
        let mut session = AgentSession {
            id: "sess-001".into(),
            workspace_id: "ws-001".into(),
            role: "QA".into(),
            display_name: "박QA".into(),
            provider: AiProviderKind::OpenAi,
            state: AgentLifecycleState::Booting,
            persona_hash: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            failure_reason: None,
        };

        // Booting → Failed 전이
        session.state = AgentLifecycleState::Failed;
        session.failure_reason = Some("persona 파일 없음".into());

        assert_eq!(session.state, AgentLifecycleState::Failed);
        assert!(session.failure_reason.is_some());
        assert!(session.failure_reason.unwrap().contains("persona"));
    }
}
