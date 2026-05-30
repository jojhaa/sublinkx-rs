<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { extractApiError } from '../api/client'
import { listNodes } from '../api/nodes'
import { listSubscriptions } from '../api/subscriptions'

const nodeCount = ref(0)
const subscriptionCount = ref(0)
const loading = ref(false)
const errorMessage = ref('')

const summaryText = computed(() => {
  return `当前已有 ${nodeCount.value} 个节点和 ${subscriptionCount.value} 个订阅。节点、模板、订阅链接已经形成完整分发链路。`
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
        <span class="eyebrow">Overview</span>
        <h2 class="page-title">总览</h2>
        <p class="page-copy">把节点库存、订阅分发和客户端导出状态放在同一个仪表盘里。</p>
      </div>
      <button class="button button-ghost" type="button" :disabled="loading" @click="load">
        {{ loading ? '刷新中...' : '刷新数据' }}
      </button>
    </header>

    <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>

    <div class="dashboard-grid">
      <article class="card stat-card">
        <div class="hint">节点总数</div>
        <p class="stat-value">{{ nodeCount }}</p>
        <p class="stat-note">覆盖 SS、VMess、VLESS、Trojan、Hysteria2、TUIC、WireGuard、AnyTLS 等协议。</p>
      </article>

      <article class="card stat-card">
        <div class="hint">订阅总数</div>
        <p class="stat-value">{{ subscriptionCount }}</p>
        <p class="stat-note">每个订阅拥有独立随机 token，可轮换，可按客户端自动识别。</p>
      </article>

      <article class="card stat-card">
        <div class="hint">导出体系</div>
        <p class="stat-value">Multi</p>
        <p class="stat-note">已接入 Clash / Mihomo / Surge / sing-box / Xray / QuanX / Loon 等导出目标。</p>
      </article>

      <article class="card wide-card">
        <div class="hint">当前状态</div>
        <p class="card-copy">{{ summaryText }}</p>
      </article>
    </div>
  </section>
</template>
