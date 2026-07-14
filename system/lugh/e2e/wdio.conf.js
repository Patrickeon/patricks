// lugh E2E WebdriverIO 설정 (Tauri v2 공식 "Manual setup" 가이드 기준)
// https://v2.tauri.app/develop/tests/webdriver/example/webdriverio/
//
// Phase 1 스코프: Linux(webkit2gtk) 전용. macOS는 tauri-driver가 WKWebView 드라이버를
// 제공하지 않아 미지원(레드마인 #22 조사 보고서 §3.1 참조) — 로컬 macOS에서는
// onPrepare 빌드 단계까지만 검증 가능하고, 실제 세션 구동은 Linux CI가 주 경로.

import os from 'os';
import path from 'path';
import { spawn, spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const APP_ROOT = path.resolve(__dirname, '..'); // system/lugh

// keep track of the `tauri-driver` child process
let tauriDriver;
let exit = false;

export const config = {
  host: '127.0.0.1',
  port: 4444,
  specs: ['./test/specs/**/*.js'],
  maxInstances: 1,
  capabilities: [
    {
      maxInstances: 1,
      'tauri:options': {
        // 디버그(--no-bundle) 빌드 바이너리. Linux 기준 확장자 없음.
        application: path.resolve(APP_ROOT, 'src-tauri/target/debug/lugh'),
      },
    },
  ],
  reporters: ['spec'],
  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    timeout: 90000,
  },

  // Rust/Tauri 앱을 디버그 + 번들 없이 빌드 (세션 시작 전 바이너리 존재 보장)
  onPrepare: () => {
    const result = spawnSync(
      'pnpm',
      ['run', 'tauri', 'build', '--debug', '--no-bundle'],
      {
        cwd: APP_ROOT,
        stdio: 'inherit',
        shell: true,
      },
    );
    if (result.status !== 0) {
      throw new Error(`tauri build --debug --no-bundle failed (exit code ${result.status})`);
    }
  },

  // 세션 시작 전 tauri-driver를 구동해 WebDriver 요청을 프록시한다
  beforeSession: () => {
    tauriDriver = spawn(
      path.resolve(os.homedir(), '.cargo', 'bin', 'tauri-driver'),
      [],
      { stdio: [null, process.stdout, process.stderr] },
    );

    tauriDriver.on('error', (error) => {
      console.error('tauri-driver error:', error);
      process.exit(1);
    });
    tauriDriver.on('exit', (code) => {
      if (!exit) {
        console.error('tauri-driver exited with code:', code);
        process.exit(1);
      }
    });
  },

  // 세션 종료 후 tauri-driver 프로세스 정리
  afterSession: () => {
    closeTauriDriver();
  },
};

function closeTauriDriver() {
  exit = true;
  tauriDriver?.kill();
}

function onShutdown(fn) {
  const cleanup = () => {
    try {
      fn();
    } finally {
      process.exit();
    }
  };

  process.on('exit', cleanup);
  process.on('SIGINT', cleanup);
  process.on('SIGTERM', cleanup);
  process.on('SIGHUP', cleanup);
  process.on('SIGBREAK', cleanup);
}

// 테스트 프로세스 종료 시 tauri-driver도 함께 종료
onShutdown(() => {
  closeTauriDriver();
});
