<!-- 내장 브라우저 패널 — WebviewWindow 오버레이 방식 (DS-50 §11) -->
<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

// ── refs ─────────────────────────────────────────────────────
const webviewRef    = ref<HTMLElement | null>(null)
const addressValue  = ref('')
const isOpen        = ref(false)
const isLoading     = ref(false)
const openError     = ref('')

// ── 바로가기 목록 ─────────────────────────────────────────────
const quickLinks = [
  { icon: '📋', label: '레드마인',      url: 'http://211.117.60.5:8080/',  sub: '211.117.60.5:8080' },
  { icon: '🐙', label: 'GitHub',        url: 'https://github.com/',         sub: 'github.com' },
  { icon: '🤖', label: 'Anthropic Docs',url: 'https://docs.anthropic.com/', sub: 'docs.anthropic.com' },
]

// ── URL 정규화 ────────────────────────────────────────────────
function normalizeUrl(raw: string): string {
  if (!raw) return ''
  if (raw === 'about:blank') return raw
  if (!/^https?:\/\//i.test(raw)) return `https://${raw}`
  return raw
}

// ── WebviewWindow 위치·크기 계산 ──────────────────────────────
function getRect(): { x: number; y: number; width: number; height: number } | null {
  if (!webviewRef.value) return null
  const r = webviewRef.value.getBoundingClientRect()
  if (r.width < 1 || r.height < 1) return null
  return {
    x:      Math.round(r.left),
    y:      Math.round(r.top),
    width:  Math.round(r.width),
    height: Math.round(r.height),
  }
}

// ── 브라우저 열기 ─────────────────────────────────────────────
async function openBrowser(url: string) {
  const rect = getRect()
  if (!rect) return
  isLoading.value = true
  openError.value = ''
  try {
    await invoke('browser_open', { url, ...rect })
    isOpen.value = true
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
      await invoke('browser_navigate', { url })
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
  try { await invoke('browser_back') } catch { /* 커맨드 미구현 시 무시 */ }
}

async function goForward() {
  try { await invoke('browser_forward') } catch { /* 커맨드 미구현 시 무시 */ }
}

// ── 브라우저 닫기 → 홈 화면 복귀 ─────────────────────────────
async function closeBrowser() {
  if (!isOpen.value) return
  try { await invoke('browser_close') } catch { /* 무시 */ }
  isOpen.value = false
  addressValue.value = ''
  openError.value = ''
}

// ── 브라우저 리포지션 ─────────────────────────────────────────
async function repositionBrowser() {
  if (!isOpen.value) return
  const rect = getRect()
  if (!rect) return
  try {
    await invoke('browser_resize', rect)
  } catch { /* 무시 */ }
}

// 에러바가 나타나면 webviewRef의 y가 바뀌므로 reposition
watch(openError, () => {
  requestAnimationFrame(() => repositionBrowser())
})

// ── 라이프사이클 ─────────────────────────────────────────────
let resizeObserver: ResizeObserver | null = null
let unlistenNavigation: UnlistenFn | null = null
let isUnmounted = false

onMounted(async () => {
  // ① webview-placeholder 크기 변화 감지
  resizeObserver = new ResizeObserver(() => repositionBrowser())
  if (webviewRef.value) resizeObserver.observe(webviewRef.value)

  // ② 윈도우 리사이즈 감지
  window.addEventListener('resize', repositionBrowser)

  // ③ 웹뷰 내 네비게이션 → 주소창 동기화 (박개발 emit 연동)
  const unlisten = await listen<string>('browser:navigation', (e) => {
    addressValue.value = e.payload
  })
  // listen resolve 전에 unmount된 경우 즉시 해제 (리스너 누수 방지)
  if (isUnmounted) unlisten()
  else unlistenNavigation = unlisten
})

onUnmounted(async () => {
  isUnmounted = true
  resizeObserver?.disconnect()
  window.removeEventListener('resize', repositionBrowser)
  unlistenNavigation?.()
  unlistenNavigation = null
  if (isOpen.value) {
    try { await invoke('browser_close') } catch { /* 무시 */ }
    isOpen.value = false
  }
})
</script>

<template>
  <div class="browser-panel">

    <!-- ── URL 주소창 44px (항상 HTML로 표시) ── -->
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
        title="브라우저 닫기 (홈으로)"
        @click="closeBrowser"
      >×</button>
    </div>

    <!-- 에러 메시지 (나타나면 webview y 좌표 자동 갱신됨) -->
    <div v-if="openError" class="error-bar">⚠️ {{ openError }}</div>

    <!-- ── WebviewWindow이 덮는 영역 ── -->
    <div ref="webviewRef" class="webview-placeholder">

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

      <!-- 브라우저 로딩 중 -->
      <div v-if="isLoading" class="loading-state">
        <span class="spinner" />
        <span class="loading-text">브라우저 열리는 중…</span>
      </div>

      <!-- 브라우저가 열리면 WebviewWindow가 이 영역을 완전히 덮음 -->
      <!-- (시각적으로 투명하게 유지) -->
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

/* ── WebviewWindow 오버레이 영역 ── */
.webview-placeholder {
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
</style>
