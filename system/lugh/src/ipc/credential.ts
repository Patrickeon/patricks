import { invoke } from '@tauri-apps/api/core'
import type { CredentialRef, CommandResult, ProviderHealth } from './types'

export async function saveCredential(
  provider: string,
  account: string,
  secret: string,
): Promise<CredentialRef> {
  return invoke<CredentialRef>('save_credential', { provider, account, secret })
}

export async function deleteCredential(
  provider: string,
  account: string,
): Promise<CommandResult> {
  return invoke<CommandResult>('delete_credential', { provider, account })
}

export async function validateCredential(
  provider: string,
  account: string,
): Promise<ProviderHealth> {
  return invoke<ProviderHealth>('validate_credential', { provider, account })
}

export async function getMaskedCredential(
  provider: string,
  account: string,
): Promise<{ provider: string; account: string; has_secret: boolean }> {
  return invoke('get_masked_credential', { provider, account })
}

/** Claude Code OAuth 로그인 감지 (#18 fix: SettingsView 직접 invoke 이동) */
export async function checkClaudeOauth(): Promise<{ ok: boolean }> {
  return invoke<{ ok: boolean }>('check_claude_oauth')
}
