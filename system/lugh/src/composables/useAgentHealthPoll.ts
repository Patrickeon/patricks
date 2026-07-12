// useAgentHealthPoll — 30초 주기로 에이전트 세션 상태를 폴링한다.
// WorkspaceView에서 useAgentHealthPoll() 호출 한 번으로 등록.
import { onMounted, onUnmounted } from 'vue'
import { useRoleStore } from '@/stores/role'
// #18 fix: DS-60 §3 — invoke 직접 호출 금지, ipc 래퍼 경유
import { getAgentSession } from '@/ipc/agent'

export function useAgentHealthPoll(intervalMs = 30_000) {
  const roleStore = useRoleStore()
  let timer: ReturnType<typeof setInterval> | null = null

  async function pollOnce() {
    const sessions = Array.from(roleStore.sessions.values())
    await Promise.allSettled(
      sessions
        .filter((s): s is typeof s & { sessionId: string } => s.sessionId !== null)
        .map(async (s) => {
          try {
            const detail = await getAgentSession(s.sessionId)
            roleStore.applyStatusChanged({ role: s.role, to: detail.state })
          } catch {
            roleStore.applyStatusChanged({ role: s.role, to: 'failed' })
          }
        }),
    )
  }

  onMounted(() => {
    pollOnce()
    timer = setInterval(pollOnce, intervalMs)
  })

  onUnmounted(() => {
    if (timer) clearInterval(timer)
  })

  return { pollOnce }
}
