<!-- Screen-04a: PM 채팅 패널 (DS-50 §6) -->
<script setup lang="ts">
import { ref, computed, nextTick, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useRoleStore } from '@/stores/role'
import { useProjectStore } from '@/stores/project'
import { useWorkspaceStore } from '@/stores/workspace'
import { useDeliverableStore } from '@/stores/deliverable'
import { sendAgentMessage } from '@/ipc/agent'
import { readDocument } from '@/ipc/document'
import { useChatAttachments } from '@/composables/useChatAttachments'
import AttachmentChips from '@/components/AttachmentChips.vue'
import StatusBadge from '@/components/StatusBadge.vue'
import type { AgentLifecycleState } from '@/ipc/types'

const router           = useRouter()
const roleStore        = useRoleStore()
const projectStore     = useProjectStore()
const workspaceStore   = useWorkspaceStore()
const deliverableStore = useDeliverableStore()

const draftMessage = ref('')
const chatLogEl = ref<HTMLDivElement | null>(null)
const fileInputEl = ref<HTMLInputElement | null>(null)
const isSending = ref(false)

// ── PM 세션 ───────────────────────────────────────────────
const pmSession = computed(() => roleStore.getSession('PM'))
const pmStatus = computed<AgentLifecycleState>(() => pmSession.value?.status ?? 'idle')
const messages = computed(() => pmSession.value?.messages ?? [])

// ── 첨부 (#21: 파일 선택·클립보드 붙여넣기·드래그&드롭) ────
const attachments = useChatAttachments(() => pmSession.value?.sessionId ?? undefined)

// 텍스트 또는 ready 첨부가 있어야 전송 가능 (pending 중에는 대기)
const canSend = computed(() =>
  pmStatus.value === 'ready' &&
  !isSending.value &&
  !attachments.hasPending.value &&
  (!!draftMessage.value.trim() || attachments.readyAttachments.value.length > 0),
)

// ── 자동 스크롤 ───────────────────────────────────────────
watch(messages, async () => {
  await nextTick()
  if (chatLogEl.value) {
    chatLogEl.value.scrollTop = chatLogEl.value.scrollHeight
  }
}, { deep: true })

// ── 메시지 전송 ───────────────────────────────────────────
async function sendMessage() {
  if (!canSend.value) return
  const content = draftMessage.value.trim()
  const readyAtts = attachments.readyAttachments.value

  const sessionId = pmSession.value?.sessionId
  if (!sessionId) return

  isSending.value = true
  draftMessage.value = ''
  try {
    // #21: failed 칩은 payload에서 제외 — ready 첨부만 전달 (DS-60 §4.1)
    const ack = await sendAgentMessage(sessionId, content, readyAtts)
    roleStore.addUserMessage('PM', content, ack.user_message_id)
    attachments.clear() // 전송 성공 시에만 첨부 초기화
  } catch (e) {
    draftMessage.value = content // 실패 시 복원 (첨부 칩은 유지)
    console.error('PM 메시지 전송 실패', e)
  } finally {
    isSending.value = false
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
    e.preventDefault()
    sendMessage()
  }
}

// #16 fix: 실제 파일 읽기 → deliverableStore에 반영
async function openStartupFile(filePath: string) {
  // 1. 사이드바를 산출물 패널로 열고 라우트 이동
  workspaceStore.openSidebar('deliverables')
  router.push('/workspace/deliverables')

  const wsId = projectStore.workspaceId
  if (!wsId) return

  // 2. 파일 내용 읽기
  try {
    const content = await readDocument(wsId, filePath)
    deliverableStore.openFile(filePath, content)
  } catch {
    // 백엔드 미연동 시 mock 콘텐츠로 대체
    deliverableStore.openFile(filePath, {
      path: filePath,
      content: `# ${filePath.split('/').pop()}\n\n(파일 로드 실패 — 백엔드 연동 확인 필요)`,
      frontmatter: {},
    })
  }
}

function isStreamingMsg(msgId: string) {
  return pmSession.value?.streamingMessageId === msgId
}
</script>

<template>
  <div class="pm-panel">
    <!-- 헤더 -->
    <div class="panel-header">
      <div class="header-left">
        <span class="pm-dot">PM</span>
        <span class="role-label">{{ projectStore.pmConfig?.name ?? 'PM' }}</span>
        <span class="agent-badge">{{ projectStore.pmConfig?.agent ?? 'claude' }}</span>
      </div>
      <div class="header-right">
        <StatusBadge :state="pmStatus" size="sm" />
      </div>
    </div>

    <!-- 대화 로그 -->
    <div ref="chatLogEl" class="chat-log">
      <div v-if="messages.length === 0" class="chat-empty">
        PM이 READY 상태가 되면 여기에 대화 로그가 표시됩니다.
      </div>
      <div
        v-for="msg in messages"
        :key="msg.id"
        class="msg"
        :class="msg.role"
      >
        <div class="msg-role-label">{{ msg.role === 'user' ? '나' : (projectStore.pmConfig?.name ?? 'PM') }}</div>
        <div class="msg-content">
          {{ msg.content }}
          <span v-if="isStreamingMsg(msg.id)" class="streaming-cursor">▌</span>
        </div>
      </div>
    </div>

    <!-- Startup Files 칩 -->
    <div v-if="(projectStore.pmConfig?.startupFiles?.length ?? 0) > 0" class="file-chips">
      <span class="file-chips-label">📎</span>
      <button
        v-for="f in projectStore.pmConfig?.startupFiles"
        :key="f"
        class="file-chip"
        @click="openStartupFile(f)"
      >
        {{ f.split('/').pop() }}
      </button>
    </div>

    <!-- 입력 영역 (#21: drop zone — 파일 드래그&드롭 첨부) -->
    <div
      class="input-area"
      :class="{ 'drag-over': attachments.isDragOver.value }"
      @dragover="attachments.onDragOver"
      @dragleave="attachments.onDragLeave"
      @drop="attachments.onDrop"
    >
      <!-- 첨부 미리보기 칩 -->
      <AttachmentChips :chips="attachments.chips.value" @remove="attachments.removeChip" />

      <textarea
        v-model="draftMessage"
        class="msg-input"
        placeholder="메시지 입력 (Ctrl+Enter 전송, 이미지·문서 붙여넣기 가능)"
        rows="3"
        :disabled="pmStatus !== 'ready' || isSending"
        @keydown="onKeydown"
        @paste="attachments.onPaste"
      />
      <div class="input-footer">
        <div class="footer-left">
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
            :disabled="pmStatus !== 'ready'"
            @click="fileInputEl?.click()"
          >📎</button>
          <span class="hint">Ctrl+Enter 전송</span>
        </div>
        <button
          class="send-btn"
          :disabled="!canSend"
          @click="sendMessage"
        >
          {{ isSending ? '전송 중…' : '전송' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.pm-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-panel);
  border-right: 1px solid var(--line);
  border-top: 2.5px solid #7c3aed;
  overflow: hidden;
  box-shadow: var(--card-shadow);
}

/* ── 헤더 ── */
.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 9px 14px;
  border-bottom: 1px solid var(--line-soft);
  background: rgba(124,58,237,0.06);
  flex-shrink: 0;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 6px;
}

.pm-dot {
  width: 24px; height: 24px;
  border-radius: 6px;
  display: grid;
  place-items: center;
  font-size: 9px;
  font-weight: 800;
  background: #7c3aed;
  color: #fff;
  flex-shrink: 0;
  letter-spacing: -0.02em;
}

.role-label {
  font-size: 13px;
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: -0.01em;
}

.agent-badge {
  font-size: 10px;
  color: var(--text-muted);
  background: var(--bg-panel-2);
  padding: 2px 7px;
  border-radius: 999px;
  border: 1px solid var(--line);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

/* ── 대화 로그 ── */
.chat-log {
  flex: 1;
  overflow-y: auto;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.chat-empty {
  font-size: 12px;
  color: var(--text-muted);
  text-align: center;
  margin-top: 40px;
}

.msg {
  padding: 10px 12px;
  border-radius: 8px;
  background: var(--bg-input);
  border: 1px solid var(--line-soft);
  font-size: 13px;
  line-height: 1.5;
}

.msg.user {
  border-color: rgba(99, 102, 241, 0.3);
  background: rgba(99, 102, 241, 0.06);
}

[data-theme="light"] .msg.user {
  border-color: rgba(124, 92, 191, 0.25);
  background: rgba(124, 92, 191, 0.06);
}

.msg-role-label {
  font-size: 10px;
  color: var(--text-muted);
  font-weight: 600;
  text-transform: uppercase;
  margin-bottom: 5px;
}

.msg.user .msg-role-label { color: #818cf8; }

[data-theme="light"] .msg.user .msg-role-label { color: #7c5cbf; }

.msg-content {
  color: var(--text-primary);
  white-space: pre-wrap;
  word-break: break-word;
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

/* ── 파일 칩 ── */
.file-chips {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  border-top: 1px solid var(--line-soft);
  overflow-x: auto;
  flex-shrink: 0;
}

.file-chips-label {
  font-size: 12px;
  flex-shrink: 0;
}

.file-chip {
  font-size: 11px;
  padding: 3px 8px;
  border-radius: 999px;
  border: 1px solid var(--line);
  background: var(--bg-panel-2);
  color: var(--text-muted);
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.12s;
}

.file-chip:hover {
  border-color: var(--accent);
  color: var(--text-primary);
}

/* ── 입력 영역 ── */
.input-area {
  padding: 12px 14px;
  border-top: 1px solid var(--line-soft);
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex-shrink: 0;
  transition: background 0.12s, outline 0.12s;
}

/* #21: 드래그&드롭 hover 상태 */
.input-area.drag-over {
  background: var(--accent-soft, rgba(99,102,241,0.08));
  outline: 2px dashed var(--accent);
  outline-offset: -4px;
}

.footer-left { display: flex; align-items: center; gap: 8px; }

.file-input-hidden { display: none; }

.attach-btn {
  width: 26px; height: 26px;
  border: 1px solid var(--line);
  background: var(--bg-panel-2);
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
  display: grid;
  place-items: center;
  transition: all 0.12s;
  padding: 0;
}

.attach-btn:hover:not(:disabled) { border-color: var(--accent); }
.attach-btn:disabled { opacity: 0.4; cursor: not-allowed; }

.msg-input {
  width: 100%;
  background: var(--bg-input);
  border: 1px solid var(--line);
  border-radius: 7px;
  padding: 8px 10px;
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
  resize: none;
  line-height: 1.4;
  transition: border-color 0.12s;
}

.msg-input:focus { outline: none; border-color: var(--accent); }
.msg-input:disabled { opacity: 0.5; }

.input-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.hint {
  font-size: 11px;
  color: var(--text-muted);
}

.send-btn {
  height: 30px;
  padding: 0 12px;
  border-radius: 6px;
  background: var(--accent);
  color: #fff;
  border: none;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: opacity 0.12s;
}

.send-btn:disabled { opacity: 0.4; cursor: not-allowed; }
.send-btn:not(:disabled):hover { opacity: 0.85; }
</style>
