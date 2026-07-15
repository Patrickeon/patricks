<!-- Screen-04: 팀 워크스페이스 — 메인 작업 화면 (DS-50 §5) -->
<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useProjectStore } from '@/stores/project'
import { useRoleStore } from '@/stores/role'
import { useWorkspaceStore } from '@/stores/workspace'
import { useBootStore } from '@/stores/boot'
import { useThemeStore } from '@/stores/theme'
import { listenAgentEvents, listenDocumentEvents } from '@/ipc/events'
import { stopRole } from '@/ipc/agent'
import { showToast } from '@/composables/toast'
import { useAgentHealthPoll } from '@/composables/useAgentHealthPoll'
import type { RecentProject } from '@/stores/project'
import PmChatPanel from '@/components/PmChatPanel.vue'
import RoleChatPanel from '@/components/RoleChatPanel.vue'
import StatusBadge from '@/components/StatusBadge.vue'
import TeamStatusPanel from '@/components/TeamStatusPanel.vue'
import SettingsView from '@/views/SettingsView.vue'
import type { AgentLifecycleState } from '@/ipc/types'

const router = useRouter()
const projectStore = useProjectStore()
const roleStore = useRoleStore()
const workspaceStore = useWorkspaceStore()
const bootStore = useBootStore()
const themeStore = useThemeStore()

// 30초 주기 에이전트 상태 폴링
useAgentHealthPoll()

// ── 팀원 패널 목록 (PM 제외 6명) ─────────────────────────
const teamPanels = computed(() =>
  projectStore.teamMembers.map((m) => ({
    role: m.role,
    name: m.name,
    agent: m.agent,
  })),
)

// ── 헤더 READY 카운터 ─────────────────────────────────────
const readyCount = computed(() => roleStore.readyCount)
const totalCount = computed(() => roleStore.totalCount)

// ── 사이드바 상태 ─────────────────────────────────────────
const activeSidebar = computed(() => workspaceStore.activeSidebar)

// store 액션(openBrowserSearch 등)으로 사이드바가 외부에서 전환되면 라우트도 동기화
watch(activeSidebar, (panel) => {
  if (panel === 'deliverables') router.push('/workspace/deliverables')
  else if (panel === 'redmine')  router.push('/workspace/redmine')
  else if (panel === 'browser')  router.push('/workspace/browser')
})

// ── 이벤트 리스너 ─────────────────────────────────────────
let unlistenAgent: (() => void) | null = null
let unlistenDoc: (() => void) | null = null

onMounted(async () => {
  unlistenAgent = await listenAgentEvents({
    onStatusChanged(p) {
      roleStore.applyStatusChanged({ role: p.role, to: p.to })
    },
    onMessageStarted(p) {
      const session = roleStore.getSessionBySessionId(p.session_id)
      if (session) roleStore.startStreaming(session.role, p.message_id)
    },
    onMessageDelta(p) {
      roleStore.appendMessageDelta(p)
    },
    onMessageCompleted(p) {
      const session = roleStore.getSessionBySessionId(p.session_id)
      if (session) roleStore.completeStreaming(session.role, p.message_id)
    },
    onMessageFailed(p) {
      const session = roleStore.getSessionBySessionId(p.session_id)
      if (session) roleStore.failStreaming(session.role)
    },
    onMessagesCleared(p) {
      // Redmine #24: 대화 초기화 — 세션·페르소나는 유지, 메시지 로그만 비운다 (DS-60 §5.3)
      roleStore.applyMessagesCleared(p)
    },
  })

  unlistenDoc = await listenDocumentEvents((_p) => {
    // 산출물 변경 → deliverableStore 갱신 (향후 연동)
  })

  // 산출물 패널 기본 열기 (처음 진입 시)
  workspaceStore.openSidebar('deliverables')

  // 창 크기 변경 시 사이드바 폭 재클램프 (#20)
  window.addEventListener('resize', onWindowResize)
})

onUnmounted(() => {
  unlistenAgent?.()
  unlistenDoc?.()
  window.removeEventListener('resize', onWindowResize)
  // 드래그 도중 unmount 안전 정리 (#20)
  onSplitterMouseUp()
})

// ── 액션 ──────────────────────────────────────────────────
function toggleSidebar(panel: 'deliverables' | 'redmine' | 'browser' | 'team-status') {
  workspaceStore.toggleSidebar(panel)
  // 사이드바 열릴 때 RouterView 라우트 이동 (team-status는 컴포넌트 직접 렌더)
  if (panel === 'deliverables') router.push('/workspace/deliverables')
  else if (panel === 'redmine')      router.push('/workspace/redmine')
  else if (panel === 'browser')      router.push('/workspace/browser')
}

// ── 사이드바 리사이즈 스플리터 (#20) ─────────────────────────
// 패널이 우측에 있으므로 왼쪽으로 드래그하면 폭 증가
let resizeStartX = 0
let resizeStartWidth = 0

function onSplitterMouseDown(e: MouseEvent) {
  e.preventDefault()
  resizeStartX = e.clientX
  resizeStartWidth = workspaceStore.sidebarWidth
  workspaceStore.beginSidebarResize()
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  window.addEventListener('mousemove', onSplitterMouseMove)
  window.addEventListener('mouseup', onSplitterMouseUp)
}

function onSplitterMouseMove(e: MouseEvent) {
  workspaceStore.setSidebarWidth(resizeStartWidth + (resizeStartX - e.clientX))
}

function onSplitterMouseUp() {
  window.removeEventListener('mousemove', onSplitterMouseMove)
  window.removeEventListener('mouseup', onSplitterMouseUp)
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  workspaceStore.endSidebarResize()
}

// 창 크기 변경 시 min~max(화면 60%) 범위 재클램프
function onWindowResize() {
  workspaceStore.reclampSidebarWidth()
}

function openSettings() {
  workspaceStore.openSettings()
}

// ── #27: 프로젝트 메뉴(열기/전환/닫기) + 종료 확인 모달 분기 ──────────
type ExitMode = 'close' | 'switch' | 'new'
const showProjectMenu = ref(false)
const exitMode = ref<ExitMode>('close')
const pendingSwitchPath = ref<string | null>(null)

const recentOthers = computed(() =>
  projectStore.recentProjects
    .filter((p) => p.path !== projectStore.workspacePath)
    .slice(0, 5),
)

// 종료 확인 모달 문구 (앱 종료가 아닌 프로젝트 닫기/전환 의미로 분기, DS-10 §8.3)
const exitDialog = computed(() => {
  if (exitMode.value === 'switch' || exitMode.value === 'new') {
    return {
      msg: '현재 팀을 종료하고 다른 프로젝트로 전환하시겠습니까?',
      sub: '실행 중인 모든 에이전트 세션이 닫힙니다.',
      confirm: '전환',
    }
  }
  return {
    msg: '프로젝트를 닫고 홈으로 돌아가시겠습니까?',
    sub: '실행 중인 모든 에이전트 세션이 닫힙니다.',
    confirm: '닫기',
  }
})

function toggleProjectMenu() {
  showProjectMenu.value = !showProjectMenu.value
}

function closeProjectMenu() {
  showProjectMenu.value = false
}

/** 종료 확인 모달을 특정 의도로 연다. */
function requestExit(mode: ExitMode, path: string | null = null) {
  exitMode.value = mode
  pendingSwitchPath.value = path
  closeProjectMenu()
  workspaceStore.promptExit()
}

// 헤더 ⏏ 버튼(팀 종료) = 프로젝트 닫기(홈으로)
function promptExit() {
  requestExit('close')
}

function switchToRecent(p: RecentProject) {
  requestExit('switch', p.path)
}

async function openProjectFromMenu() {
  try {
    const { open: dialogOpen } = await import('@tauri-apps/plugin-dialog')
    const selected = await dialogOpen({
      filters: [{ name: 'agiteam.json', extensions: ['json'] }],
      multiple: false,
    })
    if (selected && typeof selected === 'string') {
      const dirPath = selected.replace(/[/\\][^/\\]+$/, '')
      requestExit('switch', dirPath)
    }
  } catch (e) {
    console.error('[workspace] openProjectFromMenu dialog failed:', e)
    showToast('파일 선택 창을 열 수 없습니다', 'error')
  }
}

function newProjectFromMenu() {
  requestExit('new')
}

/** 실행 중인 모든 역할 세션을 종료한다 (DS-60 §6.3 step 1). */
async function stopAllRoles() {
  const targets = Array.from(roleStore.sessions.values())
    .map((s) => s.sessionId)
    .filter((id): id is string => !!id)
  await Promise.allSettled(targets.map((id) => stopRole(id)))
}

// 프로젝트 전환/닫기 확정 (DS-10 §8.3 / DS-60 §6.3 세션 정리 순서)
async function confirmExit() {
  workspaceStore.cancelExit()

  // 1) 세션 종료
  await stopAllRoles()

  // 2) FE 스토어 초기화
  projectStore.reset()
  workspaceStore.deactivate()
  roleStore.reset()
  bootStore.reset()

  // 3) 목적지 이동
  if (exitMode.value === 'switch' && pendingSwitchPath.value) {
    const path = pendingSwitchPath.value
    pendingSwitchPath.value = null
    try {
      await projectStore.load(path)
      router.push('/boot')
    } catch (e) {
      console.error('[workspace] switch load failed:', e)
      projectStore.removeRecentProject(path)
      showToast('프로젝트를 열 수 없습니다. 홈으로 이동합니다', 'error')
      router.push('/launcher')
    }
  } else if (exitMode.value === 'new') {
    router.push('/settings?mode=new')
  } else {
    router.push('/launcher')
  }
}

// ── 하단 상태 바 ──────────────────────────────────────────
const statusBarItems = computed(() => [
  { label: '프로젝트', value: projectStore.name },
  { label: '사업유형', value: projectStore.businessType },
  { label: '모드', value: projectStore.currentMode === 'project' ? '프로젝트' : '운영' },
  { label: '마일스톤', value: projectStore.milestone },
  { label: 'WBS', value: `${projectStore.wbsTrack}트랙` },
])

// 색상 세트 (workspace.color에 따라)
const headerAccent = computed(() => {
  const color = projectStore.workspaceColor.toLowerCase()
  if (color === 'red')   return '#ef4444'
  if (color === 'green') return '#22c55e'
  if (color === 'blue')  return '#3b82f6'
  return '#6366f1' // Indigo default
})
</script>

<template>
  <div class="workspace-root">
    <!-- ── 헤더 바 ── -->
    <header class="ws-header" :style="{ '--ws-accent': headerAccent }">
      <!-- #27: 프로젝트 메뉴 (열기/전환/닫기) -->
      <div class="ws-brand-wrap">
        <button class="ws-brand" title="프로젝트 메뉴" @click="toggleProjectMenu">
          <div class="ws-mark">A</div>
          <span class="ws-project-name">{{ projectStore.displayName || projectStore.name }}</span>
          <span class="ws-app-name">AgiTeamBuilder Desktop</span>
          <span class="ws-caret">▾</span>
        </button>

        <!-- 드롭다운 -->
        <template v-if="showProjectMenu">
          <div class="proj-menu-backdrop" @click="closeProjectMenu" />
          <div class="proj-menu" role="menu">
            <p class="proj-menu-label">최근 프로젝트</p>
            <button
              v-for="p in recentOthers"
              :key="p.path"
              class="proj-menu-item"
              @click="switchToRecent(p)"
            >
              <span class="proj-menu-item-name">{{ p.displayName || p.name }}</span>
              <span class="proj-menu-item-path">{{ p.path }}</span>
            </button>
            <p v-if="recentOthers.length === 0" class="proj-menu-empty">다른 최근 프로젝트 없음</p>

            <div class="proj-menu-divider" />
            <button class="proj-menu-item action" @click="openProjectFromMenu">📂 프로젝트 열기…</button>
            <button class="proj-menu-item action" @click="newProjectFromMenu">＋ 새 프로젝트…</button>
            <div class="proj-menu-divider" />
            <button class="proj-menu-item danger" @click="promptExit">⏏ 프로젝트 닫기 (홈으로)</button>
          </div>
        </template>
      </div>

      <div class="ws-nav">
        <!-- READY 카운터 -->
        <div class="ready-badge">
          <span class="ready-dot" :class="{ all: readyCount === totalCount }" />
          {{ readyCount }}/{{ totalCount }} READY
        </div>

        <!-- 사이드바 토글 버튼 -->
        <button
          class="nav-btn"
          :class="{ active: activeSidebar === 'deliverables' }"
          title="산출물 뷰어"
          @click="toggleSidebar('deliverables')"
        >📄</button>
        <button
          class="nav-btn"
          :class="{ active: activeSidebar === 'redmine' }"
          title="레드마인"
          @click="toggleSidebar('redmine')"
        >📋</button>
        <button
          class="nav-btn"
          :class="{ active: activeSidebar === 'browser' }"
          title="내장 브라우저"
          @click="toggleSidebar('browser')"
        >🌐</button>

        <div class="nav-divider" />

        <button
          class="nav-btn"
          :title="themeStore.theme === 'dark' ? '밝은 테마로 전환' : '어두운 테마로 전환'"
          @click="themeStore.toggleTheme()"
        >{{ themeStore.theme === 'dark' ? '☀️' : '🌙' }}</button>
        <button class="nav-btn" title="설정" @click="openSettings">⚙️</button>
        <button class="nav-btn danger" title="팀 종료" @click="promptExit">⏏</button>
      </div>
    </header>

    <!-- ── 메인 컨텐츠 ── -->
    <div class="ws-body">
      <!-- PM 채팅 패널 (좌측 고정폭) -->
      <section class="pm-section">
        <PmChatPanel />
      </section>

      <!-- 팀원 패널 그리드 (2열 3행) -->
      <section
        class="team-section"
        :class="{ 'has-maximized': !!workspaceStore.maximizedRole }"
      >
        <RoleChatPanel
          v-for="member in teamPanels"
          :key="member.role"
          :role="member.role"
          :name="member.name"
          :agent="member.agent"
        />
      </section>

      <!-- 사이드바 영역 (64px 아이콘 또는 확장 패널) -->
      <aside class="ws-sidebar">
        <!-- 사이드바가 닫힌 경우 — 아이콘만 표시 -->
        <template v-if="!activeSidebar">
          <button class="sidebar-icon" @click="toggleSidebar('deliverables')" title="산출물">📄</button>
          <button class="sidebar-icon" @click="toggleSidebar('redmine')" title="레드마인">📋</button>
          <button class="sidebar-icon" @click="toggleSidebar('browser')" title="브라우저">🌐</button>
          <button class="sidebar-icon" @click="toggleSidebar('team-status')" title="팀 상태">👥</button>
        </template>
      </aside>

      <!-- 사이드바 확장 패널 -->
      <Transition name="slide-in">
        <div
          v-if="activeSidebar"
          class="sidebar-panel"
          :class="{ resizing: workspaceStore.isSidebarResizing }"
          :style="{ '--sidebar-width': workspaceStore.sidebarWidth + 'px' }"
        >
          <!-- 리사이즈 스플리터 (#20) — 안쪽(좌측) 경계 드래그로 폭 조절 -->
          <div
            class="sidebar-resizer"
            title="드래그하여 폭 조절"
            @mousedown="onSplitterMouseDown"
          />
          <div class="sidebar-panel-header">
            <span class="sidebar-panel-title">
              <template v-if="activeSidebar === 'deliverables'">📄 산출물</template>
              <template v-else-if="activeSidebar === 'redmine'">📋 레드마인</template>
              <template v-else-if="activeSidebar === 'team-status'">👥 팀 상태</template>
              <template v-else>🌐 브라우저</template>
            </span>
            <button class="close-btn" @click="workspaceStore.closeSidebar()">×</button>
          </div>
          <div class="sidebar-panel-body">
            <TeamStatusPanel v-if="activeSidebar === 'team-status'" />
            <template v-else>
              <RouterView v-if="$route.path.startsWith('/workspace/')" />
              <template v-else>
                <router-link
                  v-if="activeSidebar === 'deliverables'"
                  :to="'/workspace/deliverables'"
                />
                <router-link
                  v-else-if="activeSidebar === 'redmine'"
                  :to="'/workspace/redmine'"
                />
                <router-link
                  v-else-if="activeSidebar === 'browser'"
                  :to="'/workspace/browser'"
                />
              </template>
            </template>
          </div>
        </div>
      </Transition>
    </div>

    <!-- ── 하단 상태 바 ── -->
    <footer class="ws-statusbar">
      <span
        v-for="item in statusBarItems"
        :key="item.label"
        class="statusbar-item"
      >
        <span class="statusbar-label">{{ item.label }}</span>
        <span class="statusbar-value">{{ item.value }}</span>
      </span>
    </footer>

    <!-- ── 설정 오버레이 ── -->
    <Teleport to="body">
      <div
        v-if="workspaceStore.showSettingsOverlay"
        class="overlay-backdrop"
        @click.self="workspaceStore.closeSettings()"
      >
        <div class="settings-overlay-box">
          <div class="overlay-header">
            <span>⚙️ 설정</span>
            <button @click="workspaceStore.closeSettings()">×</button>
          </div>
          <div class="overlay-body settings-body-wrap">
            <SettingsView />
          </div>
        </div>
      </div>
    </Teleport>

    <!-- ── 종료 확인 모달 ── -->
    <Teleport to="body">
      <div
        v-if="workspaceStore.showExitConfirm"
        class="overlay-backdrop"
        @click.self="workspaceStore.cancelExit()"
      >
        <div class="confirm-box">
          <p class="confirm-msg">{{ exitDialog.msg }}<br><small>{{ exitDialog.sub }}</small></p>
          <div class="confirm-btns">
            <button class="btn btn-ghost" @click="workspaceStore.cancelExit()">취소</button>
            <button class="btn btn-danger" @click="confirmExit">{{ exitDialog.confirm }}</button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.workspace-root {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bg-base);
}

/* ── 헤더 ── */
.ws-header {
  height: 52px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 16px;
  border-bottom: 1px solid rgba(255,255,255,0.07);
  background: var(--bg-ws-header);
  flex-shrink: 0;
}

[data-theme="light"] .ws-header { border-bottom-color: rgba(0,0,0,0.08); }

.ws-brand-wrap { position: relative; }

.ws-brand {
  display: flex;
  align-items: center;
  gap: 10px;
  background: none;
  border: 1px solid transparent;
  border-radius: 8px;
  padding: 3px 8px 3px 4px;
  cursor: pointer;
  transition: all 0.12s;
}

.ws-brand:hover { background: rgba(255,255,255,0.06); border-color: rgba(255,255,255,0.12); }
[data-theme="light"] .ws-brand:hover { background: rgba(0,0,0,0.04); border-color: rgba(0,0,0,0.10); }

.ws-caret { font-size: 10px; color: #94a3b8; margin-left: 2px; }
[data-theme="light"] .ws-caret { color: #64748b; }

/* ── #27: 프로젝트 메뉴 드롭다운 ── */
.proj-menu-backdrop { position: fixed; inset: 0; z-index: 40; }

.proj-menu {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  z-index: 41;
  min-width: 300px;
  max-width: 380px;
  padding: 8px;
  background: var(--bg-panel);
  border: 1px solid var(--line);
  border-radius: 10px;
  box-shadow: var(--shadow);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.proj-menu-label {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--text-muted);
  font-weight: 600;
  padding: 4px 8px;
}

.proj-menu-item {
  display: flex;
  flex-direction: column;
  gap: 1px;
  text-align: left;
  padding: 7px 8px;
  background: none;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  color: var(--text-primary);
  font-size: 13px;
}

.proj-menu-item:hover { background: rgba(99,102,241,0.1); }
.proj-menu-item.action { flex-direction: row; align-items: center; font-weight: 500; }
.proj-menu-item.danger { color: var(--error); }
.proj-menu-item.danger:hover { background: rgba(239,68,68,0.1); }

.proj-menu-item-name { font-size: 13px; font-weight: 500; }
.proj-menu-item-path {
  font-size: 11px;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.proj-menu-empty { font-size: 12px; color: var(--text-muted); padding: 6px 8px; }

.proj-menu-divider { height: 1px; background: var(--line-soft); margin: 6px 4px; }

.ws-mark {
  width: 30px; height: 30px;
  border-radius: 7px;
  background: linear-gradient(135deg, #5eead4, #38bdf8);
  display: grid;
  place-items: center;
  font-size: 15px;
  font-weight: 900;
  color: #041016;
  flex-shrink: 0;
}

/* 헤더 내부 요소 — 다크 모드: 밝은 글씨 */
.ws-project-name {
  font-size: 14px;
  font-weight: 700;
  color: #f1f5f9;
}

.ws-app-name {
  font-size: 11px;
  color: #94a3b8;
}

/* 라이트 모드: 아이보리 헤더 → 다크 글씨 */
[data-theme="light"] .ws-project-name { color: #1e293b; }
[data-theme="light"] .ws-app-name     { color: #64748b; }

.ws-nav { display: flex; align-items: center; gap: 4px; }

.ready-badge {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 3px 10px;
  border: 1px solid rgba(255,255,255,0.15);
  border-radius: 999px;
  font-size: 11px;
  color: #94a3b8;
  margin-right: 6px;
  background: rgba(255,255,255,0.05);
}

[data-theme="light"] .ready-badge {
  border-color: rgba(0,0,0,0.12);
  background: rgba(0,0,0,0.04);
  color: #475569;
}

.ready-dot {
  width: 6px; height: 6px;
  border-radius: 50%;
  background: #64748b;
}

.ready-dot.all { background: var(--ok); box-shadow: 0 0 8px rgba(34,197,94,0.6); }

.nav-btn {
  width: 30px; height: 30px;
  border: 1px solid rgba(255,255,255,0.12);
  background: rgba(255,255,255,0.06);
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
  display: grid;
  place-items: center;
  transition: all 0.12s;
}

[data-theme="light"] .nav-btn {
  border-color: rgba(0,0,0,0.10);
  background: rgba(0,0,0,0.04);
}

.nav-btn:hover { border-color: rgba(99,102,241,0.7); background: rgba(99,102,241,0.15); }
.nav-btn.active { border-color: #6366f1; background: rgba(99,102,241,0.2); }
.nav-btn.danger:hover { border-color: rgba(239,68,68,0.7); background: rgba(239,68,68,0.12); }

.nav-divider { width: 1px; height: 20px; background: rgba(255,255,255,0.12); margin: 0 4px; }
[data-theme="light"] .nav-divider { background: rgba(0,0,0,0.10); }

/* ── 바디 ── */
.ws-body {
  flex: 1;
  display: flex;
  overflow: hidden;
  position: relative;
}

.pm-section {
  width: 380px;
  flex-shrink: 0;
  overflow: hidden;
}

.team-section {
  flex: 1;
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  grid-template-rows: repeat(3, 1fr);
  gap: 7px;
  padding: 8px;
  overflow: hidden;
  position: relative;
  background: var(--bg-base);
}

.team-section.has-maximized {
  position: relative;
}

.ws-sidebar {
  width: 48px;
  background: var(--bg-ws-header);
  border-left: 1px solid rgba(0,0,0,0.15);
  display: flex;
  flex-direction: column;
  align-items: center;
  padding-top: 14px;
  gap: 8px;
  flex-shrink: 0;
}

.sidebar-icon {
  width: 30px; height: 30px;
  border: 1px solid rgba(255,255,255,0.12);
  background: rgba(255,255,255,0.06);
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
  display: grid;
  place-items: center;
  transition: all 0.12s;
}

.sidebar-icon:hover { border-color: rgba(99,102,241,0.7); background: rgba(99,102,241,0.15); }

[data-theme="light"] .sidebar-icon {
  border-color: rgba(0,0,0,0.10);
  background: rgba(0,0,0,0.04);
}

/* ── 사이드바 확장 패널 ── */
.sidebar-panel {
  /* #20: 폭은 CSS 변수로 바인딩 (inline width 대신 — slide-in 전환과 충돌 방지) */
  width: var(--sidebar-width, 560px);
  flex-shrink: 0;
  background: var(--bg-panel);
  border-left: 1px solid var(--line-soft);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
}

/* ── 리사이즈 스플리터 (#20) ── */
.sidebar-resizer {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 5px;
  cursor: col-resize;
  z-index: 10;
  background: transparent;
  transition: background 0.12s;
}

.sidebar-resizer:hover,
.sidebar-panel.resizing .sidebar-resizer {
  background: rgba(99,102,241,0.45);
}

/* 드래그 중 패널 내부 iframe/텍스트가 mousemove를 가로채지 않도록 */
.sidebar-panel.resizing .sidebar-panel-body {
  pointer-events: none;
}

.sidebar-panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 14px;
  border-bottom: 1px solid var(--line-soft);
  background: var(--bg-panel-header);
  flex-shrink: 0;
}

.sidebar-panel-title { font-size: 13px; font-weight: 600; color: var(--text-primary); }

.close-btn {
  background: none; border: none;
  color: var(--text-muted); font-size: 18px; cursor: pointer;
}

.sidebar-panel-body { flex: 1; overflow: hidden; }

/* slide-in transition */
.slide-in-enter-active { transition: width 0.18s ease; }
.slide-in-leave-active { transition: width 0.15s ease; }
.slide-in-enter-from  { width: 0; }
.slide-in-leave-to    { width: 0; }

/* ── 하단 상태 바 ── */
.ws-statusbar {
  height: 26px;
  display: flex;
  align-items: center;
  padding: 0 16px;
  gap: 20px;
  border-top: 1px solid rgba(0,0,0,0.15);
  background: var(--bg-ws-header);
  flex-shrink: 0;
}

.statusbar-item { display: flex; align-items: center; gap: 5px; font-size: 11px; }
.statusbar-label { color: #64748b; }
.statusbar-value { color: #94a3b8; font-weight: 500; }

[data-theme="light"] .statusbar-label { color: #8a7060; }
[data-theme="light"] .statusbar-value { color: #5a4232; }

/* ── 오버레이 공통 ── */
.overlay-backdrop {
  position: fixed; inset: 0;
  background: rgba(0,0,0,0.55); z-index: 100;
  display: flex; align-items: center; justify-content: center;
}

.settings-overlay-box {
  background: var(--bg-panel);
  border: 1px solid var(--line);
  border-radius: 10px;
  width: 720px;
  height: 520px;
  overflow: hidden;
  box-shadow: var(--shadow);
  display: flex;
  flex-direction: column;
}

.confirm-box {
  background: var(--bg-panel);
  border: 1px solid var(--line);
  border-radius: 10px;
  min-width: 360px;
  overflow: hidden;
  box-shadow: var(--shadow);
}

.overlay-header {
  display: flex; justify-content: space-between; align-items: center;
  padding: 14px 18px;
  border-bottom: 1px solid var(--line-soft);
  font-size: 14px; font-weight: 600;
}

.overlay-header button { background: none; border: none; color: var(--text-muted); font-size: 18px; cursor: pointer; }
.overlay-body { padding: 20px; }
.settings-body-wrap { padding: 0; flex: 1; overflow: hidden; }

.confirm-box { padding: 24px; display: flex; flex-direction: column; gap: 20px; }
.confirm-msg { font-size: 14px; color: var(--text-primary); line-height: 1.6; text-align: center; }
.confirm-msg small { color: var(--text-muted); font-size: 12px; }
.confirm-btns { display: flex; gap: 10px; justify-content: flex-end; }

.btn { height: 34px; padding: 0 14px; border-radius: 6px; font-size: 13px; font-weight: 600; cursor: pointer; border: 1px solid var(--line); }
.btn-ghost { background: var(--bg-panel-2); color: var(--text-primary); }
.btn-danger { background: rgba(239,68,68,0.12); color: var(--error); border-color: rgba(239,68,68,0.3); }
</style>
