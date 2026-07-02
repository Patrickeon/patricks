// useWorkspaceStore — 워크스페이스 활성화 상태 + 레이아웃 + 사이드바
import { defineStore } from 'pinia'
import { ref } from 'vue'

export type SidebarPanel = 'deliverables' | 'redmine' | 'browser' | 'team-status' | null

export const useWorkspaceStore = defineStore('workspace', () => {
  // ── 상태 ──────────────────────────────────────────────────
  const isActive = ref(false)
  const activeSidebar = ref<SidebarPanel>(null)
  const maximizedRole = ref<string | null>(null)
  const showSettingsOverlay = ref(false)
  const showExitConfirm = ref(false)

  // ── 레이아웃 슬롯 매핑 (role → layout slot)
  // agiteam.json team[].layout 값 기준
  const layoutSlots = ref<Record<string, string>>({})

  // ── 액션 ──────────────────────────────────────────────────
  function activate() {
    isActive.value = true
  }

  function deactivate() {
    isActive.value = false
  }

  function toggleSidebar(panel: SidebarPanel) {
    if (activeSidebar.value === panel) {
      activeSidebar.value = null
    } else {
      activeSidebar.value = panel
    }
  }

  function openSidebar(panel: Exclude<SidebarPanel, null>) {
    activeSidebar.value = panel
  }

  function closeSidebar() {
    activeSidebar.value = null
  }

  function maximizePanel(role: string) {
    maximizedRole.value = role
  }

  function restorePanel() {
    maximizedRole.value = null
  }

  function openSettings() {
    showSettingsOverlay.value = true
  }

  function closeSettings() {
    showSettingsOverlay.value = false
  }

  function promptExit() {
    showExitConfirm.value = true
  }

  function cancelExit() {
    showExitConfirm.value = false
  }

  function setLayoutSlots(slots: Record<string, string>) {
    layoutSlots.value = slots
  }

  return {
    isActive,
    activeSidebar,
    maximizedRole,
    showSettingsOverlay,
    showExitConfirm,
    layoutSlots,
    activate,
    deactivate,
    toggleSidebar,
    openSidebar,
    closeSidebar,
    maximizePanel,
    restorePanel,
    openSettings,
    closeSettings,
    promptExit,
    cancelExit,
    setLayoutSlots,
  }
})
