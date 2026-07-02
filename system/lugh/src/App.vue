<script setup lang="ts">
import { RouterView } from 'vue-router'
import AppToast from '@/components/AppToast.vue'
import { useThemeStore } from '@/stores/theme'
import { onMounted } from 'vue'

const themeStore = useThemeStore()

// 샘플 프로젝트 자동 열기 주입 (한 번만 실행, 이미 설정돼 있으면 스킵)
onMounted(() => {
  if (!localStorage.getItem('lugh:dev-auto-open')) {
    localStorage.setItem('lugh:dev-auto-open', '/Users/patrickukim/Projects/patricks')
  }
})
</script>

<template>
  <div class="app-root" :data-theme="themeStore.theme">
    <RouterView />
    <AppToast />
  </div>
</template>

<style scoped>
.app-root {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bg-base);
}
</style>
