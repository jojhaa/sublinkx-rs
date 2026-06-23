import { defineStore } from 'pinia'
import {
  changeCredentialsRequest,
  fetchMeRequest,
  loginRequest,
  logoutRequest,
  type ChangeCredentialsPayload,
  type LoginPayload,
} from '../api/auth'
import { clearAuthToken, readAuthToken, readCsrfToken, writeCsrfToken } from '../utils/authToken'

export interface AuthUser {
  user_id: number
  username: string
  nickname: string
  role: string
  status: string
  must_change_credentials: boolean
}

export const useAuthStore = defineStore('auth', {
  state: () => ({
    token: readCsrfToken() || readAuthToken(),
    user: null as AuthUser | null,
  }),
  actions: {
    async login(payload: LoginPayload) {
      const response = await loginRequest(payload)
      const csrfToken = response.data.csrf_token
      this.token = csrfToken
      this.user = response.data.user
      writeCsrfToken(csrfToken)
    },
    async changeCredentials(payload: ChangeCredentialsPayload) {
      const response = await changeCredentialsRequest(payload)
      this.user = response.data
      this.token = readCsrfToken()
      return response.data
    },
    async fetchMe() {
      const response = await fetchMeRequest()
      this.user = response.data
      return response.data
    },
    clearAuth() {
      this.token = ''
      this.user = null
      clearAuthToken()
    },
    async logout() {
      try {
        await logoutRequest()
      } finally {
        this.clearAuth()
      }
    },
  },
})
