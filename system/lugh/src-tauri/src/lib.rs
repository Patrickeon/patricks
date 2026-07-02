// lib.rs — AgiTeamBuilder Tauri 앱 진입점 및 모듈 등록
// DS-20 §3.2, DS-60 §3, §7

// ---- 모듈 선언 ----
pub mod agent_session;
pub mod app_state;
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
        boot_role, boot_team, get_agent_session, list_agent_messages, send_agent_message,
        stop_role,
    },
    browser::{
        browser_open, browser_navigate, browser_close, browser_resize,
        browser_back, browser_forward,
    },
    credential::{check_claude_oauth, delete_credential, get_masked_credential, save_credential, validate_credential},
    document::{list_documents, read_document, write_latest_document},
    health::run_health_check,
    persona::build_persona_bundle,
    redmine::{redmine_create_issue, redmine_get_issue, redmine_list_issues, redmine_update_issue},
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
            // 임베디드 브라우저 rect 상태 (browser_open/resize 갱신, Moved 이벤트에서 참조)
            app.manage(commands::browser::BrowserEmbedState::default());
            Ok(())
        })
        // ── 임베디드 브라우저 ↔ 메인 창 동기화 ──────────────────
        // 개선 1: main 이동 시 embedded-browser 따라가기
        // 개선 2: main 최소화/복원 시 embedded hide/show
        // 개선 3: main 블러 시 embedded hide (단, embedded 자체 클릭으로 인한 블러는 예외)
        .on_window_event(|window, event| {
            use tauri::WindowEvent;
            let app = window.app_handle();
            match event {
                // 개선 1: 메인 창 이동 → embedded 위치 재계산
                WindowEvent::Moved(_) => {
                    if window.label() == "main" {
                        commands::browser::sync_embedded_browser_position(app);
                    }
                }
                // 개선 2: 최소화/복원 감지 (macOS 최소화는 Resized로 관측)
                WindowEvent::Resized(_) => {
                    if window.label() == "main" {
                        if let Some(embedded) =
                            app.get_webview_window(commands::browser::EMBEDDED_LABEL)
                        {
                            if window.is_minimized().unwrap_or(false) {
                                let _ = embedded.hide();
                            } else {
                                let _ = embedded.show();
                                commands::browser::sync_embedded_browser_position(app);
                            }
                        }
                    }
                }
                // 개선 3: z-order — 앱 백그라운드 시 다른 앱 위 덮기 방지
                WindowEvent::Focused(focused) => {
                    if window.label() == "main" {
                        if let Some(embedded) =
                            app.get_webview_window(commands::browser::EMBEDDED_LABEL)
                        {
                            if *focused {
                                let _ = embedded.show();
                                commands::browser::sync_embedded_browser_position(app);
                            } else if !embedded.is_focused().unwrap_or(false) {
                                // embedded 클릭으로 main이 블러된 경우는 hide하지 않음
                                let _ = embedded.hide();
                            }
                        }
                    } else if window.label() == commands::browser::EMBEDDED_LABEL && !*focused {
                        // embedded 블러 + main도 비포커스 → 앱 전체가 백그라운드
                        if let Some(main) = app.get_webview_window("main") {
                            if !main.is_focused().unwrap_or(false) {
                                let _ = window.hide();
                            }
                        }
                    }
                }
                _ => {}
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
            // Browser (임베디드 WebviewWindow)
            browser_open,
            browser_navigate,
            browser_close,
            browser_resize,
            browser_back,
            browser_forward,
            // Redmine (DV60-003)
            redmine_list_issues,
            redmine_get_issue,
            redmine_create_issue,
            redmine_update_issue,
        ])
        .run(tauri::generate_context!())
        .expect("AgiTeamBuilder 실행 중 오류 발생");
}
