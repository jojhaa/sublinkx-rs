import { createRouter, createWebHistory } from 'vue-router'
import { useAuthStore } from '../store/auth'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/login',
      name: 'login',
      component: () => import('../views/LoginView.vue'),
      meta: { public: true },
    },
    {
      path: '/change-credentials',
      name: 'changeCredentials',
      component: () => import('../views/ChangeCredentialsView.vue'),
    },
    {
      path: '/',
      component: () => import('../layouts/AppShell.vue'),
      children: [
        {
          path: '',
          name: 'dashboard',
          component: () => import('../views/DashboardView.vue'),
        },
        {
          path: 'nodes',
          name: 'nodes',
          component: () => import('../views/NodesView.vue'),
        },
        {
          path: 'subscriptions',
          name: 'subscriptions',
          component: () => import('../views/SubscriptionsView.vue'),
        },
        {
          path: 'templates',
          name: 'templates',
          component: () => import('../views/TemplatesView.vue'),
        },
        {
          path: 'settings',
          name: 'settings',
          component: () => import('../views/SettingsView.vue'),
        },
        {
          path: 'about',
          name: 'about',
          component: () => import('../views/AboutView.vue'),
        },
      ],
    },
  ],
})

router.beforeEach(async (to) => {
  const auth = useAuthStore()

  if (!to.meta.public && !auth.token) {
    return { name: 'login', query: { redirect: to.fullPath } }
  }

  if (auth.token && !auth.user && to.name !== 'login') {
    try {
      await auth.fetchMe()
    } catch {
      auth.clearAuth()
      return { name: 'login', query: { redirect: to.fullPath } }
    }
  }

  if (auth.token && auth.user?.must_change_credentials && to.name !== 'changeCredentials') {
    return { name: 'changeCredentials' }
  }

  if (auth.token && !auth.user?.must_change_credentials && to.name === 'changeCredentials') {
    return { name: 'dashboard' }
  }

  if (to.name === 'login' && auth.token) {
    return { name: auth.user?.must_change_credentials ? 'changeCredentials' : 'dashboard' }
  }

  return true
})

export default router
