// lib.rs — AgiTeamBuilder Tauri 앱 진입점 및 모듈 등록
// DS-20 §3.2, DS-60 §3, §7

// ---- 모듈 선언 ----
pub mod agent_session;
pub mod app_state;
pub mod chat_attachment;
pub mod commands;
pub mod credential;
pub mod file_document;
pub mod health_check;
pub mod models;
pub mod persona_bundle;
pub mod provider;
pub mod redmine_client;

use app_state::AppState;
use tauri::Manager;
use commands::{
    agent::{
        boot_role, boot_team, get_agent_session, list_agent_messages, prepare_chat_attachment,
        send_agent_message, stop_role,
    },
    browser::{
        browser_open, browser_navigate, browser_close,
        browser_back, browser_forward,
    },
    credential::{check_claude_oauth, delete_credential, get_masked_credential, save_credential, validate_credential},
    document::{list_documents, read_document, write_latest_document},
    health::run_health_check,
    persona::build_persona_bundle,
    redmine::{redmine_create_issue, redmine_get_issue, redmine_list_issues, redmine_update_issue},
    web::fetch_url_content,
    workspace::{load_workspace_config, open_workspace, save_workspace_config, validate_workspace, write_project_state},
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // AppState 초기화 (앱 시작 시 setup hook에서 주입)
        .setup(|app| {
            let app_handle = app.handle().clone();
            app.manage(AppState::new(app_handle));
            // ── [#17 디버그 임시 훅] LUGH_BROWSER_AUTOTEST=1 이면 3초 후 임베디드 브라우저 자동 오픈
            #[cfg(debug_assertions)]
            if std::env::var("LUGH_BROWSER_AUTOTEST").is_ok() {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    let r = crate::commands::browser::browser_open(
                        "https://example.com".into(), Some(1300.0), Some(60.0), Some(380.0), Some(700.0), handle,
                    )
                    .await;
                    crate::commands::browser::dbg_log(&format!("autotest browser_open → {:?}", r.err()));
                });
            }
            Ok(())
        })
        // ── [#17 전환] embedded-browser 독립 창 ──────────────────
        // `embedded-browser`는 더 이상 main 창의 child가 아니라 완전 독립된 최상위 창이다
        // (commands/browser.rs 헤더 주석 참조). main 창의 Moved/Resized 추종·always_on_top·
        // 좌표 강제 재배치 로직은 전부 제거되었다 — 위치/크기는 사용자가 OS 창처럼 직접 소유한다.
        // 남은 책임은 단 하나: main 창이 닫힐 때 embedded-browser도 함께 정리하는 것뿐이다
        // (parent() 관계가 없으므로 OS가 자동으로 자식 창을 닫아주지 않는다).
        .on_window_event(|window, event| {
            use tauri::WindowEvent;
            let app = window.app_handle();
            if let WindowEvent::CloseRequested { .. } = event {
                if window.label() == "main" {
                    if let Some(embedded) = app.get_webview_window(commands::browser::EMBEDDED_LABEL)
                    {
                        let _ = embedded.close();
                    }
                }
            }
        })
        // DS-60 §3에 정의된 모든 invoke commands 등록
        .invoke_handler(tauri::generate_handler![
            // Workspace
            open_workspace,
            load_workspace_config,
            validate_workspace,
            save_workspace_config,
            write_project_state,
            // Agent
            boot_team,
            boot_role,
            stop_role,
            send_agent_message,
            prepare_chat_attachment,
            get_agent_session,
            list_agent_messages,
            // Persona
            build_persona_bundle,
            // Document
            list_documents,
            read_document,
            write_latest_document,
            // Credential
            save_credential,
            delete_credential,
            validate_credential,
            get_masked_credential,
            check_claude_oauth,
            // Health
            run_health_check,
            // Browser (독립 이동 가능 WebviewWindow, #17 전환)
            browser_open,
            browser_navigate,
            browser_close,
            browser_back,
            browser_forward,
            // Redmine (DV60-003)
            redmine_list_issues,
            redmine_get_issue,
            redmine_create_issue,
            redmine_update_issue,
            // Web (AI 에이전트 웹검색 연동 1단계)
            fetch_url_content,
        ])
        .run(tauri::generate_context!())
        .expect("AgiTeamBuilder 실행 중 오류 발생");
}
