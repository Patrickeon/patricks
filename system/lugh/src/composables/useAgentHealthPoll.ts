// useAgentHealthPoll — 30초 주기로 에이전트 세션 상태를 폴링한다.
// WorkspaceView에서 useAgentHealthPoll() 호출 한 번으로 등록.
import { onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useRoleStore } from '@/stores/role'
import type { AgentSessionDetail } from '@/ipc/types'

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
            const detail = await invoke<AgentSessionDetail>(
              'get_agent_session',
              { sessionId: s.sessionId },
            )
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
