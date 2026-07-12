// useWorkspaceStore — 워크스페이스 활성화 상태 + 레이아웃 + 사이드바
import { defineStore } from 'pinia'
import { ref } from 'vue'

export type SidebarPanel = 'deliverables' | 'redmine' | 'browser' | 'team-status' | null

// ── 사이드바 패널 리사이즈 (#20) ─────────────────────────────
/** 사이드바 패널 기본 폭(px) — 기존 CSS 고정값과 동일 */
export const SIDEBAR_DEFAULT_WIDTH = 560
/** 최소 폭(px) */
export const SIDEBAR_MIN_WIDTH = 320
/** 최대 폭 비율 (화면 폭 대비) */
export const SIDEBAR_MAX_RATIO = 0.6

const SIDEBAR_WIDTH_KEY = 'lugh:sidebar-width'

/** min 320px ~ max 화면 60% 범위로 클램프 */
export function clampSidebarWidth(width: number): number {
  const max = Math.max(SIDEBAR_MIN_WIDTH, Math.round(window.innerWidth * SIDEBAR_MAX_RATIO))
  return Math.min(Math.max(Math.round(width), SIDEBAR_MIN_WIDTH), max)
}

function loadSidebarWidth(): number {
  try {
    const raw = localStorage.getItem(SIDEBAR_WIDTH_KEY)
    const parsed = raw === null ? NaN : Number(raw)
    if (Number.isFinite(parsed)) return clampSidebarWidth(parsed)
  } catch { /* ignore */ }
  return SIDEBAR_DEFAULT_WIDTH
}

// ── 웹검색 (검색 UX) ─────────────────────────────────────────
/** 기본 검색 엔진 URL prefix */
export const SEARCH_ENGINE_URL = 'https://duckduckgo.com/?q='

/** 검색어 → 검색 엔진 쿼리 URL */
export function toSearchUrl(query: string): string {
  return `${SEARCH_ENGINE_URL}${encodeURIComponent(query)}`
}

export const useWorkspaceStore = defineStore('workspace', () => {
  // ── 상태 ──────────────────────────────────────────────────
  const isActive = ref(false)
  const activeSidebar = ref<SidebarPanel>(null)
  const maximizedRole = ref<string | null>(null)
  const showSettingsOverlay = ref(false)
  const showExitConfirm = ref(false)

  /** 브라우저 패널이 열릴 때(또는 열려 있을 때) 이동할 대기 URL — openBrowserSearch가 설정 */
  const pendingBrowserUrl = ref<string | null>(null)

  // ── 브라우저 독립 창 열림 상태 (Redmine #17) ────────────────
  // [lifecycle 확정: 계속 띄워두기] embedded-browser 독립 창은 BrowserPanel(사이드바 탭)의
  // mount/unmount와 수명이 다르다. 사용자가 사이드바 탭을 전환해도 창은 계속 떠 있으므로,
  // 패널 로컬 ref가 아니라 workspaceStore에 상태를 둬서 탭 재진입 시에도 실제 창 존재 여부와
  // 일치하도록 한다.
  /** embedded-browser 독립 창이 열려 있는지 여부 */
  const isBrowserOpen = ref(false)
  /** 열려 있는 embedded-browser 창의 현재 주소 (패널 재마운트 시 상태표시 복원용) */
  const browserAddress = ref('')

  // ── 사이드바 패널 폭 (#20 리사이즈 스플리터) ────────────────
  /** 현재 사이드바 패널 폭(px) — localStorage에서 복원 */
  const sidebarWidth = ref<number>(loadSidebarWidth())
  /** 스플리터 드래그 진행 중 여부 — BrowserPanel이 mouseup 시 최종 재배치에 사용 */
  const isSidebarResizing = ref(false)

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

  // ── 사이드바 리사이즈 액션 (#20) ────────────────────────────
  /** 드래그 중 폭 갱신 (클램프만, 영속화는 endSidebarResize에서) */
  function setSidebarWidth(width: number) {
    sidebarWidth.value = clampSidebarWidth(width)
  }

  /** 스플리터 드래그 시작 */
  function beginSidebarResize() {
    isSidebarResizing.value = true
  }

  /** 스플리터 드래그 종료 — 확정 폭을 localStorage에 저장 */
  function endSidebarResize() {
    isSidebarResizing.value = false
    try { localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth.value)) } catch { /* ignore */ }
  }

  /** 창 크기 변경 시 현재 폭을 범위 내로 재클램프 */
  function reclampSidebarWidth() {
    sidebarWidth.value = clampSidebarWidth(sidebarWidth.value)
  }

  // ── 웹검색 트리거 (검색 UX) ─────────────────────────────────
  /**
   * 브라우저 패널을 열고 검색 엔진 쿼리로 이동한다.
   * 외부(AI 에이전트 웹검색 도구 등)에서 호출하는 진입점.
   * - 사이드바를 'browser'로 전환 → BrowserPanel이 mount/watch로 pendingBrowserUrl 소비
   */
  function openBrowserSearch(query: string) {
    pendingBrowserUrl.value = toSearchUrl(query)
    activeSidebar.value = 'browser'
  }

  /** 검색이 아닌 임의 URL로 브라우저 패널 열기 */
  function openBrowserUrl(url: string) {
    pendingBrowserUrl.value = url
    activeSidebar.value = 'browser'
  }

  /** BrowserPanel이 대기 URL을 소비한다 (1회성). */
  function consumePendingBrowserUrl(): string | null {
    const url = pendingBrowserUrl.value
    pendingBrowserUrl.value = null
    return url
  }

  // ── 브라우저 독립 창 상태 갱신 액션 (Redmine #17) ───────────
  /** embedded-browser 독립 창 열림/닫힘 상태를 갱신한다. */
  function setBrowserOpen(open: boolean) {
    isBrowserOpen.value = open
  }

  /** embedded-browser 독립 창의 현재 주소를 갱신한다. */
  function setBrowserAddress(url: string) {
    browserAddress.value = url
  }

  return {
    isActive,
    activeSidebar,
    maximizedRole,
    showSettingsOverlay,
    showExitConfirm,
    layoutSlots,
    pendingBrowserUrl,
    isBrowserOpen,
    browserAddress,
    sidebarWidth,
    isSidebarResizing,
    setSidebarWidth,
    beginSidebarResize,
    endSidebarResize,
    reclampSidebarWidth,
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
    openBrowserSearch,
    openBrowserUrl,
    consumePendingBrowserUrl,
    setBrowserOpen,
    setBrowserAddress,
  }
})
