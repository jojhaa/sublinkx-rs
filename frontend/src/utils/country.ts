const chineseRegionNames = new Intl.DisplayNames(['zh-CN'], { type: 'region' })

export function formatCountryName(countryCode: string | null | undefined) {
  const normalized = countryCode?.trim().toUpperCase()
  if (!normalized || !/^[A-Z]{2}$/.test(normalized)) return null

  const localized = chineseRegionNames.of(normalized)
  return localized && localized !== normalized ? localized : normalized
}
