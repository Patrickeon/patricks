import { invoke } from '@tauri-apps/api/core'
import type {
  BootTeamResult,
  AgentSessionSummary,
  AgentSessionDetail,
  MessageAck,
  MessagePage,
  CommandResult,
  ChatAttachmentInput,
  PreparedChatAttachment,
} from './types'

export async function bootTeam(workspaceId: string): Promise<BootTeamResult> {
  return invoke<BootTeamResult>('boot_team', { workspaceId })
}

export async function bootRole(
  workspaceId: string,
  role: string,
): Promise<AgentSessionSummary> {
  return invoke<AgentSessionSummary>('boot_role', { workspaceId, role })
}

export async function stopRole(sessionId: string): Promise<CommandResult> {
  return invoke<CommandResult>('stop_role', { sessionId })
}

export async function sendAgentMessage(
  sessionId: string,
  content: string,
  attachments: PreparedChatAttachment[] = [],
): Promise<MessageAck> {
  // #21: DS-60 v0.7 §3.2 — attachments는 ready 상태(PreparedChatAttachment)만 포함한다
  return invoke<MessageAck>('send_agent_message', {
    sessionId,
    content,
    attachments,
  })
}

/**
 * 채팅 첨부 1건을 검증·정규화한다 (#21, DS-60 v0.7 §3.2 prepare_chat_attachment).
 * - 이미지: base64 검증 후 content_base64 유지 (png/jpeg/webp/gif)
 * - 문서: extracted_text 생성 (md/markdown/txt/csv/json/yaml/yml/log/pdf, 100KB 초과 시 truncated)
 * - 오류: ATTACHMENT_INVALID_TYPE / ATTACHMENT_TOO_LARGE / ATTACHMENT_EXTRACT_FAILED 등
 */
export async function prepareChatAttachment(
  sessionId: string,
  attachment: ChatAttachmentInput,
): Promise<PreparedChatAttachment> {
  return invoke<PreparedChatAttachment>('prepare_chat_attachment', {
    sessionId,
    attachment,
  })
}

export async function getAgentSession(
  sessionId: string,
): Promise<AgentSessionDetail> {
  return invoke<AgentSessionDetail>('get_agent_session', { sessionId })
}

export async function listAgentMessages(
  sessionId: string,
  cursor?: string,
  limit?: number,
): Promise<MessagePage> {
  return invoke<MessagePage>('list_agent_messages', {
    sessionId,
    cursor,
    limit,
  })
}
