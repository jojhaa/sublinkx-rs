<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { extractApiError } from '../api/client'
import { listNodes } from '../api/nodes'
import { listSubscriptions } from '../api/subscriptions'
import { useI18n } from '../i18n'

const { t } = useI18n()
const nodeCount = ref(0)
const subscriptionCount = ref(0)
const loading = ref(false)
const errorMessage = ref('')

const summaryText = computed(() => {
  return t('overviewSummary', { nodes: nodeCount.value, subscriptions: subscriptionCount.value })
})

async function load() {
  loading.value = true
  errorMessage.value = ''

  try {
    const [nodes, subscriptions] = await Promise.all([listNodes(), listSubscriptions()])
    nodeCount.value = nodes.data.length
    subscriptionCount.value = subscriptions.data.length
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    loading.value = false
  }
}

onMounted(load)
</script>

<template>
  <section class="stack">
    <header class="page-header">
      <div>
        <span class="eyebrow">{{ t('overviewEyebrow') }}</span>
        <h2 class="page-title">{{ t('overviewTitle') }}</h2>
        <p class="page-copy">{{ t('overviewCopy') }}</p>
      </div>
      <button class="button button-ghost" type="button" :disabled="loading" @click="load">
        {{ loading ? t('refreshing') : t('refreshData') }}
      </button>
    </header>

    <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>

    <div class="dashboard-grid">
      <article class="card stat-card">
        <div class="hint">{{ t('nodeTotal') }}</div>
        <p class="stat-value">{{ nodeCount }}</p>
        <p class="stat-note">{{ t('nodeTotalNote') }}</p>
      </article>

      <article class="card stat-card">
        <div class="hint">{{ t('subscriptionTotal') }}</div>
        <p class="stat-value">{{ subscriptionCount }}</p>
        <p class="stat-note">{{ t('subscriptionTotalNote') }}</p>
      </article>

      <article class="card stat-card">
        <div class="hint">{{ t('exportSystem') }}</div>
        <p class="stat-value">Multi</p>
        <p class="stat-note">{{ t('exportSystemNote') }}</p>
      </article>

      <article class="card wide-card">
        <div class="hint">{{ t('currentStatus') }}</div>
        <p class="card-copy">{{ summaryText }}</p>
      </article>
    </div>
  </section>
</template>
