import axios from 'axios'
import { readAuthToken, readCsrfToken, writeCsrfToken } from '../utils/authToken'

const apiBaseUrl = import.meta.env.VITE_API_BASE_URL || ''

export interface PaginationMeta {
  page: number
  page_size: number
  total: number
  total_pages: number
}

const apiClient = axios.create({
  baseURL: apiBaseUrl,
  timeout: 15000,
  withCredentials: true,
  headers: {
    'Cache-Control': 'no-cache',
    Pragma: 'no-cache',
  },
})

function appendCacheBust(url: string | undefined): string | undefined {
  if (!url) {
    return url
  }

  const separator = url.includes('?') ? '&' : '?'
  return `${url}${separator}_=${Date.now()}`
}

apiClient.interceptors.request.use((config) => {
  const token = readAuthToken()
  const csrfToken = readCsrfToken()

  if (token) {
    config.headers.Authorization = `Bearer ${token}`
  }
  if (csrfToken) {
    config.headers['X-CSRF-Token'] = csrfToken
  }

  if (config.method?.toLowerCase() === 'get') {
    config.url = appendCacheBust(config.url)
  }

  return config
})

apiClient.interceptors.response.use((response) => {
  const csrfToken = response.headers['x-csrf-token']
  if (typeof csrfToken === 'string' && csrfToken) {
    writeCsrfToken(csrfToken)
  }
  return response
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

export function extractApiErrorCode(error: unknown): string | null {
  if (axios.isAxiosError(error)) {
    const data = error.response?.data as
      | { code?: string }
      | undefined

    return data?.code ?? null
  }
  if (error && typeof error === 'object' && 'code' in error) {
    const code = (error as { code?: unknown }).code
    return typeof code === 'string' ? code : null
  }

  return null
}

export default apiClient
