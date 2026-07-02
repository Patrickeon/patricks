// useBootStore — 7단계 부팅 상태머신
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { AgentLifecycleState } from '@/ipc/types'

export type BootStepId =
  | 'load_config'
  | 'validate_env'
  | 'validate_auth'
  | 'validate_workspace'
  | 'setup_workspace'
  | 'spawn_team'
  | 'launch_pm'

export type BootStepStatus = 'idle' | 'running' | 'done' | 'error'

export type BootStep = {
  id: BootStepId
  label: string
  status: BootStepStatus
  detail: string
}

export type RoleBootState = {
  role: string
  name: string
  status: AgentLifecycleState | 'waiting'
  sessionId?: string
}

const STEPS: BootStep[] = [
  { id: 'load_config',       label: '설정 파일 로드',      status: 'idle', detail: '' },
  { id: 'validate_env',      label: '환경 검증',           status: 'idle', detail: '' },
  { id: 'validate_auth',     label: '인증·네트워크 확인',   status: 'idle', detail: '' },
  { id: 'validate_workspace',label: '프로젝트 구조 검증',   status: 'idle', detail: '' },
  { id: 'setup_workspace',   label: '워크스페이스 설정',    status: 'idle', detail: '' },
  { id: 'spawn_team',        label: '팀원 부팅',           status: 'idle', detail: '' },
  { id: 'launch_pm',         label: 'PM 실행',             status: 'idle', detail: '' },
]

export const useBootStore = defineStore('boot', () => {
  const steps = ref<BootStep[]>(STEPS.map((s) => ({ ...s })))
  const roleStates = ref<RoleBootState[]>([])
  const autoSubmitCount = ref(0)
  const errorMessage = ref<string | null>(null)
  const isDone = ref(false)

  // ── Computed ───────────────────────────────────────────────
  const currentStepIndex = computed(() =>
    steps.value.findIndex((s) => s.status === 'running'),
  )
  const currentStep = computed(() =>
    currentStepIndex.value >= 0 ? steps.value[currentStepIndex.value] : null,
  )
  const readyCount = computed(() =>
    roleStates.value.filter((r) => r.status === 'ready').length,
  )
  const hasError = computed(() =>
    steps.value.some((s) => s.status === 'error') || !!errorMessage.value,
  )

  // ── 액션 ──────────────────────────────────────────────────
  function startStep(id: BootStepId, detail = '') {
    const step = steps.value.find((s) => s.id === id)
    if (!step) return
    step.status = 'running'
    step.detail = detail
  }

  function completeStep(id: BootStepId, detail = '') {
    const step = steps.value.find((s) => s.id === id)
    if (!step) return
    step.status = 'done'
    if (detail) step.detail = detail
  }

  function failStep(id: BootStepId, message: string) {
    const step = steps.value.find((s) => s.id === id)
    if (!step) return
    step.status = 'error'
    step.detail = message
    errorMessage.value = message
  }

  function setRoleStates(roles: { role: string; name: string }[]) {
    roleStates.value = roles.map((r) => ({
      role: r.role,
      name: r.name,
      status: 'waiting',
    }))
  }

  function updateRoleState(
    role: string,
    status: AgentLifecycleState | 'waiting',
    sessionId?: string,
  ) {
    const r = roleStates.value.find((s) => s.role === role)
    if (!r) return
    r.status = status
    if (sessionId) r.sessionId = sessionId
  }

  function incrementAutoSubmit() {
    autoSubmitCount.value++
  }

  function markDone() {
    isDone.value = true
  }

  function reset() {
    steps.value = STEPS.map((s) => ({ ...s }))
    roleStates.value = []
    autoSubmitCount.value = 0
    errorMessage.value = null
    isDone.value = false
  }

  return {
    steps,
    roleStates,
    autoSubmitCount,
    errorMessage,
    isDone,
    currentStepIndex,
    currentStep,
    readyCount,
    hasError,
    startStep,
    completeStep,
    failStep,
    setRoleStates,
    updateRoleState,
    incrementAutoSubmit,
    markDone,
    reset,
  }
})
