<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { RouterLink, RouterView, useRouter } from 'vue-router'
import { useAuthStore } from '../store/auth'

const router = useRouter()
const auth = useAuthStore()

const navItems = [
  { name: 'dashboard', label: '总览', to: '/' },
  { name: 'nodes', label: '节点管理', to: '/nodes' },
  { name: 'subscriptions', label: '订阅管理', to: '/subscriptions' },
  { name: 'templates', label: '模板管理', to: '/templates' },
  { name: 'settings', label: '系统设置', to: '/settings' },
]

const userLabel = computed(() => auth.user?.nickname || auth.user?.username || '管理员')

async function logout() {
  auth.clearAuth()
  await router.replace('/login')
}

onMounted(async () => {
  if (auth.token && !auth.user) {
    try {
      await auth.fetchMe()
    } catch {
      auth.clearAuth()
      await router.replace('/login')
    }
  }
})
</script>

<template>
  <div class="page-shell shell-layout">
    <aside class="sidebar">
      <div class="brand-block">
        <span class="eyebrow">SublinkX RS</span>
        <h1 class="brand-title">订阅控制台</h1>
        <p class="brand-copy">
          面向多协议节点、客户端模板和订阅分发的运维工作台。
        </p>
      </div>

      <nav class="nav-list" aria-label="主导航">
        <RouterLink
          v-for="item in navItems"
          :key="item.name"
          :to="item.to"
          class="nav-link"
        >
          <span>{{ item.label }}</span>
        </RouterLink>
      </nav>

      <div class="sidebar-footer">
        <div class="user-card">
          <div class="hint">当前登录</div>
          <strong>{{ userLabel }}</strong>
          <div class="muted">{{ auth.user?.role ?? 'admin' }}</div>
        </div>
        <button class="button button-ghost" type="button" @click="logout">退出登录</button>
      </div>
    </aside>

    <main class="main-panel">
      <RouterView />
    </main>
  </div>
</template>
