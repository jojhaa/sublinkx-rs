<script setup lang="ts">
import { useI18n, type Locale } from '../i18n'

const { locale, setLocale, t } = useI18n()

function updateLocale(event: Event) {
  setLocale((event.target as HTMLSelectElement).value as Locale)
}
</script>

<template>
  <label class="language-switch">
    <span>{{ t('language') }}</span>
    <select :value="locale" @change="updateLocale">
      <option value="zh-CN">{{ t('chinese') }}</option>
      <option value="en-US">{{ t('english') }}</option>
    </select>
  </label>
</template>
