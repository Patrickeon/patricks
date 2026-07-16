// commands/web.rs — AI 에이전트 웹검색 연동 1단계: URL 본문 텍스트 추출
//
// fetch_url_content: GET → <script>/<style>/주석 제거 → 태그 스트립 → 공백 정리 → 최대 50KB
// 외부 크레이트 추가 없이 reqwest(기존 의존성) + 수동 파서로 구현.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::models::attachment::{
    AttachmentKind, AttachmentSource, AttachmentStatus, PreparedChatAttachment,
};
use crate::models::error::AppError;

/// 본문 텍스트 최대 크기 (50KB)
const MAX_TEXT_BYTES: usize = 50 * 1024;
/// 요청 타임아웃 (초)
const FETCH_TIMEOUT_SECS: u64 = 15;
/// 일부 사이트의 기본 UA 차단 회피용 User-Agent
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36 AgiTeamBuilder/0.1";
/// 리다이렉트 최대 추종 hop 수 (SSRF 우회 방지, DS-40 §10.7)
const MAX_REDIRECTS: u8 = 3;
/// 메시지당 최대 fetch URL 수 (MVP, DS-40 §10.7)
pub const MAX_URLS_PER_MESSAGE: usize = 3;
/// 내부망 IP 규칙으로 커버되지 않는 공인 사내 호스트 전면 차단 목록 (MVP 허용목록 예외 없음)
/// 사내 레드마인 등. 호스트명·해석된 IP 문자열 양쪽에 대해 검사한다.
const DENIED_HOSTS: [&str; 2] = ["211.117.60.5", "localhost"];

/// 추출된 페이지 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedPage {
    /// 실제 요청한 URL (정규화 + 리다이렉트 최종 도달 URL)
    pub url: String,
    /// <title> 태그 내용 (없으면 None)
    pub title: Option<String>,
    /// 태그 제거 + 공백 정리된 본문 텍스트 (최대 50KB)
    pub text: String,
    /// 조회 시각 (RFC3339)
    pub fetched_at: String,
    /// 본문이 50KB 한도로 절단되었는지 여부 (#29)
    #[serde(default)]
    pub truncated: bool,
}

/// URL 본문을 가져와 텍스트로 추출한다. (#29 SSRF/내부망 전면 차단 가드 포함)
///
/// - http(s):// 없으면 https:// 자동 추가, scheme은 http/https만 허용
/// - 요청 전(그리고 매 리다이렉트 hop마다) 호스트를 해석된 최종 IP까지 검사해
///   내부망/loopback/link-local/사설/사내 호스트를 전면 차단 (URL_BLOCKED)
/// - 리다이렉트는 최대 3회, 매 hop 재검증 (공개→내부 우회 차단)
/// - 응답 Content-Type이 text/html·text/plain일 때만 본문 추출·주입
/// - 에러 코드: URL_BLOCKED / INVALID_URL / FETCH_TIMEOUT / FETCH_FAILED
#[tauri::command]
pub async fn fetch_url_content(url: String) -> Result<FetchedPage, AppError> {
    let normalized = if url.starts_with("http://") || url.starts_with("https://") {
        url.clone()
    } else {
        format!("https://{}", url)
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
        .user_agent(USER_AGENT)
        // 자동 추종 비활성화 → 매 hop을 직접 SSRF 재검증한다
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AppError::new("FETCH_FAILED", format!("HTTP 클라이언트 생성 실패: {}", e)))?;

    let mut current = reqwest::Url::parse(&normalized)
        .map_err(|e| AppError::new("INVALID_URL", format!("URL 파싱 실패: {}", e)))?;

    let mut hops = 0u8;
    loop {
        // ── SSRF 가드: scheme + 호스트/해석 IP 검사 (매 hop) ──
        validate_scheme(&current)?;
        guard_url(&current).await?;

        let resp = client.get(current.clone()).send().await.map_err(|e| {
            if e.is_timeout() {
                AppError::new(
                    "FETCH_TIMEOUT",
                    format!("요청 타임아웃({}초): {}", FETCH_TIMEOUT_SECS, current),
                )
                .recoverable()
            } else {
                AppError::new("FETCH_FAILED", format!("요청 실패: {}", e)).recoverable()
            }
        })?;

        let status = resp.status();

        // ── 리다이렉트 수동 추종 (재검증) ──
        if status.is_redirection() {
            if hops >= MAX_REDIRECTS {
                return Err(AppError::new(
                    "FETCH_FAILED",
                    format!("리다이렉트 한도({}) 초과: {}", MAX_REDIRECTS, current),
                )
                .recoverable());
            }
            let location = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| {
                    AppError::new("FETCH_FAILED", format!("리다이렉트 응답에 Location 없음: {}", current))
                })?;
            // 상대 경로 Location도 현재 URL 기준으로 절대화
            current = current.join(location).map_err(|e| {
                AppError::new("INVALID_URL", format!("리다이렉트 URL 파싱 실패: {}", e))
            })?;
            hops += 1;
            continue;
        }

        if !status.is_success() {
            return Err(AppError::new(
                "FETCH_FAILED",
                format!("HTTP {} 응답: {}", status.as_u16(), current),
            ));
        }

        let final_url = current.to_string();

        // ── Content-Type 제한: text/html·text/plain만 본문 추출 ──
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        if !content_type_allowed(&content_type) {
            let ct = if content_type.is_empty() { "unknown" } else { content_type.as_str() };
            return Ok(FetchedPage {
                url: final_url,
                title: None,
                text: format!("[본문 생략: 지원하지 않는 콘텐츠 유형({}) — text/html·text/plain만 주입]", ct),
                fetched_at: chrono::Utc::now().to_rfc3339(),
                truncated: false,
            });
        }

        let html = resp
            .text()
            .await
            .map_err(|e| AppError::new("FETCH_FAILED", format!("본문 읽기 실패: {}", e)))?;

        let title = extract_title(&html);
        let full = html_to_text(&html);
        let truncated = full.len() > MAX_TEXT_BYTES;
        let text = truncate_at_char_boundary(&full, MAX_TEXT_BYTES);

        return Ok(FetchedPage {
            url: final_url,
            title,
            text,
            fetched_at: chrono::Utc::now().to_rfc3339(),
            truncated,
        });
    }
}

// ── SSRF / 내부망 차단 가드 (DS-40 §10.7) ──────────────────────────────────────

/// scheme을 http/https로 제한한다 (file:/ftp:/data: 등 차단)
fn validate_scheme(url: &reqwest::Url) -> Result<(), AppError> {
    match url.scheme() {
        "http" | "https" => Ok(()),
        other => Err(AppError::url_blocked(format!(
            "허용되지 않은 scheme: {} (http/https만 허용)",
            other
        ))),
    }
}

/// 호스트명 → 해석된 최종 IP까지 검사해 내부망/사내 호스트를 전면 차단한다.
/// (DNS 재바인딩 방어: 호스트명이 아니라 실제 해석 IP를 본다)
async fn guard_url(url: &reqwest::Url) -> Result<(), AppError> {
    let host = url
        .host_str()
        .ok_or_else(|| AppError::url_blocked(format!("호스트 없는 URL: {}", url)))?;

    if is_denied_host(host) {
        return Err(AppError::url_blocked(format!("차단된 호스트: {}", host)));
    }

    let port = url.port_or_known_default().unwrap_or(80);
    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| AppError::url_blocked(format!("호스트 확인 실패({}): {}", host, e)))?;

    let mut resolved = false;
    for addr in addrs {
        resolved = true;
        let ip = addr.ip();
        if is_denied_host(&ip.to_string()) {
            return Err(AppError::url_blocked(format!("차단된 IP: {}", ip)));
        }
        if is_blocked_ip(ip) {
            return Err(AppError::url_blocked(format!(
                "내부망/예약 IP 차단: {} ({})",
                ip, host
            )));
        }
    }
    if !resolved {
        return Err(AppError::url_blocked(format!("호스트 IP를 확인할 수 없습니다: {}", host)));
    }
    Ok(())
}

/// 명시적 차단 호스트 목록 검사 (호스트명 또는 IP 문자열)
fn is_denied_host(host: &str) -> bool {
    let h = host.trim_end_matches('.').to_ascii_lowercase();
    DENIED_HOSTS.iter().any(|d| h == *d)
}

/// 내부망/예약 IP 대역 여부 (loopback·사설·link-local·ULA 등 전면 차단)
fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_ipv4(v4),
        IpAddr::V6(v6) => {
            // IPv4-mapped(::ffff:a.b.c.d)는 내부 IPv4 규칙으로 검사
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_blocked_ipv4(mapped);
            }
            is_blocked_ipv6(v6)
        }
    }
}

fn is_blocked_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()            // 127.0.0.0/8
        || ip.is_private()      // 10/8, 172.16/12, 192.168/16
        || ip.is_link_local()   // 169.254.0.0/16 (메타데이터 169.254.169.254 포함)
        || ip.is_unspecified()  // 0.0.0.0
        || ip.is_broadcast()    // 255.255.255.255
        || ip.is_documentation()
        || ip.octets()[0] == 0  // 0.0.0.0/8
        || is_shared_ipv4(ip)   // 100.64.0.0/10 (CGNAT)
}

/// 100.64.0.0/10 (RFC6598 공유 주소 대역)
fn is_shared_ipv4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && (o[1] & 0xc0) == 0x40
}

fn is_blocked_ipv6(ip: Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
        return true;
    }
    let seg0 = ip.segments()[0];
    // ULA fc00::/7
    if (seg0 & 0xfe00) == 0xfc00 {
        return true;
    }
    // link-local fe80::/10
    if (seg0 & 0xffc0) == 0xfe80 {
        return true;
    }
    false
}

/// 응답 Content-Type이 본문 주입 대상(text/html·text/plain)인지 (DS-40 §10.7)
/// Content-Type 미표기(빈 값)는 허용(태그 스트립 후 텍스트만 남으므로 무해).
fn content_type_allowed(content_type: &str) -> bool {
    let ct = content_type.to_ascii_lowercase();
    let main = ct.split(';').next().unwrap_or("").trim();
    main.is_empty() || main == "text/html" || main == "text/plain"
}

// ── 채팅 주입 변환 (FetchedPage → Document 첨부, DS-40 §10.6 / DS-60 §4.4) ──────

/// 메시지당 fetch URL 개수 상한 검증 (MVP 3개, DS-40 §10.7)
pub fn validate_url_count(count: usize) -> Result<(), AppError> {
    if count > MAX_URLS_PER_MESSAGE {
        return Err(AppError::url_blocked(format!(
            "메시지당 웹 URL 수 한도 {}개 초과({}개)",
            MAX_URLS_PER_MESSAGE, count
        )));
    }
    Ok(())
}

/// 웹 자료 본문 머리표기 접두 (DS-40 §10.6)
/// 예: "[웹 자료: 제목 — https://... (조회 2026-...)]"
pub fn web_document_header(page: &FetchedPage) -> String {
    let title = page.title.as_deref().unwrap_or(page.url.as_str());
    format!("[웹 자료: {} — {} (조회 {})]", title, page.url, page.fetched_at)
}

/// fetch 결과를 채팅 첨부 파이프라인용 Document 첨부로 변환한다.
/// `AttachmentKind::Web` 신설 없이 기존 `Document`를 재사용해
/// validate_attachments_for_send(예산 검증)·build_content_blocks(→DocumentText)에
/// 그대로 태운다 → Claude/OpenAI/Gemini/claude_cli 전 어댑터 무변경 처리. (DS-40 §10.6)
pub fn fetched_page_to_attachment(page: &FetchedPage) -> PreparedChatAttachment {
    let title = page.title.clone().unwrap_or_else(|| page.url.clone());
    let header = web_document_header(page);
    let body = format!("{}\n{}", header, page.text);
    let sha256 = hex::encode(Sha256::digest(body.as_bytes()));
    let id = format!("web-{}", &sha256[..16]);
    PreparedChatAttachment {
        id,
        kind: AttachmentKind::Document,
        // 전용 Web source 미신설 — 중립적으로 FilePicker 재사용 (MVP)
        source: AttachmentSource::FilePicker,
        filename: title,
        media_type: "text/html".into(),
        size_bytes: body.len() as u64,
        sha256,
        status: AttachmentStatus::Ready,
        content_base64: None,
        extracted_text: Some(body),
        truncated: page.truncated,
        error: None,
    }
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

    // ── SSRF 가드 (#29) ──────────────────────────────────────────────────────

    fn v4(s: &str) -> IpAddr {
        IpAddr::V4(s.parse::<Ipv4Addr>().unwrap())
    }
    fn v6(s: &str) -> IpAddr {
        IpAddr::V6(s.parse::<Ipv6Addr>().unwrap())
    }

    #[test]
    fn test_is_blocked_ipv4_internal_ranges() {
        assert!(is_blocked_ip(v4("127.0.0.1")), "loopback");
        assert!(is_blocked_ip(v4("10.0.0.1")), "사설 10/8");
        assert!(is_blocked_ip(v4("172.16.0.1")), "사설 172.16/12");
        assert!(is_blocked_ip(v4("192.168.1.1")), "사설 192.168/16");
        assert!(is_blocked_ip(v4("169.254.169.254")), "link-local 메타데이터");
        assert!(is_blocked_ip(v4("0.0.0.0")), "unspecified");
        assert!(is_blocked_ip(v4("100.64.0.1")), "CGNAT 공유대역");
    }

    #[test]
    fn test_is_blocked_ipv4_public_allowed() {
        assert!(!is_blocked_ip(v4("8.8.8.8")), "공개 DNS");
        assert!(!is_blocked_ip(v4("93.184.216.34")), "공개 예제 IP");
        assert!(!is_blocked_ip(v4("1.1.1.1")));
    }

    #[test]
    fn test_is_blocked_ipv6_internal_and_public() {
        assert!(is_blocked_ip(v6("::1")), "loopback");
        assert!(is_blocked_ip(v6("fc00::1")), "ULA");
        assert!(is_blocked_ip(v6("fd12:3456::1")), "ULA fd");
        assert!(is_blocked_ip(v6("fe80::1")), "link-local");
        assert!(is_blocked_ip(v6("::")), "unspecified");
        // IPv4-mapped 내부주소도 차단
        assert!(is_blocked_ip(v6("::ffff:127.0.0.1")), "IPv4-mapped loopback");
        // 공개 IPv6는 허용
        assert!(!is_blocked_ip(v6("2001:4860:4860::8888")), "공개 IPv6");
    }

    #[test]
    fn test_is_denied_host_corporate() {
        assert!(is_denied_host("211.117.60.5"), "사내 레드마인 IP");
        assert!(is_denied_host("localhost"));
        assert!(is_denied_host("LOCALHOST"), "대소문자 무시");
        assert!(is_denied_host("localhost."), "trailing dot 무시");
        assert!(!is_denied_host("example.com"));
        assert!(!is_denied_host("8.8.8.8"));
    }

    #[test]
    fn test_validate_scheme() {
        let ok = reqwest::Url::parse("https://example.com").unwrap();
        assert!(validate_scheme(&ok).is_ok());
        let http = reqwest::Url::parse("http://example.com").unwrap();
        assert!(validate_scheme(&http).is_ok());
        let ftp = reqwest::Url::parse("ftp://example.com").unwrap();
        assert_eq!(validate_scheme(&ftp).unwrap_err().code, "URL_BLOCKED");
        let file = reqwest::Url::parse("file:///etc/passwd").unwrap();
        assert_eq!(validate_scheme(&file).unwrap_err().code, "URL_BLOCKED");
    }

    #[test]
    fn test_content_type_allowed() {
        assert!(content_type_allowed("text/html"));
        assert!(content_type_allowed("text/html; charset=utf-8"));
        assert!(content_type_allowed("TEXT/PLAIN"));
        assert!(content_type_allowed(""), "미표기는 허용");
        assert!(!content_type_allowed("application/pdf"));
        assert!(!content_type_allowed("application/octet-stream"));
        assert!(!content_type_allowed("image/png"));
    }

    // guard_url은 IP 리터럴 URL로 검증 (DNS 네트워크 불필요, lookup_host가 리터럴을 로컬 해석)
    #[tokio::test]
    async fn test_guard_url_blocks_internal_and_corporate() {
        for u in [
            "http://127.0.0.1/",
            "http://10.0.0.1/",
            "http://169.254.169.254/",
            "http://211.117.60.5:8080/",
            "http://[::1]/",
        ] {
            let url = reqwest::Url::parse(u).unwrap();
            let e = guard_url(&url).await.unwrap_err();
            assert_eq!(e.code, "URL_BLOCKED", "차단 대상: {}", u);
        }
    }

    #[tokio::test]
    async fn test_guard_url_allows_public_ip_literal() {
        let url = reqwest::Url::parse("http://8.8.8.8/").unwrap();
        assert!(guard_url(&url).await.is_ok(), "공개 IP는 통과");
    }

    // ── URL 개수 상한 ──
    #[test]
    fn test_validate_url_count() {
        assert!(validate_url_count(0).is_ok());
        assert!(validate_url_count(MAX_URLS_PER_MESSAGE).is_ok());
        let e = validate_url_count(MAX_URLS_PER_MESSAGE + 1).unwrap_err();
        assert_eq!(e.code, "URL_BLOCKED");
    }

    // ── FetchedPage → Document 첨부 변환 ──
    fn sample_page(truncated: bool) -> FetchedPage {
        FetchedPage {
            url: "https://example.com/doc".into(),
            title: Some("샘플 문서".into()),
            text: "본문 내용".into(),
            fetched_at: "2026-07-16T00:00:00+00:00".into(),
            truncated,
        }
    }

    #[test]
    fn test_web_document_header_uses_title_then_url() {
        let h = web_document_header(&sample_page(false));
        assert!(h.starts_with("[웹 자료: 샘플 문서 — https://example.com/doc (조회 2026-07-16"));
        // title 없으면 url을 제목 자리에 사용
        let mut p = sample_page(false);
        p.title = None;
        assert!(web_document_header(&p).contains("[웹 자료: https://example.com/doc — https://example.com/doc"));
    }

    #[test]
    fn test_fetched_page_to_attachment_maps_document() {
        let att = fetched_page_to_attachment(&sample_page(true));
        assert_eq!(att.kind, AttachmentKind::Document);
        assert_eq!(att.status, AttachmentStatus::Ready);
        assert_eq!(att.media_type, "text/html");
        assert_eq!(att.filename, "샘플 문서", "filename = title");
        assert!(att.content_base64.is_none());
        assert!(att.truncated, "truncated 전파");
        assert!(att.id.starts_with("web-"));
        let text = att.extracted_text.unwrap();
        assert!(text.starts_with("[웹 자료: 샘플 문서 — https://example.com/doc"), "머리표기 접두");
        assert!(text.contains("본문 내용"));
    }

    #[test]
    fn test_fetched_page_to_attachment_flows_through_build_content_blocks() {
        // 변환 결과가 기존 채팅 파이프라인(build_content_blocks)을 거쳐
        // DocumentText 블록으로 흘러가는지 확인 → 어댑터 무변경 처리 보장
        use crate::chat_attachment::{build_content_blocks, validate_attachments_for_send};
        use crate::models::provider::ProviderContentBlock;

        let att = fetched_page_to_attachment(&sample_page(false));
        // vision 미지원 provider에도 문서 텍스트로 통과해야 함
        assert!(validate_attachments_for_send("질문", std::slice::from_ref(&att), false).is_ok());

        let blocks = build_content_blocks("질문", std::slice::from_ref(&att));
        assert!(blocks.iter().any(|b| matches!(
            b,
            ProviderContentBlock::DocumentText { extracted_text, .. } if extracted_text.contains("[웹 자료:")
        )));
    }
}
