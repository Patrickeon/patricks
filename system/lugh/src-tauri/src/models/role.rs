// models/role.rs — RoleConfig (DS-20 §3.3)

use serde::{Deserialize, Serialize};

use super::provider::AiProviderKind;

/// 팀 구성 역할 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleConfig {
    pub role: String,
    pub display_name: String,
    pub provider: AiProviderKind,
    /// agiteam.json의 layout 슬롯
    pub layout_slot: String,
    /// 기본 모델 (provider별 기본값 사용 시 None)
    pub model: Option<String>,
}

/// DS-20 §3.2 레이아웃 슬롯 목록
pub const LAYOUT_SLOTS: &[&str] = &[
    "middle_top",
    "middle_mid",
    "middle_bottom",
    "right_top",
    "right_mid",
    "right_bottom",
];
