<!-- 첨부 미리보기 칩 목록 (#21, DS-60 v0.7 §4.1) — pending 스피너 / ready 썸네일·파일명 / failed 오류 사유 -->
<script setup lang="ts">
import type { AttachmentChip } from '@/composables/useChatAttachments'

defineProps<{
  chips: AttachmentChip[]
}>()

const emit = defineEmits<{
  remove: [id: string]
}>()

function sizeLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)}KB`
  return `${(bytes / 1024 / 1024).toFixed(1)}MB`
}
</script>

<template>
  <div v-if="chips.length > 0" class="attachment-chips">
    <div
      v-for="chip in chips"
      :key="chip.id"
      class="att-chip"
      :class="chip.status"
      :title="chip.status === 'failed' ? chip.error : `${chip.filename} (${sizeLabel(chip.size_bytes)})`"
    >
      <!-- 썸네일 / 아이콘 / 스피너 -->
      <span v-if="chip.status === 'pending'" class="att-spinner" />
      <img
        v-else-if="chip.kind === 'image' && chip.previewUrl"
        class="att-thumb"
        :src="chip.previewUrl"
        :alt="chip.filename"
      />
      <span v-else class="att-icon">{{ chip.status === 'failed' ? '⚠️' : (chip.kind === 'image' ? '🖼️' : '📄') }}</span>

      <!-- 파일명 + 상태 -->
      <span class="att-name">{{ chip.filename }}</span>
      <span v-if="chip.status === 'failed'" class="att-error">{{ chip.error }}</span>
      <span v-else-if="chip.prepared?.truncated" class="att-truncated">일부만</span>

      <!-- 제거 버튼 -->
      <button class="att-remove" title="첨부 제거" @click="emit('remove', chip.id)">×</button>
    </div>
  </div>
</template>

<style scoped>
.attachment-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  padding: 6px 0 0;
}

.att-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 100%;
  padding: 3px 6px 3px 4px;
  border-radius: 999px;
  border: 1px solid var(--line);
  background: var(--bg-panel-2);
  font-size: 11px;
  color: var(--text-primary);
  overflow: hidden;
}

.att-chip.ready  { border-color: rgba(34,197,94,0.4); }
.att-chip.failed { border-color: rgba(239,68,68,0.45); background: rgba(239,68,68,0.07); }

.att-thumb {
  width: 20px; height: 20px;
  border-radius: 4px;
  object-fit: cover;
  flex-shrink: 0;
}

.att-icon { font-size: 12px; flex-shrink: 0; }

.att-spinner {
  width: 12px; height: 12px;
  border: 2px solid var(--line);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: att-spin 0.7s linear infinite;
  flex-shrink: 0;
}

@keyframes att-spin { to { transform: rotate(360deg); } }

.att-name {
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.att-error {
  max-width: 180px;
  color: var(--error);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.att-truncated {
  color: var(--text-muted);
  font-size: 10px;
  flex-shrink: 0;
}

.att-remove {
  width: 16px; height: 16px;
  border: none;
  background: none;
  color: var(--text-muted);
  font-size: 13px;
  line-height: 1;
  cursor: pointer;
  padding: 0;
  flex-shrink: 0;
  border-radius: 50%;
  display: grid;
  place-items: center;
}

.att-remove:hover { color: var(--error); background: rgba(239,68,68,0.1); }
</style>
