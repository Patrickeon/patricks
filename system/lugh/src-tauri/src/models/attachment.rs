// models/attachment.rs — 채팅 입력 첨부 DTO (Redmine #21)
// DS-60 v0.7 §2.5 Chat Attachment DTO, DS-40 v0.6 §3.2.1 첨부 표준 모델

use serde::{Deserialize, Serialize};

use super::error::AppError;

/// 첨부 종류 (DS-60 §2.5 ChatAttachmentKind)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentKind {
    Image,
    Document,
}

/// 첨부 유입 경로 (DS-60 §2.5 ChatAttachmentSource)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentSource {
    FilePicker,
    Clipboard,
    DragDrop,
}

/// 첨부 준비 상태 (DS-60 §2.5 ChatAttachmentStatus)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentStatus {
    Pending,
    Ready,
    Failed,
}

/// 문서 추출 상태 (DS-40 §3.2.1 extraction_status)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentExtractionStatus {
    NotRequired,
    Pending,
    Completed,
    Failed,
}

/// prepare_chat_attachment 요청의 첨부 입력 (DS-60 §2.5 ChatAttachmentInput)
/// `content_base64`는 frontend가 File/Clipboard Blob을 읽어 전달하는 transport 필드.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatAttachmentInput {
    pub id: String,
    pub kind: AttachmentKind,
    pub source: AttachmentSource,
    pub filename: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub content_base64: String,
}

/// prepare_chat_attachment 응답 (DS-60 §2.5 PreparedChatAttachment)
/// 이미지는 `content_base64` 유지, 문서는 `extracted_text` 생성 후 원본 base64 폐기.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedChatAttachment {
    pub id: String,
    pub kind: AttachmentKind,
    pub source: AttachmentSource,
    pub filename: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub status: AttachmentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extracted_text: Option<String>,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<AppError>,
}

/// chat:attachment_prepared event payload (DS-60 §5.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatAttachmentPrepared {
    pub session_id: String,
    pub attachment: PreparedChatAttachment,
}

/// chat:attachment_failed event payload (DS-60 §5.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatAttachmentFailed {
    pub session_id: String,
    pub attachment_id: String,
    pub filename: String,
    pub error: AppError,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attachment_enum_serialization() {
        // DS-60 §2.5 문자열 표기와 일치해야 함
        assert_eq!(serde_json::to_string(&AttachmentKind::Image).unwrap(), "\"image\"");
        assert_eq!(serde_json::to_string(&AttachmentKind::Document).unwrap(), "\"document\"");
        assert_eq!(serde_json::to_string(&AttachmentSource::FilePicker).unwrap(), "\"file_picker\"");
        assert_eq!(serde_json::to_string(&AttachmentSource::DragDrop).unwrap(), "\"drag_drop\"");
        assert_eq!(serde_json::to_string(&AttachmentStatus::Ready).unwrap(), "\"ready\"");
        assert_eq!(
            serde_json::to_string(&AttachmentExtractionStatus::NotRequired).unwrap(),
            "\"not_required\""
        );
    }

    #[test]
    fn test_chat_attachment_input_deserialization() {
        // frontend camelCase가 아닌 snake_case 필드로 수신 (DS-60 §2.5)
        let json = r#"{
            "id": "att-1",
            "kind": "image",
            "source": "clipboard",
            "filename": "shot.png",
            "media_type": "image/png",
            "size_bytes": 1234,
            "content_base64": "aGVsbG8="
        }"#;
        let input: ChatAttachmentInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.kind, AttachmentKind::Image);
        assert_eq!(input.source, AttachmentSource::Clipboard);
        assert_eq!(input.size_bytes, 1234);
    }

    #[test]
    fn test_prepared_attachment_optional_fields_skipped() {
        let prepared = PreparedChatAttachment {
            id: "att-1".into(),
            kind: AttachmentKind::Document,
            source: AttachmentSource::DragDrop,
            filename: "a.md".into(),
            media_type: "text/markdown".into(),
            size_bytes: 10,
            sha256: "deadbeef".into(),
            status: AttachmentStatus::Ready,
            content_base64: None,
            extracted_text: Some("hello".into()),
            truncated: false,
            error: None,
        };
        let json = serde_json::to_string(&prepared).unwrap();
        assert!(!json.contains("content_base64"), "None 필드는 직렬화에서 제외");
        assert!(!json.contains("\"error\""), "None error는 직렬화에서 제외");
        assert!(json.contains("extracted_text"));
    }
}
