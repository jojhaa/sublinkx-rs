import { shadowrocketProfileInstallUrl } from './clientProfileLinks'

export interface ClientLaunchOption {
  label: string
  href: string
}

export interface PortalLaunchLink {
  target: string
  url: string
}

// Only verified import schemes belong here. HTTP links must never be launch actions.
export function clientLaunchUrl(target: string, url: string, name: string): string | null {
  const parsed = new URL(url)
  if (!['http:', 'https:'].includes(parsed.protocol) || parsed.username || parsed.password) {
    throw new Error('Invalid subscription URL')
  }
  const encoded = encodeURIComponent(parsed.href)
  switch (target) {
    case 'mihomo':
    case 'clash': return `clash://install-config?url=${encoded}`
    case 'sing-box': return `sing-box://import-remote-profile?url=${encoded}#${encodeURIComponent(name)}`
    case 'surge': return `surge:///install-config?url=${encoded}`
    case 'loon': return `loon://import?sub=${encoded}`
    case 'surfboard': return `surfboard:///install-config?url=${encoded}`
    case 'shadowrocket-profile': return shadowrocketProfileInstallUrl(parsed.href)
    case 'mixed': return `v2rayng://install-sub?url=${encoded}#${encodeURIComponent(name)}`
    default: return null
  }
}

export function portalLaunchOptions(
  target: string,
  links: PortalLaunchLink[],
  name: string,
  android: boolean,
): ClientLaunchOption[] {
  const labels: Record<string, string> = {
    mihomo: 'Clash / Mihomo 客户端', clash: 'Clash 客户端',
    'sing-box': 'sing-box', surge: 'Surge', loon: 'Loon',
    surfboard: 'Surfboard', 'shadowrocket-profile': 'Shadowrocket', mixed: 'v2rayNG',
  }
  const selected = target === 'adaptive'
    ? links.filter(item => item.target !== 'clash')
    : links.filter(item => item.target === (target === 'xray' ? 'mixed' : target))
  return selected.flatMap(item => {
    if (item.target === 'mixed' && !android) return []
    const href = clientLaunchUrl(item.target, item.url, name)
    return href ? [{ label: labels[item.target] ?? item.target, href }] : []
  })
}
