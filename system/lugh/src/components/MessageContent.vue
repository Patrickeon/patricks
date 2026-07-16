<!-- 채팅 메시지 본문 렌더 (Redmine #30) -->
<!-- AI(assistant) 응답은 마크다운으로, 사용자(user) 입력은 평문으로 렌더한다. -->
<script setup lang="ts">
import { computed } from 'vue'
import { renderMarkdown } from '@/lib/markdown'
import { openExternalUrl } from '@/ipc/opener'
import { showToast } from '@/composables/toast'

const props = defineProps<{
  content: string
  isUser?: boolean
  streaming?: boolean
}>()

// assistant 메시지만 마크다운 렌더. 사용자 입력은 평문 유지(원문 그대로 안전).
const html = computed(() => (props.isUser ? '' : renderMarkdown(props.content)))

// 링크 클릭 → 웹뷰 네비게이션 대신 OS 기본 브라우저로 위임 (안전)
async function onClick(e: MouseEvent) {
  const anchor = (e.target as HTMLElement | null)?.closest?.('a[data-external]') as
    | HTMLAnchorElement
    | null
  if (!anchor) return
  e.preventDefault()
  const href = anchor.getAttribute('href')
  if (!href) return // 안전하지 않아 무력화된 링크(href="")는 무시
  try {
    await openExternalUrl(href)
  } catch (err) {
    console.error('[chat] open external link failed:', err)
    showToast('링크를 열 수 없습니다', 'error')
  }
}
</script>

<template>
  <!-- 사용자 입력: 평문 (pre-wrap로 줄바꿈·공백 보존) -->
  <div v-if="isUser" class="msg-content plain">{{ content
    }}<span v-if="streaming" class="streaming-cursor">▌</span></div>

  <!-- AI 응답: 마크다운 -->
  <div v-else class="msg-content markdown-body" @click="onClick">
    <span v-html="html" /><span v-if="streaming" class="streaming-cursor">▌</span>
  </div>
</template>

<style scoped>
.msg-content {
  color: var(--text-primary);
  word-break: break-word;
}

.msg-content.plain {
  white-space: pre-wrap;
}

.streaming-cursor {
  display: inline-block;
  animation: blink 0.8s infinite;
  color: var(--accent);
}

@keyframes blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0; }
}

/* ── 마크다운 본문 (v-html 주입 → :deep 필요) ── */
.markdown-body :deep(p) { margin: 0 0 8px; line-height: 1.5; }
.markdown-body :deep(p:last-child) { margin-bottom: 0; }

.markdown-body :deep(h1),
.markdown-body :deep(h2),
.markdown-body :deep(h3),
.markdown-body :deep(h4),
.markdown-body :deep(h5),
.markdown-body :deep(h6) {
  margin: 12px 0 6px;
  font-weight: 700;
  line-height: 1.3;
}
.markdown-body :deep(h1) { font-size: 1.35em; }
.markdown-body :deep(h2) { font-size: 1.22em; }
.markdown-body :deep(h3) { font-size: 1.1em; }
.markdown-body :deep(h4),
.markdown-body :deep(h5),
.markdown-body :deep(h6) { font-size: 1em; }

.markdown-body :deep(ul),
.markdown-body :deep(ol) { margin: 0 0 8px; padding-left: 20px; }
.markdown-body :deep(li) { margin: 2px 0; line-height: 1.5; }
.markdown-body :deep(li > p) { margin: 0; }

.markdown-body :deep(a) {
  color: var(--accent);
  text-decoration: underline;
  cursor: pointer;
  word-break: break-all;
}
.markdown-body :deep(a:hover) { opacity: 0.85; }

.markdown-body :deep(strong) { font-weight: 700; }
.markdown-body :deep(em) { font-style: italic; }
.markdown-body :deep(del) { text-decoration: line-through; opacity: 0.7; }

.markdown-body :deep(blockquote) {
  margin: 0 0 8px;
  padding: 2px 12px;
  border-left: 3px solid var(--line);
  color: var(--text-muted);
}

.markdown-body :deep(hr) {
  border: none;
  border-top: 1px solid var(--line-soft);
  margin: 12px 0;
}

/* 인라인 코드 */
.markdown-body :deep(code) {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 0.9em;
  background: var(--bg-panel-2);
  border: 1px solid var(--line-soft);
  border-radius: 4px;
  padding: 1px 5px;
  word-break: break-word;
}

/* 코드블록 */
.markdown-body :deep(pre) {
  margin: 0 0 8px;
  padding: 10px 12px;
  background: var(--bg-code, rgba(0, 0, 0, 0.28));
  border: 1px solid var(--line-soft);
  border-radius: 7px;
  overflow-x: auto;         /* 가로 스크롤 (넘침 방지) */
  max-width: 100%;
}
[data-theme="light"] .markdown-body :deep(pre) {
  background: rgba(0, 0, 0, 0.05);
}

/* 코드블록 내부 code는 인라인 스타일 초기화 */
.markdown-body :deep(pre code) {
  display: block;
  background: none;
  border: none;
  padding: 0;
  border-radius: 0;
  font-size: 0.86em;
  line-height: 1.5;
  white-space: pre;
  color: var(--text-primary);
}

/* 표 */
.markdown-body :deep(table) {
  border-collapse: collapse;
  margin: 0 0 8px;
  font-size: 0.92em;
  display: block;
  overflow-x: auto;         /* 넓은 표 가로 스크롤 */
  max-width: 100%;
}
.markdown-body :deep(th),
.markdown-body :deep(td) {
  border: 1px solid var(--line);
  padding: 5px 9px;
  text-align: left;
}
.markdown-body :deep(th) {
  background: var(--bg-panel-2);
  font-weight: 600;
}

/* 이미지 */
.markdown-body :deep(img) {
  max-width: 100%;
  border-radius: 6px;
}
</style>
