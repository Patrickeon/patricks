// src/ipc/opener.ts — 외부 링크 열기 (Redmine #30)
// DS-60 §3 IPC 규칙: 컴포넌트는 플러그인 API를 직접 호출하지 않고 이 모듈을 경유한다.
//
// AI 응답 등 신뢰할 수 없는 콘텐츠의 링크는 앱 웹뷰가 아니라 OS 기본 브라우저로 연다.
// 이렇게 하면 링크가 앱 컨텍스트(Tauri IPC) 밖에서 완전히 격리되어 가장 안전하다.
// capability: `opener:default`(src-tauri/capabilities/default.json)로 이미 허용됨.
import { openUrl } from '@tauri-apps/plugin-opener'

/** URL을 OS 기본 브라우저로 연다. */
export async function openExternalUrl(url: string): Promise<void> {
  return openUrl(url)
}
