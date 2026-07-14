// lugh E2E 최소 스모크 (Phase 1)
// 시나리오 ① 앱 부팅(Vue 마운트 + Tauri core IPC) ② 워크스페이스 열기·검증
// 관련: 레드마인 #22 조사 보고서 reports/202607140112_lugh_E2E도입조사_v0.1.md §3.4/§3.5 Phase 1
//
// 설계 메모:
// - LauncherView.vue에 이미 존재하는 개발용 훅(localStorage 'lugh:dev-auto-open')을 재사용해
//   네이티브 OS 파일 다이얼로그(WebDriver로 제어 불가) 없이 워크스페이스를 연다. FE 코드 변경 없음.
// - 'lugh:first-run' 플래그를 함께 세팅해 최초 실행 시 /guide로 자동 리다이렉트되는 것을 방지한다.
// - BootView 진입 후에는 실제 boot_team(팀 부팅) IPC까지 진행되지만(약 2.1초 뒤 발화),
//   본 스모크는 그 전에 /boot 라우트 도달 + 파싱된 상태(역할 카운트)만 검증하고 종료한다.
//   CI에는 claude/codex 등 실제 에이전트 CLI가 없어 이후 단계는 실패하지만, 그 실패는
//   본 스모크의 검증 범위 밖이며 세션 종료로 함께 정리된다.

import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const FIXTURE_WORKSPACE = path.resolve(__dirname, '../../fixtures/sample-workspace');

describe('① 앱 부팅 스모크', () => {
  before(async () => {
    // 최초 실행 가이드 리다이렉트를 건너뛰고 런처 화면에서 시작하도록 플래그 설정
    await browser.execute(() => {
      localStorage.setItem('lugh:first-run', '1');
    });
    await browser.refresh();
  });

  it('런처 화면이 정상 렌더링된다 (Vue 마운트 + 라우팅 확인)', async () => {
    const title = await $('.brand-title');
    await title.waitForExist({ timeout: 10000 });
    await expect(title).toHaveText('AgiTeamBuilder');
  });

  it('앱 버전이 표시된다 (Tauri core IPC 브리지 확인, getVersion())', async () => {
    const version = await $('.version');
    await version.waitForExist({ timeout: 10000 });
    await expect(version).toHaveTextContaining('v');
  });
});

describe('② 워크스페이스 열기·검증 스모크', () => {
  before(async () => {
    await browser.execute((fixturePath) => {
      localStorage.setItem('lugh:first-run', '1');
      localStorage.setItem('lugh:dev-auto-open', fixturePath);
    }, FIXTURE_WORKSPACE);
    await browser.refresh();
  });

  it('agiteam.json 로드 후 /boot 화면으로 진입한다 (open_workspace + load_workspace_config IPC)', async () => {
    await browser.waitUntil(
      async () => (await browser.execute(() => window.location.pathname)) === '/boot',
      {
        timeout: 10000,
        timeoutMsg: '/boot 라우트로 진입하지 못했습니다 (워크스페이스 로드 실패 가능성)',
      },
    );

    const bootTitle = await $('.boot-title');
    await bootTitle.waitForExist({ timeout: 5000 });
    await expect(bootTitle).toHaveText('팀 부팅 진행');
  });

  it('워크스페이스 설정이 정상 파싱되었다 (team=[] + PM 1명 → roleStates 반영 확인)', async () => {
    // team: [] 픽스처이므로 PM 1명만 roleStates에 반영된다 → "0/1 READY 수신"
    const readyCounter = await $('.ready-counter');
    await readyCounter.waitForExist({ timeout: 5000 });
    await expect(readyCounter).toHaveTextContaining('0/1');
  });
});
