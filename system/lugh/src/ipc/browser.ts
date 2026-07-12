// src/ipc/browser.ts — 임베디드 브라우저 Tauri IPC 래퍼
// DS-60 §3 IPC 규칙 — 컴포넌트는 invoke를 직접 호출하지 않고 이 모듈을 경유한다. (#18 fix)
// [Redmine #17 독립 창 전환] embedded-browser는 main에 좌표 추종하는 child 창이 아니라
// 독립 최상위 창(decorations/resizable)이다. 위치/크기는 생성 후 OS·사용자가 소유하므로
// FE는 좌표를 계산·동기화하지 않는다. wrapper는 5종(open/navigate/back/forward/close)만
// 유지하고 `resizeBrowser`(구 좌표 동기화용)는 제거한다. (DS-60 §3.8/§7 v0.10, DS-40 §9 v0.8)
// Rust 커맨드: browser_open / browser_navigate / browser_back / browser_forward / browser_close
import { invoke } from '@tauri-apps/api/core'

/** 임베디드 브라우저 독립 창을 연다. 위치/크기는 선택적 초기값이며 생략 시 백엔드 기본값으로 연다. */
export async function openBrowser(url: string): Promise<void> {
  return invoke<void>('browser_open', { url })
}

/** 열린 브라우저에서 새 URL로 이동한다. */
export async function navigateBrowser(url: string): Promise<void> {
  return invoke<void>('browser_navigate', { url })
}

/** 브라우저 히스토리 뒤로 가기. */
export async function browserBack(): Promise<void> {
  return invoke<void>('browser_back')
}

/** 브라우저 히스토리 앞으로 가기. */
export async function browserForward(): Promise<void> {
  return invoke<void>('browser_forward')
}

/** 임베디드 브라우저 창을 닫는다. */
export async function closeBrowser(): Promise<void> {
  return invoke<void>('browser_close')
}
