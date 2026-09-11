export function shadowrocketProfileInstallUrl(profileUrl: string) {
  const parsed = new URL(profileUrl)
  if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') {
    throw new Error('Shadowrocket profile URL must use HTTP or HTTPS')
  }

  return `shadowrocket://config/add/${parsed.toString()}`
}
