// E2E 공용 네비게이션 헬퍼
// lugh는 vue-router createWebHistory(히스토리 모드)를 쓰고, 라우터 인스턴스를 window에
// 노출하지 않는다. 네이티브 파일 다이얼로그(WebDriver 제어 불가)를 거치지 않고 특정
// 라우트로 진입하기 위해, History API pushState + popstate 이벤트로 vue-router의
// 네비게이션을 유도한다(FE 코드 변경 불필요). 스모크에서 검증됐듯 이 앱의
// location.pathname은 루트 상대 경로('/boot' 등)로 깨끗하게 노출된다.
//
// WDIO는 browser/$/expect 등을 글로벌로 주입하므로 임포트된 헬퍼에서도 browser 사용 가능.

export async function navigateVia(urlWithQuery) {
  await browser.execute((url) => {
    history.pushState(history.state, '', url);
    window.dispatchEvent(new PopStateEvent('popstate', { state: history.state }));
  }, urlWithQuery);
}

export async function currentPath() {
  return browser.execute(() => window.location.pathname);
}

export async function waitForPath(expected, timeout = 10000) {
  await browser.waitUntil(
    async () => (await currentPath()) === expected,
    { timeout, timeoutMsg: `라우트가 '${expected}'로 전이되지 않았습니다 (현재: ${await currentPath()})` },
  );
}
