<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { RouterLink, RouterView, useRouter } from 'vue-router'
import LanguageSwitch from '../components/LanguageSwitch.vue'
import { useI18n, type MessageKey } from '../i18n'
import { useAuthStore } from '../store/auth'

const router = useRouter()
const auth = useAuthStore()
const { t } = useI18n()

const navItems = [
  { name: 'dashboard', labelKey: 'dashboard', to: '/' },
  { name: 'nodes', labelKey: 'nodes', to: '/nodes' },
  { name: 'subscriptions', labelKey: 'subscriptions', to: '/subscriptions' },
  { name: 'templates', labelKey: 'templates', to: '/templates' },
  { name: 'settings', labelKey: 'settings', to: '/settings' },
] satisfies Array<{ name: string; labelKey: MessageKey; to: string }>

const userLabel = computed(() => auth.user?.nickname || auth.user?.username || t('admin'))

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
        <h1 class="brand-title">{{ t('brandTitle') }}</h1>
        <p class="brand-copy">
          {{ t('brandCopy') }}
        </p>
      </div>

      <nav class="nav-list" :aria-label="t('mainNavigation')">
        <RouterLink
          v-for="item in navItems"
          :key="item.name"
          :to="item.to"
          class="nav-link"
        >
          <span>{{ t(item.labelKey) }}</span>
        </RouterLink>
      </nav>

      <div class="sidebar-footer">
        <LanguageSwitch />
        <div class="user-card">
          <div class="hint">{{ t('currentLogin') }}</div>
          <strong>{{ userLabel }}</strong>
          <div class="muted">{{ auth.user?.role ?? 'admin' }}</div>
        </div>
        <button class="button button-ghost" type="button" @click="logout">{{ t('logout') }}</button>
      </div>
    </aside>

    <main class="main-panel">
      <RouterView />
    </main>
  </div>
</template>
