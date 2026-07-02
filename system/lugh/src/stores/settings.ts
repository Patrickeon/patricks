// useSettingsStore — 로그인 상태, API 키, project_state.yaml 편집
// DV60-004: redmineApiKeys 평문 저장 제거 → OS Keychain (NFR-0301)
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { saveCredential, getMaskedCredential } from '@/ipc/credential'
import type { ProviderHealth, ToolCheck, NetworkCheck } from '@/ipc/types'

export type AuthState = 'unknown' | 'ok' | 'missing' | 'error'

export type ProviderAuthStatus = {
  provider: 'claude' | 'codex' | 'gemini'
  label: string
  state: AuthState
  detail?: string
}

export const useSettingsStore = defineStore('settings', () => {
  // ── 에이전트 인증 상태 ─────────────────────────────────────
  const authStatuses = ref<ProviderAuthStatus[]>([
    { provider: 'claude', label: 'Claude Code', state: 'unknown' },
    { provider: 'codex',  label: 'Codex',       state: 'unknown' },
    { provider: 'gemini', label: 'Gemini CLI',   state: 'unknown' },
  ])

  // ── API 연결 상태 ─────────────────────────────────────────
  const networkChecks = ref<NetworkCheck[]>([
    { endpoint: 'Anthropic API', reachable: false },
    { endpoint: 'OpenAI API',    reachable: false },
  ])

  // ── 도구 진단 ─────────────────────────────────────────────
  const toolChecks = ref<ToolCheck[]>([])

  // ── 레드마인 설정 (DV60-004: 키 저장 상태만 추적, 평문 저장 없음) ──
  const REDMINE_URL_KEY        = 'lugh:redmine_url'
  const REDMINE_PROJECT_ID_KEY = 'lugh:redmine_project_id'

  const redmineUrl = ref<string>(
    localStorage.getItem(REDMINE_URL_KEY) ?? 'http://211.117.60.5:8080/',
  )
  /** 레드마인 프로젝트 ID (예: 'lugh'). 미설정 시 빈 문자열. */
  const redmineProjectId = ref<string>(
    localStorage.getItem(REDMINE_PROJECT_ID_KEY) ?? '',
  )
  /** role → OS Keychain에 저장됐는지 여부 (true/false). 평문 값은 보관하지 않는다. */
  const redmineApiKeyStored = ref<Record<string, boolean>>({})

  // ── project_state.yaml 편집 ───────────────────────────────
  const projectStateEdit = ref({
    business_type: '',
    current_mode: 'project' as 'project' | 'operation',
    milestone: '',
    wbs_track: '',
  })

  const isTestingNetwork = ref(false)
  const isSaving = ref(false)

  // ── 액션 ──────────────────────────────────────────────────
  function applyHealthReport(report: {
    providers: ProviderHealth[]
    tools: ToolCheck[]
    network: NetworkCheck[]
  }) {
    for (const p of report.providers) {
      const auth = authStatuses.value.find(
        (a) => a.provider === (p.provider as 'claude' | 'codex' | 'gemini'),
      )
      if (auth) {
        auth.state = p.ok ? 'ok' : 'error'
        auth.detail = p.error
      }
    }
    toolChecks.value = report.tools
    networkChecks.value = report.network
  }

  function updateAuthStatus(
    provider: 'claude' | 'codex' | 'gemini',
    state: AuthState,
    detail?: string,
  ) {
    const auth = authStatuses.value.find((a) => a.provider === provider)
    if (auth) {
      auth.state = state
      auth.detail = detail
    }
  }

  function setNetworkChecks(checks: NetworkCheck[]) {
    networkChecks.value = checks
  }

  function setRedmineUrl(url: string) {
    redmineUrl.value = url
    // localStorage에 persist (재시작 후에도 유지)
    try { localStorage.setItem(REDMINE_URL_KEY, url) } catch { /* ignore */ }
    // Rust backend가 읽을 수 있도록 Keychain에도 저장 (fire-and-forget)
    saveCredential('redmine', 'url', url).catch(() => { /* ignore */ })
  }

  function setRedmineProjectId(id: string) {
    redmineProjectId.value = id.trim()
    try { localStorage.setItem(REDMINE_PROJECT_ID_KEY, id.trim()) } catch { /* ignore */ }
  }

  /**
   * 역할별 Redmine API 키를 OS Keychain에 저장한다. (DV60-004, #15 fix)
   * account 이름을 `api_key_${role}` 로 분리하여 역할별로 독립 저장한다.
   * Pinia 상태에 평문 저장하지 않고, 저장 여부(boolean)만 기록한다.
   */
  async function saveRedmineApiKey(role: string, key: string): Promise<void> {
    if (!key.trim()) return
    try {
      // 역할별 account 분리: 'api_key_PM', 'api_key_DeveloperBE', 'api_key_QA' …
      await saveCredential('redmine', `api_key_${role}`, key)
      redmineApiKeyStored.value[role] = true
    } catch {
      redmineApiKeyStored.value[role] = false
    }
  }

  /**
   * OS Keychain에 역할별 API 키가 저장돼 있는지 확인한다. (#15 fix)
   */
  async function loadRedmineApiKeyStatus(role: string): Promise<void> {
    try {
      const masked = await getMaskedCredential('redmine', `api_key_${role}`)
      redmineApiKeyStored.value[role] = masked.has_secret
    } catch {
      redmineApiKeyStored.value[role] = false
    }
  }

  function initProjectStateEdit(state: {
    business_type: string
    current_mode: 'project' | 'operation'
    milestone: string
    wbs_track: string
  }) {
    projectStateEdit.value = { ...state }
  }

  return {
    authStatuses,
    networkChecks,
    toolChecks,
    redmineUrl,
    redmineProjectId,
    /** 역할별 API 키 저장 여부 (boolean). 평문 값은 포함하지 않음. */
    redmineApiKeyStored,
    projectStateEdit,
    isTestingNetwork,
    isSaving,
    applyHealthReport,
    updateAuthStatus,
    setNetworkChecks,
    setRedmineUrl,
    setRedmineProjectId,
    saveRedmineApiKey,
    loadRedmineApiKeyStatus,
    initProjectStateEdit,
  }
})
