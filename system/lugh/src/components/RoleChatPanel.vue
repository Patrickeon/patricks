<!-- Screen-04b: 팀원 채팅 패널 × 6 (DS-50 §7) -->
<script setup lang="ts">
import { ref, computed, nextTick, watch } from 'vue'
import { useRoleStore } from '@/stores/role'
import { useWorkspaceStore } from '@/stores/workspace'
import { sendAgentMessage, clearSessionMessages } from '@/ipc/agent'
import { useChatAttachments } from '@/composables/useChatAttachments'
import { showToast } from '@/composables/toast'
import AttachmentChips from '@/components/AttachmentChips.vue'
import AppModal from '@/components/AppModal.vue'
import StatusBadge from '@/components/StatusBadge.vue'
import type { AgentLifecycleState } from '@/ipc/types'

const props = defineProps<{
  role: string
  name: string
  agent: string
}>()

// 역할별 컬러 — 세련된 단색 + 헤더 틴트
const ROLE_COLORS: Record<string, { accent: string; tint: string }> = {
  DeveloperBE: { accent: '#2563eb', tint: 'rgba(37,99,235,0.07)' },
  DeveloperFE: { accent: '#0891b2', tint: 'rgba(8,145,178,0.07)' },
  QA:          { accent: '#059669', tint: 'rgba(5,150,105,0.07)' },
  Architect:   { accent: '#d97706', tint: 'rgba(217,119,6,0.07)' },
  DevOps:      { accent: '#e11d48', tint: 'rgba(225,29,72,0.07)' },
  Designer:    { accent: '#9333ea', tint: 'rgba(147,51,234,0.07)' },
}
const roleColor = computed(() => ROLE_COLORS[props.role] ?? { accent: '#6366f1', tint: 'rgba(99,102,241,0.07)' })

const roleStore = useRoleStore()
const workspaceStore = useWorkspaceStore()

const draftMessage = ref('')
const chatLogEl = ref<HTMLDivElement | null>(null)
const fileInputEl = ref<HTMLInputElement | null>(null)
const isSending = ref(false)

// ── 세션 ──────────────────────────────────────────────────
const session = computed(() => roleStore.getSession(props.role))
const status = computed<AgentLifecycleState>(() => session.value?.status ?? 'idle')
const messages = computed(() => session.value?.messages ?? [])
const isMaximized = computed(() => workspaceStore.maximizedRole === props.role)

// ── 첨부 (#21: 파일 선택·클립보드 붙여넣기·드래그&드롭) ────
const attachments = useChatAttachments(() => session.value?.sessionId ?? undefined)

// ── 버튼 활성화 규칙 (DS-60 §5.2 + #21 첨부) ─────────────
// 텍스트 또는 ready 첨부가 있어야 전송 가능 (pending 중에는 대기)
const canSend = computed(() =>
  status.value === 'ready' &&
  !isSending.value &&
  !attachments.hasPending.value &&
  (!!draftMessage.value.trim() || attachments.readyAttachments.value.length > 0),
)

// ── 자동 스크롤 ───────────────────────────────────────────
watch(messages, async () => {
  await nextTick()
  if (chatLogEl.value) chatLogEl.value.scrollTop = chatLogEl.value.scrollHeight
}, { deep: true })

// ── 전송 ──────────────────────────────────────────────────
async function sendMessage() {
  if (!canSend.value) return
  const content = draftMessage.value.trim()
  const readyAtts = attachments.readyAttachments.value
  const sessionId = session.value?.sessionId
  if (!sessionId) return

  isSending.value = true
  draftMessage.value = ''
  try {
    // #21: failed 칩은 payload에서 제외 — ready 첨부만 전달 (DS-60 §4.1)
    const ack = await sendAgentMessage(sessionId, content, readyAtts)
    roleStore.addUserMessage(props.role, content, ack.user_message_id)
    attachments.clear() // 전송 성공 시에만 첨부 초기화
  } catch (e) {
    draftMessage.value = content // 실패 시 복원 (첨부 칩은 유지)
  } finally {
    isSending.value = false
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    sendMessage()
  }
}

function toggleMaximize() {
  if (isMaximized.value) {
    workspaceStore.restorePanel()
  } else {
    workspaceStore.maximizePanel(props.role)
  }
}

function isStreamingMsg(msgId: string) {
  return session.value?.streamingMessageId === msgId
}

// agent 단축명
function agentLabel(a: string) {
  if (a === 'claude') return 'CC'
  if (a === 'codex')  return 'CX'
  if (a === 'gemini') return 'GM'
  return a.toUpperCase().slice(0, 2)
}

// ── 대화 초기화 (Redmine #24, DS-60 §3.2 clear_session_messages) ─
// running/booting 상태에서는 SESSION_BUSY 가드에 걸리므로 버튼을 비활성화한다.
const showClearConfirm = ref(false)
const isClearing = ref(false)

const canClear = computed(() =>
  !!session.value?.sessionId &&
  messages.value.length > 0 &&
  status.value !== 'running' &&
  status.value !== 'booting',
)

function openClearConfirm() {
  if (!canClear.value) return
  showClearConfirm.value = true
}

async function confirmClear() {
  const sessionId = session.value?.sessionId
  if (!sessionId) {
    showClearConfirm.value = false
    return
  }
  isClearing.value = true
  try {
    // 실제 store 반영은 agent:messages_cleared 이벤트 구독(WorkspaceView → roleStore.applyMessagesCleared)에서 수행한다
    const result = await clearSessionMessages(sessionId)
    showToast(`대화가 초기화되었습니다 (${result.cleared_count}건 삭제)`, 'ok')
  } catch (e) {
    showToast(`대화 초기화 실패: ${String(e)}`, 'error')
  } finally {
    isClearing.value = false
    showClearConfirm.value = false
  }
}
</script>

<template>
  <div class="role-panel" :class="{ maximized: isMaximized }"
    :style="{ '--role-accent': roleColor.accent }">
    <!-- 헤더 -->
    <div class="panel-header" :style="{ background: roleColor.tint }">
      <div class="header-left">
        <span class="agent-dot" :class="props.agent">{{ agentLabel(props.agent) }}</span>
        <span class="role-name">{{ props.role.replace('Developer', 'Dev') }}</span>
        <span class="member-name">{{ props.name }}</span>
      </div>
      <div class="header-right">
        <StatusBadge :state="status" size="sm" />
        <button
          class="icon-btn"
          title="대화 초기화"
          :disabled="!canClear"
          @click="openClearConfirm"
        >🗑</button>
        <button class="icon-btn" :title="isMaximized ? '원래대로' : '최대화'" @click="toggleMaximize">
          {{ isMaximized ? '⊡' : '⤢' }}
        </button>
      </div>
    </div>

    <!-- 대화 로그 -->
    <div ref="chatLogEl" class="chat-log">
      <div v-if="messages.length === 0" class="chat-empty">
        <div class="empty-dot" />
        <span>대기 중</span>
      </div>
      <div
        v-for="msg in messages"
        :key="msg.id"
        class="msg"
        :class="msg.role"
      >
        <span class="msg-role-label">{{ msg.role === 'user' ? '▶' : props.name }}</span>
        <span class="msg-content">
          {{ msg.content }}
          <span v-if="isStreamingMsg(msg.id)" class="streaming-cursor">▌</span>
        </span>
      </div>
    </div>

    <!-- 입력 (#21: drop zone — 파일 드래그&드롭 첨부) -->
    <div
      class="input-zone"
      :class="{ 'drag-over': attachments.isDragOver.value }"
      @dragover="attachments.onDragOver"
      @dragleave="attachments.onDragLeave"
      @drop="attachments.onDrop"
    >
      <!-- 첨부 미리보기 칩 -->
      <AttachmentChips :chips="attachments.chips.value" @remove="attachments.removeChip" />

      <div class="input-row">
        <!-- #21: 첨부 버튼 → 숨김 file input (OS 파일 선택 다이얼로그) -->
        <input
          ref="fileInputEl"
          type="file"
          multiple
          class="file-input-hidden"
          accept="image/png,image/jpeg,image/webp,image/gif,.md,.markdown,.txt,.csv,.json,.yaml,.yml,.log,.pdf"
          @change="attachments.onFileInputChange"
        />
        <button
          class="attach-btn"
          title="이미지·문서 첨부"
          :disabled="status !== 'ready'"
          @click="fileInputEl?.click()"
        >📎</button>
        <input
          v-model="draftMessage"
          class="msg-input"
          :placeholder="status === 'ready' ? '메시지 입력 (Enter 전송)' : status.toUpperCase()"
          :disabled="status !== 'ready' || isSending"
          @keydown="onKeydown"
          @paste="attachments.onPaste"
        />
        <button class="send-btn" :disabled="!canSend" @click="sendMessage">→</button>
      </div>
    </div>

    <!-- 대화 초기화 확인 다이얼로그 (Redmine #24) -->
    <AppModal
      v-if="showClearConfirm"
      title="대화 초기화"
      width="380px"
      @close="showClearConfirm = false"
    >
      <p class="confirm-text">
        {{ props.name }}과의 대화 히스토리를 전부 삭제합니다.
        삭제된 대화는 복구할 수 없습니다. 계속하시겠습니까?
      </p>
      <template #footer>
        <button class="btn btn-ghost" :disabled="isClearing" @click="showClearConfirm = false">취소</button>
        <button class="btn btn-danger" :disabled="isClearing" @click="confirmClear">
          {{ isClearing ? '초기화 중…' : '초기화' }}
        </button>
      </template>
    </AppModal>
  </div>
</template>

<style scoped>
.role-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-panel);
  border: 1px solid var(--line-soft);
  border-top: 2.5px solid var(--role-accent, #6366f1);
  border-radius: 8px;
  overflow: hidden;
  transition: box-shadow 0.15s;
  box-shadow: var(--card-shadow);
}

.role-panel:hover {
  box-shadow: 0 4px 16px rgba(0,0,0,0.08), 0 1px 4px rgba(0,0,0,0.04);
}

.role-panel.maximized {
  position: absolute;
  inset: 0;
  z-index: 10;
  border-radius: 0;
  border-top: 2.5px solid var(--role-accent, #6366f1);
}

/* ── 헤더 ── */
.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 10px;
  border-bottom: 1px solid var(--line-soft);
  flex-shrink: 0;
  gap: 6px;
  transition: background 0.2s;
}

.header-left { display: flex; align-items: center; gap: 7px; min-width: 0; }
.header-right { display: flex; align-items: center; gap: 4px; flex-shrink: 0; }

.agent-dot {
  width: 22px; height: 22px;
  border-radius: 5px;
  display: grid;
  place-items: center;
  font-size: 9px;
  font-weight: 800;
  flex-shrink: 0;
  background: var(--role-accent, #6366f1);
  color: #fff;
  letter-spacing: -0.02em;
}

.role-name {
  font-size: 12px;
  font-weight: 700;
  color: var(--text-primary);
  white-space: nowrap;
  letter-spacing: -0.01em;
}

.member-name {
  font-size: 11px;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 60px;
}

.icon-btn {
  width: 22px; height: 22px;
  border: 1px solid var(--line);
  background: transparent;
  border-radius: 5px;
  color: var(--text-muted);
  font-size: 11px;
  cursor: pointer;
  display: grid;
  place-items: center;
  padding: 0;
  transition: all 0.12s;
}

.icon-btn:hover {
  border-color: var(--role-accent, #6366f1);
  color: var(--role-accent, #6366f1);
  background: var(--accent-soft);
}

/* ── 대화 로그 ── */
.chat-log {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.chat-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: var(--text-muted);
  text-align: center;
  padding: 24px 0;
  opacity: 0.6;
}

.empty-dot {
  width: 6px; height: 6px;
  border-radius: 50%;
  background: var(--role-accent, #6366f1);
  opacity: 0.4;
}

.msg {
  font-size: 12px;
  line-height: 1.45;
  display: flex;
  gap: 6px;
  align-items: flex-start;
  padding: 6px 8px;
  border-radius: 5px;
  background: var(--bg-input);
  border: 1px solid var(--line-soft);
}

.msg.user { border-color: rgba(99,102,241,0.25); background: rgba(99,102,241,0.05); }

[data-theme="light"] .msg.user { border-color: rgba(124,92,191,0.2); background: rgba(124,92,191,0.05); }

.msg-role-label {
  color: var(--text-muted);
  font-size: 10px;
  flex-shrink: 0;
  padding-top: 2px;
}

.msg.user .msg-role-label { color: #818cf8; }

[data-theme="light"] .msg.user .msg-role-label { color: #7c5cbf; }

.msg-content {
  color: var(--text-primary);
  white-space: pre-wrap;
  word-break: break-word;
  flex: 1;
}

.streaming-cursor {
  display: inline-block;
  animation: blink 0.8s infinite;
  color: var(--accent);
}

@keyframes blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0; }
}

/* ── 입력 ── */
.input-zone {
  border-top: 1px solid var(--line-soft);
  flex-shrink: 0;
  padding: 0 8px;
  transition: background 0.12s, outline 0.12s;
}

/* #21: 드래그&드롭 hover 상태 */
.input-zone.drag-over {
  background: var(--accent-soft, rgba(99,102,241,0.08));
  outline: 2px dashed var(--accent);
  outline-offset: -3px;
}

.input-row {
  display: flex;
  gap: 4px;
  padding: 6px 0;
  align-items: center;
}

.file-input-hidden { display: none; }

.attach-btn {
  width: 28px; height: 28px;
  border: 1px solid var(--line);
  background: var(--bg-panel-2);
  border-radius: 5px;
  cursor: pointer;
  font-size: 12px;
  display: grid;
  place-items: center;
  flex-shrink: 0;
  transition: all 0.12s;
  padding: 0;
}

.attach-btn:hover:not(:disabled) { border-color: var(--role-accent, var(--accent)); }
.attach-btn:disabled { opacity: 0.35; cursor: not-allowed; }

.msg-input {
  flex: 1;
  background: var(--bg-input);
  border: 1px solid var(--line);
  border-radius: 5px;
  padding: 5px 8px;
  color: var(--text-primary);
  font-size: 12px;
  font-family: inherit;
  min-width: 0;
  transition: border-color 0.12s;
}

.msg-input:focus { outline: none; border-color: var(--accent); }
.msg-input:disabled { opacity: 0.4; }

.send-btn {
  width: 28px; height: 28px;
  border: none;
  background: var(--accent);
  color: #fff;
  border-radius: 5px;
  font-size: 14px;
  cursor: pointer;
  flex-shrink: 0;
  transition: opacity 0.12s;
}

.send-btn:disabled { opacity: 0.35; cursor: not-allowed; }
.send-btn:not(:disabled):hover { opacity: 0.8; }

/* ── 대화 초기화 확인 다이얼로그 ── */
.confirm-text {
  font-size: 13px;
  line-height: 1.6;
  color: var(--text-primary);
  margin: 0;
}

.btn { height: 34px; padding: 0 14px; border-radius: 6px; font-size: 13px; font-weight: 600; cursor: pointer; border: 1px solid var(--line); }
.btn-ghost { background: var(--bg-panel-2); color: var(--text-primary); }
.btn-danger { background: #dc2626; color: #fff; border: none; }
.btn:disabled { opacity: 0.4; cursor: not-allowed; }
</style>
