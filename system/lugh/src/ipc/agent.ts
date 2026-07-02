import { invoke } from '@tauri-apps/api/core'
import type {
  BootTeamResult,
  AgentSessionSummary,
  AgentSessionDetail,
  MessageAck,
  MessagePage,
  CommandResult,
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
): Promise<MessageAck> {
  return invoke<MessageAck>('send_agent_message', { sessionId, content })
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
