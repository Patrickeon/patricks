// useRoleStore — 역할별 세션, 상태 뱃지, 대화 로그
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { AgentLifecycleState, AgentMessage, AgentMessageDelta } from '@/ipc/types'

export type RoleStatus = AgentLifecycleState

export type RoleSession = {
  role: string
  name: string
  agent: string
  sessionId: string | null
  status: RoleStatus
  messages: AgentMessage[]
  streamingMessageId: string | null
  streamingBuffer: string
  pendingSequences: Map<number, string> // sequence → delta
  lastSequence: number
}

// PM도 포함하여 총 7개 역할 관리
export const useRoleStore = defineStore('role', () => {
  const sessions = ref<Map<string, RoleSession>>(new Map())

  // ── Computed ───────────────────────────────────────────────
  const readyCount = computed(() => {
    let count = 0
    sessions.value.forEach((s) => {
      if (s.status === 'ready') count++
    })
    return count
  })

  const totalCount = computed(() => sessions.value.size)

  // ── 초기화 ────────────────────────────────────────────────
  function initRoles(roles: { role: string; name: string; agent: string }[]) {
    sessions.value.clear()
    for (const r of roles) {
      sessions.value.set(r.role, {
        role: r.role,
        name: r.name,
        agent: r.agent,
        sessionId: null,
        status: 'idle',
        messages: [],
        streamingMessageId: null,
        streamingBuffer: '',
        pendingSequences: new Map(),
        lastSequence: -1,
      })
    }
  }

  // ── 세션 업데이트 ──────────────────────────────────────────
  function setSessionId(role: string, sessionId: string) {
    const s = sessions.value.get(role)
    if (s) s.sessionId = sessionId
  }

  function applyStatusChanged(payload: {
    role: string
    to: AgentLifecycleState
  }) {
    const s = sessions.value.get(payload.role)
    if (s) s.status = payload.to
  }

  // ── 메시지 관리 ───────────────────────────────────────────
  function addUserMessage(role: string, content: string, messageId: string) {
    const s = sessions.value.get(role)
    if (!s) return
    s.messages.push({
      id: messageId,
      session_id: s.sessionId ?? '',
      role: 'user',
      content,
      created_at: new Date().toISOString(),
      is_streaming: false,
    })
  }

  function startStreaming(role: string, messageId: string) {
    const s = sessions.value.get(role)
    if (!s) return
    s.streamingMessageId = messageId
    s.streamingBuffer = ''
    s.lastSequence = -1
    s.pendingSequences.clear()
    // 빈 assistant 메시지 추가 (스트리밍 placeholder)
    s.messages.push({
      id: messageId,
      session_id: s.sessionId ?? '',
      role: 'assistant',
      content: '',
      created_at: new Date().toISOString(),
      is_streaming: true,
    })
  }

  function appendMessageDelta(payload: AgentMessageDelta) {
    // sessionId → role 역방향 조회
    let session: RoleSession | undefined
    sessions.value.forEach((s) => {
      if (s.sessionId === payload.session_id) session = s
    })
    if (!session) return

    // sequence 순서 보정 (DS-60 §4.3)
    if (payload.sequence === session.lastSequence + 1) {
      session.streamingBuffer += payload.delta
      session.lastSequence = payload.sequence

      // 밀려있던 시퀀스 처리
      let next = session.lastSequence + 1
      while (session.pendingSequences.has(next)) {
        session.streamingBuffer += session.pendingSequences.get(next)!
        session.pendingSequences.delete(next)
        session.lastSequence = next
        next++
      }
    } else {
      session.pendingSequences.set(payload.sequence, payload.delta)
    }

    // messages 배열의 스트리밍 메시지 content 갱신
    const msg = session.messages.find(
      (m) => m.id === session!.streamingMessageId,
    )
    if (msg) msg.content = session.streamingBuffer
  }

  function completeStreaming(role: string, messageId: string) {
    const s = sessions.value.get(role)
    if (!s) return
    const msg = s.messages.find((m) => m.id === messageId)
    if (msg) {
      msg.content = s.streamingBuffer
      msg.is_streaming = false
    }
    s.streamingMessageId = null
    s.streamingBuffer = ''
  }

  function failStreaming(role: string) {
    const s = sessions.value.get(role)
    if (!s) return
    const msg = s.messages.find((m) => m.id === s.streamingMessageId)
    if (msg) msg.is_streaming = false
    s.streamingMessageId = null
  }

  function getSession(role: string): RoleSession | undefined {
    return sessions.value.get(role)
  }

  function getSessionBySessionId(sessionId: string): RoleSession | undefined {
    let found: RoleSession | undefined
    sessions.value.forEach((s) => {
      if (s.sessionId === sessionId) found = s
    })
    return found
  }

  function clearMessages(role: string) {
    const s = sessions.value.get(role)
    if (s) s.messages = []
  }

  // 전체 세션 일괄 초기화 (Redmine #27, DS-10 §8.3 / DS-60 §6.3)
  // 프로젝트 전환·닫기 시 이전 프로젝트의 역할 세션·대화·상태 잔존을 방지한다.
  // (세션 자체 종료는 stop_role IPC로 선행하고, 여기서는 FE 상태만 비운다.)
  function reset() {
    sessions.value.clear()
  }

  // agent:messages_cleared 이벤트 반영 (Redmine #24, DS-60 §5.3)
  // session_id 기준으로 세션을 찾아 메시지 로그·스트리밍 상태를 비운다.
  // 같은 세션이 여러 패널/최대화 뷰에 렌더링되어도 store가 단일 진실 공급원이므로 동기 반영된다.
  function applyMessagesCleared(payload: { session_id: string }) {
    const s = getSessionBySessionId(payload.session_id)
    if (!s) return
    s.messages = []
    s.streamingMessageId = null
    s.streamingBuffer = ''
    s.pendingSequences.clear()
    s.lastSequence = -1
  }

  return {
    sessions,
    readyCount,
    totalCount,
    initRoles,
    setSessionId,
    applyStatusChanged,
    addUserMessage,
    startStreaming,
    appendMessageDelta,
    completeStreaming,
    failStreaming,
    getSession,
    getSessionBySessionId,
    clearMessages,
    applyMessagesCleared,
    reset,
  }
})
