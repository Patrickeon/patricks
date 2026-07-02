import { defineStore } from 'pinia'
import { ref } from 'vue'

export type Theme = 'dark' | 'light'

const STORAGE_KEY = 'lugh:theme'

export const useThemeStore = defineStore('theme', () => {
  const theme = ref<Theme>(
    (localStorage.getItem(STORAGE_KEY) as Theme) ?? 'dark'
  )

  function applyTheme(t: Theme) {
    document.documentElement.setAttribute('data-theme', t)
  }

  function setTheme(t: Theme) {
    theme.value = t
    localStorage.setItem(STORAGE_KEY, t)
    applyTheme(t)
  }

  function toggleTheme() {
    setTheme(theme.value === 'dark' ? 'light' : 'dark')
  }

  // 초기 적용
  applyTheme(theme.value)

  return { theme, setTheme, toggleTheme }
})
