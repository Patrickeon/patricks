// useBrowserStore — 내장 브라우저 URL 및 이력
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export type BrowserHistoryEntry = {
  url: string
  title?: string
  visitedAt: string
}

export const useBrowserStore = defineStore('browser', () => {
  const currentUrl = ref('about:blank')
  const history = ref<BrowserHistoryEntry[]>([])
  const historyIndex = ref(-1)
  const isLoading = ref(false)
  const addressBarValue = ref('')

  const canGoBack = computed(() => historyIndex.value > 0)
  const canGoForward = computed(() => historyIndex.value < history.value.length - 1)

  function navigate(url: string) {
    const normalized = normalizeUrl(url)
    // 현재 위치 이후의 이력 삭제 (포워드 이력 제거)
    history.value.splice(historyIndex.value + 1)
    history.value.push({
      url: normalized,
      visitedAt: new Date().toISOString(),
    })
    historyIndex.value = history.value.length - 1
    currentUrl.value = normalized
    addressBarValue.value = normalized
    isLoading.value = true
  }

  function goBack() {
    if (!canGoBack.value) return
    historyIndex.value--
    currentUrl.value = history.value[historyIndex.value].url
    addressBarValue.value = currentUrl.value
  }

  function goForward() {
    if (!canGoForward.value) return
    historyIndex.value++
    currentUrl.value = history.value[historyIndex.value].url
    addressBarValue.value = currentUrl.value
  }

  function setLoaded(title?: string) {
    isLoading.value = false
    if (title && history.value[historyIndex.value]) {
      history.value[historyIndex.value].title = title
    }
  }

  function updateAddressBar(value: string) {
    addressBarValue.value = value
  }

  function reset() {
    currentUrl.value = 'about:blank'
    history.value = []
    historyIndex.value = -1
    isLoading.value = false
    addressBarValue.value = ''
  }

  return {
    currentUrl,
    history,
    historyIndex,
    isLoading,
    addressBarValue,
    canGoBack,
    canGoForward,
    navigate,
    goBack,
    goForward,
    setLoaded,
    updateAddressBar,
    reset,
  }
})

function normalizeUrl(url: string): string {
  if (!url || url === 'about:blank') return url
  if (!/^https?:\/\//i.test(url)) return `https://${url}`
  return url
}
