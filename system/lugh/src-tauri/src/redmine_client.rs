// redmine_client.rs — RedmineClient
// Redmine REST API HTTP 클라이언트 서비스
// Shared persona §2 (레드마인 공용 접속 정보), DS-40 §레드마인 기반
//
// Redmine은 AI Provider가 아니므로 provider 모듈과 분리한다.
// API 키는 OS vault (AiProviderKind::Redmine, "api_key")에서 가져온다.

use serde::{Deserialize, Serialize};

use crate::models::error::{AppError, AppResult};

// ── 응답 타입 ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedmineStatusRef {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedmineRef {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedmineUserRef {
    pub id: u32,
    pub name: String,
}

/// 이슈 단건 — list / get / create / update 응답에 공통 사용
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedmineIssueItem {
    pub id: u32,
    pub subject: String,
    #[serde(default)]
    pub description: String,
    pub status: RedmineStatusRef,
    pub tracker: RedmineRef,
    pub assigned_to: Option<RedmineUserRef>,
    #[serde(default)]
    pub done_ratio: u32,
    pub created_on: String,
    pub updated_on: String,
}

// ── 내부 API 래퍼 ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct IssuesResponse {
    issues: Vec<RedmineIssueItem>,
}

#[derive(Deserialize)]
struct IssueWrapper {
    issue: RedmineIssueItem,
}

// ── 요청 페이로드 ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct CreateIssuePayload {
    issue: CreateIssueFields,
}

#[derive(Serialize)]
struct CreateIssueFields {
    project_id: String,
    tracker_id: u32,
    subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assigned_to_id: Option<u32>,
}

#[derive(Serialize)]
struct UpdateIssuePayload {
    issue: UpdateIssueFields,
}

#[derive(Serialize)]
struct UpdateIssueFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    status_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    done_ratio: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
}

// ── RedmineClient ─────────────────────────────────────────────────────────────

/// Redmine REST API 클라이언트
/// 인스턴스 1개 = 커맨드 1회 요청. 커넥션 풀은 reqwest 내부에서 관리.
pub struct RedmineClient {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

impl RedmineClient {
    /// base_url 예: "http://211.117.60.5:8080"
    pub fn new(base_url: String, api_key: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent(format!("AgiTeamBuilder/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        Self { base_url, api_key, http }
    }

    // ── 내부 헬퍼 ──────────────────────────────────────────────────────────────

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url.trim_end_matches('/'), path.trim_start_matches('/'))
    }

    fn api_key_header(&self) -> (&'static str, String) {
        ("X-Redmine-API-Key", self.api_key.clone())
    }

    async fn check_status(resp: reqwest::Response, op: &str) -> AppResult<reqwest::Response> {
        if resp.status().is_success() {
            Ok(resp)
        } else {
            Err(AppError::new(
                "REDMINE_API_ERROR",
                format!("{} 실패: HTTP {}", op, resp.status().as_u16()),
            ))
        }
    }

    // ── 공개 API ───────────────────────────────────────────────────────────────

    /// GET /issues.json?project_id=<id>&status_id=<open|all|id>&limit=100
    pub async fn list_issues(
        &self,
        project_id: Option<&str>,
        status_id: Option<&str>,
    ) -> AppResult<Vec<RedmineIssueItem>> {
        let mut url = self.url("issues.json?limit=100");
        if let Some(pid) = project_id {
            url.push_str(&format!("&project_id={}", pid));
        }
        url.push_str(&format!("&status_id={}", status_id.unwrap_or("open")));

        let (key, val) = self.api_key_header();
        let resp = self.http
            .get(&url)
            .header(key, val)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| AppError::new("REDMINE_REQUEST_FAILED", e.to_string()))?;

        let resp = Self::check_status(resp, "이슈 목록 조회").await?;
        let body: IssuesResponse = resp.json().await
            .map_err(|e| AppError::new("REDMINE_PARSE_FAILED", e.to_string()))?;
        Ok(body.issues)
    }

    /// GET /issues/<id>.json
    pub async fn get_issue(&self, issue_id: u32) -> AppResult<RedmineIssueItem> {
        let url = self.url(&format!("issues/{}.json", issue_id));

        let (key, val) = self.api_key_header();
        let resp = self.http
            .get(&url)
            .header(key, val)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| AppError::new("REDMINE_REQUEST_FAILED", e.to_string()))?;

        let resp = Self::check_status(resp, "이슈 단건 조회").await?;
        let body: IssueWrapper = resp.json().await
            .map_err(|e| AppError::new("REDMINE_PARSE_FAILED", e.to_string()))?;
        Ok(body.issue)
    }

    /// POST /issues.json
    pub async fn create_issue(
        &self,
        project_id: &str,
        tracker_id: u32,
        subject: &str,
        description: Option<&str>,
        assigned_to_id: Option<u32>,
    ) -> AppResult<RedmineIssueItem> {
        let url = self.url("issues.json");
        let payload = CreateIssuePayload {
            issue: CreateIssueFields {
                project_id: project_id.to_string(),
                tracker_id,
                subject: subject.to_string(),
                description: description.map(|s| s.to_string()),
                assigned_to_id,
            },
        };

        let (key, val) = self.api_key_header();
        let resp = self.http
            .post(&url)
            .header(key, val)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::new("REDMINE_REQUEST_FAILED", e.to_string()))?;

        let resp = Self::check_status(resp, "이슈 생성").await?;
        let body: IssueWrapper = resp.json().await
            .map_err(|e| AppError::new("REDMINE_PARSE_FAILED", e.to_string()))?;
        Ok(body.issue)
    }

    /// PUT /issues/<id>.json
    /// Redmine은 204 No Content 반환 → 성공 시 ()
    pub async fn update_issue(
        &self,
        issue_id: u32,
        status_id: Option<u32>,
        done_ratio: Option<u32>,
        notes: Option<&str>,
    ) -> AppResult<()> {
        let url = self.url(&format!("issues/{}.json", issue_id));
        let payload = UpdateIssuePayload {
            issue: UpdateIssueFields {
                status_id,
                done_ratio,
                notes: notes.map(|s| s.to_string()),
            },
        };

        let (key, val) = self.api_key_header();
        let resp = self.http
            .put(&url)
            .header(key, val)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::new("REDMINE_REQUEST_FAILED", e.to_string()))?;

        Self::check_status(resp, "이슈 갱신").await?;
        Ok(())
    }
}

// ── 단위 테스트 ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_client() -> RedmineClient {
        RedmineClient::new(
            "http://211.117.60.5:8080".to_string(),
            "test-api-key".to_string(),
        )
    }

    #[test]
    fn test_url_helper_trims_slash() {
        // base_url 끝 슬래시가 있어도 이중 슬래시가 되지 않아야 한다
        let c1 = RedmineClient::new("http://host:8080/".to_string(), "k".to_string());
        let c2 = RedmineClient::new("http://host:8080".to_string(), "k".to_string());
        assert_eq!(c1.url("issues.json"), c2.url("issues.json"));
        assert!(!c1.url("issues.json").contains("//issues"));
    }

    #[test]
    fn test_new_stores_fields() {
        let c = make_client();
        assert!(c.base_url.contains("211.117.60.5"));
        assert_eq!(c.api_key, "test-api-key");
    }

    #[test]
    fn test_create_issue_payload_serialization() {
        // CreateIssueFields가 올바르게 직렬화되는지 확인
        let payload = CreateIssuePayload {
            issue: CreateIssueFields {
                project_id: "proj-1".to_string(),
                tracker_id: 2,
                subject: "테스트 이슈".to_string(),
                description: Some("상세 내용".to_string()),
                assigned_to_id: Some(42),
            },
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("proj-1"));
        assert!(json.contains("tracker_id"));
        assert!(json.contains("테스트 이슈"));
        assert!(json.contains("42"));
    }

    #[test]
    fn test_update_issue_payload_skip_none() {
        // None 필드는 직렬화에서 제외되어야 한다 (skip_serializing_if)
        let payload = UpdateIssuePayload {
            issue: UpdateIssueFields {
                status_id: Some(3),
                done_ratio: None,
                notes: None,
            },
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("status_id"));
        assert!(!json.contains("done_ratio"), "None 필드는 직렬화에서 제외되어야 함");
        assert!(!json.contains("notes"), "None 필드는 직렬화에서 제외되어야 함");
    }

    #[test]
    fn test_issue_item_deserialize() {
        // RedmineIssueItem 역직렬화 (description 누락 시 default 빈 문자열)
        let json = r#"{
            "id": 101,
            "subject": "버그 수정",
            "status": {"id": 1, "name": "신규"},
            "tracker": {"id": 1, "name": "결함"},
            "done_ratio": 0,
            "created_on": "2026-06-01T00:00:00Z",
            "updated_on": "2026-06-02T00:00:00Z"
        }"#;
        let item: RedmineIssueItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.id, 101);
        assert_eq!(item.subject, "버그 수정");
        assert_eq!(item.description, "");  // default
        assert!(item.assigned_to.is_none());
    }

    #[tokio::test]
    async fn test_list_issues_network_unavailable() {
        // 네트워크 미연결 환경에서 REDMINE_REQUEST_FAILED 오류를 반환해야 한다
        let c = RedmineClient::new("http://127.0.0.1:19999".to_string(), "key".to_string());
        let result = c.list_issues(Some("test-proj"), Some("open")).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "REDMINE_REQUEST_FAILED");
    }

    #[tokio::test]
    async fn test_get_issue_network_unavailable() {
        let c = RedmineClient::new("http://127.0.0.1:19999".to_string(), "key".to_string());
        let result = c.get_issue(999).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "REDMINE_REQUEST_FAILED");
    }
}
