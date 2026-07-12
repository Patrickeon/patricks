// useChatAttachments — 채팅 첨부 3경로(파일 선택·클립보드·드래그&드롭) 공용 로직 (#21)
// DS-60 v0.7 §4.1: 세 경로 모두 pending → ready | failed 상태의 attachment chip으로 표시.
// failed chip은 전송 payload에서 제외하고 오류 사유를 표시한다.
import { ref, computed } from 'vue'
import { prepareChatAttachment } from '@/ipc/agent'
import type {
  ChatAttachmentInput,
  ChatAttachmentKind,
  ChatAttachmentSource,
  ChatAttachmentStatus,
  PreparedChatAttachment,
} from '@/ipc/types'

// ── 클라이언트 선검증 상수 (DS-60 v0.7 §3.2 처리 규칙과 동일) ──
/** 허용 이미지 MIME */
const IMAGE_MIMES = new Set(['image/png', 'image/jpeg', 'image/webp', 'image/gif'])
/** 허용 문서 확장자 */
const DOC_EXTS = new Set(['md', 'markdown', 'txt', 'csv', 'json', 'yaml', 'yml', 'log', 'pdf'])
/** 이미지 한도 10MB/건 */
const MAX_IMAGE_BYTES = 10 * 1024 * 1024
/** 문서 한도 20MB/건 */
const MAX_DOC_BYTES = 20 * 1024 * 1024
/** 메시지당 첨부 한도 */
const MAX_ATTACHMENTS = 10

/** 첨부 칩 표시 모델 */
export type AttachmentChip = {
  id: string
  kind: ChatAttachmentKind
  source: ChatAttachmentSource
  filename: string
  media_type: string
  size_bytes: number
  status: ChatAttachmentStatus
  /** 이미지 칩 썸네일용 data URL */
  previewUrl?: string
  /** failed 시 오류 사유 */
  error?: string
  /** ready 시 prepare_chat_attachment 응답 원본 (전송 payload용) */
  prepared?: PreparedChatAttachment
}

function fileExt(name: string): string {
  const idx = name.lastIndexOf('.')
  return idx < 0 ? '' : name.slice(idx + 1).toLowerCase()
}

/** MIME·확장자로 첨부 종류 판별. 미지원 형식이면 null */
function detectKind(file: File): ChatAttachmentKind | null {
  if (IMAGE_MIMES.has(file.type)) return 'image'
  if (DOC_EXTS.has(fileExt(file.name))) return 'document'
  return null
}

/** File → data URL (썸네일·base64 전송 겸용) */
function readAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result))
    reader.onerror = () => reject(reader.error ?? new Error('파일 읽기 실패'))
    reader.readAsDataURL(file)
  })
}

/**
 * 채팅 패널 1개당 1인스턴스로 사용한다.
 * @param getSessionId prepare 시점의 세션 ID 공급자 (세션 미보유 시 undefined)
 */
export function useChatAttachments(getSessionId: () => string | undefined) {
  const chips = ref<AttachmentChip[]>([])
  const isDragOver = ref(false)

  /** 전송 payload — ready 칩의 PreparedChatAttachment만 (failed/pending 제외) */
  const readyAttachments = computed<PreparedChatAttachment[]>(() =>
    chips.value
      .filter((c) => c.status === 'ready' && c.prepared)
      .map((c) => c.prepared as PreparedChatAttachment),
  )

  /** 준비 중 첨부 존재 여부 — 전송 버튼 비활성화에 사용 */
  const hasPending = computed(() => chips.value.some((c) => c.status === 'pending'))

  function removeChip(id: string) {
    chips.value = chips.value.filter((c) => c.id !== id)
  }

  /** 전송 성공 후 초기화 */
  function clear() {
    chips.value = []
  }

  /** 파일 1건 추가 → 선검증 → prepare_chat_attachment 호출 */
  async function addFile(file: File, source: ChatAttachmentSource) {
    // 메시지당 건수 한도 (failed 칩은 payload에서 제외되므로 유효 칩 기준)
    const effective = chips.value.filter((c) => c.status !== 'failed').length
    if (effective >= MAX_ATTACHMENTS) {
      chips.value.push({
        id: crypto.randomUUID(),
        kind: 'document',
        source,
        filename: file.name || '(이름 없음)',
        media_type: file.type,
        size_bytes: file.size,
        status: 'failed',
        error: `메시지당 첨부는 최대 ${MAX_ATTACHMENTS}건입니다`,
      })
      return
    }

    const kind = detectKind(file)
    const chip: AttachmentChip = {
      id: crypto.randomUUID(),
      kind: kind ?? 'document',
      source,
      filename: file.name || (kind === 'image' ? 'clipboard-image.png' : '(이름 없음)'),
      media_type: file.type || 'application/octet-stream',
      size_bytes: file.size,
      status: 'pending',
    }
    chips.value.push(chip)
    const patch = (p: Partial<AttachmentChip>) => {
      const i = chips.value.findIndex((c) => c.id === chip.id)
      if (i >= 0) chips.value[i] = { ...chips.value[i], ...p }
    }

    // ── 클라이언트 선검증 (빠른 실패 — Rust 검증과 동일 규칙) ──
    if (!kind) {
      patch({ status: 'failed', error: '지원하지 않는 형식 (이미지 png/jpeg/webp/gif, 문서 md/txt/csv/json/yaml/log/pdf)' })
      return
    }
    const maxBytes = kind === 'image' ? MAX_IMAGE_BYTES : MAX_DOC_BYTES
    if (file.size > maxBytes) {
      patch({ status: 'failed', error: `${kind === 'image' ? '이미지 10MB' : '문서 20MB'}/건 한도 초과 (${(file.size / 1024 / 1024).toFixed(1)}MB)` })
      return
    }

    // ── base64 읽기 + 이미지 썸네일 ──
    let dataUrl: string
    try {
      dataUrl = await readAsDataUrl(file)
    } catch (e) {
      patch({ status: 'failed', error: `파일 읽기 실패: ${e}` })
      return
    }
    if (kind === 'image') patch({ previewUrl: dataUrl })

    // ── prepare_chat_attachment 호출 ──
    const sessionId = getSessionId()
    if (!sessionId) {
      patch({ status: 'failed', error: '에이전트 세션이 없습니다 (READY 후 첨부하세요)' })
      return
    }
    const input: ChatAttachmentInput = {
      id: chip.id,
      kind,
      source,
      filename: chip.filename,
      media_type: chip.media_type,
      size_bytes: file.size,
      content_base64: dataUrl.slice(dataUrl.indexOf(',') + 1),
    }
    try {
      const prepared = await prepareChatAttachment(sessionId, input)
      if (prepared.status === 'failed') {
        patch({ status: 'failed', error: prepared.error?.message ?? '첨부 준비 실패' })
      } else {
        patch({ status: 'ready', prepared })
      }
    } catch (e) {
      // AppError { code, message } 또는 문자열
      const msg = (e as { message?: string })?.message ?? String(e)
      patch({ status: 'failed', error: msg })
    }
  }

  function addFiles(files: Iterable<File>, source: ChatAttachmentSource) {
    for (const f of files) void addFile(f, source)
  }

  // ── ① 파일 선택 다이얼로그 ──────────────────────────────
  function onFileInputChange(e: Event) {
    const input = e.target as HTMLInputElement
    if (input.files?.length) addFiles(input.files, 'file_picker')
    input.value = '' // 같은 파일 재선택 허용
  }

  // ── ② 클립보드 붙여넣기 (Cmd+V) ─────────────────────────
  // DS-60 §4.3: plain text는 기본 붙여넣기로 처리, 파일형 item만 첨부 경로
  function onPaste(e: ClipboardEvent) {
    const items = e.clipboardData?.items
    if (!items) return
    const files: File[] = []
    for (const item of items) {
      if (item.kind !== 'file') continue
      const f = item.getAsFile()
      if (f) files.push(f)
    }
    if (files.length === 0) return // 텍스트 → 기본 동작 유지
    e.preventDefault()
    addFiles(files, 'clipboard')
  }

  // ── ③ 드래그&드롭 ────────────────────────────────────────
  function onDragOver(e: DragEvent) {
    if (!e.dataTransfer?.types.includes('Files')) return
    e.preventDefault()
    isDragOver.value = true
  }

  function onDragLeave() {
    isDragOver.value = false
  }

  function onDrop(e: DragEvent) {
    isDragOver.value = false
    if (!e.dataTransfer?.files.length) return
    e.preventDefault()
    addFiles(e.dataTransfer.files, 'drag_drop')
  }

  return {
    chips,
    isDragOver,
    readyAttachments,
    hasPending,
    removeChip,
    clear,
    onFileInputChange,
    onPaste,
    onDragOver,
    onDragLeave,
    onDrop,
  }
}
