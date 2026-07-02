<!-- Screen-07: 설정/환경 (DS-50 §10) — 모달 오버레이 방식 -->
<script setup lang="ts">
import { onMounted } from 'vue'
import { useSettingsStore } from '@/stores/settings'
import { useWorkspaceStore } from '@/stores/workspace'
import { invoke } from '@tauri-apps/api/core'
import { runHealthCheck } from '@/ipc/health'
import { validateCredential, saveCredential } from '@/ipc/credential'
import { useProjectStore } from '@/stores/project'
import { useThemeStore } from '@/stores/theme'

const settingsStore = useSettingsStore()
const workspaceStore = useWorkspaceStore()
const projectStore = useProjectStore()
const themeStore = useThemeStore()

onMounted(async () => {
  if (projectStore.projectState) {
    settingsStore.initProjectStateEdit({
      business_type: projectStore.projectState.business_type,
      current_mode: projectStore.projectState.current_mode,
      milestone: projectStore.projectState.milestone,
      wbs_track: projectStore.projectState.wbs_track,
    })
  }
  checkClaudeOAuth()
})

async function testNetwork() {
  settingsStore.isTestingNetwork = true
  try {
    if (projectStore.workspaceId) {
      const report = await runHealthCheck(projectStore.workspaceId)
      settingsStore.applyHealthReport(report)
    }
  } catch {
    settingsStore.setNetworkChecks([
      { endpoint: 'Anthropic API', reachable: false },
      { endpoint: 'OpenAI API', reachable: false },
    ])
  } finally {
    settingsStore.isTestingNetwork = false
  }
}

async function saveProjectState() {
  if (!projectStore.workspaceId) return
  settingsStore.isSaving = true
  try {
    await invoke('write_project_state', {
      workspaceId: projectStore.workspaceId,
      state: settingsStore.projectStateEdit,
    })
  } catch (e) {
    console.error('project_state 저장 실패', e)
  } finally {
    settingsStore.isSaving = false
  }
}

async function reauth(provider: 'claude' | 'codex' | 'gemini') {
  try {
    const credProvider = provider === 'codex' ? 'openai' : provider
    const credAccount = `default-${credProvider}`
    const result = await validateCredential(credProvider, credAccount)
    settingsStore.updateAuthStatus(provider, result.ok ? 'ok' : 'error', result.error)
  } catch {
    settingsStore.updateAuthStatus(provider, 'error', '인증 실패')
  }
}

function stateIcon(state: string): string {
  if (state === 'ok')      return '✅'
  if (state === 'missing') return '⚠️'
  if (state === 'error')   return '❌'
  return '○'
}

const sectionItems = [
  { id: 'agent',   label: '에이전트' },
  { id: 'api',     label: 'API 키' },
  { id: 'state',   label: '프로젝트 상태' },
  { id: 'doctor',  label: '진단' },
  { id: 'theme',   label: '테마' },
]

import { ref } from 'vue'
const activeSection = ref('agent')

// ── Claude OAuth 로그인 감지 ───────────────────────────────
const oauthStatus = ref<'unknown' | 'ok' | 'none'>('unknown')

async function checkClaudeOAuth() {
  try {
    const result = await invoke<{ ok: boolean }>('check_claude_oauth')
    oauthStatus.value = result.ok ? 'ok' : 'none'
  } catch {
    oauthStatus.value = 'none'
  }
}

// API 키 입력 상태 (provider별 임시 입력값)
const apiKeyInputs = ref<Record<string, string>>({
  claude: '',
  codex: '',
  gemini: '',
})

async function saveApiKey(provider: 'claude' | 'codex' | 'gemini') {
  const key = (apiKeyInputs.value[provider] ?? '').trim()
  if (!key) return
  try {
    // codex는 openai credential 사용
    const credProvider = provider === 'codex' ? 'openai' : provider
    const credAccount = `default-${credProvider}`
    await saveCredential(credProvider, credAccount, key)
    settingsStore.updateAuthStatus(provider, 'ok')
    apiKeyInputs.value[provider] = ''  // 저장 후 입력창 초기화
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    settingsStore.updateAuthStatus(provider, 'error', msg)
  }
}
</script>

<template>
  <div class="settings-overlay">
    <!-- 사이드 네비 -->
    <nav class="settings-nav">
      <button
        v-for="item in sectionItems"
        :key="item.id"
        class="nav-item"
        :class="{ active: activeSection === item.id }"
        @click="activeSection = item.id"
      >
        {{ item.label }}
      </button>
    </nav>

    <!-- 본문 -->
    <div class="settings-body">

      <!-- 에이전트 인증 -->
      <div v-if="activeSection === 'agent'" class="section">
        <div
          v-for="auth in settingsStore.authStatuses"
          :key="auth.provider"
          class="auth-card"
        >
          <div class="auth-info">
            <span class="auth-name">{{ auth.label }}</span>
            <span class="auth-state" :class="auth.state">
              {{ stateIcon(auth.state) }}
              {{ auth.state === 'ok' ? '인증됨' : auth.state === 'missing' ? '미인증' : auth.state === 'error' ? '오류' : '확인 중' }}
            </span>
            <span v-if="auth.detail" class="auth-detail">{{ auth.detail }}</span>
            <!-- Claude Code OAuth 로그인 감지 배너 (Claude 카드에만 표시) -->
            <div v-if="auth.provider === 'claude'" class="oauth-banner" :class="oauthStatus">
              <template v-if="oauthStatus === 'ok'">
                <span class="oauth-icon">✅</span>
                <span>Claude Code 로그인 감지됨 — API 키 없이 사용 가능</span>
                <button class="btn-refresh" @click="checkClaudeOAuth">새로고침</button>
              </template>
              <template v-else-if="oauthStatus === 'none'">
                <span class="oauth-icon">⚠️</span>
                <span>Claude Code 미로그인 — 아래에서 API 키를 입력하거나 <code>claude</code> CLI로 로그인하세요</span>
                <button class="btn-refresh" @click="checkClaudeOAuth">재확인</button>
              </template>
              <template v-else>
                <span class="oauth-icon">○</span>
                <span>Claude Code 로그인 상태 확인 중...</span>
              </template>
            </div>
            <!-- API 키 입력 (claude / codex / gemini) -->
            <div v-if="['claude','codex','gemini'].includes(auth.provider)" class="api-key-inline">
              <input
                v-model="apiKeyInputs[auth.provider]"
                type="password"
                class="input key-input"
                :placeholder="auth.state === 'ok' ? '새 API 키 (변경 시)' : 'API 키 입력'"
                @keydown.enter.prevent="saveApiKey(auth.provider as 'claude'|'codex'|'gemini')"
              />
              <button
                class="sm-btn primary"
                :disabled="!apiKeyInputs[auth.provider]?.trim()"
                @click="saveApiKey(auth.provider as 'claude'|'codex'|'gemini')"
              >저장</button>
            </div>
          </div>
          <button class="sm-btn" @click="reauth(auth.provider)">
            {{ auth.state === 'ok' ? '재인증' : '인증하기' }}
          </button>
        </div>

        <div class="network-section">
          <div class="section-label">API 연결 테스트</div>
          <div class="network-checks">
            <div
              v-for="check in settingsStore.networkChecks"
              :key="check.endpoint"
              class="network-item"
            >
              <span>{{ check.reachable ? '✅' : '❌' }}</span>
              <span>{{ check.endpoint }}</span>
              <span v-if="check.latency_ms" class="muted">{{ check.latency_ms }}ms</span>
            </div>
          </div>
          <button class="sm-btn" :disabled="settingsStore.isTestingNetwork" @click="testNetwork">
            {{ settingsStore.isTestingNetwork ? '테스트 중…' : '테스트' }}
          </button>
        </div>
      </div>

      <!-- API 키 관리 -->
      <div v-if="activeSection === 'api'" class="section">
        <div class="field">
          <label class="field-label">레드마인 서버 URL</label>
          <input
            :value="settingsStore.redmineUrl"
            class="input"
            @input="settingsStore.setRedmineUrl(($event.target as HTMLInputElement).value)"
          />
        </div>
        <div class="field">
          <label class="field-label">레드마인 프로젝트 ID</label>
          <input
            :value="settingsStore.redmineProjectId"
            class="input"
            placeholder="예: lugh"
            @input="settingsStore.setRedmineProjectId(($event.target as HTMLInputElement).value)"
          />
          <span class="field-hint">이슈 필터링에 사용됩니다. 미입력 시 전체 이슈가 표시됩니다.</span>
        </div>
        <div class="section-label" style="margin-top: 14px;">역할별 레드마인 API 키</div>
        <p class="muted-text" style="margin: -8px 0 4px;">입력 후 [저장] 버튼을 누르세요. 키는 OS Keychain에만 저장됩니다.</p>
        <div class="api-key-list">
          <div
            v-for="role in ['PM', 'DeveloperBE', 'DeveloperFE', 'QA', 'Architect', 'DevOps', 'Designer']"
            :key="role"
            class="api-key-row"
          >
            <span class="api-key-role">
              {{ role }}
              <span v-if="settingsStore.redmineApiKeyStored[role]" class="stored-badge">✓</span>
            </span>
            <div class="api-key-input-wrap">
              <input
                type="password"
                class="input"
                :placeholder="settingsStore.redmineApiKeyStored[role] ? '●●●●●●●● (저장됨)' : 'API 키 입력'"
                @blur="(e) => {
                  const val = (e.target as HTMLInputElement).value.trim()
                  if (val) settingsStore.saveRedmineApiKey(role, val)
                }"
              />
            </div>
          </div>
        </div>
      </div>

      <!-- 프로젝트 상태 -->
      <div v-if="activeSection === 'state'" class="section">
        <p class="muted-text">project_state.yaml 편집</p>
        <div class="field">
          <label class="field-label">business_type</label>
          <input v-model="settingsStore.projectStateEdit.business_type" class="input" />
        </div>
        <div class="field">
          <label class="field-label">current_mode</label>
          <select v-model="settingsStore.projectStateEdit.current_mode" class="select">
            <option value="project">project</option>
            <option value="operation">operation</option>
          </select>
        </div>
        <div class="field">
          <label class="field-label">milestone</label>
          <input v-model="settingsStore.projectStateEdit.milestone" class="input" />
        </div>
        <div class="field">
          <label class="field-label">wbs_track</label>
          <input v-model="settingsStore.projectStateEdit.wbs_track" class="input" placeholder="A or B" />
        </div>
        <button class="sm-btn primary" :disabled="settingsStore.isSaving" @click="saveProjectState">
          {{ settingsStore.isSaving ? '저장 중…' : '저장' }}
        </button>
      </div>

      <!-- 진단 -->
      <div v-if="activeSection === 'doctor'" class="section">
        <p class="muted-text">필수 도구 설치 확인</p>
        <div v-if="settingsStore.toolChecks.length === 0" class="empty-state">
          <button class="sm-btn" @click="testNetwork">진단 실행</button>
        </div>
        <div v-else class="tool-list">
          <div
            v-for="tool in settingsStore.toolChecks"
            :key="tool.name"
            class="tool-item"
          >
            <span>{{ tool.available ? '✅' : '❌' }}</span>
            <span class="tool-name">{{ tool.name }}</span>
            <span v-if="tool.version" class="muted">{{ tool.version }}</span>
          </div>
        </div>
      </div>

      <!-- 테마 -->
      <div v-if="activeSection === 'theme'" class="section">
        <div class="section-label">테마</div>
        <div class="theme-cards">
          <button
            class="theme-card" :class="{ active: themeStore.theme === 'dark' }"
            @click="themeStore.setTheme('dark')">
            <span class="theme-icon">🌙</span>
            <span>어두운 테마</span>
            <span v-if="themeStore.theme === 'dark'" class="check">✓</span>
          </button>
          <button
            class="theme-card" :class="{ active: themeStore.theme === 'light' }"
            @click="themeStore.setTheme('light')">
            <span class="theme-icon">☀️</span>
            <span>밝은 테마</span>
            <span v-if="themeStore.theme === 'light'" class="check">✓</span>
          </button>
        </div>
      </div>

    </div>
  </div>
</template>

<style scoped>
.settings-overlay {
  display: flex;
  height: 100%;
  overflow: hidden;
}

.settings-nav {
  width: 110px;
  flex-shrink: 0;
  padding: 14px 0;
  border-right: 1px solid var(--line-soft);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nav-item {
  text-align: left;
  padding: 8px 14px;
  background: none;
  border: none;
  color: var(--text-muted);
  font-size: 13px;
  cursor: pointer;
  border-left: 2px solid transparent;
}

.nav-item:hover { color: var(--text-primary); }
.nav-item.active {
  color: var(--text-primary);
  border-left-color: var(--accent);
  background: rgba(99,102,241,0.07);
}

.settings-body {
  flex: 1;
  padding: 20px;
  overflow-y: auto;
}

.section { display: flex; flex-direction: column; gap: 14px; max-width: 400px; }
.section-label { font-size: 11px; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.06em; font-weight: 600; }

.auth-card {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px;
  background: var(--bg-panel-2);
  border: 1px solid var(--line-soft);
  border-radius: 7px;
  gap: 10px;
}

.auth-info { display: flex; flex-direction: column; gap: 3px; flex: 1; }
.auth-name { font-size: 13px; font-weight: 600; color: var(--text-primary); }
.auth-state { font-size: 12px; display: flex; align-items: center; gap: 5px; }
.auth-state.ok      { color: var(--ok); }
.auth-state.error   { color: var(--error); }
.auth-state.missing { color: var(--busy); }
.auth-state.unknown { color: var(--text-muted); }
.auth-detail { font-size: 11px; color: var(--text-muted); }

.network-section { padding: 12px; background: var(--bg-panel-2); border: 1px solid var(--line-soft); border-radius: 7px; display: flex; flex-direction: column; gap: 8px; }
.network-checks { display: flex; flex-direction: column; gap: 5px; }
.network-item { display: flex; align-items: center; gap: 8px; font-size: 13px; }
.muted { color: var(--text-muted); font-size: 11px; }

.field { display: flex; flex-direction: column; gap: 6px; }
.field-label { font-size: 12px; color: var(--text-muted); }
.field-hint  { font-size: 11px; color: var(--text-muted); }

.input, .select {
  background: var(--bg-input);
  border: 1px solid var(--line);
  border-radius: 6px;
  padding: 7px 10px;
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
}

.input:focus, .select:focus { outline: none; border-color: var(--accent); }

.api-key-inline { display: flex; gap: 6px; align-items: center; margin-top: 6px; }
.key-input { flex: 1; min-width: 0; }

.oauth-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  border-radius: 7px;
  font-size: 13px;
  margin-bottom: 12px;
  border: 1px solid var(--line-soft);
  background: var(--bg-input);
}
.oauth-banner.ok   { border-color: rgba(34,197,94,0.4);  background: rgba(34,197,94,0.06);  color: #86efac; }
.oauth-banner.none { border-color: rgba(251,191,36,0.4); background: rgba(251,191,36,0.05); color: #fde68a; }
.oauth-icon { font-size: 15px; flex-shrink: 0; }
.btn-refresh {
  margin-left: auto;
  font-size: 11px;
  padding: 3px 8px;
  border-radius: 5px;
  border: 1px solid var(--line);
  background: var(--bg-panel-2);
  color: var(--text-muted);
  cursor: pointer;
}

.api-key-list { display: flex; flex-direction: column; gap: 6px; }
.api-key-row { display: grid; grid-template-columns: 110px 1fr; gap: 8px; align-items: center; }
.api-key-role { font-size: 12px; color: var(--text-muted); display: flex; align-items: center; gap: 4px; }
.stored-badge { font-size: 10px; color: var(--ok); font-weight: 700; }
.api-key-input-wrap { display: flex; gap: 6px; }

.sm-btn {
  height: 30px;
  padding: 0 12px;
  border-radius: 5px;
  border: 1px solid var(--line);
  background: var(--bg-panel-2);
  color: var(--text-primary);
  font-size: 12px;
  cursor: pointer;
  align-self: flex-start;
}

.sm-btn:disabled { opacity: 0.4; cursor: not-allowed; }
.sm-btn.primary { background: rgba(99,102,241,0.12); border-color: rgba(99,102,241,0.4); color: var(--accent); }

.muted-text { font-size: 12px; color: var(--text-muted); }
.empty-state { display: flex; justify-content: center; padding: 20px 0; }
.tool-list { display: flex; flex-direction: column; gap: 5px; }
.tool-item { display: flex; align-items: center; gap: 8px; font-size: 13px; }
.tool-name { color: var(--text-primary); }

.theme-cards { display: flex; gap: 12px; }
.theme-card {
  display: flex; flex-direction: column; align-items: center; gap: 8px;
  padding: 20px 32px; border-radius: 12px;
  border: 2px solid var(--line); background: var(--bg-panel-2); color: var(--text-primary);
  cursor: pointer; transition: border-color 0.2s;
}
.theme-card.active { border-color: var(--accent); }
.theme-icon { font-size: 28px; }
.check { color: var(--accent); font-weight: 700; }
</style>
