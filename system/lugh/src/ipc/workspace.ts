import { invoke } from '@tauri-apps/api/core'
import type {
  WorkspaceSummary,
  WorkspaceConfig,
  ValidationReport,
} from './types'

export async function openWorkspace(path: string): Promise<WorkspaceSummary> {
  return invoke<WorkspaceSummary>('open_workspace', { path })
}

export async function loadWorkspaceConfig(
  workspaceId: string,
): Promise<WorkspaceConfig> {
  return invoke<WorkspaceConfig>('load_workspace_config', {
    workspaceId,
  })
}

export async function validateWorkspace(
  workspaceId: string,
): Promise<ValidationReport> {
  return invoke<ValidationReport>('validate_workspace', { workspaceId })
}

/** agiteam.json을 디스크에 저장한다 (DV60-005) */
export async function saveWorkspaceConfig(
  workspaceId: string,
  config: import('./types').AgiteamConfig,
): Promise<void> {
  return invoke<void>('save_workspace_config', { workspaceId, config })
}

/** project_state.yaml을 디스크에 저장한다 (#18 fix: SettingsView 직접 invoke 이동) */
export async function writeProjectState(
  workspaceId: string,
  state: import('./types').ProjectState,
): Promise<void> {
  return invoke<void>('write_project_state', { workspaceId, state })
}
