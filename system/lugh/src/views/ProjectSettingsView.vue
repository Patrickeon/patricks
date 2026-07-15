<!-- Screen-02: 프로젝트 설정 (DS-50 §3) -->
<script setup lang="ts">
import { ref, computed, onMounted, nextTick } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useProjectStore } from '@/stores/project'
import { openWorkspace, saveWorkspaceConfig } from '@/ipc/workspace'
import { showToast } from '@/composables/toast'
import type { AgiteamConfig, TeamMember } from '@/ipc/types'

const router = useRouter()
const route = useRoute()
const projectStore = useProjectStore()

type ActiveTab = 'basic' | 'pm' | 'team' | 'timing' | 'persona'
const activeTab = ref<ActiveTab>('basic')
const isSaving = ref(false)
const saveError = ref<string | null>(null)

// ── #27: 신규 작성(mode=new) 워크스페이스 폴더 ──────────────
// dialog 폴백으로 경로 없이 진입했을 때 여기서 폴더를 지정할 수 있게 한다.
// 경로 미설정 시 저장/부팅을 막아 "메모리에만 저장" 반복 토스트를 방지한다.
const isNewMode = computed(() => route.query.mode === 'new')
const workspacePath = ref<string>((route.query.path as string) ?? '')
// 신규 작성인데 워크스페이스 경로가 없으면 디스크에 프로젝트를 세울 수 없다 → 저장/부팅 비활성
const needsWorkspacePath = computed(
  () => isNewMode.value && !projectStore.workspaceId && !workspacePath.value.trim(),
)

async function selectWorkspaceFolder() {
  try {
    const { open: dialogOpen } = await import('@tauri-apps/plugin-dialog')
    const selected = await dialogOpen({ directory: true, multiple: false })
    if (selected && typeof selected === 'string') workspacePath.value = selected
  } catch (e) {
    // dialog 실패 시에도 필드에 직접 입력할 수 있으므로 안내만 남긴다 (silent swallow 금지)
    console.error('[settings] workspace folder dialog failed:', e)
    showToast('폴더 선택 창을 열 수 없습니다. 경로를 직접 입력해주세요', 'warn')
  }
}

// ── 폼 데이터 (agiteam.json 구조 그대로) ─────────────────
const form = ref<AgiteamConfig>({
  project: {
    name: '',
    displayName: '',
    workspace: { name: 'AGI개발팀', color: 'Indigo' },
  },
  persona: { dir: 'brain', commonFile: 'brain/Shared/persona.md' },
  team: [] as TeamMember[],
  pm: {
    name: '박피엠',
    agent: 'claude',
    command: 'claude --dangerously-skip-permissions',
    startupMessage: '',
    startupFiles: [],
  },
  settings: {
    readyTimeout: 30,
    postLaunchDelay: 3,
    readySignalTimeout: 60,
    maxAutoSubmits: 5,
  },
})

// ── 시작 파일 인라인 입력 ────────────────────────────────────
const newStartupFile = ref('')
const showStartupFileInput = ref(false)

function openStartupFileInput() {
  showStartupFileInput.value = true
  nextTick(() => {
    const el = document.getElementById('startup-file-input')
    if (el) (el as HTMLInputElement).focus()
  })
}

function confirmStartupFile() {
  const p = newStartupFile.value.trim()
  if (p) form.value.pm.startupFiles.push(p)
  newStartupFile.value = ''
  showStartupFileInput.value = false
}

function cancelStartupFile() {
  newStartupFile.value = ''
  showStartupFileInput.value = false
}

const colorOptions = ['Indigo', 'Red', 'Green', 'Blue', 'Purple']
const agentOptions = ['claude', 'codex', 'gemini'] as const
const layoutOptions = [
  'middle_top', 'middle_mid', 'middle_bottom',
  'right_top', 'right_mid', 'right_bottom',
]

onMounted(async () => {
  const pathParam = route.query.path as string | undefined
  if (pathParam) {
    try {
      await projectStore.load(pathParam)
      if (projectStore.config) {
        form.value = JSON.parse(JSON.stringify(projectStore.config))
      }
    } catch (e) {
      saveError.value = '설정 파일 로드 실패: ' + String(e)
    }
  } else if (projectStore.config) {
    form.value = JSON.parse(JSON.stringify(projectStore.config))
  }
})

// ── 팀원 카드 조작 ────────────────────────────────────────
function addTeamMember() {
  form.value.team.push({
    role: '',
    name: '',
    agent: 'claude',
    command: 'claude --dangerously-skip-permissions',
    layout: 'middle_top',
  })
}

function removeTeamMember(idx: number) {
  form.value.team.splice(idx, 1)
}

// ── 시작 파일 조작 ────────────────────────────────────────
function addStartupFile() {
  openStartupFileInput()
}

function removeStartupFile(idx: number) {
  form.value.pm.startupFiles.splice(idx, 1)
}

// ── 저장 (DV60-005: IPC를 통해 agiteam.json 디스크 저장 + 토스트) ──
async function save() {
  isSaving.value = true
  saveError.value = null
  try {
    // 메모리 상태 먼저 갱신
    projectStore.setConfig(form.value)

    if (projectStore.workspaceId) {
      // 기존 워크스페이스 파일 저장
      await saveWorkspaceConfig(projectStore.workspaceId, form.value)
      showToast('설정이 저장되었습니다', 'ok')
    } else if (workspacePath.value.trim()) {
      // #13/#27: 신규 프로젝트 + 워크스페이스 경로 지정됨
      // 1) openWorkspace 로 workspace_id 확보 (agiteam.json 미존재 허용)
      // 2) saveWorkspaceConfig 로 agiteam.json 디스크 생성
      // 3) projectStore.load 로 store 완전 반영 + recentProjects 등록
      const wsPath = workspacePath.value.trim()
      const summary = await openWorkspace(wsPath)
      await saveWorkspaceConfig(summary.workspace_id, form.value)
      await projectStore.load(wsPath)
      showToast('프로젝트가 생성되었습니다', 'ok')
    } else {
      // #27: 경로 없음 — 저장/부팅 버튼이 비활성이므로 여기 도달하지 않아야 한다(방어).
      //      "메모리에만 저장" 반복 토스트 대신 다음 행동을 안내한다.
      showToast('워크스페이스 폴더를 먼저 선택해주세요', 'warn')
      return
    }
  } catch (e) {
    saveError.value = String(e)
    showToast(`저장 실패: ${String(e)}`, 'error')
  } finally {
    isSaving.value = false
  }
}

async function saveThenBoot() {
  // #13 fix: save() 내부에서 openWorkspace + load 처리가 완료되므로
  // 별도 load() 호출 불필요 — 에러 없으면 /boot 이동
  await save()
  if (!saveError.value) {
    router.push('/boot')
  }
}

function goBack() {
  router.push('/launcher')
}

const tabs: { id: ActiveTab; label: string }[] = [
  { id: 'basic',   label: '기본 정보' },
  { id: 'pm',      label: 'PM 설정' },
  { id: 'team',    label: '팀 구성' },
  { id: 'timing',  label: '타이밍' },
  { id: 'persona', label: '페르소나' },
]
</script>

<template>
  <div class="settings-view">
    <!-- 헤더 -->
    <div class="settings-header">
      <button class="back-btn" @click="goBack">← 뒤로</button>
      <h2>프로젝트 설정</h2>
    </div>

    <!-- 탭 + 폼 -->
    <div class="settings-body">
      <!-- 탭 메뉴 -->
      <nav class="tab-nav">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          class="tab-btn"
          :class="{ active: activeTab === tab.id }"
          @click="activeTab = tab.id"
        >
          {{ tab.label }}
        </button>
      </nav>

      <!-- 폼 본문 -->
      <div class="tab-content">

        <!-- ── 탭1: 기본 정보 ── -->
        <div v-if="activeTab === 'basic'" class="form-section">
          <!-- #27: 신규 작성 시 워크스페이스 폴더 지정 (경로 미설정 시 저장/부팅 비활성) -->
          <div v-if="isNewMode" class="field">
            <label class="field-label">워크스페이스 폴더 <span class="req">*</span></label>
            <div class="folder-row">
              <input
                v-model="workspacePath"
                class="input folder-input"
                placeholder="/Users/yourname/Projects/myproject"
              />
              <button type="button" class="folder-btn" @click="selectWorkspaceFolder">폴더 선택…</button>
            </div>
            <p v-if="needsWorkspacePath" class="folder-hint">
              프로젝트를 디스크에 저장하려면 워크스페이스 폴더가 필요합니다.
            </p>
          </div>
          <div class="field">
            <label class="field-label">프로젝트 이름 (내부)</label>
            <input v-model="form.project.name" class="input" placeholder="lugh" />
          </div>
          <div class="field">
            <label class="field-label">프로젝트 표시 이름</label>
            <input v-model="form.project.displayName" class="input" placeholder="TOS" />
          </div>
          <div class="field">
            <label class="field-label">워크스페이스 이름</label>
            <input v-model="form.project.workspace.name" class="input" placeholder="AGI개발팀" />
          </div>
          <div class="field">
            <label class="field-label">워크스페이스 색상</label>
            <div class="color-picker">
              <button
                v-for="c in colorOptions"
                :key="c"
                class="color-dot"
                :class="{ selected: form.project.workspace.color === c, ['c-' + c.toLowerCase()]: true }"
                @click="form.project.workspace.color = c"
              >
                {{ c }}
              </button>
            </div>
          </div>
        </div>

        <!-- ── 탭2: PM 설정 ── -->
        <div v-if="activeTab === 'pm'" class="form-section">
          <div class="field">
            <label class="field-label">PM 이름</label>
            <input v-model="form.pm.name" class="input" placeholder="박피엠" />
          </div>
          <div class="field">
            <label class="field-label">에이전트 타입</label>
            <select v-model="form.pm.agent" class="select">
              <option v-for="a in agentOptions" :key="a" :value="a">{{ a }}</option>
            </select>
          </div>
          <div class="field">
            <label class="field-label">실행 명령어</label>
            <input v-model="form.pm.command" class="input" placeholder="claude --dangerously-skip-permissions" />
          </div>
          <div class="field">
            <label class="field-label">시작 메시지</label>
            <textarea v-model="form.pm.startupMessage" class="textarea" rows="3" placeholder="팀 워크스페이스 셋업이 완료되었습니다." />
          </div>
          <div class="field">
            <label class="field-label">시작 파일 목록</label>
            <div class="file-list">
              <div
                v-for="(f, idx) in form.pm.startupFiles"
                :key="idx"
                class="file-item"
              >
                <span class="file-path">{{ f }}</span>
                <button class="rm-btn" @click="removeStartupFile(idx)">×</button>
              </div>
            </div>
            <!-- 인라인 파일 추가 입력 -->
            <div v-if="showStartupFileInput" class="inline-input-row">
              <input
                id="startup-file-input"
                v-model="newStartupFile"
                class="input"
                placeholder="brain/PM/persona.md"
                @keyup.enter="confirmStartupFile"
                @keyup.escape="cancelStartupFile"
              />
              <button class="btn-confirm" @click="confirmStartupFile">추가</button>
              <button class="btn-cancel" @click="cancelStartupFile">취소</button>
            </div>
            <button v-else class="add-btn" @click="addStartupFile">＋ 파일 추가</button>
          </div>
        </div>

        <!-- ── 탭3: 팀 구성 ── -->
        <div v-if="activeTab === 'team'" class="form-section">
          <div
            v-for="(member, idx) in form.team"
            :key="idx"
            class="role-card"
          >
            <div class="role-card-header">
              <span class="role-card-num">{{ idx + 1 }}</span>
              <button class="rm-btn" @click="removeTeamMember(idx)">×</button>
            </div>
            <div class="role-grid">
              <div class="field">
                <label class="field-label">역할 ID</label>
                <input v-model="member.role" class="input" placeholder="DeveloperBE" />
              </div>
              <div class="field">
                <label class="field-label">담당자 이름</label>
                <input v-model="member.name" class="input" placeholder="박개발" />
              </div>
              <div class="field">
                <label class="field-label">에이전트</label>
                <select v-model="member.agent" class="select">
                  <option v-for="a in agentOptions" :key="a" :value="a">{{ a }}</option>
                </select>
              </div>
              <div class="field">
                <label class="field-label">레이아웃 슬롯</label>
                <select v-model="member.layout" class="select">
                  <option v-for="l in layoutOptions" :key="l" :value="l">{{ l }}</option>
                </select>
              </div>
            </div>
            <details class="advanced">
              <summary class="advanced-toggle">고급: 실행 명령어</summary>
              <input v-model="member.command" class="input" style="margin-top:8px" />
            </details>
          </div>
          <button class="add-btn" @click="addTeamMember">＋ 역할 추가</button>
        </div>

        <!-- ── 탭4: 타이밍 ── -->
        <div v-if="activeTab === 'timing'" class="form-section">
          <div class="field">
            <label class="field-label">READY 대기 타임아웃 (초)</label>
            <input v-model.number="form.settings.readyTimeout" type="number" class="input" min="10" max="300" />
          </div>
          <div class="field">
            <label class="field-label">부팅 후 지연 (초)</label>
            <input v-model.number="form.settings.postLaunchDelay" type="number" class="input" min="0" max="30" />
          </div>
          <div class="field">
            <label class="field-label">READY 신호 최대 대기 (초)</label>
            <input v-model.number="form.settings.readySignalTimeout" type="number" class="input" min="10" max="600" />
          </div>
          <div class="field">
            <label class="field-label">자동 Enter 최대 횟수</label>
            <input v-model.number="form.settings.maxAutoSubmits" type="number" class="input" min="0" max="20" />
          </div>
        </div>

        <!-- ── 탭5: 페르소나 경로 ── -->
        <div v-if="activeTab === 'persona'" class="form-section">
          <div class="field">
            <label class="field-label">페르소나 디렉토리</label>
            <input v-model="form.persona.dir" class="input" placeholder="brain" />
          </div>
          <div class="field">
            <label class="field-label">Shared 공통 파일</label>
            <input v-model="form.persona.commonFile" class="input" placeholder="brain/Shared/persona.md" />
          </div>
        </div>

      </div>
    </div>

    <!-- 에러 -->
    <div v-if="saveError" class="error-bar">❌ {{ saveError }}</div>

    <!-- #27: 경로 미설정 안내 -->
    <div v-if="needsWorkspacePath" class="hint-bar">
      ⓘ 워크스페이스 폴더를 선택하면 저장·부팅을 진행할 수 있습니다.
    </div>

    <!-- 액션 버튼 -->
    <div class="settings-footer">
      <button class="btn btn-ghost" @click="save" :disabled="isSaving || needsWorkspacePath">
        {{ isSaving ? '저장 중…' : '저장' }}
      </button>
      <button class="btn btn-primary" @click="saveThenBoot" :disabled="isSaving || needsWorkspacePath">
        팀 부팅 시작 →
      </button>
    </div>
  </div>
</template>

<style scoped>
.settings-view {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-base);
  overflow: hidden;
}

.settings-header {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 16px 24px;
  border-bottom: 1px solid var(--line-soft);
  background: var(--bg-panel-header);
  flex-shrink: 0;
}

.back-btn {
  background: none; border: none;
  color: var(--text-muted); font-size: 13px; cursor: pointer;
  padding: 4px 8px;
  border: 1px solid var(--line);
  border-radius: 5px;
}

.back-btn:hover { color: var(--text-primary); border-color: var(--accent); }

h2 { font-size: 16px; font-weight: 600; color: var(--text-primary); }

.settings-body {
  flex: 1;
  display: flex;
  overflow: hidden;
}

/* ── 탭 ── */
.tab-nav {
  width: 120px;
  flex-shrink: 0;
  padding: 16px 0;
  border-right: 1px solid var(--line-soft);
  display: flex;
  flex-direction: column;
  gap: 2px;
  background: var(--bg-panel-2);
}

.tab-btn {
  text-align: left;
  padding: 8px 16px;
  background: none;
  border: none;
  color: var(--text-muted);
  font-size: 13px;
  cursor: pointer;
  border-left: 2px solid transparent;
  transition: all 0.1s;
}

.tab-btn:hover { color: var(--text-primary); }
.tab-btn.active { color: var(--text-primary); border-left-color: var(--accent); background: rgba(99,102,241,0.07); }

.tab-content {
  flex: 1;
  overflow-y: auto;
  padding: 24px;
}

/* ── 폼 ── */
.form-section { display: flex; flex-direction: column; gap: 18px; max-width: 520px; }

.field { display: flex; flex-direction: column; gap: 6px; }

.field-label {
  font-size: 12px;
  color: var(--text-muted);
  font-weight: 500;
}

.input, .select, .textarea {
  background: var(--bg-input);
  border: 1px solid var(--line);
  border-radius: 6px;
  padding: 8px 10px;
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
  transition: border-color 0.1s;
}

.input:focus, .select:focus, .textarea:focus {
  outline: none;
  border-color: var(--accent);
}

.textarea { min-height: 80px; resize: vertical; line-height: 1.4; }

.select { cursor: pointer; }

/* ── 색상 선택 ── */
.color-picker { display: flex; flex-wrap: wrap; gap: 6px; }

.color-dot {
  padding: 5px 12px;
  border-radius: 999px;
  border: 2px solid var(--line);
  background: var(--bg-panel-2);
  color: var(--text-muted);
  font-size: 12px;
  cursor: pointer;
}

.color-dot.selected { border-color: var(--accent); color: var(--text-primary); }
.color-dot.c-indigo.selected { border-color: #6366f1; }
.color-dot.c-red.selected    { border-color: #ef4444; }
.color-dot.c-green.selected  { border-color: #22c55e; }
.color-dot.c-blue.selected   { border-color: #3b82f6; }
.color-dot.c-purple.selected { border-color: #a78bfa; }

/* ── 파일 목록 ── */
.file-list { display: flex; flex-direction: column; gap: 4px; margin-bottom: 6px; }

.inline-input-row {
  display: flex;
  gap: 6px;
  align-items: center;
  margin-bottom: 4px;
}
.inline-input-row .input { flex: 1; }

.btn-confirm {
  height: 34px; padding: 0 12px;
  background: rgba(99,102,241,0.15); border: 1px solid var(--accent);
  border-radius: 6px; color: var(--accent); font-size: 12px; font-weight: 600;
  cursor: pointer; white-space: nowrap;
}
.btn-confirm:hover { background: rgba(99,102,241,0.25); }

.btn-cancel {
  height: 34px; padding: 0 12px;
  background: transparent; border: 1px solid var(--line);
  border-radius: 6px; color: var(--text-muted); font-size: 12px;
  cursor: pointer; white-space: nowrap;
}
.btn-cancel:hover { border-color: var(--error); color: var(--error); }

.file-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  background: var(--bg-panel-2);
  border: 1px solid var(--line-soft);
  border-radius: 5px;
}

.file-path { flex: 1; font-size: 12px; color: var(--text-primary); font-family: monospace; }

/* ── 역할 카드 ── */
.role-card {
  padding: 14px;
  background: var(--bg-panel-2);
  border: 1px solid var(--line-soft);
  border-radius: 7px;
}

.role-card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.role-card-num { font-size: 12px; color: var(--text-muted); font-weight: 600; }

.role-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.advanced summary.advanced-toggle {
  font-size: 11px;
  color: var(--text-muted);
  cursor: pointer;
  margin-top: 10px;
}

/* ── 공통 버튼 ── */
.rm-btn {
  background: none; border: none;
  color: var(--text-muted); font-size: 16px; cursor: pointer;
  width: 24px; height: 24px; display: grid; place-items: center;
  border-radius: 4px;
}

.rm-btn:hover { color: var(--error); background: rgba(239,68,68,0.1); }

.add-btn {
  height: 34px;
  background: rgba(99,102,241,0.08);
  border: 1px dashed var(--accent);
  border-radius: 6px;
  color: var(--accent);
  font-size: 13px;
  cursor: pointer;
  padding: 0 14px;
}

.add-btn:hover { background: rgba(99,102,241,0.15); }

/* ── #27: 워크스페이스 폴더 필드 ── */
.req { color: var(--error); }
.folder-row { display: flex; gap: 8px; align-items: center; }
.folder-input { flex: 1; }
.folder-btn {
  height: 34px; padding: 0 12px; white-space: nowrap;
  background: rgba(99,102,241,0.12); border: 1px solid var(--accent);
  border-radius: 6px; color: var(--accent); font-size: 12px; font-weight: 600;
  cursor: pointer;
}
.folder-btn:hover { background: rgba(99,102,241,0.22); }
.folder-hint { font-size: 11px; color: var(--text-muted); margin-top: 2px; }

/* ── #27: 안내 바 ── */
.hint-bar {
  padding: 8px 24px;
  background: rgba(99,102,241,0.08);
  border-top: 1px solid rgba(99,102,241,0.2);
  color: var(--text-muted);
  font-size: 12px;
  flex-shrink: 0;
}

/* ── 에러 바 ── */
.error-bar {
  padding: 10px 24px;
  background: rgba(239,68,68,0.08);
  border-top: 1px solid rgba(239,68,68,0.2);
  color: var(--error);
  font-size: 13px;
  flex-shrink: 0;
}

/* ── 하단 버튼 ── */
.settings-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 14px 24px;
  border-top: 1px solid var(--line-soft);
  background: var(--bg-panel-header);
  flex-shrink: 0;
}

.btn { height: 36px; padding: 0 16px; border-radius: 6px; font-size: 13px; font-weight: 600; cursor: pointer; border: 1px solid var(--line); transition: all 0.12s; }
.btn-ghost { background: var(--bg-panel-2); color: var(--text-primary); }
.btn-ghost:hover { border-color: var(--accent); }
.btn-primary { background: linear-gradient(135deg, #5eead4, #38bdf8); color: #041016; border: none; }
.btn-primary:hover { opacity: 0.88; }
.btn:disabled { opacity: 0.45; cursor: not-allowed; }
</style>
