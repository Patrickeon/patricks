// useDeliverableStore — 산출물 파일 트리, 편집, 아카이브
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { DocumentNode, DocumentContent } from '@/ipc/types'

export type ArchiveEntry = {
  name: string
  path: string
  timestamp: string // YYYYMMDDhhmmss
  displayTime: string
}

export const useDeliverableStore = defineStore('deliverable', () => {
  // ── 파일 트리 ──────────────────────────────────────────────
  const treeRoot = ref<DocumentNode | null>(null)
  const isLoadingTree = ref(false)

  // ── 현재 열린 파일 ────────────────────────────────────────
  const currentPath = ref<string | null>(null)
  const currentContent = ref<DocumentContent | null>(null)
  const isEditing = ref(false)
  const editBuffer = ref('')
  const isDirty = ref(false)
  const isSaving = ref(false)

  // ── 아카이브 이력 ─────────────────────────────────────────
  const archiveEntries = ref<ArchiveEntry[]>([])
  const selectedArchivePath = ref<string | null>(null)
  const archiveContent = ref<DocumentContent | null>(null)
  const showDiff = ref(false)

  // ── Computed ───────────────────────────────────────────────
  const frontmatter = computed(() => currentContent.value?.frontmatter ?? {})
  const hasChanges = computed(() => isDirty.value)

  // ── 액션 ──────────────────────────────────────────────────
  function setTree(root: DocumentNode) {
    treeRoot.value = root
  }

  function openFile(path: string, content: DocumentContent) {
    currentPath.value = path
    currentContent.value = content
    editBuffer.value = content.content
    isDirty.value = false
    isEditing.value = false
    archiveEntries.value = []
    selectedArchivePath.value = null
    archiveContent.value = null
    showDiff.value = false
  }

  function startEdit() {
    isEditing.value = true
    editBuffer.value = currentContent.value?.content ?? ''
  }

  function cancelEdit() {
    isEditing.value = false
    editBuffer.value = currentContent.value?.content ?? ''
    isDirty.value = false
  }

  function updateEditBuffer(text: string) {
    editBuffer.value = text
    isDirty.value = text !== (currentContent.value?.content ?? '')
  }

  function afterSave(archivePath: string | null) {
    if (currentContent.value) {
      currentContent.value = {
        ...currentContent.value,
        content: editBuffer.value,
      }
    }
    isDirty.value = false
    isEditing.value = false
    isSaving.value = false

    // 아카이브 경로가 있을 때만 이력 추가
    if (archivePath && currentPath.value) {
      const fileName = archivePath.split('/').pop() ?? ''
      const ts = fileName.replace(/^.*?_(\d{14})\.md$/, '$1') || fileName
      archiveEntries.value.unshift({
        name: fileName,
        path: archivePath,
        timestamp: ts,
        displayTime: formatArchiveTimestamp(ts),
      })
    }
  }

  function setArchiveEntries(entries: ArchiveEntry[]) {
    archiveEntries.value = entries
  }

  function selectArchive(path: string, content: DocumentContent) {
    selectedArchivePath.value = path
    archiveContent.value = content
  }

  function toggleDiff() {
    showDiff.value = !showDiff.value
  }

  function closePanel() {
    currentPath.value = null
    currentContent.value = null
    isEditing.value = false
    isDirty.value = false
    archiveEntries.value = []
  }

  return {
    treeRoot,
    isLoadingTree,
    currentPath,
    currentContent,
    isEditing,
    editBuffer,
    isDirty,
    isSaving,
    archiveEntries,
    selectedArchivePath,
    archiveContent,
    showDiff,
    frontmatter,
    hasChanges,
    setTree,
    openFile,
    startEdit,
    cancelEdit,
    updateEditBuffer,
    afterSave,
    setArchiveEntries,
    selectArchive,
    toggleDiff,
    closePanel,
  }
})

function formatArchiveTimestamp(ts: string): string {
  // YYYYMMDDhhmmss → YYYY-MM-DD HH:mm:ss
  if (ts.length !== 14) return ts
  return `${ts.slice(0, 4)}-${ts.slice(4, 6)}-${ts.slice(6, 8)} ${ts.slice(8, 10)}:${ts.slice(10, 12)}:${ts.slice(12, 14)}`
}
