// commands/web.rs — AI 에이전트 웹검색 연동 1단계: URL 본문 텍스트 추출
//
// fetch_url_content: GET → <script>/<style>/주석 제거 → 태그 스트립 → 공백 정리 → 최대 50KB
// 외부 크레이트 추가 없이 reqwest(기존 의존성) + 수동 파서로 구현.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::models::error::AppError;

/// 본문 텍스트 최대 크기 (50KB)
const MAX_TEXT_BYTES: usize = 50 * 1024;
/// 요청 타임아웃 (초)
const FETCH_TIMEOUT_SECS: u64 = 15;
/// 일부 사이트의 기본 UA 차단 회피용 User-Agent
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36 AgiTeamBuilder/0.1";

/// 추출된 페이지 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedPage {
    /// 실제 요청한 URL (정규화 후)
    pub url: String,
    /// <title> 태그 내용 (없으면 None)
    pub title: Option<String>,
    /// 태그 제거 + 공백 정리된 본문 텍스트 (최대 50KB)
    pub text: String,
    /// 조회 시각 (RFC3339)
    pub fetched_at: String,
}

/// URL 본문을 가져와 텍스트로 추출한다.
///
/// - http(s):// 없으면 https:// 자동 추가
/// - 에러 코드: INVALID_URL / FETCH_TIMEOUT / FETCH_FAILED
#[tauri::command]
pub async fn fetch_url_content(url: String) -> Result<FetchedPage, AppError> {
    let normalized = if url.starts_with("http://") || url.starts_with("https://") {
        url.clone()
    } else {
        format!("https://{}", url)
    };

    let parsed = reqwest::Url::parse(&normalized)
        .map_err(|e| AppError::new("INVALID_URL", format!("URL 파싱 실패: {}", e)))?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| AppError::new("FETCH_FAILED", format!("HTTP 클라이언트 생성 실패: {}", e)))?;

    let resp = client.get(parsed).send().await.map_err(|e| {
        if e.is_timeout() {
            AppError::new("FETCH_TIMEOUT", format!("요청 타임아웃({}초): {}", FETCH_TIMEOUT_SECS, normalized))
                .recoverable()
        } else {
            AppError::new("FETCH_FAILED", format!("요청 실패: {}", e)).recoverable()
        }
    })?;

    let status = resp.status();
    if !status.is_success() {
        return Err(AppError::new(
            "FETCH_FAILED",
            format!("HTTP {} 응답: {}", status.as_u16(), normalized),
        ));
    }

    let html = resp
        .text()
        .await
        .map_err(|e| AppError::new("FETCH_FAILED", format!("본문 읽기 실패: {}", e)))?;

    let title = extract_title(&html);
    let text = truncate_at_char_boundary(&html_to_text(&html), MAX_TEXT_BYTES);

    Ok(FetchedPage {
        url: normalized,
        title,
        text,
        fetched_at: chrono::Utc::now().to_rfc3339(),
    })
}

// ── HTML 파싱 헬퍼 ─────────────────────────────────────────────────────────────

/// <title> 태그 내용 추출 (대소문자 무시)
fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start_tag = lower.find("<title")?;
    let content_start = html[start_tag..].find('>')? + start_tag + 1;
    let content_end = lower[content_start..].find("</title")? + content_start;
    let title = decode_entities(html[content_start..content_end].trim());
    if title.is_empty() { None } else { Some(title) }
}

/// HTML → 본문 텍스트: 블록 제거 → 태그 스트립 → 엔티티 디코드 → 공백 정리
fn html_to_text(html: &str) -> String {
    let mut cleaned = remove_block(html, "script");
    cleaned = remove_block(&cleaned, "style");
    cleaned = remove_block(&cleaned, "noscript");
    cleaned = remove_comments(&cleaned);
    let stripped = strip_tags(&cleaned);
    let decoded = decode_entities(&stripped);
    // 공백 정리: 연속 공백·개행 → 단일 공백
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `<tag ...> ... </tag>` 블록 전체 제거 (대소문자 무시)
fn remove_block(html: &str, tag: &str) -> String {
    let lower = html.to_lowercase();
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let mut out = String::with_capacity(html.len());
    let mut pos = 0;
    while let Some(rel) = lower[pos..].find(&open) {
        let abs = pos + rel;
        out.push_str(&html[pos..abs]);
        match lower[abs..].find(&close) {
            Some(end_rel) => pos = abs + end_rel + close.len(),
            None => {
                // 닫는 태그 없음 → 나머지 전부 버림
                return out;
            }
        }
    }
    out.push_str(&html[pos..]);
    out
}

/// HTML 주석 `<!-- ... -->` 제거
fn remove_comments(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut pos = 0;
    while let Some(rel) = html[pos..].find("<!--") {
        let abs = pos + rel;
        out.push_str(&html[pos..abs]);
        match html[abs..].find("-->") {
            Some(end_rel) => pos = abs + end_rel + 3,
            None => return out,
        }
    }
    out.push_str(&html[pos..]);
    out
}

/// 태그를 공백으로 치환하며 스트립 (단어 경계 유지)
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                out.push(' '); // 태그 경계에 공백 삽입 → 단어 붙음 방지
            }
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// 자주 쓰는 HTML 엔티티 디코드
fn decode_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

/// UTF-8 문자 경계를 지키며 최대 max_bytes로 절단
fn truncate_at_char_boundary(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

// ── 단위 테스트 ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_to_text_strips_script_style_and_tags() {
        let html = r#"<html><head>
            <title>테스트 페이지</title>
            <style>body { color: red; }</style>
            <script>alert("evil");</script>
        </head><body>
            <!-- 주석은 제거 -->
            <h1>제목</h1>
            <p>본문 &amp; 내용입니다.</p>
            <SCRIPT>console.log("대문자도 제거")</SCRIPT>
        </body></html>"#;

        let text = html_to_text(html);
        assert!(text.contains("제목"));
        assert!(text.contains("본문 & 내용입니다."));
        assert!(!text.contains("alert"));
        assert!(!text.contains("color: red"));
        assert!(!text.contains("console.log"));
        assert!(!text.contains("주석은 제거"));
        // 연속 공백 없음
        assert!(!text.contains("  "));
    }

    #[test]
    fn test_extract_title() {
        assert_eq!(
            extract_title("<html><head><TITLE> Hello &amp; World </TITLE></head></html>"),
            Some("Hello & World".to_string())
        );
        assert_eq!(extract_title("<html><body>no title</body></html>"), None);
    }

    #[test]
    fn test_truncate_at_char_boundary_keeps_utf8_valid() {
        // '한' = 3바이트. 4바이트 제한이면 1글자(3바이트)만 남아야 함
        let s = "한국어텍스트";
        let cut = truncate_at_char_boundary(s, 4);
        assert_eq!(cut, "한");
        // 제한보다 짧으면 그대로
        assert_eq!(truncate_at_char_boundary("abc", 10), "abc");
    }
}
