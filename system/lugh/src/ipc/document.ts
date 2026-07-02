import { invoke } from '@tauri-apps/api/core'
import type { DocumentTree, DocumentContent, DocumentWriteResult } from './types'

export async function listDocuments(workspaceId: string): Promise<DocumentTree> {
  return invoke<DocumentTree>('list_documents', { workspaceId })
}

export async function readDocument(
  workspaceId: string,
  path: string,
): Promise<DocumentContent> {
  return invoke<DocumentContent>('read_document', { workspaceId, path })
}

export async function writeLatestDocument(
  workspaceId: string,
  path: string,
  content: string,
): Promise<DocumentWriteResult> {
  return invoke<DocumentWriteResult>('write_latest_document', {
    workspaceId,
    path,
    content,
  })
}
