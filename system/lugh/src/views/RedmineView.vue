<!-- Screen-06: 레드마인 패널 (DS-50 §9) -->
<script setup lang="ts">
import { onMounted, computed } from 'vue'
import { useRedmineStore, REDMINE_STATUSES } from '@/stores/redmine'
import type { RedmineIssue, IssueCreatePayload } from '@/stores/redmine'
import { useProjectStore } from '@/stores/project'
import { useSettingsStore } from '@/stores/settings'
import { useWorkspaceStore } from '@/stores/workspace'
import AppModal from '@/components/AppModal.vue'
import { ref } from 'vue'

const redmineStore    = useRedmineStore()
const projectStore    = useProjectStore()
const settingsStore   = useSettingsStore()
const workspaceStore  = useWorkspaceStore()

// ── 프로젝트 ID 헬퍼 ──────────────────────────────────────────
const projectId = computed(() =>
  settingsStore.redmineProjectId || undefined
)

// #15: 레드마인 패널은 PM 시점 화면 — 역할별 키 api_key_PM 사용 (없으면 백엔드가 단일 키 fallback)
// 향후 에이전트별 호출 경로에서는 각 역할명을 전달해 재사용
const CURRENT_PANEL_ROLE = 'PM'

onMounted(async () => {
  // DV60-004: API 키 상태 확인
  await redmineStore.loadApiKeyStatus()
  // DV60-003: 실 API 시도 (#14 fix: projectId 전달, #15 fix: role 전달)
  await redmineStore.fetchIssues(
    projectStore.workspaceId ?? undefined,
    projectId.value,
    CURRENT_PANEL_ROLE,
  )
})

// ── 이슈 생성 폼 ──────────────────────────────────────────
const createForm = ref<IssueCreatePayload>({
  project_id: 'lugh',
  tracker_id: 1,
  subject: '',
  description: '',
})

function resetCreateForm() {
  createForm.value = {
    project_id: 'lugh',
    tracker_id: 1,
    subject: '',
    description: '',
  }
}

async function submitCreate() {
  if (!createForm.value.subject.trim()) return
  // DV60-003: 실 API로 생성 (실패 시 로컬 추가 폴백)
  await redmineStore.createIssueApi(
    projectStore.workspaceId ?? undefined,
    createForm.value,
    CURRENT_PANEL_ROLE,
  )
  redmineStore.closeCreateModal()
  resetCreateForm()
}

// ── 상태 변경 (DV60-003: 실 API 갱신) ───────────────────────
async function changeStatus(statusId: number) {
  if (!redmineStore.selectedIssue) return
  const status = REDMINE_STATUSES.find((s) => s.id === statusId)
  if (!status) return
  const updated = {
    ...redmineStore.selectedIssue,
    status,
    updated_on: new Date().toISOString(),
  }
  await redmineStore.updateIssueApi(
    projectStore.workspaceId ?? undefined,
    updated,
    undefined,
    CURRENT_PANEL_ROLE,
  )
}

async function changeDoneRatio(e: Event) {
  if (!redmineStore.selectedIssue) return
  const ratio = Number((e.target as HTMLInputElement).value)
  const updated = {
    ...redmineStore.selectedIssue,
    done_ratio: ratio,
    updated_on: new Date().toISOString(),
  }
  await redmineStore.updateIssueApi(
    projectStore.workspaceId ?? undefined,
    updated,
    undefined,
    CURRENT_PANEL_ROLE,
  )
}

// ── 상태 색상 ─────────────────────────────────────────────
function statusColor(statusId: number): string {
  if (statusId === 1) return '#94a3b8'
  if (statusId === 2) return '#fbbf24'
  if (statusId === 3) return '#38bdf8'
  if (statusId === 4) return '#a78bfa'
  if (statusId === 5) return '#22c55e'
  if (statusId === 6) return '#ef4444'
  return '#94a3b8'
}

function trackerLabel(trackerId: number): string {
  if (trackerId === 1) return '결함'
  if (trackerId === 2) return '새기능'
  return '지원'
}

const trackerOptions = [
  { id: 1, name: '결함' },
  { id: 2, name: '새기능' },
  { id: 3, name: '지원' },
]
</script>

<template>
  <div class="redmine-panel">
    <!-- 툴바 -->
    <div class="rm-toolbar">
      <span class="rm-title">레드마인</span>
      <div class="rm-actions">
        <button
          class="tb-btn"
          @click="redmineStore.fetchIssues(projectStore.workspaceId ?? undefined, projectId, CURRENT_PANEL_ROLE)"
        >🔄</button>
        <button class="tb-btn primary" @click="redmineStore.openCreateModal()">＋ 이슈 생성</button>
      </div>
    </div>

    <!-- #14 fix: 프로젝트 ID 미설정 안내 -->
    <div v-if="!settingsStore.redmineProjectId" class="project-id-notice">
      ⚙️ 레드마인 프로젝트 ID가 설정되지 않아 전체 이슈가 표시됩니다.
      <button class="notice-link" @click="workspaceStore.openSettings()">설정에서 입력하기 →</button>
    </div>

    <!-- 이슈 목록 -->
    <div class="issue-list">
      <div class="issue-header">
        <span class="col-id">#</span>
        <span class="col-subject">제목</span>
        <span class="col-status">상태</span>
        <span class="col-user">담당자</span>
        <span class="col-ratio">진척</span>
      </div>
      <div
        v-for="issue in redmineStore.filteredIssues"
        :key="issue.id"
        class="issue-row"
        :class="{ selected: redmineStore.selectedIssue?.id === issue.id }"
        @click="redmineStore.selectIssue(issue)"
      >
        <span class="col-id muted">{{ issue.id }}</span>
        <span class="col-subject">{{ issue.subject }}</span>
        <span class="col-status">
          <span class="status-pill" :style="{ borderColor: statusColor(issue.status.id), color: statusColor(issue.status.id) }">
            {{ issue.status.name }}
          </span>
        </span>
        <span class="col-user muted">{{ issue.assigned_to?.name ?? '—' }}</span>
        <span class="col-ratio">
          <div class="ratio-bar">
            <div class="ratio-fill" :style="{ width: issue.done_ratio + '%' }" />
          </div>
          <span class="ratio-text">{{ issue.done_ratio }}%</span>
        </span>
      </div>
      <div v-if="redmineStore.filteredIssues.length === 0" class="empty-list">
        이슈 없음
      </div>
    </div>

    <!-- 이슈 상세 슬라이드오버 -->
    <Transition name="slide-over">
      <div v-if="redmineStore.showDetail && redmineStore.selectedIssue" class="detail-panel">
        <div class="detail-header">
          <div>
            <span class="detail-id">#{{ redmineStore.selectedIssue.id }}</span>
            <span class="detail-tracker">{{ trackerLabel(redmineStore.selectedIssue.tracker.id) }}</span>
          </div>
          <button class="close-btn" @click="redmineStore.closeDetail()">×</button>
        </div>

        <div class="detail-title">{{ redmineStore.selectedIssue.subject }}</div>

        <!-- 상태 전이 버튼 -->
        <div class="status-btns">
          <button
            v-for="s in REDMINE_STATUSES"
            :key="s.id"
            class="status-btn"
            :class="{ active: redmineStore.selectedIssue.status.id === s.id }"
            :style="{
              borderColor: redmineStore.selectedIssue.status.id === s.id ? statusColor(s.id) : undefined,
              color: redmineStore.selectedIssue.status.id === s.id ? statusColor(s.id) : undefined,
            }"
            :disabled="redmineStore.selectedIssue.status.id === s.id"
            @click="changeStatus(s.id)"
          >{{ s.name }}</button>
        </div>

        <!-- 진척률 슬라이더 -->
        <div class="ratio-section">
          <label class="field-label">진척률: {{ redmineStore.selectedIssue.done_ratio }}%</label>
          <input
            type="range"
            min="0" max="100" step="10"
            :value="redmineStore.selectedIssue.done_ratio"
            class="slider"
            @input="changeDoneRatio"
          />
        </div>

        <!-- 내용 -->
        <div v-if="redmineStore.selectedIssue.description" class="detail-desc">
          {{ redmineStore.selectedIssue.description }}
        </div>
      </div>
    </Transition>

    <!-- 이슈 생성 모달 -->
    <AppModal
      v-if="redmineStore.showCreateModal"
      title="이슈 생성"
      @close="redmineStore.closeCreateModal()"
    >
      <div class="create-form">
        <div class="field">
          <label class="field-label">트래커</label>
          <div class="tracker-radio">
            <label
              v-for="t in trackerOptions"
              :key="t.id"
              class="radio-label"
              :class="{ selected: createForm.tracker_id === t.id }"
            >
              <input v-model.number="createForm.tracker_id" type="radio" :value="t.id" hidden />
              {{ t.name }}
            </label>
          </div>
        </div>
        <div class="field">
          <label class="field-label">제목 *</label>
          <input v-model="createForm.subject" class="input" placeholder="이슈 제목" />
        </div>
        <div class="field">
          <label class="field-label">내용</label>
          <textarea v-model="createForm.description" class="textarea" rows="4" placeholder="이슈 내용 (마크다운 지원)" />
        </div>
      </div>
      <template #footer>
        <button class="btn btn-ghost" @click="redmineStore.closeCreateModal()">취소</button>
        <button class="btn btn-primary" @click="submitCreate" :disabled="!createForm.subject.trim()">생성</button>
      </template>
    </AppModal>
  </div>
</template>

<style scoped>
.redmine-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
}

.rm-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 14px;
  border-bottom: 1px solid var(--line-soft);
  background: var(--bg-panel-header);
  flex-shrink: 0;
}

.rm-title { font-size: 13px; font-weight: 600; color: var(--text-primary); }
.rm-actions { display: flex; gap: 6px; }

.tb-btn {
  height: 28px;
  padding: 0 10px;
  border-radius: 5px;
  border: 1px solid var(--line);
  background: var(--bg-panel-2);
  color: var(--text-primary);
  font-size: 12px;
  cursor: pointer;
}

.tb-btn.primary {
  background: rgba(99,102,241,0.12);
  border-color: rgba(99,102,241,0.4);
  color: var(--accent);
}

/* ── 프로젝트 ID 미설정 안내 (#14) ── */
.project-id-notice {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 14px;
  background: rgba(251,191,36,0.08);
  border-bottom: 1px solid rgba(251,191,36,0.25);
  color: var(--busy);
  font-size: 12px;
  flex-shrink: 0;
}

.notice-link {
  background: none;
  border: none;
  color: var(--accent);
  font-size: 12px;
  cursor: pointer;
  padding: 0;
  text-decoration: underline;
  white-space: nowrap;
}

/* ── 이슈 목록 ── */
.issue-list { flex: 1; overflow-y: auto; }

.issue-header, .issue-row {
  display: grid;
  grid-template-columns: 40px 1fr 70px 70px 80px;
  gap: 8px;
  padding: 8px 12px;
  font-size: 12px;
  align-items: center;
}

.issue-header {
  border-bottom: 1px solid var(--line-soft);
  background: var(--bg-panel-2);
  color: var(--text-muted);
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  position: sticky;
  top: 0;
}

.issue-row {
  border-bottom: 1px solid var(--line-soft);
  cursor: pointer;
  transition: background 0.1s;
}

.issue-row:hover   { background: rgba(99,102,241,0.05); }
.issue-row.selected { background: rgba(99,102,241,0.1); }

.muted { color: var(--text-muted); }

.status-pill {
  display: inline-block;
  padding: 2px 7px;
  border-radius: 4px;
  border: 1px solid;
  font-size: 11px;
}

.ratio-bar {
  height: 4px;
  background: var(--bg-input);
  border-radius: 2px;
  overflow: hidden;
  margin-bottom: 2px;
}

.ratio-fill {
  height: 100%;
  background: linear-gradient(90deg, #5eead4, #38bdf8);
  border-radius: 2px;
  transition: width 0.3s;
}

.ratio-text { font-size: 10px; color: var(--text-muted); }

.empty-list { text-align: center; padding: 24px; font-size: 13px; color: var(--text-muted); }

/* ── 상세 슬라이드오버 ── */
.detail-panel {
  position: absolute;
  top: 0; right: 0; bottom: 0;
  width: 320px;
  background: var(--bg-panel);
  border-left: 1px solid var(--line-soft);
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 14px;
  overflow-y: auto;
  box-shadow: -8px 0 24px rgba(0,0,0,0.3);
  z-index: 5;
}

.slide-over-enter-active { transition: transform 0.18s ease; }
.slide-over-leave-active { transition: transform 0.15s ease; }
.slide-over-enter-from   { transform: translateX(100%); }
.slide-over-leave-to     { transform: translateX(100%); }

.detail-header { display: flex; justify-content: space-between; align-items: flex-start; }
.detail-id { font-size: 12px; color: var(--text-muted); margin-right: 8px; }
.detail-tracker {
  font-size: 11px;
  padding: 2px 7px;
  border-radius: 4px;
  border: 1px solid var(--accent);
  color: var(--accent);
}

.close-btn { background: none; border: none; color: var(--text-muted); font-size: 18px; cursor: pointer; }

.detail-title { font-size: 14px; font-weight: 600; color: var(--text-primary); line-height: 1.4; }

.status-btns { display: flex; flex-wrap: wrap; gap: 5px; }

.status-btn {
  padding: 4px 10px;
  border-radius: 5px;
  border: 1px solid var(--line);
  background: var(--bg-panel-2);
  color: var(--text-muted);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.1s;
}

.status-btn:disabled { opacity: 0.5; cursor: default; }
.status-btn.active { font-weight: 600; }

.ratio-section { display: flex; flex-direction: column; gap: 6px; }
.field-label { font-size: 12px; color: var(--text-muted); }

.slider {
  width: 100%;
  accent-color: var(--accent);
  cursor: pointer;
}

.detail-desc {
  font-size: 13px;
  color: var(--text-primary);
  line-height: 1.6;
  white-space: pre-wrap;
}

/* ── 모달 폼 ── */
.create-form { display: flex; flex-direction: column; gap: 14px; }
.field { display: flex; flex-direction: column; gap: 6px; }

.tracker-radio { display: flex; gap: 8px; }

.radio-label {
  padding: 5px 12px;
  border-radius: 5px;
  border: 1px solid var(--line);
  background: var(--bg-panel-2);
  color: var(--text-muted);
  font-size: 12px;
  cursor: pointer;
}

.radio-label.selected { border-color: var(--accent); color: var(--accent); background: rgba(99,102,241,0.08); }

.input, .textarea {
  background: var(--bg-input);
  border: 1px solid var(--line);
  border-radius: 6px;
  padding: 8px 10px;
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
}

.input:focus, .textarea:focus { outline: none; border-color: var(--accent); }
.textarea { min-height: 80px; resize: vertical; line-height: 1.4; }

.btn { height: 34px; padding: 0 14px; border-radius: 6px; font-size: 13px; font-weight: 600; cursor: pointer; border: 1px solid var(--line); }
.btn-ghost { background: var(--bg-panel-2); color: var(--text-primary); }
.btn-primary { background: linear-gradient(135deg, #5eead4, #38bdf8); color: #041016; border: none; }
.btn:disabled { opacity: 0.4; cursor: not-allowed; }

/* ── 라이트 모드 보정 ── */
[data-theme="light"] .issue-row:hover    { background: rgba(124,92,191,0.07); }
[data-theme="light"] .issue-row.selected { background: rgba(124,92,191,0.13); }
[data-theme="light"] .detail-panel       { box-shadow: -4px 0 16px rgba(80,40,0,0.10); }
</style>
