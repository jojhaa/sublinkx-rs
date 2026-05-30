<script setup lang="ts">
import { useI18n, type Locale } from '../i18n'

defineProps<{
  compact?: boolean
}>()

const { locale, setLocale, t } = useI18n()

function updateLocale(event: Event) {
  setLocale((event.target as HTMLSelectElement).value as Locale)
}

function chooseLocale(nextLocale: Locale) {
  setLocale(nextLocale)
}
</script>

<template>
  <div v-if="compact" class="language-switch language-switch-compact" :aria-label="t('language')">
    <button
      type="button"
      :class="{ active: locale === 'zh-CN' }"
      :aria-pressed="locale === 'zh-CN'"
      @click="chooseLocale('zh-CN')"
    >
      中
    </button>
    <button
      type="button"
      :class="{ active: locale === 'en-US' }"
      :aria-pressed="locale === 'en-US'"
      @click="chooseLocale('en-US')"
    >
      EN
    </button>
  </div>
  <label v-else class="language-switch">
    <span>{{ t('language') }}</span>
    <select :value="locale" @change="updateLocale">
      <option value="zh-CN">{{ t('chinese') }}</option>
      <option value="en-US">{{ t('english') }}</option>
    </select>
  </label>
</template>
