import apiClient from './client'

export interface AppSettings {
  public_base_url: string
  latency_auto_enabled: boolean
  latency_interval_minutes: number
  latency_core_path: string
  latency_test_url: string
  latency_timeout_secs: number
}

interface SettingsResponse {
  code: string
  data: AppSettings
}

export interface MihomoCoreStatus {
  os: string
  arch: string
  supported: boolean
  installed: boolean
  path: string | null
  version: string | null
  message: string
}

export interface MihomoCoreDownloadResult {
  os: string
  arch: string
  version: string
  asset_name: string
  path: string
  size: number
}

interface MihomoCoreStatusResponse {
  code: string
  data: MihomoCoreStatus
}

interface MihomoCoreDownloadResponse {
  code: string
  data: MihomoCoreDownloadResult
}

export async function getSettings() {
  const { data } = await apiClient.get<SettingsResponse>('/api/v1/settings')
  return data
}

export async function updateSettings(payload: AppSettings) {
  const { data } = await apiClient.put<SettingsResponse>('/api/v1/settings', payload)
  return data
}

export async function getMihomoCoreStatus() {
  const { data } = await apiClient.get<MihomoCoreStatusResponse>('/api/v1/settings/mihomo-core')
  return data
}

export async function downloadMihomoCore() {
  const { data } = await apiClient.post<MihomoCoreDownloadResponse>('/api/v1/settings/mihomo-core/download')
  return data
}
