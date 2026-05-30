import axios from 'axios'
import { TOKEN_KEY } from '../store/auth'

const apiClient = axios.create({
  baseURL: import.meta.env.VITE_API_BASE_URL ?? 'http://127.0.0.1:8080',
  timeout: 15000,
})

apiClient.interceptors.request.use((config) => {
  const token = localStorage.getItem(TOKEN_KEY)

  if (token) {
    config.headers.Authorization = `Bearer ${token}`
  }

  return config
})

export function extractApiError(error: unknown): string {
  if (axios.isAxiosError(error)) {
    const data = error.response?.data as
      | { error?: string; message?: string }
      | undefined

    return data?.error ?? data?.message ?? error.message
  }

  if (error instanceof Error) {
    return error.message
  }

  return 'Request failed. Please try again later.'
}

export default apiClient
