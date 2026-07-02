// useRedmineStore — 레드마인 이슈 목록, 선택 이슈, 필터
// DV60-003: 실 API invoke 연동 (src/ipc/redmine.ts 래퍼 경유 — DS-60 §3 IPC 규칙)
// DV60-004: API 키 OS Keychain 저장 — 평문 저장 없음 (NFR-0301)
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { saveCredential, getMaskedCredential } from '@/ipc/credential'
import { listRedmineIssues, createRedmineIssue, updateRedmineIssue } from '@/ipc/redmine'
import type { RedmineIssueItem as RawRedmineIssue } from '@/ipc/types'

export type RedmineStatus = {
  id: number
  name: string
  is_closed: boolean
}

export type RedmineUser = {
  id: number
  name: string
}

export type RedmineIssue = {
  id: number
  subject: string
  description: string
  status: RedmineStatus
  tracker: { id: number; name: string }
  assigned_to?: RedmineUser
  done_ratio: number
  created_on: string
  updated_on: string
  notes?: string
}

export type RedmineProject = {
  id: string
  name: string
}

export type IssueCreatePayload = {
  project_id: string
  tracker_id: number
  subject: string
  description?: string
  assigned_to_id?: number
}

// Shared persona §3 기준 상태 목록
export const REDMINE_STATUSES: RedmineStatus[] = [
  { id: 1, name: '신규',   is_closed: false },
  { id: 2, name: '진행',   is_closed: false },
  { id: 3, name: '해결',   is_closed: false },
  { id: 4, name: '의견',   is_closed: false },
  { id: 5, name: '완료',   is_closed: true  },
  { id: 6, name: '거절',   is_closed: true  },
]

/** Rust RedmineIssueItem → 프론트 RedmineIssue 변환 (is_closed 추가) */
function toRedmineIssue(raw: RawRedmineIssue): RedmineIssue {
  const statusDef = REDMINE_STATUSES.find((s) => s.id === raw.status.id)
  return {
    ...raw,
    status: {
      id: raw.status.id,
      name: raw.status.name,
      is_closed: statusDef?.is_closed ?? (raw.status.id >= 5),
    },
  }
}

export const useRedmineStore = defineStore('redmine', () => {
  // ── 상태 ──────────────────────────────────────────────────
  const projects = ref<RedmineProject[]>([])
  const selectedProjectId = ref<string | null>(null)
  const issues = ref<RedmineIssue[]>([])
  const selectedIssue = ref<RedmineIssue | null>(null)
  const isLoading = ref(false)
  const error = ref<string | null>(null)
  const showCreateModal = ref(false)
  const showDetail = ref(false)

  // ── API 키 상태 (DV60-004) ────────────────────────────────
  /** OS Keychain에 키가 저장돼 있는지 여부 */
  const apiKeyStored = ref(false)

  // ── 필터 ──────────────────────────────────────────────────
  const filterStatusId = ref<number | 'open' | 'all'>('open')

  // ── Computed ───────────────────────────────────────────────
  const filteredIssues = computed(() => {
    if (filterStatusId.value === 'all') return issues.value
    if (filterStatusId.value === 'open')
      return issues.value.filter((i) => !i.status.is_closed)
    return issues.value.filter((i) => i.status.id === filterStatusId.value)
  })

  // ── Mock 데이터 (백엔드 미연동 또는 API 키 미설정 시 폴백) ──
  function loadMockIssues() {
    issues.value = [
      {
        id: 1,
        subject: 'Vue3 프론트엔드 LauncherView 구현',
        description: 'Screen-01 런처 화면 구현',
        status: REDMINE_STATUSES[1], // 진행
        tracker: { id: 2, name: '새기능' },
        assigned_to: { id: 3, name: '프개발' },
        done_ratio: 40,
        created_on: '2026-06-23T00:00:00',
        updated_on: '2026-06-24T00:00:00',
      },
      {
        id: 2,
        subject: 'Tauri IPC boot_team 커맨드 구현',
        description: '팀 부팅 Rust 커맨드 구현',
        status: REDMINE_STATUSES[2], // 해결
        tracker: { id: 2, name: '새기능' },
        assigned_to: { id: 2, name: '박개발' },
        done_ratio: 100,
        created_on: '2026-06-23T00:00:00',
        updated_on: '2026-06-24T00:00:00',
      },
    ]
  }

  // ── DV60-003: 실 API 액션 ─────────────────────────────────

  /**
   * Redmine 이슈 목록을 실 API로 조회한다.
   * 실패 시 loadMockIssues() 폴백.
   * @param role #15: 역할별 API 키 선택. 미전달 시 단일 키 fallback (사람용 패널 기본값)
   */
  async function fetchIssues(workspaceId?: string, projectId?: string, role?: string) {
    if (!workspaceId) {
      loadMockIssues()
      return
    }
    isLoading.value = true
    error.value = null
    try {
      const statusParam =
        filterStatusId.value === 'all'   ? 'all'  :
        filterStatusId.value === 'open'  ? 'open' :
        String(filterStatusId.value)

      const rawList = await listRedmineIssues(workspaceId, projectId ?? null, statusParam, role)
      issues.value = rawList.map(toRedmineIssue)
    } catch (e) {
      error.value = String(e)
      // mock 폴백 제거 — API 실패는 error.value로 노출, 성공처럼 보이지 않음
    } finally {
      isLoading.value = false
    }
  }

  /**
   * 이슈를 생성한다 (실 API → 실패 시 로컬 추가).
   */
  async function createIssueApi(
    workspaceId: string | undefined,
    payload: IssueCreatePayload,
    role?: string,
  ): Promise<RedmineIssue> {
    if (!workspaceId) {
      const localIssue = makeLocalIssue(payload)
      issues.value.unshift(localIssue)
      return localIssue
    }
    try {
      const raw = await createRedmineIssue(
        workspaceId,
        payload.project_id,
        payload.tracker_id,
        payload.subject,
        payload.description ?? null,
        payload.assigned_to_id ?? null,
        role,
      )
      const issue = toRedmineIssue(raw)
      issues.value.unshift(issue)
      return issue
    } catch (e) {
      error.value = String(e)
      throw e
    }
  }

  /**
   * 이슈 상태·진척률·코멘트를 갱신한다 (실 API → 실패 시 로컬 갱신).
   */
  async function updateIssueApi(
    workspaceId: string | undefined,
    updated: RedmineIssue,
    notes?: string,
    role?: string,
  ) {
    if (!workspaceId) {
      updateIssueLocal(updated)
      return
    }
    try {
      await updateRedmineIssue(
        workspaceId,
        updated.id,
        updated.status.id,
        updated.done_ratio,
        notes ?? null,
        role,
      )
      updateIssueLocal(updated)
    } catch (e) {
      error.value = String(e)
      throw e
    }
  }

  // ── DV60-004: API 키 OS Keychain 저장 ────────────────────

  /**
   * Redmine API 키를 OS Keychain에 저장한다.
   * 기존처럼 Pinia 상태에 평문 저장하지 않는다.
   */
  /**
   * Redmine API 키를 OS Keychain에 저장한다. (DV60-004)
   * account = 'api_key' — Rust redmine_client.rs get_redmine_api_key()와 동일한 이름.
   * Pinia 상태에 평문을 보관하지 않는다 (NFR-0301 준수).
   */
  async function saveApiKey(key: string) {
    try {
      await saveCredential('redmine', 'api_key', key)
      apiKeyStored.value = true
    } catch (e) {
      error.value = `API 키 저장 실패: ${String(e)}`
    }
  }

  /**
   * Redmine URL을 OS Keychain에 저장한다.
   */
  async function saveRedmineUrl(url: string) {
    try {
      await saveCredential('redmine', 'url', url)
    } catch (e) {
      error.value = `URL 저장 실패: ${String(e)}`
    }
  }

  /**
   * OS Keychain에 API 키가 저장돼 있는지 확인한다. (DV60-004)
   * account = 'api_key' — 저장 시와 동일한 이름으로 조회.
   */
  async function loadApiKeyStatus() {
    try {
      const masked = await getMaskedCredential('redmine', 'api_key')
      apiKeyStored.value = masked.has_secret
    } catch {
      apiKeyStored.value = false
    }
  }

  // ── 로컬 상태 조작 (내부 helper) ─────────────────────────

  function makeLocalIssue(payload: IssueCreatePayload): RedmineIssue {
    const now = new Date().toISOString()
    const tracker = REDMINE_STATUSES[0] // fallback
    return {
      id: Date.now(),
      subject: payload.subject,
      description: payload.description ?? '',
      status: REDMINE_STATUSES[0],
      tracker: {
        id: payload.tracker_id,
        name: payload.tracker_id === 1 ? '결함' : payload.tracker_id === 2 ? '새기능' : '지원',
      },
      done_ratio: 0,
      created_on: now,
      updated_on: now,
    }
  }

  function updateIssueLocal(updated: RedmineIssue) {
    const idx = issues.value.findIndex((i) => i.id === updated.id)
    if (idx !== -1) issues.value[idx] = updated
    if (selectedIssue.value?.id === updated.id) {
      selectedIssue.value = updated
    }
  }

  // ── 기존 로컬 액션 (하위 호환) ───────────────────────────

  function setIssues(list: RedmineIssue[]) {
    issues.value = list
  }

  function selectIssue(issue: RedmineIssue) {
    selectedIssue.value = issue
    showDetail.value = true
  }

  function closeDetail() {
    showDetail.value = false
    selectedIssue.value = null
  }

  function openCreateModal() {
    showCreateModal.value = true
  }

  function closeCreateModal() {
    showCreateModal.value = false
  }

  function addIssue(issue: RedmineIssue) {
    issues.value.unshift(issue)
  }

  function updateIssue(updated: RedmineIssue) {
    updateIssueLocal(updated)
  }

  return {
    projects,
    selectedProjectId,
    issues,
    selectedIssue,
    isLoading,
    error,
    showCreateModal,
    showDetail,
    filterStatusId,
    filteredIssues,
    apiKeyStored,
    // Mock
    loadMockIssues,
    // 실 API
    fetchIssues,
    createIssueApi,
    updateIssueApi,
    // DV60-004: 보안 키 관리
    saveApiKey,
    saveRedmineUrl,
    loadApiKeyStatus,
    // 로컬 액션
    setIssues,
    selectIssue,
    closeDetail,
    openCreateModal,
    closeCreateModal,
    addIssue,
    updateIssue,
  }
})
