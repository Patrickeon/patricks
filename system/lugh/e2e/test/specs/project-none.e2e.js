// lugh E2E Phase 2 — 시나리오 1: 프로젝트 없음(none) 진입 (레드마인 #27, #28)
// 검증: 프로젝트 미선택 상태로 앱이 /launcher(홈)에 머무는지, config 의존 진입점(/boot)이
//       비활성(가드로 차단)인지. 모두 헤드리스 Linux에서 Tauri 런타임/API키 없이 동작.

import { navigateVia, currentPath, waitForPath } from '../helpers/nav.js';

describe('시나리오1 — 프로젝트 없음(none) 진입 (#27)', () => {
  before(async () => {
    // 이전 spec 세션이 남긴 dev-auto-open 잔여를 제거하고(자동 /boot 진입 방지),
    // first-run 플래그를 세팅해 최초 실행 가이드(/guide) 자동 이동을 차단한다.
    await browser.execute(() => {
      localStorage.removeItem('lugh:dev-auto-open');
      localStorage.setItem('lugh:first-run', '1');
    });
    await browser.refresh();
  });

  it('config 없이 앱이 /launcher(홈)에 머문다 — /boot로 자동 진입하지 않는다', async () => {
    await waitForPath('/launcher', 10000);
    // 라우팅이 안정된 뒤에도 여전히 런처인지 재확인 (자동 리다이렉트 없음 보장)
    await browser.pause(500);
    await expect(await currentPath()).toBe('/launcher');
  });

  it('프로젝트 미선택 상태 배지가 노출된다 (isNone 파생 상태 확인)', async () => {
    const badge = await $('.shell-badge');
    await badge.waitForExist({ timeout: 5000 });
    await expect(badge).toHaveText('프로젝트 미선택 상태');
  });

  it('none 상태에서도 셸 진입점(내장 브라우저·가이드)이 렌더된다', async () => {
    const shell = await $('.shell-section');
    await shell.waitForExist({ timeout: 5000 });
    const shellButtons = await $$('.shell-section .shell-btn');
    await expect(shellButtons).toBeElementsArrayOfSize(2);
  });

  it('config 의존 진입점(/boot)은 비활성 — 직접 이동해도 홈으로 되돌린다 (router guard #27)', async () => {
    await navigateVia('/boot');
    await waitForPath('/launcher', 5000);
    await expect(await currentPath()).toBe('/launcher');
  });
});
