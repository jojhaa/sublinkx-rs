<script setup lang="ts">
import { reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { extractApiError } from '../api/client'
import LanguageSwitch from '../components/LanguageSwitch.vue'
import { useI18n } from '../i18n'
import { useAuthStore } from '../store/auth'

const router = useRouter()
const auth = useAuthStore()
const { t } = useI18n()

const form = reactive({
  username: '',
  current_password: '',
  new_password: '',
  confirm_password: '',
})

const loading = ref(false)
const errorMessage = ref('')

if (auth.user?.username) {
  form.username = auth.user.username === 'admin' ? '' : auth.user.username
}

async function submit() {
  loading.value = true
  errorMessage.value = ''

  try {
    await auth.changeCredentials(form)
    await router.replace('/')
  } catch (error) {
    errorMessage.value = extractApiError(error)
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="auth-page">
    <section class="auth-card">
      <div class="auth-topbar">
        <span class="eyebrow">Security Setup</span>
        <LanguageSwitch compact />
      </div>
      <h1 class="hero-title">{{ t('firstLoginTitle') }}</h1>
      <p class="hero-copy">
        {{ t('firstLoginCopy') }}
      </p>

      <form class="form-grid" @submit.prevent="submit">
        <div v-if="errorMessage" class="error-banner">{{ errorMessage }}</div>

        <div>
          <label class="field-label" for="new-username">{{ t('newUsername') }}</label>
          <input id="new-username" v-model.trim="form.username" class="input" autocomplete="username" />
        </div>

        <div>
          <label class="field-label" for="current-password">{{ t('currentPassword') }}</label>
          <input
            id="current-password"
            v-model="form.current_password"
            class="input"
            type="password"
            autocomplete="current-password"
          />
        </div>

        <div>
          <label class="field-label" for="new-password">{{ t('newPassword') }}</label>
          <input
            id="new-password"
            v-model="form.new_password"
            class="input"
            type="password"
            autocomplete="new-password"
          />
        </div>

        <div>
          <label class="field-label" for="confirm-password">{{ t('confirmPassword') }}</label>
          <input
            id="confirm-password"
            v-model="form.confirm_password"
            class="input"
            type="password"
            autocomplete="new-password"
          />
        </div>

        <button class="button button-accent" type="submit" :disabled="loading">
          {{ loading ? t('changingCredentials') : t('changeCredentials') }}
        </button>

        <div class="hint">
          {{ t('passwordPolicyHint') }}
        </div>
      </form>
    </section>
  </div>
</template>
