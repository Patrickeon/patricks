// src/ipc/web.ts — 웹 콘텐츠 조회 Tauri IPC 래퍼
// DS-60 §3 IPC 규칙 — 컴포넌트는 invoke를 직접 호출하지 않고 이 모듈을 경유한다. (#18 fix)
// Rust 커맨드: fetch_url_content (DS-40 v0.5 / DS-60 v0.6)
import { invoke } from '@tauri-apps/api/core'
import type { FetchedPage } from './types'

/**
 * URL 본문을 가져와 텍스트로 추출한다 (AI 에이전트 웹검색 연동 1단계).
 * - http(s):// 없으면 백엔드가 https:// 자동 추가
 * - 본문은 태그 제거·공백 정리 후 최대 50KB
 * - 오류 코드: INVALID_URL / FETCH_TIMEOUT / FETCH_FAILED
 */
export async function fetchUrlContent(url: string): Promise<FetchedPage> {
  return invoke<FetchedPage>('fetch_url_content', { url })
}
