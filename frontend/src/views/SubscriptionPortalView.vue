<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'
import { useRoute } from 'vue-router'
import QRCode from 'qrcode'
import { extractApiError } from '../api/client'
import {
  unlockSubscriptionPortal,
  type SubscriptionPortalData,
  type SubscriptionPortalLink,
} from '../api/subscriptions'
import LanguageSwitch from '../components/LanguageSwitch.vue'
import { useI18n } from '../i18n'
import PortalClientLaunch from '../components/PortalClientLaunch.vue'
import { portalLaunchOptions } from '../utils/portalClientLaunch'

const route = useRoute()
const { t } = useI18n()
const form = reactive({ accessCode: '' })
const loading = ref(false)
const errorMessage = ref('')
const portal = ref<SubscriptionPortalData | null>(null)
const copiedTarget = ref('')
const qrDialog = ref<{ label: string; dataUrl: string; url: string } | null>(null)
const originalTitle = document.title

const portalSlug = computed(() => String(route.params.portalSlug ?? ''))
const defaultTarget = computed(() => portal.value?.default_client ?? 'adaptive')

onMounted(() => {
  document.title = t('portalDocumentTitle')
})

onUnmounted(() => {
  document.title = originalTitle
})

async function unlock() {
  loading.value = true
  errorMessage.value = ''
  try {
    const response = await unlockSubscriptionPortal(portalSlug.value, form.accessCode)
    portal.value = response.data
    form.accessCode = ''
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    loading.value = false
  }
}

function linkUrl(item: SubscriptionPortalLink) {
  const configuredBase = import.meta.env.VITE_API_BASE_URL
  const base = configuredBase || window.location.origin
  return new URL(item.path, base).toString()
}

function targetLabel(item: SubscriptionPortalLink) {
  if (item.target === 'adaptive') {
    return t('portalAdaptive')
  }
  return item.label
}

function targetDescription(item: SubscriptionPortalLink) {
  if (portal.value?.include_rules === false) return '此订阅保留节点、国家分组、负载均衡和故障转移等策略组，不携带 DNS、网站分流规则或远程规则集。Clash/Mihomo 保留指向“节点选择”的兜底规则，流量跟随所选策略组；自定义模板使用现有策略组。'
  if (item.target === 'quanx') {
    return '完整配置需在 Quantumult X 的配置文件页面手动导入；此处提供链接和操作说明，不会自动下载。'
  }
  if (item.target === 'shadowrocket-profile') {
    return t('portalShadowrocketHint')
  }
  if (item.target === 'adaptive') {
    return t('portalAdaptiveHint')
  }
  return item.full_profile ? t('portalFullProfileHint') : t('portalNodeLinkHint')
}

function launchOptions(item: SubscriptionPortalLink) {
  return portalLaunchOptions(item.target, (portal.value?.links ?? []).map(link => ({
    target: link.target, url: linkUrl(link),
  })), portal.value?.name ?? '', /Android/i.test(navigator.userAgent))
}

function launchHint(item: SubscriptionPortalLink) {
  if (item.target === 'quanx') return 'Quantumult X 的公开唤起协议只支持添加远程资源，不能一键安装这里的完整配置。请复制链接，在应用的配置文件页面导入。'
  if (item.target === 'xray' || item.target === 'mixed') return 'Android 可唤起 v2rayNG 导入节点订阅。v2rayN 请复制链接后在订阅管理中添加；网页无法确认其唤起协议支持情况。'
  return '选择后将把对应格式的订阅地址交给客户端，由客户端获取订阅并确认导入。'
}

async function copyLink(item: SubscriptionPortalLink) {
  try {
    await navigator.clipboard.writeText(linkUrl(item))
    copiedTarget.value = item.target
    window.setTimeout(() => {
      if (copiedTarget.value === item.target) {
        copiedTarget.value = ''
      }
    }, 1800)
  } catch {
    errorMessage.value = t('copyFailed')
  }
}

async function showQr(item: SubscriptionPortalLink) {
  const url = linkUrl(item)
  qrDialog.value = {
    label: targetLabel(item),
    url,
    dataUrl: await QRCode.toDataURL(url, {
      width: 360,
      margin: 2,
      color: { dark: '#063f46', light: '#ffffff' },
    }),
  }
}

function formatExpiry(value: string | null) {
  if (!value) {
    return t('longTerm')
  }
  return new Intl.DateTimeFormat(undefined, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value))
}
</script>

<template>
  <main class="portal-page">
    <div class="portal-atmosphere portal-atmosphere-one"></div>
    <div class="portal-atmosphere portal-atmosphere-two"></div>

    <header class="portal-topbar">
      <a class="portal-brand" href="/" aria-label="SublinkX RS">
        <span class="portal-brand-mark">RS</span>
        <span>
          <strong>SublinkX RS</strong>
          <small>{{ t('portalBrandCaption') }}</small>
        </span>
      </a>
      <LanguageSwitch compact />
    </header>

    <section v-if="!portal" class="portal-gate glass-panel">
      <div class="portal-gate-index">ACCESS / 01</div>
      <div class="portal-gate-copy">
        <span class="eyebrow">{{ t('portalEyebrow') }}</span>
        <h1>{{ t('portalUnlockTitle') }}</h1>
        <p>{{ t('portalUnlockCopy') }}</p>
      </div>

      <form class="portal-access-form" @submit.prevent="unlock">
        <label class="field-label" for="portal-access-code">{{ t('portalAccessCode') }}</label>
        <div class="portal-access-row">
          <input
            id="portal-access-code"
            v-model="form.accessCode"
            class="input portal-access-input"
            type="password"
            autocomplete="one-time-code"
            autofocus
            :placeholder="t('portalAccessCodePlaceholder')"
          />
          <button class="button button-accent portal-unlock-button" type="submit" :disabled="loading || form.accessCode.length === 0">
            {{ loading ? t('portalUnlocking') : t('portalUnlock') }}
          </button>
        </div>
        <div v-if="errorMessage" class="error-banner">{{ t('portalUnlockFailed') }}</div>
        <p class="portal-privacy-note">{{ t('portalPrivacyNote') }}</p>
      </form>
    </section>

    <section v-else class="portal-workspace">
      <div class="portal-hero glass-panel">
        <div>
          <span class="eyebrow">{{ t('portalEyebrow') }}</span>
          <h1>{{ portal.name }}</h1>
          <p>{{ portal.description || t('portalNoDescription') }}</p>
        </div>
        <div class="portal-status-stack">
          <span class="status-badge status-badge-ok">{{ t('portalActive') }}</span>
          <div>
            <small>{{ t('expiresAt') }}</small>
            <strong>{{ formatExpiry(portal.expires_at) }}</strong>
          </div>
        </div>
      </div>

      <div class="portal-section-heading">
        <div>
          <span class="eyebrow">{{ t('portalClientEyebrow') }}</span>
          <h2>{{ t('portalClientTitle') }}</h2>
        </div>
        <p>{{ t('portalClientCopy') }}</p>
      </div>

      <div class="portal-client-grid">
        <article
          v-for="(item, index) in portal.links"
          :key="item.target"
          class="portal-client-card glass-panel"
          :class="{ 'portal-client-card-featured': item.target === defaultTarget || (defaultTarget === 'adaptive' && item.target === 'adaptive') }"
        >
          <div class="portal-client-card-head">
            <span class="portal-client-index">{{ String(index + 1).padStart(2, '0') }}</span>
            <span v-if="item.target === defaultTarget" class="status-badge status-badge-ok">{{ t('portalRecommended') }}</span>
            <span v-else-if="item.full_profile" class="status-badge status-badge-neutral">{{ t('portalFullProfile') }}</span>
          </div>
          <h3>{{ targetLabel(item) }}</h3>
          <p>{{ targetDescription(item) }}</p>
          <div class="portal-client-actions">
            <PortalClientLaunch :options="launchOptions(item)" :hint="launchHint(item)" :copied="copiedTarget === item.target" @copy="copyLink(item)" />
            <button class="button button-ghost portal-secondary-action" type="button" @click="copyLink(item)">
              {{ copiedTarget === item.target ? t('portalCopied') : t('copyLink') }}
            </button>
            <button class="button button-ghost portal-secondary-action" type="button" @click="showQr(item)">{{ t('qrCode') }}</button>
          </div>
        </article>
      </div>

      <footer class="portal-footer">
        <span>{{ t('portalLiveSync') }}</span>
        <button class="button button-ghost button-compact" type="button" @click="portal = null; errorMessage = ''">
          {{ t('portalLockAgain') }}
        </button>
      </footer>
    </section>

    <Teleport to="body">
      <div v-if="qrDialog" class="modal-backdrop" @click.self="qrDialog = null">
        <section class="modal-panel qr-modal-panel portal-qr-panel">
          <header class="modal-header">
            <div>
              <span class="eyebrow">QR / EXPORT</span>
              <h3>{{ qrDialog.label }}</h3>
            </div>
            <button class="icon-button" type="button" :aria-label="t('close')" @click="qrDialog = null">×</button>
          </header>
          <img class="qr-image" :src="qrDialog.dataUrl" :alt="qrDialog.label" />
          <code class="qr-link-preview">{{ qrDialog.url }}</code>
        </section>
      </div>
    </Teleport>
  </main>
</template>
