<script setup lang="ts">
import { onUnmounted } from 'vue'
import { useToastList } from '@/composables/toast'

const { toasts, dismissToast } = useToastList()

onUnmounted(() => {
  toasts.value.forEach((t) => {
    if (t.timer) clearTimeout(t.timer)
  })
})
</script>

<template>
  <Teleport to="body">
    <div class="toast-container">
      <TransitionGroup name="toast">
        <div
          v-for="t in toasts"
          :key="t.id"
          class="toast"
          :class="t.type"
          @click="dismissToast(t.id)"
        >
          <span class="toast-icon">
            <template v-if="t.type === 'ok'">✅</template>
            <template v-else-if="t.type === 'error'">❌</template>
            <template v-else-if="t.type === 'warn'">⚠️</template>
            <template v-else>ℹ️</template>
          </span>
          <span class="toast-msg">{{ t.message }}</span>
          <button class="toast-close" @click.stop="dismissToast(t.id)">×</button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-container {
  position: fixed;
  bottom: 24px;
  right: 24px;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 8px;
  pointer-events: none;
}

.toast {
  pointer-events: all;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  border-radius: 8px;
  border: 1px solid var(--line);
  background: var(--bg-panel);
  min-width: 280px;
  max-width: 400px;
  font-size: 13px;
  cursor: pointer;
  box-shadow: var(--shadow);
}

.toast.ok    { border-color: rgba(34, 197, 94, 0.4); }
.toast.error { border-color: rgba(239, 68, 68, 0.4);  }
.toast.warn  { border-color: rgba(251, 191, 36, 0.4); }
.toast.info  { border-color: rgba(99, 102, 241, 0.4); }

.toast-msg {
  flex: 1;
  line-height: 1.4;
  color: var(--text-primary);
}

.toast-close {
  background: none;
  border: none;
  color: var(--text-muted);
  font-size: 16px;
  cursor: pointer;
  padding: 0 2px;
  line-height: 1;
}

/* Transition */
.toast-enter-active { transition: all 0.2s ease; }
.toast-leave-active { transition: all 0.2s ease; }
.toast-enter-from  { opacity: 0; transform: translateX(20px); }
.toast-leave-to    { opacity: 0; transform: translateX(20px); }
</style>
