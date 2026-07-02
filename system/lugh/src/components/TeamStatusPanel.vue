<script setup lang="ts">
import { computed } from 'vue'
import { useRoleStore } from '@/stores/role'
import StatusBadge from '@/components/StatusBadge.vue'

const roleStore = useRoleStore()
const members = computed(() => Array.from(roleStore.sessions.values()))

const roleIcons: Record<string, string> = {
  PM: '🧭', Architect: '🏛️', DeveloperBE: '⚙️',
  DeveloperFE: '🖥️', DevOps: '🚀', Designer: '🎨', QA: '🔍',
}
</script>

<template>
  <div class="team-status-panel">
    <div class="panel-header">
      <span class="panel-title">팀 상태</span>
      <span class="panel-count">
        {{ members.filter(m => m.status === 'ready' || m.status === 'running').length }}
        / {{ members.length }} 활성
      </span>
    </div>
    <div class="member-list">
      <div v-for="m in members" :key="m.role" class="member-row">
        <span class="member-icon">{{ roleIcons[m.role] ?? '🤖' }}</span>
        <div class="member-info">
          <span class="member-name">{{ m.name }}</span>
          <span class="member-role">{{ m.role }}</span>
        </div>
        <StatusBadge :state="m.status" size="sm" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.team-status-panel { display: flex; flex-direction: column; height: 100%; }
.panel-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 12px 16px; border-bottom: 1px solid var(--line);
}
.panel-title { font-size: 13px; font-weight: 700; color: var(--text-primary); }
.panel-count { font-size: 12px; color: var(--text-muted); }
.member-list { flex: 1; overflow-y: auto; padding: 8px; display: flex; flex-direction: column; gap: 4px; }
.member-row {
  display: flex; align-items: center; gap: 10px; padding: 10px 12px;
  border-radius: 8px; background: var(--bg-panel-2); border: 1px solid var(--line-soft);
}
.member-icon { font-size: 18px; flex-shrink: 0; }
.member-info { flex: 1; display: flex; flex-direction: column; gap: 1px; }
.member-name { font-size: 13px; font-weight: 600; color: var(--text-primary); }
.member-role { font-size: 11px; color: var(--text-muted); }
</style>
