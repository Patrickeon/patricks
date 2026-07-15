<!-- Screen-01: 런처 / 시작 화면 (DS-50 §2) -->
<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useProjectStore } from '@/stores/project'
import { openBrowser } from '@/ipc/browser'
import { showToast } from '@/composables/toast'
import { useProjectStatus } from '@/composables/useProjectStatus'
import type { RecentProject } from '@/stores/project'

const router = useRouter()
const projectStore = useProjectStore()
// #27: 프로젝트 컨텍스트 상태(none/configured/active) — 홈 안내 카피 노출 판단에 사용 (DS-10 §8.1)
const { isNone } = useProjectStatus()
const appVersion = ref('0.1.0')

// 인라인 경로 입력 상태 (#27: newProject/openExisting 폴백 공용)
// mode='open' → agiteam.json 파일 경로 입력, mode='new' → 새 프로젝트 폴더 경로 입력
const showPathInput = ref(false)
const pathInputMode = ref<'open' | 'new'>('open')
const manualPath = ref('')
const pathInputError = ref('')

const pathInputLabel = computed(() =>
  pathInputMode.value === 'new'
    ? '새 프로젝트 폴더 경로 직접 입력'
    : 'agiteam.json 경로 직접 입력',
)
const pathInputPlaceholder = computed(() =>
  pathInputMode.value === 'new'
    ? '/Users/yourname/Projects/myproject'
    : '/Users/yourname/Projects/myproject/agiteam.json',
)

// Tauri App의 실제 버전 가져오기 (mock 폴백)
onMounted(async () => {
  // 개발 모드: 샘플 프로젝트 자동 로드 (DEV_AUTO_OPEN 설정 시)
  const autoOpenPath = localStorage.getItem('lugh:dev-auto-open')
  if (autoOpenPath) {
    try {
      await projectStore.load(autoOpenPath)
      router.push('/boot')
      return
    } catch {
      // 자동 열기 실패 시 일반 런처로 계속
      localStorage.removeItem('lugh:dev-auto-open')
    }
  }

  // 첫 실행 시 가이드 화면으로 자동 이동
  if (!localStorage.getItem('lugh:first-run')) {
    localStorage.setItem('lugh:first-run', '1')
    router.push('/guide')
    return
  }

  try {
    const { getVersion } = await import('@tauri-apps/api/app')
    appVersion.value = await getVersion()
  } catch {
    // 개발 환경 폴백
  }
})

function formatRelativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime()
  const mins = Math.floor(diff / 60000)
  if (mins < 1) return '방금'
  if (mins < 60) return `${mins}분 전`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}시간 전`
  const days = Math.floor(hours / 24)
  return `${days}일 전`
}

async function openRecent(project: RecentProject) {
  try {
    await projectStore.load(project.path)
    router.push('/boot')
  } catch {
    projectStore.removeRecentProject(project.path)
    alert('프로젝트 경로를 찾을 수 없습니다. 목록에서 제거했습니다.')
  }
}

function openManualPathInput(mode: 'open' | 'new') {
  pathInputMode.value = mode
  showPathInput.value = true
  pathInputError.value = ''
  manualPath.value = ''
}

async function newProject() {
  // #13 fix: 새 프로젝트 경로를 먼저 선택 → path 쿼리 포함하여 이동해야 persist 가능
  try {
    const { open: dialogOpen } = await import('@tauri-apps/plugin-dialog')
    const selected = await dialogOpen({ directory: true, multiple: false })
    if (selected && typeof selected === 'string') {
      router.push(`/settings?mode=new&path=${encodeURIComponent(selected)}`)
    }
    // 취소 시 아무것도 하지 않음
  } catch (e) {
    // #27: dialog 실패 시 openExisting과 대칭으로 인라인 폴더 경로 입력 폴백 제공
    //      (경로 없는 mode=new 강제 이동 제거 — 막다른 길 차단)
    console.error('[launcher] newProject folder dialog failed:', e)
    openManualPathInput('new')
  }
}

async function openExisting() {
  try {
    const { open: dialogOpen } = await import('@tauri-apps/plugin-dialog')
    const selected = await dialogOpen({
      filters: [{ name: 'agiteam.json', extensions: ['json'] }],
      multiple: false,
    })
    if (selected && typeof selected === 'string') {
      // agiteam.json 파일 경로에서 부모 디렉토리 추출
      // '/path/to/project/agiteam.json' → '/path/to/project'
      const dirPath = selected.replace(/[/\\][^/\\]+$/, '')
      router.push(`/settings?path=${encodeURIComponent(dirPath)}`)
    }
  } catch (e) {
    // #27: tauri-plugin-dialog 미설치/예외 환경 — 인라인 경로 입력 UI 표시 (silent swallow 금지)
    console.error('[launcher] openExisting dialog failed:', e)
    openManualPathInput('open')
  }
}

async function confirmManualPath() {
  const p = manualPath.value.trim()
  if (!p) {
    pathInputError.value = '경로를 입력해주세요'
    return
  }
  if (pathInputMode.value === 'new') {
    // 새 프로젝트: 입력한 폴더 경로를 path 쿼리로 실어 설정(신규 작성) 화면으로 이동
    showPathInput.value = false
    router.push(`/settings?mode=new&path=${encodeURIComponent(p)}`)
    return
  }
  // 기존 열기: 즉시 로드 후 부팅 화면으로
  try {
    await projectStore.load(p)
    showPathInput.value = false
    router.push('/boot')
  } catch (e) {
    console.error('[launcher] manual path load failed:', e)
    pathInputError.value = '파일을 열 수 없습니다. 경로를 확인해주세요'
  }
}

function cancelPathInput() {
  showPathInput.value = false
  manualPath.value = ''
  pathInputError.value = ''
}

// #27: 내장 브라우저는 config 불요(독립 창, DS-60 §6.3) — none 상태(홈)에서도 열 수 있다.
async function openEmbeddedBrowser() {
  try {
    await openBrowser('https://duckduckgo.com/')
  } catch (e) {
    console.error('[launcher] browser_open failed:', e)
    showToast('브라우저를 열 수 없습니다', 'error')
  }
}
</script>

<template>
  <div class="launcher-bg">
    <!-- 배경 그라디언트 -->
    <div class="bg-glow bg-glow-1" />
    <div class="bg-glow bg-glow-2" />

    <div class="launcher-card">
      <!-- 브랜드 -->
      <div class="brand">
        <div class="brand-mark">A</div>
        <div>
          <h1 class="brand-title">AgiTeamBuilder</h1>
          <p class="brand-sub">Desktop</p>
        </div>
      </div>
      <p class="version">v{{ appVersion }}</p>

      <!-- 최근 프로젝트 -->
      <div class="section">
        <p class="section-label">최근 프로젝트</p>
        <div v-if="projectStore.recentProjects.length === 0" class="empty-recent">
          최근 프로젝트가 없습니다
        </div>
        <ul v-else class="recent-list">
          <li
            v-for="p in projectStore.recentProjects"
            :key="p.path"
            class="recent-item"
            @click="openRecent(p)"
          >
            <div class="recent-dot" />
            <div class="recent-info">
              <span class="recent-name">{{ p.displayName || p.name }}</span>
              <span class="recent-path truncate">{{ p.path }}</span>
            </div>
            <span class="recent-time">{{ formatRelativeTime(p.lastOpened) }}</span>
          </li>
        </ul>
      </div>

      <!-- 버튼 -->
      <div class="btn-row">
        <button class="btn btn-primary" @click="newProject">＋ 새 프로젝트</button>
        <button class="btn btn-ghost" @click="openExisting">기존 열기…</button>
      </div>

      <!-- 인라인 경로 입력 (dialog 실패/미설치 폴백 — newProject·openExisting 공용, #27) -->
      <div v-if="showPathInput" class="path-input-section">
        <label class="path-input-label">{{ pathInputLabel }}</label>
        <div class="path-input-row">
          <input
            v-model="manualPath"
            class="path-input"
            :placeholder="pathInputPlaceholder"
            @keyup.enter="confirmManualPath"
            @keyup.escape="cancelPathInput"
            autofocus
          />
        </div>
        <p v-if="pathInputError" class="path-input-error">⚠ {{ pathInputError }}</p>
        <div class="path-input-actions">
          <button class="btn btn-primary" style="height:32px;font-size:12px" @click="confirmManualPath">
            {{ pathInputMode === 'new' ? '다음' : '열기' }}
          </button>
          <button class="btn btn-ghost" style="height:32px;font-size:12px" @click="cancelPathInput">취소</button>
        </div>
      </div>

      <!-- #27: 프로젝트 미선택(none)으로도 사용 가능한 셸 진입 영역 -->
      <div class="shell-section">
        <p class="shell-hint">
          프로젝트를 선택하지 않아도 앱을 사용할 수 있어요.
          준비되면 위에서 프로젝트를 만들거나 열어주세요.
        </p>
        <p v-if="isNone" class="shell-badge">프로젝트 미선택 상태</p>
        <div class="shell-row">
          <button class="btn btn-ghost shell-btn" @click="openEmbeddedBrowser">🌐 내장 브라우저</button>
          <router-link to="/guide" class="btn btn-ghost shell-btn">📖 가이드</router-link>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.launcher-bg {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-base);
  position: relative;
  overflow: hidden;
}

.bg-glow {
  position: absolute;
  border-radius: 50%;
  filter: blur(80px);
  pointer-events: none;
}

.bg-glow-1 {
  width: 400px; height: 400px;
  background: rgba(94, 234, 212, 0.07);
  top: -100px; left: -100px;
}

.bg-glow-2 {
  width: 360px; height: 360px;
  background: rgba(99, 102, 241, 0.08);
  bottom: -80px; right: -80px;
}

.launcher-card {
  width: 480px;
  padding: 40px;
  background: rgba(16, 23, 34, 0.92);
  border: 1px solid var(--line-soft);
  border-radius: 12px;
  box-shadow: var(--shadow);
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.brand {
  display: flex;
  align-items: center;
  gap: 14px;
}

.brand-mark {
  width: 44px; height: 44px;
  border-radius: 10px;
  background: linear-gradient(135deg, #5eead4, #38bdf8);
  display: grid;
  place-items: center;
  font-size: 20px;
  font-weight: 900;
  color: #041016;
  flex-shrink: 0;
}

.brand-title {
  font-size: 20px;
  font-weight: 700;
  color: var(--text-primary);
  line-height: 1.2;
}

.brand-sub {
  font-size: 12px;
  color: var(--text-muted);
  margin: 0;
}

.version {
  font-size: 11px;
  color: var(--text-muted);
  margin: -16px 0 0;
}

.section-label {
  font-size: 11px;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  font-weight: 600;
  margin-bottom: 10px;
}

.empty-recent {
  font-size: 13px;
  color: var(--text-muted);
  padding: 16px 0;
  text-align: center;
  border: 1px dashed var(--line);
  border-radius: 6px;
}

.recent-list {
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.recent-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 7px;
  cursor: pointer;
  transition: background 0.12s;
  border: 1px solid transparent;
}

.recent-item:hover {
  background: rgba(99, 102, 241, 0.08);
  border-color: var(--line-soft);
}

.recent-dot {
  width: 6px; height: 6px;
  border-radius: 50%;
  background: var(--accent);
  flex-shrink: 0;
}

.recent-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.recent-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
}

.recent-path {
  font-size: 11px;
  color: var(--text-muted);
  max-width: 280px;
}

.recent-time {
  font-size: 11px;
  color: var(--text-muted);
  flex-shrink: 0;
}

.btn-row {
  display: flex;
  gap: 10px;
}

.btn {
  height: 38px;
  border-radius: 7px;
  padding: 0 16px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.12s;
  border: 1px solid var(--line);
}

.btn-primary {
  background: linear-gradient(135deg, #5eead4, #38bdf8);
  color: #041016;
  border: none;
  flex: 1;
}

.btn-primary:hover { opacity: 0.88; }

.btn-ghost {
  background: var(--bg-panel-2);
  color: var(--text-primary);
}

.btn-ghost:hover { border-color: var(--accent); }

/* #27: 셸 진입 영역 (프로젝트 미선택으로도 가용) */
.shell-section {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-top: 18px;
  border-top: 1px solid var(--line-soft);
}

.shell-hint {
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.5;
}

.shell-badge {
  align-self: flex-start;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.04em;
  color: var(--text-muted);
  background: var(--bg-panel-2);
  border: 1px solid var(--line-soft);
  border-radius: 999px;
  padding: 2px 10px;
}

.shell-row { display: flex; gap: 10px; }

.shell-btn {
  flex: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  text-decoration: none;
}
</style>
