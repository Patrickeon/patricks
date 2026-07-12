<!-- 브라우저 패널 — [Redmine #17 독립 창 전환] 임베디드 브라우저는 더 이상 main 콘텐츠 영역에
     좌표 추종하는 child 창이 아니라 독립 최상위 창(타이틀바 있음, OS 드래그로 자유 이동/리사이즈)이다.
     이 패널은 열기/닫기 토글 + 주소창/뒤로/앞으로 등 네비 컨트롤(기존 위치 그대로 유지) + 상태표시로
     축소한다. 위치·크기 동기화(ResizeObserver → browser_resize, rAF reposition, 사이드바 리사이즈
     watch 등)는 전부 제거한다. (DS-60 §3.8/§7 v0.10, DS-40 §9 v0.8) -->
<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
// #18 fix: DS-60 §3 — invoke 직접 호출 금지, ipc 래퍼 경유
import * as browserIpc from '@/ipc/browser'
import { useWorkspaceStore, toSearchUrl } from '@/stores/workspace'

const workspaceStore = useWorkspaceStore()

// ── 열림 상태 / 현재 주소는 workspaceStore가 소유 ────────────
// [lifecycle 확정: 계속 띄워두기 후속] 로컬 ref로 두면 사이드바 탭 전환 시 패널이
// unmount/remount되며 초기화돼 실제 embedded-browser 독립 창 존재 여부와 어긋난다.
// workspaceStore에 상태를 둬서 패널 재마운트/탭 재진입 시에도 실제 창과 일치시킨다.
const isOpen = computed(() => workspaceStore.isBrowserOpen)
const addressValue = computed({
  get: () => workspaceStore.browserAddress,
  set: (v: string) => workspaceStore.setBrowserAddress(v),
})

// ── refs ─────────────────────────────────────────────────────
const isLoading     = ref(false)
const openError     = ref('')

// ── 바로가기 목록 ─────────────────────────────────────────────
const quickLinks = [
  { icon: '📋', label: '레드마인',      url: 'http://211.117.60.5:8080/',  sub: '211.117.60.5:8080' },
  { icon: '🐙', label: 'GitHub',        url: 'https://github.com/',         sub: 'github.com' },
  { icon: '🤖', label: 'Anthropic Docs',url: 'https://docs.anthropic.com/', sub: 'docs.anthropic.com' },
]

// ── URL 정규화 / 검색어 판별 ──────────────────────────────────
// - http(s):// 로 시작 → 그대로 URL
// - 공백 포함 or 도트 없는 일반 텍스트 → 검색 엔진 쿼리
// - 그 외 (예: github.com) → https:// 붙여 URL 취급
function normalizeUrl(raw: string): string {
  if (!raw) return ''
  if (raw === 'about:blank') return raw
  if (/^https?:\/\//i.test(raw)) return raw
  // 공백이 있거나 도트가 없으면 검색어로 판단
  if (/\s/.test(raw) || !raw.includes('.')) return toSearchUrl(raw)
  return `https://${raw}`
}

// ── 브라우저 열기 (독립 창 생성 — 위치/크기는 백엔드 기본값) ──
async function openBrowser(url: string) {
  isLoading.value = true
  openError.value = ''
  try {
    await browserIpc.openBrowser(url)
    workspaceStore.setBrowserOpen(true)
    addressValue.value = url
  } catch (e) {
    openError.value = String(e)
  } finally {
    isLoading.value = false
  }
}

// ── URL 이동 (공통 로직) ──────────────────────────────────────
async function doNavigate(url: string) {
  openError.value = ''
  if (!isOpen.value) {
    await openBrowser(url)
  } else {
    try {
      await browserIpc.navigateBrowser(url)
      addressValue.value = url
    } catch (e) {
      openError.value = String(e)
    }
  }
}

// ── 주소창 Enter / 이동 버튼 ─────────────────────────────────
async function navigate() {
  const url = normalizeUrl(addressValue.value.trim())
  if (!url) return
  await doNavigate(url)
}

// ── 바로가기 클릭 ─────────────────────────────────────────────
async function quickNavigate(url: string) {
  addressValue.value = url
  await doNavigate(url)
}

// ── 뒤로 / 앞으로 ─────────────────────────────────────────────
async function goBack() {
  try { await browserIpc.browserBack() } catch { /* 무시 */ }
}

async function goForward() {
  try { await browserIpc.browserForward() } catch { /* 무시 */ }
}

// ── 브라우저 닫기 → 홈 화면 복귀 ─────────────────────────────
async function closeBrowser() {
  if (!isOpen.value) return
  try { await browserIpc.closeBrowser() } catch { /* 무시 */ }
  workspaceStore.setBrowserOpen(false)
  addressValue.value = ''
  openError.value = ''
}

// ── 라이프사이클 ─────────────────────────────────────────────
let unlistenNavigation: UnlistenFn | null = null
let isUnmounted = false

onMounted(async () => {
  // ① 대기 URL 소비 — openBrowserSearch/openBrowserUrl로 패널이 열린 경우
  const pending = workspaceStore.consumePendingBrowserUrl()
  if (pending) {
    addressValue.value = pending
    doNavigate(pending)
  }

  // ② 웹뷰 내 네비게이션 → 주소창 동기화 (독립 창에서도 on_navigation emit 유지)
  const unlisten = await listen<string>('browser:navigation', (e) => {
    addressValue.value = e.payload
  })
  // listen resolve 전에 unmount된 경우 즉시 해제 (리스너 누수 방지)
  if (isUnmounted) unlisten()
  else unlistenNavigation = unlisten
})

// 패널이 이미 열려 있는 상태에서 openBrowserSearch가 호출된 경우
watch(() => workspaceStore.pendingBrowserUrl, (url) => {
  if (!url) return
  workspaceStore.consumePendingBrowserUrl()
  addressValue.value = url
  doNavigate(url)
})

// [lifecycle 확정: 계속 띄워두기] 독립 창은 패널(사이드바 탭) 언마운트와 수명을 같이하지 않는다.
// 탭을 브라우저 → 다른 탭으로 전환해도 embedded-browser 창은 계속 떠 있어야 하므로
// 언마운트 시 closeBrowser()를 호출하지 않는다. 사용자는 브라우저 창 타이틀바의 닫기 버튼
// 또는 이 패널의 × 버튼(패널이 다시 열렸을 때)으로만 명시적으로 닫는다.
// main 창 완전 종료 시 동반 정리는 Rust 쪽(commands::browser) 책임.
onUnmounted(() => {
  isUnmounted = true
  unlistenNavigation?.()
  unlistenNavigation = null
})
</script>

<template>
  <div class="browser-panel">

    <!-- ── URL 주소창 44px (항상 HTML로 표시, 기존 위치 그대로 유지) ── -->
    <div class="address-bar">
      <button class="nav-btn" title="뒤로" @click="goBack">‹</button>
      <button class="nav-btn" title="앞으로" @click="goForward">›</button>
      <input
        v-model="addressValue"
        class="url-input"
        placeholder="URL 입력 후 Enter"
        @keydown.enter="navigate"
      />
      <button
        class="go-btn"
        :class="{ loading: isLoading }"
        :disabled="isLoading"
        @click="navigate"
      >{{ isLoading ? '…' : '이동' }}</button>
      <button
        v-if="isOpen"
        class="nav-btn close"
        title="브라우저 닫기"
        @click="closeBrowser"
      >×</button>
    </div>

    <!-- 에러 메시지 -->
    <div v-if="openError" class="error-bar">⚠️ {{ openError }}</div>

    <!-- ── 패널 본문: 열기 전 홈 화면 / 로딩 / 열림 상태표시 ── -->
    <div class="browser-body">

      <!-- 브라우저 미열림 상태: 홈 화면 -->
      <div v-if="!isOpen && !isLoading" class="browser-home">
        <div class="home-header">
          <span class="home-icon">🌐</span>
          <p class="home-title">브라우저</p>
          <p class="home-hint">URL을 입력하거나 아래 바로가기를 클릭하세요</p>
        </div>

        <div class="quick-section">
          <p class="section-label">바로가기</p>
          <div class="quick-links">
            <button
              v-for="ql in quickLinks"
              :key="ql.url"
              class="quick-link"
              @click="quickNavigate(ql.url)"
            >
              <span class="ql-icon">{{ ql.icon }}</span>
              <span class="ql-text">{{ ql.label }}</span>
              <span class="ql-url">{{ ql.sub }}</span>
            </button>
          </div>
        </div>
      </div>

      <!-- 브라우저 여는 중 -->
      <div v-if="isLoading" class="loading-state">
        <span class="spinner" />
        <span class="loading-text">브라우저 창 여는 중…</span>
      </div>

      <!-- 브라우저가 별도 창으로 열려 있는 상태표시 -->
      <div v-if="isOpen && !isLoading" class="browser-open-status">
        <span class="status-icon">🟢</span>
        <p class="status-title">브라우저가 별도 창에서 열려 있습니다</p>
        <p class="status-url">{{ addressValue }}</p>
        <p class="status-hint">창을 자유롭게 이동·크기 조절할 수 있습니다. 위 주소창에서 이동하거나 × 버튼으로 닫으세요.</p>
      </div>

    </div>

  </div>
</template>

<style scoped>
.browser-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bg-panel);
}

/* ── 주소창 44px 고정 ── */
.address-bar {
  height: 44px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 0 10px;
  border-bottom: 1px solid var(--line-soft);
  background: var(--bg-panel-header);
}

.nav-btn {
  width: 26px;
  height: 26px;
  border-radius: 5px;
  border: 1px solid var(--line);
  background: var(--bg-panel-2);
  color: var(--text-muted);
  font-size: 16px;
  line-height: 1;
  cursor: pointer;
  display: grid;
  place-items: center;
  flex-shrink: 0;
  transition: all 0.1s;
}
.nav-btn:hover { border-color: var(--accent); color: var(--text-primary); }
.nav-btn.close:hover { border-color: var(--error); color: var(--error); }

.url-input {
  flex: 1;
  height: 28px;
  background: var(--bg-input);
  border: 1px solid var(--line);
  border-radius: 6px;
  padding: 0 10px;
  color: var(--text-primary);
  font-size: 12px;
  font-family: inherit;
  min-width: 0;
}
.url-input:focus { outline: none; border-color: var(--accent); }

.go-btn {
  height: 28px;
  padding: 0 12px;
  border-radius: 6px;
  border: 1px solid var(--accent);
  background: rgba(99,102,241,0.1);
  color: var(--accent);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
  flex-shrink: 0;
  transition: all 0.12s;
}
.go-btn:hover:not(:disabled) { background: rgba(99,102,241,0.2); }
.go-btn.loading { opacity: 0.6; cursor: default; }

/* ── 에러바 ── */
.error-bar {
  padding: 5px 12px;
  background: rgba(239,68,68,0.08);
  border-bottom: 1px solid rgba(239,68,68,0.2);
  color: var(--error);
  font-size: 11px;
  flex-shrink: 0;
}

/* ── 패널 본문 영역 ── */
.browser-body {
  flex: 1;
  overflow: hidden;
  position: relative;
  background: var(--bg-base);
}

/* ── 홈 화면 ── */
.browser-home {
  height: 100%;
  overflow-y: auto;
  padding: 28px 20px;
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.home-header {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding-bottom: 20px;
  border-bottom: 1px solid var(--line-soft);
}

.home-icon  { font-size: 36px; }
.home-title { font-size: 15px; font-weight: 600; color: var(--text-primary); }
.home-hint  { font-size: 12px; color: var(--text-muted); text-align: center; }

.quick-section { display: flex; flex-direction: column; gap: 10px; }

.section-label {
  font-size: 10px;
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.quick-links { display: flex; flex-direction: column; gap: 6px; }

.quick-link {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 8px;
  border: 1px solid var(--line);
  background: var(--bg-panel-2);
  cursor: pointer;
  transition: all 0.12s;
  text-align: left;
}
.quick-link:hover {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.ql-icon { font-size: 18px; flex-shrink: 0; }
.ql-text { font-size: 13px; font-weight: 600; color: var(--text-primary); flex: 1; }
.ql-url  { font-size: 11px; color: var(--text-muted); font-family: monospace; }

/* ── 로딩 상태 ── */
.loading-state {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
}

.spinner {
  width: 24px;
  height: 24px;
  border: 2px solid var(--line);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.loading-text {
  font-size: 13px;
  color: var(--text-muted);
}

/* ── 열림 상태표시 (독립 창으로 열려 있음을 안내) ── */
.browser-open-status {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 24px 20px;
  text-align: center;
}

.status-icon  { font-size: 22px; }
.status-title { font-size: 14px; font-weight: 600; color: var(--text-primary); }
.status-url   { font-size: 11px; color: var(--text-muted); font-family: monospace; word-break: break-all; }
.status-hint  { font-size: 11px; color: var(--text-muted); max-width: 320px; line-height: 1.5; }
</style>
