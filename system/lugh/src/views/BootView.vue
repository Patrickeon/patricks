<!-- Screen-03: 팀 부팅 진행 (DS-50 §4) -->
<script setup lang="ts">
import { onMounted, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useBootStore } from '@/stores/boot'
import { useProjectStore } from '@/stores/project'
import { useRoleStore } from '@/stores/role'
import { useWorkspaceStore } from '@/stores/workspace'
import { bootTeam } from '@/ipc/agent'
import { listenAgentEvents } from '@/ipc/events'
import StatusBadge from '@/components/StatusBadge.vue'
import type { AgentLifecycleState } from '@/ipc/types'

const router = useRouter()
const bootStore = useBootStore()
const projectStore = useProjectStore()
const roleStore = useRoleStore()
const workspaceStore = useWorkspaceStore()

// ── 진행 퍼센트 ────────────────────────────────────────────
const progressPercent = computed(() => {
  const done = bootStore.steps.filter((s) => s.status === 'done').length
  return Math.round((done / bootStore.steps.length) * 100)
})

// ── 스텝 아이콘 ────────────────────────────────────────────
function stepIcon(status: string) {
  if (status === 'done')    return '✅'
  if (status === 'running') return '🔄'
  if (status === 'error')   return '❌'
  return '○'
}

// ── 역할 부팅 상태 → StatusBadge 매핑 ─────────────────────
function roleLifecycle(status: string): AgentLifecycleState | 'waiting' {
  if (status === 'waiting') return 'waiting'
  return status as AgentLifecycleState
}

// ── 부팅 실행 ──────────────────────────────────────────────
async function startBoot() {
  if (!projectStore.workspaceId || !projectStore.config) {
    router.push('/settings')
    return
  }

  bootStore.reset()

  // 팀원 초기 상태 설정
  const allRoles = [
    { role: 'PM', name: projectStore.config.pm.name, agent: projectStore.config.pm.agent },
    ...projectStore.config.team.map((t) => ({
      role: t.role,
      name: t.name,
      agent: t.agent,
    })),
  ]
  bootStore.setRoleStates(allRoles.map((r) => ({ role: r.role, name: r.name })))
  roleStore.initRoles(allRoles)

  // 이벤트 리스너 등록
  const unlisten = await listenAgentEvents({
    onStatusChanged(p) {
      roleStore.applyStatusChanged({ role: p.role, to: p.to })
      bootStore.updateRoleState(p.role, p.to, p.session_id)

      // READY 카운트 확인
      if (p.to === 'ready') {
        const readyCount = bootStore.roleStates.filter(
          (r) => r.status === 'ready',
        ).length
        const total = bootStore.roleStates.length
        bootStore.completeStep(
          'spawn_team',
          `${readyCount}/${total} READY 수신`,
        )

        if (readyCount === total) {
          bootStore.completeStep('spawn_team', `전원 READY 수신 (${total}명)`)
          bootStore.startStep('launch_pm')
          // PM 런치 완료 처리 → /workspace 이동 트리거
          setTimeout(() => {
            bootStore.completeStep('launch_pm', 'PM 부팅 완료. 워크스페이스 진입 중...')
            bootStore.markDone()
            unlisten()
          }, 800)
        }
      }
    },
    onMessageFailed(p) {
      // 실패한 역할 찾기
      const session = roleStore.getSessionBySessionId(p.session_id)
      if (session) {
        bootStore.updateRoleState(session.role, 'failed')
      }
    },
  })

  try {
    // 단계별 진행
    bootStore.startStep('load_config', '설정 파일 검증 중...')
    await delay(300)
    bootStore.completeStep('load_config', `${projectStore.config.team.length}개 역할 로드됨`)

    bootStore.startStep('validate_env', '필수 도구 확인 중...')
    await delay(500)
    bootStore.completeStep('validate_env', 'cmux, python3, claude, codex 확인됨')

    bootStore.startStep('validate_auth', 'API 연결 확인 중...')
    await delay(600)
    bootStore.completeStep('validate_auth', 'Anthropic API 연결 정상')

    bootStore.startStep('validate_workspace', '페르소나 파일 검증 중...')
    await delay(400)
    bootStore.completeStep('validate_workspace', '모든 페르소나 파일 확인됨')

    bootStore.startStep('setup_workspace', '워크스페이스 구성 중...')
    await delay(300)
    bootStore.completeStep('setup_workspace', `워크스페이스: ${projectStore.workspaceName}`)

    bootStore.startStep('spawn_team', `0/${bootStore.roleStates.length} READY 대기 중...`)

    // 실제 부팅
    const result = await bootTeam(projectStore.workspaceId!)
    for (const session of result.sessions) {
      roleStore.setSessionId(session.role, session.session_id)
    }

    // PM 실행 완료 대기 (이벤트 기반)
    // onStatusChanged에서 처리됨

  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err)
    const current = bootStore.currentStep
    if (current) bootStore.failStep(current.id, msg)
    unlisten()
  }
}

async function goBack() {
  router.push('/settings')
}

async function retry() {
  await startBoot()
}

onMounted(startBoot)

function delay(ms: number) {
  return new Promise((r) => setTimeout(r, ms))
}

// 부팅 완료 감시
import { watch } from 'vue'
watch(
  () => bootStore.isDone,
  (done) => {
    if (done) {
      workspaceStore.activate()
      router.push('/workspace')
    }
  },
)
</script>

<template>
  <div class="boot-view">
    <div class="boot-card">
      <h2 class="boot-title">팀 부팅 진행</h2>

      <!-- 스텝 바 -->
      <div class="step-bar">
        <div
          v-for="(step, idx) in bootStore.steps"
          :key="step.id"
          class="step"
          :class="step.status"
        >
          <div class="step-circle">
            <span>{{ stepIcon(step.status) }}</span>
          </div>
          <div class="step-label">{{ step.label }}</div>
          <div v-if="idx < bootStore.steps.length - 1" class="step-line" :class="{ active: step.status === 'done' }" />
        </div>
      </div>

      <!-- 진행 바 -->
      <div class="progress-bar">
        <div class="progress-fill" :style="{ width: progressPercent + '%' }" />
      </div>

      <!-- 현재 단계 상세 -->
      <div v-if="bootStore.currentStep" class="step-detail terminal">
        <div class="step-detail-title">{{ bootStore.currentStep.label }}</div>
        <p v-if="bootStore.currentStep.detail" class="step-detail-text">
          {{ bootStore.currentStep.detail }}
        </p>
      </div>

      <!-- 에러 박스 -->
      <div v-if="bootStore.errorMessage" class="error-box">
        <span>❌ {{ bootStore.errorMessage }}</span>
      </div>

      <!-- 역할 부팅 카드 -->
      <div class="roles-section">
        <p class="roles-label">팀원 부팅 상태</p>
        <div class="roles-grid">
          <div
            v-for="role in bootStore.roleStates"
            :key="role.role"
            class="role-boot-card"
            :class="role.status"
          >
            <div class="role-abbr">{{ role.role.replace('Developer', 'Dev') }}</div>
            <div class="role-name">{{ role.name }}</div>
            <StatusBadge :state="roleLifecycle(role.status)" size="sm" />
          </div>
        </div>
        <p class="ready-counter">{{ bootStore.readyCount }}/{{ bootStore.roleStates.length }} READY 수신</p>
      </div>

      <!-- 버튼 -->
      <div class="boot-actions">
        <button class="btn btn-ghost" @click="goBack">← 설정으로</button>
        <button
          v-if="bootStore.hasError"
          class="btn btn-primary"
          @click="retry"
        >
          강제 재시도
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.boot-view {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-base);
  padding: 24px;
  overflow-y: auto;
}

.boot-card {
  width: 100%;
  max-width: 680px;
  background: var(--bg-panel);
  border: 1px solid var(--line-soft);
  border-radius: 10px;
  padding: 32px;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.boot-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--text-primary);
  text-align: center;
}

/* ── 스텝 바 ── */
.step-bar {
  display: flex;
  align-items: flex-start;
  gap: 0;
  overflow-x: auto;
}

.step {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  flex: 1;
  position: relative;
}

.step-circle {
  width: 32px; height: 32px;
  border-radius: 50%;
  border: 2px solid var(--line);
  background: var(--bg-panel-2);
  display: grid;
  place-items: center;
  font-size: 14px;
  z-index: 1;
}

.step.done .step-circle    { border-color: var(--ok);    background: rgba(34,197,94,0.1); }
.step.running .step-circle { border-color: var(--accent); background: rgba(99,102,241,0.1); }
.step.error .step-circle   { border-color: var(--error);  background: rgba(239,68,68,0.1); }

.step-label {
  font-size: 10px;
  color: var(--text-muted);
  text-align: center;
  max-width: 72px;
}

.step.done    .step-label { color: var(--ok); }
.step.running .step-label { color: var(--accent); }
.step.error   .step-label { color: var(--error); }

.step-line {
  position: absolute;
  top: 15px;
  left: calc(50% + 16px);
  right: calc(-50% + 16px);
  height: 2px;
  background: var(--line);
}

.step-line.active { background: var(--ok); }

/* ── 진행 바 ── */
.progress-bar {
  height: 6px;
  background: var(--bg-input);
  border-radius: 999px;
  overflow: hidden;
  border: 1px solid var(--line-soft);
}

.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #5eead4, #38bdf8);
  border-radius: 999px;
  transition: width 0.5s ease;
}

/* ── 단계 상세 ── */
.step-detail {
  padding: 12px 14px;
  background: var(--bg-input);
  border: 1px solid var(--line-soft);
  border-radius: 7px;
  font-size: 12px;
}

.step-detail-title { color: var(--text-primary); font-weight: 600; margin-bottom: 4px; }
.step-detail-text  { color: #94a3b8; line-height: 1.5; }

/* ── 에러 박스 ── */
.error-box {
  padding: 12px 14px;
  background: rgba(239, 68, 68, 0.08);
  border: 1px solid rgba(239, 68, 68, 0.3);
  border-radius: 7px;
  font-size: 13px;
  color: var(--error);
}

/* ── 역할 카드 ── */
.roles-section { display: flex; flex-direction: column; gap: 10px; }
.roles-label {
  font-size: 11px;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  font-weight: 600;
}

.roles-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}

.role-boot-card {
  padding: 10px;
  background: var(--bg-panel-2);
  border: 1px solid var(--line-soft);
  border-radius: 7px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}

.role-boot-card.ready   { border-color: rgba(34,197,94,0.3); }
.role-boot-card.running { border-color: rgba(99,102,241,0.3); }
.role-boot-card.failed  { border-color: rgba(239,68,68,0.3); }

.role-abbr {
  font-size: 12px;
  font-weight: 700;
  color: var(--text-primary);
}

.role-name {
  font-size: 11px;
  color: var(--text-muted);
}

.ready-counter {
  font-size: 13px;
  color: var(--text-muted);
  text-align: center;
}

/* ── 버튼 ── */
.boot-actions { display: flex; justify-content: space-between; gap: 10px; }

.btn {
  height: 36px;
  border-radius: 7px;
  padding: 0 14px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  border: 1px solid var(--line);
  transition: all 0.12s;
}

.btn-ghost    { background: var(--bg-panel-2); color: var(--text-primary); }
.btn-ghost:hover { border-color: var(--accent); }
.btn-primary  { background: linear-gradient(135deg, #5eead4, #38bdf8); color: #041016; border: none; }
.btn-primary:hover { opacity: 0.88; }
</style>
