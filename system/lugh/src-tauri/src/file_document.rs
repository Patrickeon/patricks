// file_document.rs — FileDocumentService
// .latest.md 읽기/쓰기 + _archive 자동 백업
// Shared persona §6 버전관리 정책 SSOT 구현
// DS-20 §3.1, DS-60 §3.4

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::models::error::{AppError, AppResult};

/// read_document command 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentContent {
    pub path: String,
    pub content: String,
    pub size_bytes: u64,
}

/// write_latest_document command 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentWriteResult {
    pub path: String,
    pub archive_path: Option<String>,
    pub version_hint: String,
}

/// document:changed event payload (DS-60 §4.1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChanged {
    pub path: String,
    pub version: String,
    pub last_updated: String,
}

/// 문서 트리 노드
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<DocumentNode>,
}

/// list_documents command 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentTree {
    pub workspace_id: String,
    pub root: DocumentNode,
}

/// FileDocumentService — workspace 파일 읽기/쓰기 서비스
pub struct FileDocumentService {
    workspace_root: PathBuf,
}

impl FileDocumentService {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    /// workspace root 하위 경로를 안전하게 resolve한다.
    /// path traversal 방어: canonicalize 후 workspace_root prefix 검증.
    fn resolve_safe_path(&self, rel_path: &str) -> AppResult<PathBuf> {
        let joined = self.workspace_root.join(rel_path);

        // 중간 경로가 없을 수 있으므로 먼저 parent 확인
        let abs = if joined.exists() {
            joined.canonicalize().map_err(|e| {
                AppError::new("PATH_RESOLVE_FAILED", e.to_string())
            })?
        } else {
            // 아직 없는 파일은 parent를 canonicalize
            let parent = joined.parent().ok_or_else(|| {
                AppError::new("INVALID_PATH", "부모 경로가 없습니다")
            })?;
            let canonical_parent = if parent.exists() {
                parent.canonicalize().map_err(|e| AppError::new("PATH_RESOLVE_FAILED", e.to_string()))?
            } else {
                parent.to_path_buf()
            };
            canonical_parent.join(joined.file_name().ok_or_else(|| {
                AppError::new("INVALID_PATH", "파일명이 없습니다")
            })?)
        };

        // workspace root 하위인지 확인
        let canonical_root = self.workspace_root.canonicalize().map_err(|e| {
            AppError::new("PATH_RESOLVE_FAILED", e.to_string())
        })?;
        if !abs.starts_with(&canonical_root) {
            return Err(AppError::new(
                "PATH_TRAVERSAL",
                format!("workspace 외부 경로 접근 금지: {}", rel_path),
            ));
        }
        Ok(abs)
    }

    /// 문서를 읽는다.
    pub fn read_document(&self, rel_path: &str) -> AppResult<DocumentContent> {
        let abs = self.resolve_safe_path(rel_path)?;
        let metadata = std::fs::metadata(&abs).map_err(|e| {
            AppError::new("DOCUMENT_READ_FAILED", e.to_string())
        })?;
        let content = std::fs::read_to_string(&abs).map_err(|e| {
            AppError::new("DOCUMENT_READ_FAILED", e.to_string())
        })?;
        Ok(DocumentContent {
            path: rel_path.to_string(),
            content,
            size_bytes: metadata.len(),
        })
    }

    /// `.latest.md` 파일을 쓴다.
    /// 쓰기 전 `_archive/<이름>_YYYYMMDDhhmmss.md`로 현재 파일을 백업한다.
    /// (Shared persona §6.1 버전관리 정책 SSOT)
    pub fn write_latest_document(
        &self,
        rel_path: &str,
        content: &str,
    ) -> AppResult<DocumentWriteResult> {
        let abs = self.resolve_safe_path(rel_path)?;

        // 기존 파일이 있으면 _archive에 백업
        let archive_path = if abs.exists() {
            Some(self.backup_to_archive(&abs, rel_path)?)
        } else {
            // 부모 디렉토리 생성
            if let Some(parent) = abs.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    AppError::document_write_failed(e.to_string())
                })?;
            }
            None
        };

        // 최신 내용 갱신
        std::fs::write(&abs, content).map_err(|e| {
            AppError::document_write_failed(e.to_string())
        })?;

        let timestamp = Utc::now().format("%Y%m%d%H%M%S").to_string();
        Ok(DocumentWriteResult {
            path: rel_path.to_string(),
            archive_path: archive_path.map(|p| p.to_string_lossy().to_string()),
            version_hint: timestamp,
        })
    }

    /// 현재 파일을 같은 폴더의 `_archive/<이름>_YYYYMMDDhhmmss.<ext>`로 복사한다.
    fn backup_to_archive(&self, abs_path: &Path, rel_path: &str) -> AppResult<PathBuf> {
        let parent = abs_path.parent().ok_or_else(|| {
            AppError::document_write_failed("파일의 부모 디렉토리를 확인할 수 없습니다")
        })?;
        let archive_dir = parent.join("_archive");
        std::fs::create_dir_all(&archive_dir).map_err(|e| {
            AppError::document_write_failed(e.to_string())
        })?;

        let stem = abs_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("document");
        // .latest.md → "name.latest" → 최종 stem은 "name"
        let stem = stem.trim_end_matches(".latest");

        let ext = abs_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("md");

        let timestamp = Utc::now().format("%Y%m%d%H%M%S").to_string();
        let archive_name = format!("{}_{}.{}", stem, timestamp, ext);
        let archive_path = archive_dir.join(&archive_name);

        std::fs::copy(abs_path, &archive_path).map_err(|e| {
            AppError::document_write_failed(format!("_archive 백업 실패: {}", e))
        })?;

        let _ = rel_path; // 사용되지 않지만 signature 일관성 유지
        Ok(archive_path)
    }

    /// workspace 문서 트리를 조회한다 (documents/ 하위).
    pub fn list_documents(&self, workspace_id: &str) -> AppResult<DocumentTree> {
        let docs_path = self.workspace_root.join("documents");
        let root = if docs_path.exists() {
            self.build_tree(&docs_path, &self.workspace_root)?
        } else {
            DocumentNode {
                name: "documents".into(),
                path: "documents".into(),
                is_dir: true,
                children: vec![],
            }
        };
        Ok(DocumentTree {
            workspace_id: workspace_id.to_string(),
            root,
        })
    }

    fn build_tree(&self, abs_dir: &Path, workspace_root: &Path) -> AppResult<DocumentNode> {
        let rel_path = abs_dir
            .strip_prefix(workspace_root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let name = abs_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut children = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(abs_dir)
            .map_err(|e| AppError::new("DIR_READ_FAILED", e.to_string()))?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let entry_path = entry.path();
            let entry_name = entry.file_name().to_string_lossy().to_string();
            // _archive 폴더와 숨김 파일 제외
            if entry_name.starts_with('_') || entry_name.starts_with('.') {
                continue;
            }
            if entry_path.is_dir() {
                children.push(self.build_tree(&entry_path, workspace_root)?);
            } else if entry_name.ends_with(".md") || entry_name.ends_with(".json") {
                let child_rel = entry_path
                    .strip_prefix(workspace_root)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                children.push(DocumentNode {
                    name: entry_name,
                    path: child_rel,
                    is_dir: false,
                    children: vec![],
                });
            }
        }

        Ok(DocumentNode {
            name,
            path: rel_path,
            is_dir: true,
            children,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_service() -> (FileDocumentService, TempDir) {
        let dir = TempDir::new().unwrap();
        let svc = FileDocumentService::new(dir.path());
        (svc, dir)
    }

    #[test]
    fn test_write_and_read_document() {
        let (svc, _dir) = make_service();
        let content = "# Test Document\n\nHello, World!";
        svc.write_latest_document("test.latest.md", content).unwrap();
        let read = svc.read_document("test.latest.md").unwrap();
        assert_eq!(read.content, content);
    }

    #[test]
    fn test_write_creates_archive() {
        let (svc, dir) = make_service();
        let path = "doc.latest.md";

        // 첫 번째 쓰기 — 백업 없음
        svc.write_latest_document(path, "v1").unwrap();
        // 두 번째 쓰기 — _archive 백업 생성
        let result = svc.write_latest_document(path, "v2").unwrap();
        assert!(result.archive_path.is_some());

        let archive_dir = dir.path().join("_archive");
        let entries: Vec<_> = fs::read_dir(&archive_dir).unwrap().collect();
        assert!(!entries.is_empty(), "_archive에 파일이 있어야 함");
    }

    #[test]
    fn test_path_traversal_blocked() {
        let (svc, _dir) = make_service();
        let result = svc.read_document("../../etc/passwd");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "PATH_TRAVERSAL");
    }

    #[test]
    fn test_list_documents_empty() {
        let (svc, _dir) = make_service();
        let tree = svc.list_documents("ws-001").unwrap();
        assert_eq!(tree.workspace_id, "ws-001");
    }
}
