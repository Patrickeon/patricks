// src/ipc/persona.ts — Persona Bundle Tauri IPC 래퍼
// DS-60 §3 IPC 규칙 — 컴포넌트는 invoke를 직접 호출하지 않고 이 모듈을 경유한다. (#18 fix)
// Rust 커맨드: build_persona_bundle (DS-60 §3.3)
import { invoke } from '@tauri-apps/api/core'
import type { PersonaBundlePreview } from './types'

/**
 * Shared persona + 역할 persona + 부팅 대기 규칙을 결합한 bundle 미리보기를 생성한다.
 * (DS-60 §3.3 — 응답: role / content_hash / content / source_files)
 */
export async function buildPersonaBundle(
  workspaceId: string,
  role: string,
): Promise<PersonaBundlePreview> {
  return invoke<PersonaBundlePreview>('build_persona_bundle', {
    workspaceId,
    role,
  })
}
