// commands/browser.rs — 앱 내 사이드바 영역 임베디드 브라우저 (floating borderless WebviewWindow)
//
// - parent_window() 는 Tauri 2 macOS API에서 *mut c_void (NSWindow raw pointer) 를 받으므로
//   raw handle 추출 없이, position + inner_size 로 시각적 임베드 효과를 구현한다.
// - 메인 창 이동/최소화/포커스 동기화는 lib.rs 의 .on_window_event 에서
//   sync_embedded_browser_position() 을 호출하여 처리한다.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::models::error::AppError;

/// 임베디드 브라우저 창 label 상수
pub const EMBEDDED_LABEL: &str = "embedded-browser";

/// 마지막으로 요청된 임베디드 브라우저 rect (main 창 기준 논리 좌표)
#[derive(Debug, Clone, Copy)]
pub struct BrowserRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// 임베디드 브라우저 전역 상태 — lib.rs setup에서 app.manage() 로 주입
#[derive(Default)]
pub struct BrowserEmbedState {
    pub rect: Mutex<Option<BrowserRect>>,
}

/// main 창 현재 위치 + 저장된 rect 로 embedded-browser 창의 위치·크기를 재계산한다.
/// (개선 1: WindowEvent::Moved 시 lib.rs 에서 호출)
pub fn sync_embedded_browser_position(app: &AppHandle) {
    let Some(w) = app.get_webview_window(EMBEDDED_LABEL) else { return };
    let Some(main_win) = app.get_webview_window("main") else { return };

    let rect = {
        let state = app.state::<BrowserEmbedState>();
        let guard = state.rect.lock().unwrap();
        *guard
    };
    let Some(rect) = rect else { return };

    if let Ok(win_pos) = main_win.outer_position() {
        let scale = main_win.scale_factor().unwrap_or(1.0);
        let screen_x = win_pos.x as f64 / scale + rect.x;
        let screen_y = win_pos.y as f64 / scale + rect.y;
        let _ = w.set_position(tauri::LogicalPosition::<f64>::new(screen_x, screen_y));
        let _ = w.set_size(tauri::LogicalSize::<f64>::new(rect.width, rect.height));
    }
}

/// url 정규화: http(s):// 없으면 https:// 자동 추가
fn normalize_url(url: &str) -> String {
    if url.starts_with("http") {
        url.to_string()
    } else {
        format!("https://{}", url)
    }
}

/// 임베디드 브라우저 WebviewWindow를 연다.
///
/// - 기존 `embedded-browser` 창이 있으면 닫고 새로 생성
/// - x, y 는 main 창 기준 논리 좌표 (프론트엔드 레이아웃 좌표)
/// - always_on_top: main 클릭 시 브라우저가 뒤로 숨는 문제 방지 (개선 3)
/// - on_navigation: 페이지 이동 시 main 창으로 `browser:navigation` 이벤트 emit (개선 4)
#[tauri::command]
pub async fn browser_open(
    url: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    app: AppHandle,
) -> Result<(), AppError> {
    // 기존 창 닫기
    if let Some(w) = app.get_webview_window(EMBEDDED_LABEL) {
        let _ = w.close();
    }

    let normalized = normalize_url(&url);

    let main_win = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::new("NO_MAIN", "main window not found"))?;

    let parsed_url = normalized
        .parse::<tauri::Url>()
        .map_err(|e| AppError::new("INVALID_URL", e.to_string()))?;

    // rect 상태 갱신 (개선 1: Moved 이벤트에서 재사용)
    {
        let state = app.state::<BrowserEmbedState>();
        *state.rect.lock().unwrap() = Some(BrowserRect { x, y, width, height });
    }

    // main 창 외부 위치(스크린 물리 픽셀) → 논리 픽셀 변환 후 오프셋 추가
    let win_pos: tauri::PhysicalPosition<i32> = main_win
        .outer_position()
        .map_err(|e: tauri::Error| AppError::new("WIN_POS", e.to_string()))?;
    let scale = main_win.scale_factor().unwrap_or(1.0);
    let screen_x = win_pos.x as f64 / scale + x;
    let screen_y = win_pos.y as f64 / scale + y;

    // on_navigation → main 창으로 URL 동기화 이벤트 (개선 4)
    let nav_handle = app.clone();

    WebviewWindowBuilder::new(&app, EMBEDDED_LABEL, WebviewUrl::External(parsed_url))
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .position(screen_x, screen_y)
        .inner_size(width, height)
        .on_navigation(move |nav_url| {
            let _ = nav_handle.emit_to("main", "browser:navigation", nav_url.to_string());
            true // 모든 네비게이션 허용
        })
        .build()
        .map_err(|e: tauri::Error| AppError::new("BROWSER_OPEN", e.to_string()))?;

    Ok(())
}

/// 이미 열린 `embedded-browser` 창에서 새 URL로 이동한다.
#[tauri::command]
pub async fn browser_navigate(url: String, app: AppHandle) -> Result<(), AppError> {
    let normalized = normalize_url(&url);

    if let Some(w) = app.get_webview_window(EMBEDDED_LABEL) {
        let parsed = normalized
            .parse::<tauri::Url>()
            .map_err(|e| AppError::new("INVALID_URL", e.to_string()))?;
        w.navigate(parsed)
            .map_err(|e: tauri::Error| AppError::new("BROWSER_NAV", e.to_string()))?;
    }

    Ok(())
}

/// 브라우저 히스토리 뒤로 가기 (개선 4)
#[tauri::command]
pub fn browser_back(app: AppHandle) -> Result<(), AppError> {
    if let Some(w) = app.get_webview_window(EMBEDDED_LABEL) {
        w.eval("history.back()")
            .map_err(|e: tauri::Error| AppError::new("BROWSER_BACK", e.to_string()))?;
    }
    Ok(())
}

/// 브라우저 히스토리 앞으로 가기 (개선 4)
#[tauri::command]
pub fn browser_forward(app: AppHandle) -> Result<(), AppError> {
    if let Some(w) = app.get_webview_window(EMBEDDED_LABEL) {
        w.eval("history.forward()")
            .map_err(|e: tauri::Error| AppError::new("BROWSER_FORWARD", e.to_string()))?;
    }
    Ok(())
}

/// `embedded-browser` 창을 닫는다.
#[tauri::command]
pub fn browser_close(app: AppHandle) -> Result<(), AppError> {
    // rect 상태 초기화
    {
        let state = app.state::<BrowserEmbedState>();
        *state.rect.lock().unwrap() = None;
    }
    if let Some(w) = app.get_webview_window(EMBEDDED_LABEL) {
        w.close()
            .map_err(|e: tauri::Error| AppError::new("BROWSER_CLOSE", e.to_string()))?;
    }
    Ok(())
}

/// `embedded-browser` 창의 위치와 크기를 조정한다.
///
/// x, y 는 main 창 기준 논리 좌표. rect 상태를 갱신한 뒤 재배치한다. (개선 1)
#[tauri::command]
pub fn browser_resize(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    app: AppHandle,
) -> Result<(), AppError> {
    {
        let state = app.state::<BrowserEmbedState>();
        *state.rect.lock().unwrap() = Some(BrowserRect { x, y, width, height });
    }
    sync_embedded_browser_position(&app);
    Ok(())
}
