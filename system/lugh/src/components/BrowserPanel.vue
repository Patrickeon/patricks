<!-- 내장 브라우저 패널 (DS-50 §11) — 시스템 기본 브라우저로 열기 -->
<script setup lang="ts">
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useBrowserStore } from '@/stores/browser'

const browserStore = useBrowserStore()
const addressInput = ref<HTMLInputElement | null>(null)
const openError = ref('')

async function submitAddress() {
  const url = browserStore.addressBarValue.trim()
  if (!url) return
  openError.value = ''
  const normalized = normalizeUrl(url)
  browserStore.navigate(normalized)
  await openInSystemBrowser(normalized)
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter') submitAddress()
}

async function openInSystemBrowser(url: string) {
  try {
    await invoke('plugin:opener|open_url', { url })
  } catch (e) {
    openError.value = String(e)
  }
}

async function quickNavigate(url: string) {
  browserStore.navigate(url)
  await openInSystemBrowser(url)
}

function normalizeUrl(url: string): string {
  if (!url || url === 'about:blank') return url
  if (!/^https?:\/\//i.test(url)) return `http://${url}`
  return url
}

const recentHistory = computed(() =>
  [...browserStore.history]
    .reverse()
    .slice(0, 8)
    .filter((h) => h.url !== 'about:blank')
)
</script>

<template>
  <div class="browser-panel">
    <!-- 주소창 -->
    <div class="address-bar">
      <input
        ref="addressInput"
        v-model="browserStore.addressBarValue"
        class="url-input"
        placeholder="URL 입력 후 Enter 또는 [열기] 클릭"
        @keydown="onKeydown"
      />
      <button class="go-btn" @click="submitAddress">🌐 열기</button>
    </div>

    <!-- 에러 메시지 -->
    <div v-if="openError" class="error-bar">⚠️ {{ openError }}</div>

    <!-- 홈 화면 -->
    <div class="browser-home">
      <div class="home-header">
        <span class="home-icon">🌐</span>
        <p class="home-title">시스템 브라우저로 열기</p>
        <p class="home-hint">URL을 입력하면 기본 브라우저(Safari/Chrome)에서 열립니다</p>
      </div>

      <!-- 바로가기 -->
      <div class="quick-section">
        <p class="section-label">바로가기</p>
        <div class="quick-links">
          <button class="quick-link redmine" @click="quickNavigate('http://211.117.60.5:8080/')">
            <span class="ql-icon">📋</span>
            <span class="ql-text">레드마인</span>
            <span class="ql-url">211.117.60.5:8080</span>
          </button>
          <button class="quick-link docs" @click="quickNavigate('https://docs.anthropic.com/')">
            <span class="ql-icon">🤖</span>
            <span class="ql-text">Anthropic Docs</span>
            <span class="ql-url">docs.anthropic.com</span>
          </button>
          <button class="quick-link gh" @click="quickNavigate('https://github.com/')">
            <span class="ql-icon">🐙</span>
            <span class="ql-text">GitHub</span>
            <span class="ql-url">github.com</span>
          </button>
        </div>
      </div>

      <!-- 최근 방문 -->
      <div v-if="recentHistory.length > 0" class="quick-section">
        <p class="section-label">최근 방문</p>
        <div class="history-list">
          <button
            v-for="entry in recentHistory"
            :key="entry.url + entry.visitedAt"
            class="history-item"
            @click="quickNavigate(entry.url)"
          >
            <span class="hist-icon">🔗</span>
            <span class="hist-url">{{ entry.url }}</span>
          </button>
        </div>
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

.address-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 10px;
  border-bottom: 1px solid var(--line-soft);
  background: var(--bg-panel-header);
  flex-shrink: 0;
}

.url-input {
  flex: 1;
  background: var(--bg-input);
  border: 1px solid var(--line);
  border-radius: 6px;
  padding: 6px 10px;
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
  min-width: 0;
}

.url-input:focus { outline: none; border-color: var(--accent); }

.go-btn {
  height: 32px;
  padding: 0 12px;
  border-radius: 6px;
  border: 1px solid var(--accent);
  background: rgba(99,102,241,0.1);
  color: var(--accent);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.12s;
}

.go-btn:hover { background: rgba(99,102,241,0.2); }

.error-bar {
  padding: 6px 12px;
  background: rgba(239,68,68,0.08);
  border-bottom: 1px solid rgba(239,68,68,0.2);
  color: var(--error);
  font-size: 12px;
}

/* ── 홈 영역 ── */
.browser-home {
  flex: 1;
  overflow-y: auto;
  padding: 24px 20px;
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

.home-icon { font-size: 36px; }
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

/* ── 바로가기 ── */
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

/* ── 방문 기록 ── */
.history-list { display: flex; flex-direction: column; gap: 4px; }

.history-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-radius: 6px;
  border: 1px solid transparent;
  background: none;
  cursor: pointer;
  text-align: left;
  transition: all 0.1s;
}

.history-item:hover {
  background: var(--accent-soft);
  border-color: var(--line);
}

.hist-icon { font-size: 13px; color: var(--text-muted); flex-shrink: 0; }
.hist-url  { font-size: 12px; color: var(--text-soft); font-family: monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
