import apiClient from './client'

interface DashboardStatsResponse {
  code: string
  data: {
    nodes: number
    subscriptions: number
    templates: number
    upstream_subscriptions: number
  }
}

export async function getDashboardStats() {
  const { data } = await apiClient.get<DashboardStatsResponse>('/api/v1/dashboard/stats')
  return data
}
