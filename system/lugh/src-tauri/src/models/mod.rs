// models/mod.rs — 모델 모듈 재export

pub mod attachment;
pub mod error;
pub mod message;
pub mod persona;
pub mod provider;
pub mod role;
pub mod session;
pub mod workspace;

// 자주 쓰는 타입 re-export
pub use error::{AppError, AppResult};
pub use provider::AiProviderKind;
pub use session::{AgentLifecycleState, CommandResult};
