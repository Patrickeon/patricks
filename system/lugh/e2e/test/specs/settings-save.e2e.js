// lugh E2E Phase 2 — 시나리오 2: 설정 저장 흐름 (레드마인 #27, #28)
// 검증: 새 프로젝트(mode=new) → 워크스페이스 폴더 지정 → 저장 → /boot 진입.
//       폴더 미설정 시 저장 비활성(needsWorkspacePath 가드)까지 확인.
//
// 설계 메모:
// - 네이티브 폴더 선택 다이얼로그(WebDriver 제어 불가)는 클릭하지 않고, v-model 바인딩된
//   .folder-input에 경로를 직접 입력한다(FE 코드 변경 없음).
// - 저장 대상은 커밋된 fixture를 오염시키지 않도록 테스트 시점에 OS 임시 디렉토리를 새로
//   만들어 사용하고, after 훅에서 정리한다. 임시 폴더는 앱(Rust 백엔드)과 같은 파일시스템에
//   있으므로 openWorkspace/saveWorkspaceConfig IPC가 정상 접근한다.

import fs from 'fs';
import os from 'os';
import path from 'path';
import { navigateVia, currentPath, waitForPath } from '../helpers/nav.js';

describe('시나리오2 — 설정 저장 흐름 (신규 프로젝트, #27)', () => {
  let tmpDir;

  before(async () => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'lugh-e2e-save-'));
    await browser.execute(() => {
      localStorage.removeItem('lugh:dev-auto-open');
      localStorage.setItem('lugh:first-run', '1');
    });
    await browser.refresh();
    // 다이얼로그를 거치지 않고 신규 작성 설정 화면으로 직접 진입
    await navigateVia('/settings?mode=new');
    await waitForPath('/settings', 8000);
  });

  after(() => {
    try {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    } catch {
      /* best-effort 정리 */
    }
  });

  it('워크스페이스 폴더 미설정 시 저장 버튼이 비활성이다 (needsWorkspacePath 가드)', async () => {
    const saveBtn = await $('.settings-footer .btn-ghost');
    await saveBtn.waitForExist({ timeout: 5000 });
    await expect(saveBtn).toBeDisabled();
  });

  it('폴더 경로 입력 시 저장 버튼이 활성화된다', async () => {
    const folderInput = await $('.folder-input');
    await folderInput.waitForExist({ timeout: 5000 });
    await folderInput.setValue(tmpDir);

    const saveBtn = await $('.settings-footer .btn-ghost');
    await browser.waitUntil(async () => saveBtn.isEnabled(), {
      timeout: 5000,
      timeoutMsg: '폴더 경로 입력 후에도 저장 버튼이 활성화되지 않았습니다',
    });
    await expect(saveBtn).toBeEnabled();
  });

  it('팀 부팅 시작 → 저장 후 /boot로 진입하고 agiteam.json이 디스크에 생성된다', async () => {
    const bootBtn = await $('.settings-footer .btn-primary');
    await bootBtn.click();

    // saveThenBoot: openWorkspace + saveWorkspaceConfig(디스크 쓰기) + load → router.push('/boot')
    await waitForPath('/boot', 15000);

    const bootTitle = await $('.boot-title');
    await bootTitle.waitForExist({ timeout: 5000 });
    await expect(bootTitle).toHaveText('팀 부팅 진행');

    // IPC가 실제로 파일을 썼는지 파일시스템 레벨에서 검증 (단순 UI 상태가 아님)
    await expect(fs.existsSync(path.join(tmpDir, 'agiteam.json'))).toBe(true);
  });
});
