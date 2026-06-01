import apiClient from './client'

export interface UpdateCheck {
  checked: boolean
  update_available: boolean
  latest_version: string | null
  latest_url: string | null
  release_name: string | null
  published_at: string | null
  error: string | null
}

export interface VersionInfo {
  name: string
  version: string
  api_version: string
  environment: string
  repository: string
  license: string
  server_time: string
  server_timezone: string
  uptime_seconds: number
  runtime_mode: 'local' | 'docker' | string
  developer?: {
    name: string
    url: string
  }
  system: {
    os: string
    family: string
    arch: string
    display: string
  }
}

export async function getVersionInfo() {
  const { data } = await apiClient.get<VersionInfo>('/api/v1/version')
  return data
}

export async function checkForUpdates() {
  const { data } = await apiClient.get<UpdateCheck>('/api/v1/version/update-check', { timeout: 7000 })
  return data
}
