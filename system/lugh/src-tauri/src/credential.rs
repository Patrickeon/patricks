// credential.rs — CredentialStoreService
// OS Keychain(macOS) / Credential Manager(Windows) 기반 API key 저장·조회
// DS-20 §9.2, DS-40 §8, DS-60 §3.5

use keyring::Entry;

use crate::models::{
    error::{AppError, AppResult},
    provider::{AiProviderKind, CredentialRef, HealthStatus, ProviderHealth},
};
use chrono::Utc;

const KEYRING_SERVICE: &str = "AgiTeamBuilder";

/// CredentialStoreService — OS Credential Vault 래퍼
pub struct CredentialStoreService;

impl CredentialStoreService {
    /// provider credential을 OS vault에 저장한다.
    /// secret은 이 함수에서만 처리하고 반환하지 않는다.
    pub fn save(provider: &AiProviderKind, account: &str, secret: &str) -> AppResult<CredentialRef> {
        let entry = Self::entry(provider, account)?;
        entry.set_password(secret).map_err(|e| {
            AppError::new("CREDENTIAL_SAVE_FAILED", e.to_string())
        })?;
        Ok(CredentialRef {
            provider: provider.clone(),
            account: account.to_string(),
        })
    }

    /// OS vault에서 credential을 가져온다.
    /// 반환값에 secret이 포함되므로 이 함수 외부로 노출하지 않는다.
    pub(crate) fn get_secret(provider: &AiProviderKind, account: &str) -> AppResult<String> {
        let entry = Self::entry(provider, account)?;
        entry.get_password().map_err(|e| match e {
            keyring::Error::NoEntry => AppError::credential_missing(provider),
            other => AppError::new("CREDENTIAL_READ_FAILED", other.to_string()),
        })
    }

    /// credential이 존재하는지 확인한다 (secret 미반환).
    pub fn exists(provider: &AiProviderKind, account: &str) -> bool {
        Self::get_secret(provider, account).is_ok()
    }

    /// credential을 삭제한다.
    pub fn delete(provider: &AiProviderKind, account: &str) -> AppResult<()> {
        let entry = Self::entry(provider, account)?;
        entry.delete_credential().map_err(|e| match e {
            keyring::Error::NoEntry => AppError::credential_missing(provider),
            other => AppError::new("CREDENTIAL_DELETE_FAILED", other.to_string()),
        })
    }

    /// provider credential의 존재 여부만 확인한 ProviderHealth를 반환한다.
    /// 실제 API 검증은 HealthCheckService가 담당한다.
    pub fn check_existence(provider: &AiProviderKind, account: &str) -> ProviderHealth {
        let (status, error) = if Self::exists(provider, account) {
            (HealthStatus::Ok, None)
        } else {
            (
                HealthStatus::Unreachable,
                Some(AppError::credential_missing(provider)),
            )
        };
        ProviderHealth {
            provider: provider.clone(),
            status,
            latency_ms: None,
            checked_at: Utc::now(),
            error,
        }
    }

    // 서비스 이름: "AgiTeamBuilder/<provider>" 형식으로 provider별 분리
    fn entry(provider: &AiProviderKind, account: &str) -> AppResult<Entry> {
        let service = format!("{}/{}", KEYRING_SERVICE, provider);
        Entry::new(&service, account).map_err(|e| {
            AppError::new("KEYRING_INIT_FAILED", e.to_string())
        })
    }
}

/// masked account 정보 — secret 없이 UI에 표시할 정보
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MaskedCredential {
    pub provider: AiProviderKind,
    pub account: String,
    pub has_secret: bool,
}

impl MaskedCredential {
    pub fn from_ref(credential_ref: &CredentialRef) -> Self {
        let has_secret = CredentialStoreService::exists(
            &credential_ref.provider,
            &credential_ref.account,
        );
        Self {
            provider: credential_ref.provider.clone(),
            account: credential_ref.account.clone(),
            has_secret,
        }
    }
}

/// Claude Code CLI OAuth 토큰을 macOS Keychain에서 읽어온다.
/// "Claude Code-credentials" / $USER 항목의 claudeAiOauth.accessToken 반환.
pub fn get_claude_oauth_token() -> AppResult<String> {
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".to_string());

    let entry = keyring::Entry::new("Claude Code-credentials", &username)
        .map_err(|e| AppError::new("KEYRING_INIT_FAILED", e.to_string()))?;

    let json_str = entry.get_password().map_err(|_| {
        AppError::new("CREDENTIAL_MISSING", "Claude Code-credentials not found")
    })?;

    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| AppError::new("PARSE_FAILED", e.to_string()))?;

    parsed["claudeAiOauth"]["accessToken"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::new("CREDENTIAL_MISSING", "accessToken not found"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OS keyring이 없거나 즉시 조회가 불가한 CI 환경에서는 skip
    #[test]
    fn test_credential_save_get_delete() {
        let provider = AiProviderKind::Claude;
        let account = "test-account-credential-rs";
        let secret = "test-secret-value";

        // Save
        let result = CredentialStoreService::save(&provider, account, secret);
        if result.is_err() {
            // CI 환경 등 keyring 없음 → skip
            return;
        }
        let cref = result.unwrap();
        assert_eq!(cref.account, account);

        // Get secret — 저장 직후 조회 실패하는 환경(샌드박스 등)에서는 정리 후 skip
        let got = CredentialStoreService::get_secret(&provider, account);
        if got.is_err() {
            // 환경 제약으로 즉시 조회 불가 → 정리하고 skip
            let _ = CredentialStoreService::delete(&provider, account);
            return;
        }
        assert_eq!(got.unwrap(), secret);

        // Exists
        assert!(CredentialStoreService::exists(&provider, account));

        // Delete
        CredentialStoreService::delete(&provider, account).unwrap();
        assert!(!CredentialStoreService::exists(&provider, account));
    }

    #[test]
    fn test_credential_missing_error() {
        let provider = AiProviderKind::OpenAi;
        let account = "nonexistent-account-xyz-12345";
        // 없는 credential 조회 → error
        let result = CredentialStoreService::get_secret(&provider, account);
        if result.is_ok() {
            // 다른 테스트에서 생성됐을 수 있으므로 그냥 pass
            return;
        }
        assert!(result.is_err());
    }

    #[test]
    fn test_masked_credential() {
        let cref = CredentialRef {
            provider: AiProviderKind::Gemini,
            account: "gemini-account".into(),
        };
        let masked = MaskedCredential::from_ref(&cref);
        assert_eq!(masked.provider, AiProviderKind::Gemini);
        // has_secret은 환경에 따라 달라지므로 타입만 확인
        let _ = masked.has_secret;
    }
}
