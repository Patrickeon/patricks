// src/ipc/redmine.ts — Redmine Tauri IPC 래퍼
// DV60-003: DS-60 §3 IPC 규칙 — store는 invoke를 직접 호출하지 않고 이 모듈을 경유한다.
// #15: role 파라미터 — 역할별 API 키(api_key_<role>) 선택. null 시 단일 키(api_key) fallback.
// Rust 커맨드: redmine_list_issues / redmine_get_issue / redmine_create_issue / redmine_update_issue
import { invoke } from '@tauri-apps/api/core'
import type { RedmineIssueItem } from './types'

/**
 * GET /issues.json — 이슈 목록 조회
 * @param workspaceId  현재 워크스페이스 ID (Rust _workspace_id)
 * @param projectId    프로젝트 ID 필터 (null 시 전체)
 * @param statusId     'open' | 'all' | '<숫자>' (null 시 'open' 폴백)
 * @param role         역할명 (예: 'PM') — 역할별 API 키 선택. null 시 단일 키 fallback
 */
export async function listRedmineIssues(
  workspaceId: string,
  projectId?: string | null,
  statusId?: string | null,
  role?: string,
): Promise<RedmineIssueItem[]> {
  return invoke<RedmineIssueItem[]>('redmine_list_issues', {
    workspaceId,
    projectId: projectId ?? null,
    statusId: statusId ?? null,
    role: role ?? null,
  })
}

/**
 * GET /issues/<id>.json — 이슈 단건 조회
 */
export async function getRedmineIssue(
  workspaceId: string,
  issueId: number,
  role?: string,
): Promise<RedmineIssueItem> {
  return invoke<RedmineIssueItem>('redmine_get_issue', {
    workspaceId,
    issueId,
    role: role ?? null,
  })
}

/**
 * POST /issues.json — 이슈 생성
 */
export async function createRedmineIssue(
  workspaceId: string,
  projectId: string,
  trackerId: number,
  subject: string,
  description?: string | null,
  assignedToId?: number | null,
  role?: string,
): Promise<RedmineIssueItem> {
  return invoke<RedmineIssueItem>('redmine_create_issue', {
    workspaceId,
    projectId,
    trackerId,
    subject,
    description: description ?? null,
    assignedToId: assignedToId ?? null,
    role: role ?? null,
  })
}

/**
 * PUT /issues/<id>.json — 이슈 갱신
 * Redmine은 성공 시 204 No Content 반환 → void
 */
export async function updateRedmineIssue(
  workspaceId: string,
  issueId: number,
  statusId?: number | null,
  doneRatio?: number | null,
  notes?: string | null,
  role?: string,
): Promise<void> {
  return invoke<void>('redmine_update_issue', {
    workspaceId,
    issueId,
    statusId: statusId ?? null,
    doneRatio: doneRatio ?? null,
    notes: notes ?? null,
    role: role ?? null,
  })
}
