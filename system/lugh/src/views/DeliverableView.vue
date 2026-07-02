<!-- Screen-05: 산출물 뷰어/에디터 (DS-50 §8) -->
<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { marked } from 'marked'
import { useDeliverableStore } from '@/stores/deliverable'
import { useProjectStore } from '@/stores/project'
import { listDocuments, readDocument, writeLatestDocument } from '@/ipc/document'
import type { DocumentNode } from '@/ipc/types'

const deliverableStore = useDeliverableStore()
const projectStore = useProjectStore()

const renderedHtml = computed(() => {
  const content = deliverableStore.isEditing
    ? deliverableStore.editBuffer
    : (deliverableStore.currentContent?.content ?? '')
  return marked(content) as string
})

// ── 파일 트리 로드 ────────────────────────────────────────
onMounted(async () => {
  if (!projectStore.workspaceId) return
  try {
    const tree = await listDocuments(projectStore.workspaceId)
    deliverableStore.setTree(tree.root)
  } catch {
    // mock 트리
    deliverableStore.setTree({
      name: 'documents',
      path: 'documents',
      is_dir: true,
      children: [
        {
          name: '04.development',
          path: 'documents/04.development',
          is_dir: true,
          children: [
            {
              name: 'DS-10_IA구조도.latest.md',
              path: 'documents/04.development/02.설계/DS-10_IA구조도/DS-10_IA구조도.latest.md',
              is_dir: false,
              is_latest: true,
            },
            {
              name: 'DS-50_화면설계서.latest.md',
              path: 'documents/04.development/02.설계/DS-50_화면설계서/DS-50_화면설계서.latest.md',
              is_dir: false,
              is_latest: true,
            },
          ],
        },
      ],
    })
  }
})

// ── 파일 선택 ─────────────────────────────────────────────
async function selectFile(node: DocumentNode) {
  if (node.is_dir) return
  if (!projectStore.workspaceId) return
  try {
    const content = await readDocument(projectStore.workspaceId, node.path)
    deliverableStore.openFile(node.path, content)
  } catch {
    // mock 콘텐츠
    deliverableStore.openFile(node.path, {
      path: node.path,
      content: `# ${node.name}\n\n(파일 로드 실패 — 백엔드 연동 필요)`,
      frontmatter: {},
    })
  }
}

// ── 저장 ──────────────────────────────────────────────────
async function saveFile() {
  if (!deliverableStore.currentPath || !projectStore.workspaceId) return
  deliverableStore.isSaving = true
  try {
    const result = await writeLatestDocument(
      projectStore.workspaceId,
      deliverableStore.currentPath,
      deliverableStore.editBuffer,
    )
    deliverableStore.afterSave(result.archive_path ?? null)
  } catch {
    deliverableStore.isSaving = false
  }
}

// ── 파일 트리 렌더 ────────────────────────────────────────
function nodeIndent(depth: number) {
  return `${depth * 14}px`
}
</script>

<template>
  <div class="deliverable-panel">
    <!-- 파일 트리 -->
    <div class="file-tree-pane">
      <div class="tree-header">파일 트리</div>
      <div class="tree-body">
        <FileTreeNode
          v-if="deliverableStore.treeRoot"
          :node="deliverableStore.treeRoot"
          :depth="0"
          :selected-path="deliverableStore.currentPath"
          @select="selectFile"
        />
        <div v-else class="tree-empty">파일 없음</div>
      </div>
    </div>

    <!-- 뷰어/에디터 -->
    <div class="viewer-pane">
      <template v-if="deliverableStore.currentContent">
        <!-- 상단 메타 + 컨트롤 -->
        <div class="viewer-toolbar">
          <span class="current-file truncate">{{ deliverableStore.currentPath?.split('/').pop() }}</span>
          <div class="toolbar-btns">
            <button
              class="tb-btn"
              :class="{ active: !deliverableStore.isEditing }"
              @click="deliverableStore.cancelEdit()"
            >미리보기</button>
            <button
              class="tb-btn"
              :class="{ active: deliverableStore.isEditing }"
              @click="deliverableStore.startEdit()"
            >편집</button>
            <button
              v-if="deliverableStore.isDirty"
              class="tb-btn save"
              @click="saveFile"
              :disabled="deliverableStore.isSaving"
            >{{ deliverableStore.isSaving ? '저장 중…' : '저장' }}</button>
          </div>
        </div>

        <!-- Frontmatter 패널 -->
        <div v-if="Object.keys(deliverableStore.frontmatter).length > 0" class="frontmatter">
          <span
            v-for="(val, key) in deliverableStore.frontmatter"
            :key="key"
            class="fm-item"
          >
            <span class="fm-key">{{ key }}</span>
            <span class="fm-val">{{ val }}</span>
          </span>
        </div>

        <!-- 편집 모드 -->
        <textarea
          v-if="deliverableStore.isEditing"
          class="editor"
          :value="deliverableStore.editBuffer"
          @input="deliverableStore.updateEditBuffer(($event.target as HTMLTextAreaElement).value)"
        />

        <!-- 미리보기 모드 -->
        <div
          v-else
          class="markdown-body"
          v-html="renderedHtml"
        />

        <!-- 아카이브 이력 -->
        <div v-if="deliverableStore.archiveEntries.length > 0" class="archive-bar">
          <span class="archive-label">이력:</span>
          <button
            v-for="entry in deliverableStore.archiveEntries.slice(0, 5)"
            :key="entry.path"
            class="archive-btn"
            :class="{ active: deliverableStore.selectedArchivePath === entry.path }"
            @click="() => {}"
          >{{ entry.displayTime }}</button>
        </div>
      </template>

      <!-- 파일 미선택 상태 -->
      <div v-else class="viewer-empty">
        <p>파일 트리에서 파일을 선택하세요</p>
        <p class="hint">`.latest.md` 파일은 자동 버전 관리가 적용됩니다</p>
      </div>
    </div>
  </div>
</template>

<!-- ── 재귀 FileTreeNode 컴포넌트 ── -->
<script lang="ts">
// 별도 컴포넌트로 분리해야 하지만, 간략화를 위해 인라인 처리
import { defineComponent, h, ref as vueRef } from 'vue'
import type { PropType, VNode } from 'vue'
import type { DocumentNode as _DocumentNode } from '@/ipc/types'

const FileTreeNode: ReturnType<typeof defineComponent> = defineComponent({
  name: 'FileTreeNode',
  props: {
    node: { type: Object as PropType<_DocumentNode>, required: true },
    depth: { type: Number, default: 0 },
    selectedPath: { type: String as PropType<string | null>, default: null },
  },
  emits: ['select'],
  setup(props, { emit }) {
    const expanded = vueRef(true)
    return (): VNode => {
      const node = props.node
      const isSelected = !node.is_dir && node.path === props.selectedPath

      return h('div', { class: 'tree-node' }, [
        h('div',
          {
            class: ['tree-row', { dir: node.is_dir, file: !node.is_dir, selected: isSelected, latest: node.is_latest }],
            style: { paddingLeft: `${props.depth * 14 + 8}px` },
            onClick: () => {
              if (node.is_dir) { expanded.value = !expanded.value }
              else emit('select', node)
            },
          },
          [
            node.is_dir ? h('span', { class: 'tree-icon' }, expanded.value ? '▾' : '▸') : null,
            h('span', { class: 'tree-name' }, node.name + (node.is_latest ? ' ●' : '')),
          ],
        ),
        node.is_dir && expanded.value && node.children
          ? h('div', node.children.map((child: _DocumentNode) =>
              h(FileTreeNode, {
                node: child,
                depth: props.depth + 1,
                selectedPath: props.selectedPath,
                onSelect: (n: _DocumentNode) => emit('select', n),
              }),
            ))
          : null,
      ])
    }
  },
})

export default {}
</script>

<style scoped>
.deliverable-panel {
  display: flex;
  height: 100%;
  overflow: hidden;
}

/* ── 파일 트리 ── */
.file-tree-pane {
  width: 220px;
  flex-shrink: 0;
  border-right: 1px solid var(--line-soft);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.tree-header {
  padding: 8px 12px;
  font-size: 11px;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  font-weight: 600;
  border-bottom: 1px solid var(--line-soft);
  flex-shrink: 0;
}

.tree-body { flex: 1; overflow-y: auto; padding: 6px 0; }
.tree-empty { font-size: 12px; color: var(--text-muted); padding: 16px 12px; }

:deep(.tree-row) {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 4px 8px 4px 8px;
  cursor: pointer;
  font-size: 12px;
  color: var(--text-muted);
  border-radius: 4px;
  margin: 1px 4px;
  transition: background 0.1s;
}

:deep(.tree-row:hover)  { background: rgba(99,102,241,0.07); color: var(--text-primary); }
:deep(.tree-row.dir)    { color: var(--text-soft); font-weight: 500; }
:deep(.tree-row.selected) { background: rgba(99,102,241,0.12); color: var(--accent); }
:deep(.tree-row.latest .tree-name) { color: #5eead4; }
:deep(.tree-icon) { font-size: 10px; flex-shrink: 0; }

/* ── 뷰어 ── */
.viewer-pane {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.viewer-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid var(--line-soft);
  background: rgba(9,13,21,0.5);
  flex-shrink: 0;
  gap: 8px;
}

.current-file { font-size: 12px; color: var(--text-muted); flex: 1; min-width: 0; }

.toolbar-btns { display: flex; gap: 4px; flex-shrink: 0; }

.tb-btn {
  height: 26px;
  padding: 0 10px;
  border-radius: 5px;
  border: 1px solid var(--line);
  background: var(--bg-panel-2);
  color: var(--text-muted);
  font-size: 11px;
  cursor: pointer;
  transition: all 0.1s;
}

.tb-btn:hover   { color: var(--text-primary); }
.tb-btn.active  { border-color: var(--accent); color: var(--accent); background: rgba(99,102,241,0.08); }
.tb-btn.save    { border-color: rgba(34,197,94,0.4); color: var(--ok); background: rgba(34,197,94,0.06); }

.frontmatter {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 6px 12px;
  border-bottom: 1px solid var(--line-soft);
  background: rgba(8,12,19,0.4);
  flex-shrink: 0;
}

.fm-item { display: flex; gap: 4px; font-size: 11px; }
.fm-key  { color: var(--text-muted); }
.fm-val  { color: var(--text-soft); }

.editor {
  flex: 1;
  background: var(--bg-input);
  border: none;
  padding: 14px;
  color: var(--text-primary);
  font-family: 'SFMono-Regular', Consolas, monospace;
  font-size: 13px;
  line-height: 1.6;
  resize: none;
  overflow-y: auto;
}

.editor:focus { outline: none; }

.markdown-body {
  flex: 1;
  overflow-y: auto;
  padding: 16px 20px;
  font-size: 13px;
  line-height: 1.7;
  color: var(--text-primary);
}

:deep(.markdown-body h1) { font-size: 18px; margin: 16px 0 10px; color: var(--text-primary); }
:deep(.markdown-body h2) { font-size: 15px; margin: 14px 0 8px; color: var(--text-primary); }
:deep(.markdown-body h3) { font-size: 13px; margin: 12px 0 6px; color: var(--text-soft); }
:deep(.markdown-body p)  { margin: 6px 0; color: var(--text-primary); }
:deep(.markdown-body code) { background: var(--bg-input); padding: 1px 5px; border-radius: 3px; font-size: 12px; color: #5eead4; }
:deep(.markdown-body pre) { background: var(--bg-input); padding: 12px; border-radius: 6px; overflow-x: auto; }
:deep(.markdown-body table) { width: 100%; border-collapse: collapse; margin: 10px 0; }
:deep(.markdown-body th, .markdown-body td) { padding: 7px 10px; border: 1px solid var(--line-soft); font-size: 12px; }
:deep(.markdown-body th) { color: var(--text-muted); background: rgba(20,29,43,0.7); }

.archive-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border-top: 1px solid var(--line-soft);
  flex-shrink: 0;
  background: rgba(8,12,19,0.4);
  overflow-x: auto;
}

.archive-label { font-size: 11px; color: var(--text-muted); flex-shrink: 0; }

.archive-btn {
  font-size: 11px;
  padding: 2px 8px;
  border: 1px solid var(--line);
  border-radius: 4px;
  background: var(--bg-panel-2);
  color: var(--text-muted);
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.1s;
}

.archive-btn:hover  { color: var(--text-primary); }
.archive-btn.active { border-color: var(--accent); color: var(--accent); }

.viewer-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--text-muted);
}

.viewer-empty p { font-size: 13px; }
.viewer-empty .hint { font-size: 12px; }
</style>
