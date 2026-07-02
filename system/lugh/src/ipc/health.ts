import { invoke } from '@tauri-apps/api/core'
import type { HealthCheckReport } from './types'

export async function runHealthCheck(workspaceId: string): Promise<HealthCheckReport> {
  return invoke<HealthCheckReport>('run_health_check', { workspaceId })
}
