// DS-10 §3 Vue Router 라우트 정의 + DS-10 §4 접근 제어 가드
import { createRouter, createWebHistory } from 'vue-router'
import { useProjectStore } from '@/stores/project'
import { useWorkspaceStore } from '@/stores/workspace'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      redirect: '/launcher',
    },
    {
      path: '/launcher',
      name: 'launcher',
      component: () => import('@/views/LauncherView.vue'),
    },
    {
      path: '/guide',
      name: 'guide',
      component: () => import('@/views/GuideView.vue'),
    },
    {
      path: '/settings',
      name: 'settings',
      component: () => import('@/views/ProjectSettingsView.vue'),
      // ?mode=new | ?path=<path>
    },
    {
      path: '/boot',
      name: 'boot',
      component: () => import('@/views/BootView.vue'),
      meta: { requiresConfig: true },
    },
    {
      path: '/workspace',
      name: 'workspace',
      component: () => import('@/views/WorkspaceView.vue'),
      meta: { requiresBoot: true },
      children: [
        {
          path: 'deliverables',
          name: 'workspace-deliverables',
          component: () => import('@/views/DeliverableView.vue'),
        },
        {
          path: 'redmine',
          name: 'workspace-redmine',
          component: () => import('@/views/RedmineView.vue'),
        },
        {
          path: 'browser',
          name: 'workspace-browser',
          component: () => import('@/components/BrowserPanel.vue'),
        },
      ],
    },
    // catch-all → launcher
    {
      path: '/:pathMatch(.*)*',
      redirect: '/launcher',
    },
  ],
})

// ── 네비게이션 가드 (DS-10 §4) ────────────────────────────
router.beforeEach((to) => {
  const projectStore = useProjectStore()
  const workspaceStore = useWorkspaceStore()

  // /boot: agiteam.json 로드 필요 (DS-10 §4)
  // #27 완화: config 미충족 시 /settings 강제(무한 루프 원인) 제거 → 홈(/launcher)으로 안전 복귀
  if (to.meta.requiresConfig && !projectStore.config) {
    return { name: 'launcher' }
  }

  // /workspace: 부팅 완료(active) 필요 (DS-10 §4)
  // #27 완화: 미충족 시 config 있으면 /boot(부팅 재개), 없으면 홈(/launcher).
  //   활성화 직전(isDone && !isActive) 찰나에 /boot로 보내도 BootView watch가
  //   즉시 /workspace로 재이동시키므로 안전하다.
  if (to.meta.requiresBoot && !workspaceStore.isActive) {
    return projectStore.config ? { name: 'boot' } : { name: 'launcher' }
  }

  return true
})

export default router
