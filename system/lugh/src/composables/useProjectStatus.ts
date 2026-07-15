// useProjectStatus — 프로젝트 컨텍스트 3상태 파생 (Redmine #27, DS-10 §8.1)
//
// 신규 영속 상태 없이 기존 스토어 교차 파생:
//   none       : projectStore.config === null           (프로젝트 없음 = 홈, 셸 기능만 가용)
//   configured : config !== null && !workspaceStore.isActive (설정 로드됨, 팀 미부팅)
//   active     : workspaceStore.isActive === true        (팀 부팅 완료, 워크스페이스 운용 중)
//
// 상태 전이: none →(생성/열기)→ configured →(부팅)→ active,  역방향 active →(닫기/전환)→ none
import { computed } from 'vue'
import { useProjectStore } from '@/stores/project'
import { useWorkspaceStore } from '@/stores/workspace'

export type ProjectStatus = 'none' | 'configured' | 'active'

export function useProjectStatus() {
  const projectStore = useProjectStore()
  const workspaceStore = useWorkspaceStore()

  const status = computed<ProjectStatus>(() => {
    if (workspaceStore.isActive) return 'active'
    if (projectStore.config !== null) return 'configured'
    return 'none'
  })

  const isNone = computed(() => status.value === 'none')
  const isConfigured = computed(() => status.value === 'configured')
  const isActive = computed(() => status.value === 'active')

  return { status, isNone, isConfigured, isActive }
}
