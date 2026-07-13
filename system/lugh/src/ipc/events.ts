// DS-60 §4 Backend Event 구독 등록
// useRoleStore, useWorkspaceStore 등에서 onMounted 시 호출한다.

import { listen } from '@tauri-apps/api/event'
import type {
  AgentStatusChanged,
  AgentMessageStarted,
  AgentMessageDelta,
  AgentMessageCompleted,
  AgentMessageFailed,
  AgentToolRequested,
  AgentMessagesCleared,
  DocumentChanged,
  WorkspaceSummary,
  ValidationReport,
  HealthCheckReport,
  ProviderHealth,
} from './types'

type Unlistener = () => void

export async function listenAgentEvents(handlers: {
  onStatusChanged?: (p: AgentStatusChanged) => void
  onMessageStarted?: (p: AgentMessageStarted) => void
  onMessageDelta?: (p: AgentMessageDelta) => void
  onMessageCompleted?: (p: AgentMessageCompleted) => void
  onMessageFailed?: (p: AgentMessageFailed) => void
  onToolRequested?: (p: AgentToolRequested) => void
  onMessagesCleared?: (p: AgentMessagesCleared) => void
}): Promise<Unlistener> {
  const unlisteners: Unlistener[] = []

  if (handlers.onStatusChanged) {
    unlisteners.push(
      await listen<AgentStatusChanged>(
        'agent:status_changed',
        (e) => handlers.onStatusChanged!(e.payload),
      ),
    )
  }
  if (handlers.onMessageStarted) {
    unlisteners.push(
      await listen<AgentMessageStarted>(
        'agent:message_started',
        (e) => handlers.onMessageStarted!(e.payload),
      ),
    )
  }
  if (handlers.onMessageDelta) {
    unlisteners.push(
      await listen<AgentMessageDelta>(
        'agent:message_delta',
        (e) => handlers.onMessageDelta!(e.payload),
      ),
    )
  }
  if (handlers.onMessageCompleted) {
    unlisteners.push(
      await listen<AgentMessageCompleted>(
        'agent:message_completed',
        (e) => handlers.onMessageCompleted!(e.payload),
      ),
    )
  }
  if (handlers.onMessageFailed) {
    unlisteners.push(
      await listen<AgentMessageFailed>(
        'agent:message_failed',
        (e) => handlers.onMessageFailed!(e.payload),
      ),
    )
  }
  if (handlers.onToolRequested) {
    unlisteners.push(
      await listen<AgentToolRequested>(
        'agent:tool_requested',
        (e) => handlers.onToolRequested!(e.payload),
      ),
    )
  }
  if (handlers.onMessagesCleared) {
    unlisteners.push(
      await listen<AgentMessagesCleared>(
        'agent:messages_cleared',
        (e) => handlers.onMessagesCleared!(e.payload),
      ),
    )
  }

  return () => unlisteners.forEach((u) => u())
}

export async function listenDocumentEvents(
  onChanged: (p: DocumentChanged) => void,
): Promise<Unlistener> {
  return listen<DocumentChanged>('document:changed', (e) => onChanged(e.payload))
}

export async function listenWorkspaceEvents(handlers: {
  onOpened?: (p: WorkspaceSummary) => void
  onValidationFailed?: (p: ValidationReport) => void
}): Promise<Unlistener> {
  const unlisteners: Unlistener[] = []

  if (handlers.onOpened) {
    unlisteners.push(
      await listen<WorkspaceSummary>(
        'workspace:opened',
        (e) => handlers.onOpened!(e.payload),
      ),
    )
  }
  if (handlers.onValidationFailed) {
    unlisteners.push(
      await listen<ValidationReport>(
        'workspace:validation_failed',
        (e) => handlers.onValidationFailed!(e.payload),
      ),
    )
  }

  return () => unlisteners.forEach((u) => u())
}

export async function listenHealthEvent(
  onCompleted: (p: HealthCheckReport) => void,
): Promise<Unlistener> {
  return listen<HealthCheckReport>('health:completed', (e) => onCompleted(e.payload))
}

export async function listenCredentialEvent(
  onValidated: (p: ProviderHealth) => void,
): Promise<Unlistener> {
  return listen<ProviderHealth>('credential:validated', (e) => onValidated(e.payload))
}
