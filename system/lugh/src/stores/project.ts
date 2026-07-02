// useProjectStore — agiteam.json 파싱 결과 + 최근 프로젝트 목록
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { openWorkspace, loadWorkspaceConfig } from '@/ipc/workspace'
import type { AgiteamConfig, ProjectState, TeamMember } from '@/ipc/types'

export type RecentProject = {
  name: string
  displayName: string
  path: string
  lastOpened: string // ISO string
}

export const useProjectStore = defineStore('project', () => {
  // ── 상태 ──────────────────────────────────────────────────
  const workspaceId = ref<string | null>(null)
  const workspacePath = ref<string | null>(null)
  const config = ref<AgiteamConfig | null>(null)
  const projectState = ref<ProjectState | null>(null)
  const recentProjects = ref<RecentProject[]>(loadRecentFromStorage())
  const isLoading = ref(false)
  const loadError = ref<string | null>(null)

  // ── Computed ───────────────────────────────────────────────
  const name = computed(() => config.value?.project.name ?? '')
  const displayName = computed(() => config.value?.project.displayName ?? '')
  const workspaceName = computed(() => config.value?.project.workspace.name ?? '')
  const workspaceColor = computed(() => config.value?.project.workspace.color ?? 'Indigo')
  const teamMembers = computed<TeamMember[]>(() => config.value?.team ?? [])
  const pmConfig = computed(() => config.value?.pm ?? null)
  const businessType = computed(() => projectState.value?.business_type ?? '')
  const currentMode = computed(() => projectState.value?.current_mode ?? 'project')
  const milestone = computed(() => projectState.value?.milestone ?? '')
  const wbsTrack = computed(() => projectState.value?.wbs_track ?? '')

  // ── 액션 ──────────────────────────────────────────────────
  async function load(path: string) {
    isLoading.value = true
    loadError.value = null
    try {
      const summary = await openWorkspace(path)
      workspaceId.value = summary.workspace_id
      workspacePath.value = summary.path

      const cfg = await loadWorkspaceConfig(summary.workspace_id)
      config.value = cfg.agiteam
      projectState.value = cfg.project_state

      addRecentProject({
        name: cfg.agiteam.project.name,
        displayName: cfg.agiteam.project.displayName,
        path,
        lastOpened: new Date().toISOString(),
      })
    } catch (e: unknown) {
      loadError.value = e instanceof Error ? e.message : String(e)
      throw e
    } finally {
      isLoading.value = false
    }
  }

  function setConfig(cfg: AgiteamConfig) {
    config.value = cfg
  }

  function addRecentProject(p: RecentProject) {
    const idx = recentProjects.value.findIndex((r) => r.path === p.path)
    if (idx !== -1) recentProjects.value.splice(idx, 1)
    recentProjects.value.unshift(p)
    if (recentProjects.value.length > 10) recentProjects.value.splice(10)
    saveRecentToStorage(recentProjects.value)
  }

  function removeRecentProject(path: string) {
    recentProjects.value = recentProjects.value.filter((r) => r.path !== path)
    saveRecentToStorage(recentProjects.value)
  }

  function reset() {
    workspaceId.value = null
    workspacePath.value = null
    config.value = null
    projectState.value = null
  }

  return {
    workspaceId,
    workspacePath,
    config,
    projectState,
    recentProjects,
    isLoading,
    loadError,
    name,
    displayName,
    workspaceName,
    workspaceColor,
    teamMembers,
    pmConfig,
    businessType,
    currentMode,
    milestone,
    wbsTrack,
    load,
    setConfig,
    addRecentProject,
    removeRecentProject,
    reset,
  }
})

// ── localStorage 헬퍼 ─────────────────────────────────────
const STORAGE_KEY = 'lugh:recent_projects'

function loadRecentFromStorage(): RecentProject[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? (JSON.parse(raw) as RecentProject[]) : []
  } catch {
    return []
  }
}

function saveRecentToStorage(list: RecentProject[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(list))
  } catch {
    // ignore
  }
}
