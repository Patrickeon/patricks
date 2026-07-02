<script setup lang="ts">
import type { AgentLifecycleState } from '@/ipc/types'

type BadgeState = AgentLifecycleState | 'waiting'

const props = defineProps<{
  state: BadgeState
  size?: 'sm' | 'md'
}>()

const labels: Record<BadgeState, string> = {
  idle:    'IDLE',
  booting: 'BOOTING',
  ready:   'READY',
  running: 'BUSY',
  failed:  'ERROR',
  waiting: 'WAIT',
}

const dotClass: Record<BadgeState, string> = {
  idle:    '',
  booting: 'booting',
  ready:   'ready',
  running: 'busy',
  failed:  'err',
  waiting: '',
}
</script>

<template>
  <span class="status-badge" :class="[`state-${props.state}`, props.size ?? 'md']">
    <span class="dot" :class="dotClass[props.state]" />
    <span class="label">{{ labels[props.state] }}</span>
  </span>
</template>

<style scoped>
.status-badge {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 2px 8px;
  border-radius: 999px;
  border: 1px solid var(--line);
  background: var(--bg-panel-2);
  font-size: 11px;
  white-space: nowrap;
  user-select: none;
}

.status-badge.sm { font-size: 10px; padding: 1px 6px; }

.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--text-muted);
  flex-shrink: 0;
}

.dot.ready   { background: var(--ok);    box-shadow: 0 0 8px rgba(34,197,94,0.6); }
.dot.busy    { background: var(--busy);  box-shadow: 0 0 8px rgba(251,191,36,0.6); animation: pulse 1.2s infinite; }
.dot.err     { background: var(--error); box-shadow: 0 0 8px rgba(239,68,68,0.6); }
.dot.booting { background: var(--accent);box-shadow: 0 0 8px rgba(99,102,241,0.5); animation: pulse 1s infinite; }

.label { color: var(--text-muted); letter-spacing: 0.04em; font-weight: 500; }

.state-ready   .label { color: var(--ok);    }
.state-running .label { color: var(--busy);  }
.state-failed  .label { color: var(--error); }
.state-booting .label { color: var(--accent);}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}
</style>
