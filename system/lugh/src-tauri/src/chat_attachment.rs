// chat_attachment.rs — 채팅 입력 첨부 전처리 서비스 (Redmine #21)
// prepare_chat_attachment: 이미지 base64 검증·문서(md/txt/pdf 등) 텍스트 추출·sha256·한도 적용
// DS-60 v0.7 §3.2 prepare_chat_attachment, DS-40 v0.6 §7 첨부 전처리 및 문서 추출 API 규격

use base64::Engine;
use sha2::{Digest, Sha256};

use crate::models::{
    attachment::{
        AttachmentExtractionStatus, AttachmentKind, AttachmentStatus, ChatAttachmentInput,
        PreparedChatAttachment,
    },
    error::AppError,
    provider::{ProviderAttachment, ProviderContentBlock},
};

// ---- 한도 기본값 (DS-40 §3.2.1) ----

/// 이미지 원본 한도: 10MB/건
pub const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
/// 문서 원본 한도: 20MB/건
pub const MAX_DOCUMENT_BYTES: usize = 20 * 1024 * 1024;
/// 문서 추출 텍스트 한도: 100KB/건 (초과 시 truncate + truncated=true)
pub const MAX_EXTRACTED_TEXT_BYTES: usize = 100 * 1024;
/// 메시지당 첨부 한도: 10건
pub const MAX_ATTACHMENTS_PER_MESSAGE: usize = 10;
/// 전체 provider 입력 text budget: 300KB
pub const MAX_TOTAL_TEXT_BUDGET_BYTES: usize = 300 * 1024;

/// 허용 이미지 MIME (DS-40 §7.1)
const ALLOWED_IMAGE_MIME: [&str; 4] = ["image/png", "image/jpeg", "image/webp", "image/gif"];
/// 허용 문서 확장자 (DS-60 §3.2)
const ALLOWED_DOC_EXT: [&str; 9] = [
    "md", "markdown", "txt", "csv", "json", "yaml", "yml", "log", "pdf",
];

// ---- AppError 생성 헬퍼 (DS-40 §2.4 오류 코드) ----

fn err_invalid_type(msg: impl Into<String>) -> AppError {
    // 복구 불가 (파일 자체가 미지원)
    AppError::new("ATTACHMENT_INVALID_TYPE", msg)
}

fn err_too_large(msg: impl Into<String>) -> AppError {
    AppError::new("ATTACHMENT_TOO_LARGE", msg).recoverable()
}

fn err_extract_failed(msg: impl Into<String>) -> AppError {
    AppError::new("ATTACHMENT_EXTRACT_FAILED", msg).recoverable()
}

fn err_unsupported(msg: impl Into<String>) -> AppError {
    AppError::new("ATTACHMENT_UNSUPPORTED", msg).recoverable()
}

fn err_request_invalid(msg: impl Into<String>) -> AppError {
    AppError::new("REQUEST_INVALID", msg)
}

// ---- 첨부 전처리 (prepare_chat_attachment 본체) ----

/// 첨부 1건을 검증하고 이미지 base64 또는 문서 추출 텍스트로 정규화한다 (DS-60 §3.2)
pub fn prepare_attachment(input: ChatAttachmentInput) -> Result<PreparedChatAttachment, AppError> {
    // 1. 유형 검증 (확장자 + MIME 함께 검증, DS-40 §7.2)
    match input.kind {
        AttachmentKind::Image => validate_image_type(&input.media_type)?,
        AttachmentKind::Document => validate_document_type(&input.filename, &input.media_type)?,
    }

    // 2. base64 decode
    let bytes = decode_base64(&input.content_base64)?;

    // 3. 크기 한도 검증 (decode된 실제 bytes 기준)
    let max = match input.kind {
        AttachmentKind::Image => MAX_IMAGE_BYTES,
        AttachmentKind::Document => MAX_DOCUMENT_BYTES,
    };
    if bytes.len() > max {
        return Err(err_too_large(format!(
            "첨부 {} 크기 {}bytes가 한도 {}bytes를 초과했습니다",
            input.filename,
            bytes.len(),
            max
        )));
    }

    // 4. sha256
    let sha256 = hex::encode(Sha256::digest(&bytes));

    // 5. kind별 정규화
    match input.kind {
        AttachmentKind::Image => Ok(PreparedChatAttachment {
            id: input.id,
            kind: input.kind,
            source: input.source,
            filename: input.filename,
            media_type: input.media_type,
            size_bytes: bytes.len() as u64,
            sha256,
            status: AttachmentStatus::Ready,
            // 이미지는 provider 전송용 base64 유지 (정규화된 표준 base64로 재인코딩)
            content_base64: Some(base64::engine::general_purpose::STANDARD.encode(&bytes)),
            extracted_text: None,
            truncated: false,
            error: None,
        }),
        AttachmentKind::Document => {
            let raw_text = extract_document_text(&input.filename, &input.media_type, &bytes)?;
            let normalized = normalize_text(&raw_text);
            let (text, truncated) = truncate_text(&normalized, MAX_EXTRACTED_TEXT_BYTES);
            Ok(PreparedChatAttachment {
                id: input.id,
                kind: input.kind,
                source: input.source,
                filename: input.filename,
                media_type: input.media_type,
                size_bytes: bytes.len() as u64,
                sha256,
                status: AttachmentStatus::Ready,
                // 문서는 추출 후 원본 base64 폐기 (DS-60 §2.5)
                content_base64: None,
                extracted_text: Some(text),
                truncated,
                error: None,
            })
        }
    }
}

// ---- 유형 검증 ----

fn validate_image_type(media_type: &str) -> Result<(), AppError> {
    let mt = media_type.to_ascii_lowercase();
    if ALLOWED_IMAGE_MIME.contains(&mt.as_str()) {
        Ok(())
    } else {
        Err(err_invalid_type(format!(
            "허용되지 않은 이미지 MIME type: {} (허용: {})",
            media_type,
            ALLOWED_IMAGE_MIME.join(", ")
        )))
    }
}

fn validate_document_type(filename: &str, media_type: &str) -> Result<(), AppError> {
    let ext = file_extension(filename);
    if !ALLOWED_DOC_EXT.contains(&ext.as_str()) {
        return Err(err_invalid_type(format!(
            "허용되지 않은 문서 확장자: {} (허용: {})",
            filename,
            ALLOWED_DOC_EXT.join(", ")
        )));
    }
    // MIME 교차 검증: pdf 확장자인데 MIME이 명백히 다른 문서 계열이면 거부
    let mt = media_type.to_ascii_lowercase();
    if ext == "pdf" && !mt.is_empty() && mt != "application/pdf" && mt != "application/octet-stream" {
        return Err(err_invalid_type(format!(
            "pdf 확장자와 MIME type 불일치: {}",
            media_type
        )));
    }
    Ok(())
}

fn file_extension(filename: &str) -> String {
    filename
        .rsplit('.')
        .next()
        .filter(|ext| *ext != filename)
        .unwrap_or("")
        .to_ascii_lowercase()
}

// ---- base64 decode ----

fn decode_base64(input: &str) -> Result<Vec<u8>, AppError> {
    // data URL prefix 방어적 제거 (frontend는 원칙적으로 raw base64 전달)
    let raw = match input.find("base64,") {
        Some(pos) if input.starts_with("data:") => &input[pos + "base64,".len()..],
        _ => input,
    };
    // 공백/개행 제거 후 표준 base64 decode
    let cleaned: String = raw.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    base64::engine::general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .or_else(|_| {
            base64::engine::general_purpose::STANDARD_NO_PAD.decode(cleaned.as_bytes())
        })
        .map_err(|e| err_request_invalid(format!("base64 decode 실패: {}", e)))
}

// ---- 문서 텍스트 추출 (DS-40 §7.2) ----

fn extract_document_text(
    filename: &str,
    media_type: &str,
    bytes: &[u8],
) -> Result<String, AppError> {
    let ext = file_extension(filename);
    let is_pdf = ext == "pdf" || media_type.eq_ignore_ascii_case("application/pdf");
    if is_pdf {
        extract_pdf_text(filename, bytes)
    } else {
        decode_text_bytes(filename, bytes)
    }
}

/// PDF 텍스트 레이어 추출 (이미지 기반 PDF OCR은 MVP 미지원, DS-40 §7.1)
fn extract_pdf_text(filename: &str, bytes: &[u8]) -> Result<String, AppError> {
    // pdf-extract는 일부 손상 PDF에서 panic 가능 → catch_unwind로 방어
    let bytes_owned = bytes.to_vec();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        pdf_extract::extract_text_from_mem(&bytes_owned)
    }));
    match result {
        Ok(Ok(text)) if !text.trim().is_empty() => Ok(text),
        Ok(Ok(_)) => Err(err_extract_failed(format!(
            "PDF {}에서 텍스트 레이어를 찾을 수 없습니다 (스캔/이미지 PDF는 미지원)",
            filename
        ))),
        Ok(Err(e)) => Err(err_extract_failed(format!(
            "PDF {} 텍스트 추출 실패: {}",
            filename, e
        ))),
        Err(_) => Err(err_extract_failed(format!(
            "PDF {} 텍스트 추출 중 내부 오류가 발생했습니다",
            filename
        ))),
    }
}

/// 텍스트 문서 decode: UTF-8 우선, BOM 감지 (UTF-8/UTF-16 LE/BE), 실패 시 오류 (DS-40 §7.2)
fn decode_text_bytes(filename: &str, bytes: &[u8]) -> Result<String, AppError> {
    // UTF-8 BOM 제거
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return std::str::from_utf8(&bytes[3..])
            .map(|s| s.to_string())
            .map_err(|_| decode_err(filename));
    }
    // UTF-16 LE BOM
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return decode_utf16(&bytes[2..], true).ok_or_else(|| decode_err(filename));
    }
    // UTF-16 BE BOM
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return decode_utf16(&bytes[2..], false).ok_or_else(|| decode_err(filename));
    }
    // BOM 없음 → UTF-8 strict
    std::str::from_utf8(bytes)
        .map(|s| s.to_string())
        .map_err(|_| decode_err(filename))
}

fn decode_err(filename: &str) -> AppError {
    err_extract_failed(format!(
        "문서 {} 텍스트 decode 실패 (UTF-8/UTF-16 아님)",
        filename
    ))
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Option<String> {
    if bytes.len() % 2 != 0 {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| {
            if little_endian {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16(&units).ok()
}

/// CRLF → LF 정규화 + NUL 제거 (DS-40 §7.2)
fn normalize_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n").replace('\0', "")
}

/// 추출 텍스트를 max_bytes 이하로 truncate (char boundary 보존, DS-40 §3.2.1)
fn truncate_text(text: &str, max_bytes: usize) -> (String, bool) {
    if text.len() <= max_bytes {
        return (text.to_string(), false);
    }
    let mut idx = max_bytes;
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    (text[..idx].to_string(), true)
}

// ---- 전송 전 검증 (send_agent_message 경로, DS-60 §3.2 / DS-40 §7.3) ----

/// 전송 직전 첨부 목록을 검증한다.
/// 하나라도 위반이면 메시지를 전송하지 않고 AppError를 반환한다 (조용한 누락 금지).
pub fn validate_attachments_for_send(
    content: &str,
    attachments: &[PreparedChatAttachment],
    vision_supported: bool,
) -> Result<(), AppError> {
    if attachments.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(err_too_large(format!(
            "메시지당 첨부 한도 {}건 초과 ({}건)",
            MAX_ATTACHMENTS_PER_MESSAGE,
            attachments.len()
        )));
    }

    let mut text_budget = content.len();
    for att in attachments {
        // ready 상태만 전송 허용 (DS-60 §3.2 send_agent_message)
        if att.status != AttachmentStatus::Ready {
            return Err(err_request_invalid(format!(
                "첨부 {}이(가) ready 상태가 아닙니다",
                att.filename
            )));
        }
        match att.kind {
            AttachmentKind::Image => {
                if !vision_supported {
                    // 이미지 미지원 provider → 자동 텍스트 대체 없이 전체 거절 (DS-40 §7.3)
                    return Err(err_unsupported(format!(
                        "선택된 provider/model이 이미지 첨부를 지원하지 않습니다: {}",
                        att.filename
                    )));
                }
                if att.content_base64.as_deref().map(str::is_empty).unwrap_or(true) {
                    return Err(err_request_invalid(format!(
                        "이미지 첨부 {}에 content_base64가 없습니다",
                        att.filename
                    )));
                }
            }
            AttachmentKind::Document => {
                match att.extracted_text.as_deref() {
                    Some(text) => text_budget += text.len(),
                    None => {
                        return Err(err_request_invalid(format!(
                            "문서 첨부 {}에 extracted_text가 없습니다",
                            att.filename
                        )))
                    }
                }
            }
        }
    }

    if text_budget > MAX_TOTAL_TEXT_BUDGET_BYTES {
        return Err(err_too_large(format!(
            "전체 입력 text budget {}bytes 초과 ({}bytes)",
            MAX_TOTAL_TEXT_BUDGET_BYTES, text_budget
        )));
    }

    Ok(())
}

/// 사용자 입력 텍스트 + 첨부를 ProviderContentBlock 목록으로 변환한다 (DS-40 §3.2)
/// 순서: 텍스트 → 이미지 → 문서 (DS-40 §4.4 예시 기준)
pub fn build_content_blocks(
    content: &str,
    attachments: &[PreparedChatAttachment],
) -> Vec<ProviderContentBlock> {
    let mut blocks = Vec::with_capacity(1 + attachments.len());
    if !content.is_empty() {
        blocks.push(ProviderContentBlock::Text {
            text: content.to_string(),
        });
    }
    for att in attachments {
        match att.kind {
            AttachmentKind::Image => {
                if let Some(data) = &att.content_base64 {
                    blocks.push(ProviderContentBlock::Image {
                        attachment_id: att.id.clone(),
                        media_type: att.media_type.clone(),
                        base64_data: data.clone(),
                    });
                }
            }
            AttachmentKind::Document => {
                if let Some(text) = &att.extracted_text {
                    blocks.push(ProviderContentBlock::DocumentText {
                        attachment_id: att.id.clone(),
                        filename: att.filename.clone(),
                        media_type: att.media_type.clone(),
                        extracted_text: text.clone(),
                        truncated: att.truncated,
                    });
                }
            }
        }
    }
    blocks
}

/// PreparedChatAttachment → ProviderAttachment 메타 변환 (DS-40 §3.2.1)
pub fn to_provider_attachments(attachments: &[PreparedChatAttachment]) -> Vec<ProviderAttachment> {
    attachments
        .iter()
        .map(|att| ProviderAttachment {
            id: att.id.clone(),
            kind: att.kind,
            filename: att.filename.clone(),
            media_type: att.media_type.clone(),
            size_bytes: att.size_bytes,
            source: att.source,
            content_base64: att.content_base64.clone(),
            extracted_text: att.extracted_text.clone(),
            extraction_status: match att.kind {
                AttachmentKind::Image => AttachmentExtractionStatus::NotRequired,
                AttachmentKind::Document => AttachmentExtractionStatus::Completed,
            },
            truncated: att.truncated,
        })
        .collect()
}

/// DocumentText block의 provider 전송용 텍스트 표기 (DS-40 §4.3 문서 첨부 헤더 규격)
/// 예: "[첨부 문서: 요구사항.md, text/markdown]\n..."
pub fn document_block_text(filename: &str, media_type: &str, extracted_text: &str) -> String {
    format!("[첨부 문서: {}, {}]\n{}", filename, media_type, extracted_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::attachment::AttachmentSource;

    fn image_input(media_type: &str, bytes: &[u8]) -> ChatAttachmentInput {
        ChatAttachmentInput {
            id: "att-img-1".into(),
            kind: AttachmentKind::Image,
            source: AttachmentSource::Clipboard,
            filename: "shot.png".into(),
            media_type: media_type.into(),
            size_bytes: bytes.len() as u64,
            content_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        }
    }

    fn doc_input(filename: &str, media_type: &str, bytes: &[u8]) -> ChatAttachmentInput {
        ChatAttachmentInput {
            id: "att-doc-1".into(),
            kind: AttachmentKind::Document,
            source: AttachmentSource::DragDrop,
            filename: filename.into(),
            media_type: media_type.into(),
            size_bytes: bytes.len() as u64,
            content_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        }
    }

    fn ready_image(id: &str) -> PreparedChatAttachment {
        PreparedChatAttachment {
            id: id.into(),
            kind: AttachmentKind::Image,
            source: AttachmentSource::Clipboard,
            filename: format!("{}.png", id),
            media_type: "image/png".into(),
            size_bytes: 4,
            sha256: "abcd".into(),
            status: AttachmentStatus::Ready,
            content_base64: Some("aGVsbG8=".into()),
            extracted_text: None,
            truncated: false,
            error: None,
        }
    }

    fn ready_document(id: &str, text: &str) -> PreparedChatAttachment {
        PreparedChatAttachment {
            id: id.into(),
            kind: AttachmentKind::Document,
            source: AttachmentSource::FilePicker,
            filename: format!("{}.md", id),
            media_type: "text/markdown".into(),
            size_bytes: text.len() as u64,
            sha256: "abcd".into(),
            status: AttachmentStatus::Ready,
            content_base64: None,
            extracted_text: Some(text.into()),
            truncated: false,
            error: None,
        }
    }

    // ---- prepare_attachment: 이미지 ----

    #[test]
    fn test_prepare_image_ok() {
        let bytes = [0x89u8, 0x50, 0x4E, 0x47]; // PNG magic 앞부분
        let prepared = prepare_attachment(image_input("image/png", &bytes)).unwrap();
        assert_eq!(prepared.status, AttachmentStatus::Ready);
        assert_eq!(prepared.kind, AttachmentKind::Image);
        assert!(prepared.content_base64.is_some(), "이미지는 base64 유지");
        assert!(prepared.extracted_text.is_none());
        assert_eq!(prepared.sha256, hex::encode(Sha256::digest(bytes)));
        assert!(!prepared.truncated);
    }

    #[test]
    fn test_prepare_image_invalid_mime() {
        let e = prepare_attachment(image_input("image/tiff", b"xxxx")).unwrap_err();
        assert_eq!(e.code, "ATTACHMENT_INVALID_TYPE");
        assert!(!e.recoverable);
    }

    #[test]
    fn test_prepare_image_too_large() {
        let big = vec![0u8; MAX_IMAGE_BYTES + 1];
        let e = prepare_attachment(image_input("image/png", &big)).unwrap_err();
        assert_eq!(e.code, "ATTACHMENT_TOO_LARGE");
        assert!(e.recoverable);
    }

    #[test]
    fn test_prepare_image_data_url_prefix_tolerated() {
        let mut input = image_input("image/png", b"hello");
        input.content_base64 = format!("data:image/png;base64,{}", input.content_base64);
        let prepared = prepare_attachment(input).unwrap();
        assert_eq!(prepared.status, AttachmentStatus::Ready);
    }

    #[test]
    fn test_prepare_invalid_base64() {
        let mut input = image_input("image/png", b"hello");
        input.content_base64 = "!!!not-base64!!!".into();
        let e = prepare_attachment(input).unwrap_err();
        assert_eq!(e.code, "REQUEST_INVALID");
    }

    // ---- prepare_attachment: 문서 ----

    #[test]
    fn test_prepare_document_md_extraction_normalizes_crlf_and_nul() {
        let content = "# 제목\r\nline1\r\nline\0 2\rend";
        let prepared =
            prepare_attachment(doc_input("readme.md", "text/markdown", content.as_bytes()))
                .unwrap();
        assert_eq!(prepared.status, AttachmentStatus::Ready);
        assert!(prepared.content_base64.is_none(), "문서는 원본 base64 폐기");
        assert_eq!(
            prepared.extracted_text.as_deref(),
            Some("# 제목\nline1\nline 2\nend")
        );
        assert!(!prepared.truncated);
    }

    #[test]
    fn test_prepare_document_utf8_bom_stripped() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("hello".as_bytes());
        let prepared = prepare_attachment(doc_input("a.txt", "text/plain", &bytes)).unwrap();
        assert_eq!(prepared.extracted_text.as_deref(), Some("hello"));
    }

    #[test]
    fn test_prepare_document_utf16le_bom_decoded() {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "안녕".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let prepared = prepare_attachment(doc_input("a.txt", "text/plain", &bytes)).unwrap();
        assert_eq!(prepared.extracted_text.as_deref(), Some("안녕"));
    }

    #[test]
    fn test_prepare_document_invalid_extension() {
        let e = prepare_attachment(doc_input("doc.docx", "application/msword", b"x")).unwrap_err();
        assert_eq!(e.code, "ATTACHMENT_INVALID_TYPE");
    }

    #[test]
    fn test_prepare_document_decode_failure() {
        // 유효하지 않은 UTF-8 시퀀스
        let e = prepare_attachment(doc_input("a.txt", "text/plain", &[0xFF, 0xFF, 0xFF]))
            .unwrap_err();
        assert_eq!(e.code, "ATTACHMENT_EXTRACT_FAILED");
        assert!(e.recoverable);
    }

    #[test]
    fn test_prepare_document_truncated_over_100kb() {
        // 멀티바이트 문자로 char boundary 보존 확인
        let big = "가".repeat(40_000); // 40,000 * 3bytes = 120KB
        let prepared =
            prepare_attachment(doc_input("big.txt", "text/plain", big.as_bytes())).unwrap();
        assert!(prepared.truncated, "100KB 초과 시 truncated=true");
        let text = prepared.extracted_text.unwrap();
        assert!(text.len() <= MAX_EXTRACTED_TEXT_BYTES);
        assert!(text.chars().all(|c| c == '가'), "char boundary 보존");
    }

    #[test]
    fn test_prepare_pdf_garbage_bytes_extract_failed() {
        let e = prepare_attachment(doc_input("broken.pdf", "application/pdf", b"not a pdf"))
            .unwrap_err();
        assert_eq!(e.code, "ATTACHMENT_EXTRACT_FAILED");
    }

    // ---- validate_attachments_for_send ----

    #[test]
    fn test_validate_send_ok_mixed() {
        let atts = vec![ready_image("i1"), ready_document("d1", "hello")];
        assert!(validate_attachments_for_send("본문", &atts, true).is_ok());
    }

    #[test]
    fn test_validate_send_image_unsupported_provider_rejected() {
        // 이미지 미지원 provider는 전송 전체 거절 (DS-40 §7.3). #23: Claude CLI는
        // vision 지원으로 전환되어 더 이상 이 케이스의 예시가 아님 — 검증 로직 자체는
        // vision_supported 플래그 기반 범용 로직이라 변경 없음.
        let atts = vec![ready_image("i1")];
        let e = validate_attachments_for_send("본문", &atts, false).unwrap_err();
        assert_eq!(e.code, "ATTACHMENT_UNSUPPORTED");
        assert!(e.recoverable);
    }

    #[test]
    fn test_validate_send_document_only_ok_without_vision() {
        // 문서 첨부는 vision 미지원 provider에도 추출 텍스트로 전달 가능
        let atts = vec![ready_document("d1", "text")];
        assert!(validate_attachments_for_send("본문", &atts, false).is_ok());
    }

    #[test]
    fn test_validate_send_too_many_attachments() {
        let atts: Vec<_> = (0..11).map(|i| ready_image(&format!("i{}", i))).collect();
        let e = validate_attachments_for_send("", &atts, true).unwrap_err();
        assert_eq!(e.code, "ATTACHMENT_TOO_LARGE");
    }

    #[test]
    fn test_validate_send_non_ready_rejected() {
        let mut att = ready_image("i1");
        att.status = AttachmentStatus::Failed;
        let e = validate_attachments_for_send("", &[att], true).unwrap_err();
        assert_eq!(e.code, "REQUEST_INVALID");
    }

    #[test]
    fn test_validate_send_text_budget_exceeded() {
        // 100KB 문서 3건 + 본문 → 300KB budget 초과
        let big = "a".repeat(MAX_EXTRACTED_TEXT_BYTES);
        let atts = vec![
            ready_document("d1", &big),
            ready_document("d2", &big),
            ready_document("d3", &big),
        ];
        let e = validate_attachments_for_send("본문 텍스트", &atts, true).unwrap_err();
        assert_eq!(e.code, "ATTACHMENT_TOO_LARGE");
    }

    // ---- build_content_blocks ----

    #[test]
    fn test_build_content_blocks_order_text_image_document() {
        let atts = vec![ready_image("i1"), ready_document("d1", "doc text")];
        let blocks = build_content_blocks("질문입니다", &atts);
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], ProviderContentBlock::Text { text } if text == "질문입니다"));
        assert!(matches!(&blocks[1], ProviderContentBlock::Image { media_type, .. } if media_type == "image/png"));
        assert!(matches!(&blocks[2], ProviderContentBlock::DocumentText { extracted_text, .. } if extracted_text == "doc text"));
    }

    #[test]
    fn test_build_content_blocks_empty_content_skips_text_block() {
        let atts = vec![ready_image("i1")];
        let blocks = build_content_blocks("", &atts);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], ProviderContentBlock::Image { .. }));
    }

    #[test]
    fn test_document_block_text_header_format() {
        let text = document_block_text("요구사항.md", "text/markdown", "내용");
        assert_eq!(text, "[첨부 문서: 요구사항.md, text/markdown]\n내용");
    }

    #[test]
    fn test_to_provider_attachments_extraction_status() {
        let atts = vec![ready_image("i1"), ready_document("d1", "t")];
        let converted = to_provider_attachments(&atts);
        assert_eq!(converted[0].extraction_status, AttachmentExtractionStatus::NotRequired);
        assert_eq!(converted[1].extraction_status, AttachmentExtractionStatus::Completed);
    }
}
