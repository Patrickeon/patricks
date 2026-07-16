// lugh E2E Phase 2 — 시나리오 3: 대화 초기화 UI (레드마인 #24, #28)
//
// [스킵 사유 — 헤드리스 CI 도달 불가]
// 대화 초기화 버튼(🗑)과 확인 다이얼로그는 PmChatPanel.vue / RoleChatPanel.vue 에 있으며,
// 이 컴포넌트들은 WorkspaceView 내부 — 즉 workspaceStore.isActive === true(팀 부팅 완료)
// 상태에서만 렌더된다. /workspace 진입 라우터 가드도 isActive를 요구한다.
//
// 팀 부팅(bootTeam / boot_team IPC)은 claude·codex 등 실제 에이전트 CLI 프로세스를 spawn해
// READY 신호를 수신해야 완료되는데, 헤드리스 Linux CI에는 해당 CLI 바이너리가 없어
// 워크스페이스에 도달할 수 없다. 이는 API 키 문제가 아니라 '부팅 게이트' 문제다.
// (PM 지시 원칙: Tauri 런타임+실제 AI 전송이 필요한 부분은 제외, 불가 시나리오는 사유 명시 후 스킵)
//
// → Phase 2 범위에서는 스킵으로 남겨 CI 리포트에 가시화한다.
//    향후 bootTeam을 목(mock)하거나 workspaceStore를 강제 활성화하는 별도 테스트 하네스가
//    준비되면(FE 협업 필요) 이 시나리오를 정식 커버할 예정이다.

describe('시나리오3 — 대화 초기화 UI (#24)', () => {
  it.skip('초기화 버튼 → 확인 다이얼로그 → 취소/초기화 흐름 [워크스페이스 진입 필요 — 헤드리스 CI 도달 불가, 사유는 파일 상단 주석 참조]', () => {
    // 의도적으로 미구현 — 위 스킵 사유대로 실제 팀 부팅이 선행돼야 도달 가능한 화면.
  });
});
