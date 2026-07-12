// commands/browser.rs — 독립 이동 가능 `embedded-browser` WebviewWindow (Redmine #17 전환)
//
// [#17 전환] 이전에는 WebviewWindowBuilder::parent(&main) 로 main 창에 종속된 child 창을
//   만들고 main 창의 Moved/Resized 이벤트를 좇아 좌표를 강제로 재계산했다(핀 고정 방식).
//   Redmine #17 재정의(DS-60 v0.10 §3.8, DS-40 v0.8 §9)에 따라 `embedded-browser`는
//   **완전히 독립된 최상위 창**으로 전환한다:
//     - parent() 미사용 → OS 레벨에서 main 창과 무관하게 독립적으로 존재
//     - decorations(true) → 타이틀바 제공, OS 네이티브 드래그로 사용자가 자유롭게 이동
//     - resizable(true) → 사용자가 자유롭게 리사이즈
//     - 위치/크기는 `browser_open` 호출 시 선택적 초기값만 지정하고, 이후로는
//       OS와 사용자가 소유한다(백엔드가 강제 재배치하지 않는다)
//   따라서 main 창 Moved/Resized 추종, always_on_top, 좌표 동기화 로직은 전부 제거되었다.
//   앱(main 창) 종료 시 `embedded-browser`도 함께 종료되도록 lib.rs 의 `CloseRequested`
//   핸들러에서 처리한다.

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::models::error::AppError;

/// 임베디드 브라우저 창 label 상수
pub const EMBEDDED_LABEL: &str = "embedded-browser";

/// 초기 배치 생략 시 기본 크기 (DS-40 §9.3 예시값)
const DEFAULT_WIDTH: f64 = 960.0;
const DEFAULT_HEIGHT: f64 = 720.0;

/// [#17 디버그] 파일 + stderr 동시 로그
pub fn dbg_log(msg: &str) {
    use std::io::Write;
    eprintln!("[browser-dbg] {}", msg);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/lugh-browser-debug.log")
    {
        let _ = writeln!(f, "[{}] {}", chrono::Utc::now().to_rfc3339(), msg);
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

/// width/height 생략 시 기본 크기 적용 (순수 함수 — 단위 테스트 가능하도록 분리)
fn resolve_size(width: Option<f64>, height: Option<f64>) -> (f64, f64) {
    (width.unwrap_or(DEFAULT_WIDTH), height.unwrap_or(DEFAULT_HEIGHT))
}

/// 임베디드 브라우저 독립 WebviewWindow를 연다.
///
/// - 기존 `embedded-browser` 창이 있으면 닫고 새로 생성
/// - [#17 전환] parent() 미사용 — 완전 독립 최상위 창
/// - decorations(true)·resizable(true) — OS 네이티브 타이틀바 드래그로 자유 이동/리사이즈
/// - x, y, width, height 는 모두 **선택적 초기값**. 생략 시 위치는 OS 기본값, 크기는
///   960×720 기본값을 사용한다. 생성 이후 위치/크기는 OS·사용자가 소유하며 백엔드는
///   재배치하지 않는다
/// - on_navigation: 페이지 이동 시 main 창으로 `browser:navigation` 이벤트 emit
#[tauri::command]
pub async fn browser_open(
    url: String,
    x: Option<f64>,
    y: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
    app: AppHandle,
) -> Result<(), AppError> {
    // 기존 창 닫기
    if let Some(w) = app.get_webview_window(EMBEDDED_LABEL) {
        let _ = w.close();
    }

    let normalized = normalize_url(&url);
    let parsed_url = normalized
        .parse::<tauri::Url>()
        .map_err(|e| AppError::new("INVALID_URL", e.to_string()))?;

    let (width, height) = resolve_size(width, height);

    dbg_log(&format!(
        "open: url={} x={:?} y={:?} width={} height={}",
        normalized, x, y, width, height
    ));

    // on_navigation → main 창으로 URL 동기화 이벤트
    let nav_handle = app.clone();

    let mut builder = WebviewWindowBuilder::new(&app, EMBEDDED_LABEL, WebviewUrl::External(parsed_url))
        .decorations(true) // [#17 전환] 독립 창: 타이틀바 제공 → OS 네이티브 드래그 이동
        .resizable(true) // [#17 전환] 사용자가 자유롭게 리사이즈
        .inner_size(width, height)
        .on_navigation(move |nav_url| {
            let _ = nav_handle.emit_to("main", "browser:navigation", nav_url.to_string());
            true // 모든 네비게이션 허용
        });

    // 초기 위치는 x, y 둘 다 지정된 경우에만 적용 — 하나만 지정된 경우는 무시하고 OS 기본값 사용
    if let (Some(x), Some(y)) = (x, y) {
        builder = builder.position(x, y);
    }

    builder
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

/// 브라우저 히스토리 뒤로 가기
#[tauri::command]
pub fn browser_back(app: AppHandle) -> Result<(), AppError> {
    if let Some(w) = app.get_webview_window(EMBEDDED_LABEL) {
        w.eval("history.back()")
            .map_err(|e: tauri::Error| AppError::new("BROWSER_BACK", e.to_string()))?;
    }
    Ok(())
}

/// 브라우저 히스토리 앞으로 가기
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
    if let Some(w) = app.get_webview_window(EMBEDDED_LABEL) {
        w.close()
            .map_err(|e: tauri::Error| AppError::new("BROWSER_CLOSE", e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // [#17 전환] resolve_size: width/height 생략 시 기본값(960×720) 적용 검증

    #[test]
    fn resolve_size_defaults_when_both_missing() {
        assert_eq!(resolve_size(None, None), (DEFAULT_WIDTH, DEFAULT_HEIGHT));
    }

    #[test]
    fn resolve_size_uses_given_width_and_height() {
        assert_eq!(resolve_size(Some(500.0), Some(400.0)), (500.0, 400.0));
    }

    #[test]
    fn resolve_size_defaults_width_only_when_height_given() {
        assert_eq!(resolve_size(None, Some(400.0)), (DEFAULT_WIDTH, 400.0));
    }

    #[test]
    fn resolve_size_defaults_height_only_when_width_given() {
        assert_eq!(resolve_size(Some(500.0), None), (500.0, DEFAULT_HEIGHT));
    }

    #[test]
    fn normalize_url_adds_https_scheme_when_missing() {
        assert_eq!(normalize_url("example.com"), "https://example.com");
    }

    #[test]
    fn normalize_url_keeps_existing_http_scheme() {
        assert_eq!(normalize_url("http://example.com"), "http://example.com");
    }

    #[test]
    fn normalize_url_keeps_existing_https_scheme() {
        assert_eq!(normalize_url("https://example.com"), "https://example.com");
    }
}
