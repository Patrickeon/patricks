// DS-60 연동규격서 §2 공통 DTO 타입 정의
// Vue 컴포넌트는 이 타입을 import 하고, invoke는 src/ipc/*.ts 래퍼만 사용한다.

export type AppError = {
  code: string
  message: string
  detail?: unknown
  recoverable: boolean
}

export type CommandResult = {
  ok: boolean
  error?: AppError
}

export type AgentLifecycleState =
  | 'idle'
  | 'booting'
  | 'ready'
  | 'running'
  | 'failed'

// ── Workspace ──────────────────────────────────────────────
export type WorkspaceSummary = {
  workspace_id: string
  path: string
  name: string
  display_name?: string
}

export type WorkspaceConfig = {
  workspace_id: string
  agiteam: AgiteamConfig
  project_state: ProjectState
}

export type AgiteamConfig = {
  project: {
    name: string
    displayName: string
    workspace: { name: string; color: string }
  }
  persona: { dir: string; commonFile: string }
  team: TeamMember[]
  pm: PmConfig
  settings: AgiteamSettings
}

export type TeamMember = {
  role: string
  name: string
  agent: 'claude' | 'codex' | 'gemini'
  command: string
  layout: string
}

export type PmConfig = {
  name: string
  agent: 'claude' | 'codex' | 'gemini'
  command: string
  startupMessage?: string
  startupFiles: string[]
}

export type AgiteamSettings = {
  readyTimeout: number
  postLaunchDelay: number
  readySignalTimeout: number
  maxAutoSubmits: number
}

export type ProjectState = {
  business_type: string
  current_mode: 'project' | 'operation'
  milestone: string
  wbs_track: string
}

export type ValidationIssue = {
  severity: 'error' | 'warning' | 'info'
  code: string
  message: string
  path?: string
}

export type ValidationReport = {
  workspace_id: string
  valid: boolean
  issues: ValidationIssue[]
}

// ── Agent ──────────────────────────────────────────────────
export type AgentSessionSummary = {
  session_id: string
  role: string
  display_name: string
  provider: string
  state: AgentLifecycleState
}

export type AgentSessionDetail = AgentSessionSummary & {
  workspace_id: string
  persona_hash: string
  created_at: string
  updated_at: string
  failure_reason?: string
  message_count: number
}

export type MessageAck = {
  session_id: string
  user_message_id: string
  accepted_at: string
}

export type BootTeamResult = {
  workspace_id: string
  sessions: AgentSessionSummary[]
}

export type MessagePage = {
  messages: AgentMessage[]
  next_cursor?: string
}

export type AgentMessage = {
  id: string
  session_id: string
  role: 'user' | 'assistant'
  content: string
  created_at: string
  usage?: { input_tokens?: number; output_tokens?: number; total_tokens?: number }
  is_streaming: boolean
}

// ── Persona ────────────────────────────────────────────────
export type PersonaBundlePreview = {
  role: string
  content_hash: string
  content: string
  source_files: string[]
}

// ── Document ───────────────────────────────────────────────
export type DocumentNode = {
  name: string
  path: string
  is_dir: boolean
  children?: DocumentNode[]
  is_latest?: boolean
}

export type DocumentTree = {
  root: DocumentNode
}

export type DocumentContent = {
  path: string
  content: string
  frontmatter?: Record<string, unknown>
}

// DV60-006 수정: Rust DocumentWriteResult 구조체와 필드 정렬
// Rust: { path: String, archive_path: Option<String>, version_hint: String }
export type DocumentWriteResult = {
  path: string
  archive_path?: string   // Option<String> — 첫 쓰기 시 null
  version_hint: string    // YYYYMMDDhhmmss 기반 버전 힌트
}

// ── Credential ─────────────────────────────────────────────
export type CredentialRef = {
  provider: string
  account: string
}

export type ProviderHealth = {
  provider: string
  account: string
  ok: boolean
  error?: string
}

// ── Health ─────────────────────────────────────────────────
export type HealthCheckReport = {
  workspace_ok: boolean
  providers: ProviderHealth[]
  tools: ToolCheck[]
  network: NetworkCheck[]
  errors: string[]
}

export type ToolCheck = {
  name: string
  available: boolean
  version?: string
}

export type NetworkCheck = {
  endpoint: string
  reachable: boolean
  latency_ms?: number
}

// ── Events ─────────────────────────────────────────────────
export type AgentStatusChanged = {
  session_id: string
  role: string
  from: AgentLifecycleState
  to: AgentLifecycleState
  reason?: string
  changed_at: string
}

export type AgentMessageStarted = {
  session_id: string
  message_id: string
  started_at: string
}

export type AgentMessageDelta = {
  session_id: string
  message_id: string
  delta: string
  sequence: number
}

export type AgentMessageCompleted = {
  session_id: string
  message_id: string
  usage?: {
    input_tokens?: number
    output_tokens?: number
    total_tokens?: number
  }
  completed_at: string
}

export type AgentMessageFailed = {
  session_id: string
  message_id?: string
  error: AppError
}

export type AgentToolRequested = {
  session_id: string
  tool_name: string
  requires_approval: boolean
}

export type DocumentChanged = {
  workspace_id: string
  path: string
}

// ── Redmine ────────────────────────────────────────────────
/** Rust RedmineStatusRef { id, name } */
export type RedmineStatusRef = {
  id: number
  name: string
}

/** Rust RedmineRef — 트래커 등 단순 참조 객체 */
export type RedmineRef = {
  id: number
  name: string
}

/** Rust RedmineUserRef */
export type RedmineUserRef = {
  id: number
  name: string
}

/**
 * Rust RedmineIssueItem 직렬화와 1:1 대응하는 IPC 레이어 타입.
 * src/ipc/redmine.ts 래퍼 반환 타입으로 사용된다.
 * stores/redmine.ts에서 is_closed를 포함한 RedmineIssue로 변환해 사용한다.
 */
export type RedmineIssueItem = {
  id: number
  subject: string
  description: string
  status: RedmineStatusRef
  tracker: RedmineRef
  assigned_to?: RedmineUserRef
  done_ratio: number
  created_on: string
  updated_on: string
}
