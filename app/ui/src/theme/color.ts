// ============================================================================
// Dynamic colour: wallpaper seed → Material 3 colour roles.
//
// Everything here is Google's own `material-color-utilities`, the library
// Android and every other Material You implementation use. The seed itself is
// picked on the Rust side with the same library's quantiser + `Score`
// (`extract_colors` in app/src/commands.rs), so a wallpaper produces the theme
// any Material You implementation would derive from it.
//
// The output is the standard role set under its standard snake_case names
// (`primary`, `surface_container_low`, …). The same map is what the app paints
// with, what it exports to `palette.toml`, and what it pushes to the desktop,
// so there is no second derivation anywhere that could disagree with the
// window.
// ============================================================================

import {
  argbFromHex,
  hexFromArgb,
  Hct,
  MaterialDynamicColors,
  SchemeContent,
  SchemeExpressive,
  SchemeFidelity,
  SchemeFruitSalad,
  SchemeMonochrome,
  SchemeNeutral,
  SchemeRainbow,
  SchemeTonalSpot,
  SchemeVibrant,
  type DynamicColor,
  type DynamicScheme,
} from '@material/material-color-utilities'

/// Used when no wallpaper colour is known yet (first paint, empty library).
export const THEME_SEED_FALLBACK = '#6750a4'

// The ids are noctalia's for the five schemes it takes from this library, and
// its output for those matches ours exactly (scripts/compare-schemes.mts).
// noctalia's other schemes — `faithful`, `vibrant`, `soft`, `muted`,
// `dysfunctional` — are its own algorithms, not this library's; the library's
// Fidelity/Vibrant/Expressive/Neutral get `m3-` ids so they never share a name
// with a noctalia scheme that would produce different colours.
export type SchemeName =
  | 'm3-tonal-spot'
  | 'm3-content'
  | 'm3-fidelity'
  | 'm3-vibrant'
  | 'm3-expressive'
  | 'm3-fruit-salad'
  | 'm3-rainbow'
  | 'm3-neutral'
  | 'm3-monochrome'

type SchemeCtor = new (source: Hct, isDark: boolean, contrastLevel: number) => DynamicScheme

const SCHEMES: Record<SchemeName, SchemeCtor> = {
  'm3-tonal-spot': SchemeTonalSpot,
  'm3-content': SchemeContent,
  'm3-fidelity': SchemeFidelity,
  'm3-vibrant': SchemeVibrant,
  'm3-expressive': SchemeExpressive,
  'm3-fruit-salad': SchemeFruitSalad,
  'm3-rainbow': SchemeRainbow,
  'm3-neutral': SchemeNeutral,
  'm3-monochrome': SchemeMonochrome,
}

export const SCHEME_LABELS: Record<SchemeName, string> = {
  'm3-tonal-spot': '标准',
  'm3-content': '贴近内容',
  'm3-fidelity': '忠于原色',
  'm3-vibrant': '鲜艳',
  'm3-expressive': '表现力',
  'm3-fruit-salad': '跳色',
  'm3-rainbow': '多彩',
  'm3-neutral': '柔和',
  'm3-monochrome': '单色',
}

export const SCHEME_NAMES = Object.keys(SCHEMES) as SchemeName[]

export const DEFAULT_SCHEME: SchemeName = 'm3-tonal-spot'

export function isSchemeName(value: string): value is SchemeName {
  return value in SCHEMES
}

/// The standard role set, by the library's method name. Palette key colours
/// are left out: they are inputs to the scheme, not colours to paint with.
const ROLE_METHODS = [
  'background', 'onBackground',
  'surface', 'surfaceDim', 'surfaceBright',
  'surfaceContainerLowest', 'surfaceContainerLow', 'surfaceContainer',
  'surfaceContainerHigh', 'surfaceContainerHighest',
  'onSurface', 'surfaceVariant', 'onSurfaceVariant',
  'inverseSurface', 'inverseOnSurface',
  'outline', 'outlineVariant', 'shadow', 'scrim', 'surfaceTint',
  'primary', 'onPrimary', 'primaryContainer', 'onPrimaryContainer', 'inversePrimary',
  'secondary', 'onSecondary', 'secondaryContainer', 'onSecondaryContainer',
  'tertiary', 'onTertiary', 'tertiaryContainer', 'onTertiaryContainer',
  'error', 'onError', 'errorContainer', 'onErrorContainer',
  'primaryFixed', 'primaryFixedDim', 'onPrimaryFixed', 'onPrimaryFixedVariant',
  'secondaryFixed', 'secondaryFixedDim', 'onSecondaryFixed', 'onSecondaryFixedVariant',
  'tertiaryFixed', 'tertiaryFixedDim', 'onTertiaryFixed', 'onTertiaryFixedVariant',
] as const

type RoleMethod = (typeof ROLE_METHODS)[number]

const snake = (s: string) => s.replace(/[A-Z]/g, (c) => '_' + c.toLowerCase())

/// Role name (snake_case, as M3 and the exported palette spell it) → `#rrggbb`.
export type Roles = Record<string, string>

const dynamicColors = new MaterialDynamicColors()

export function buildRoles(
  seedHex: string,
  dark: boolean,
  scheme: SchemeName = DEFAULT_SCHEME,
): Roles {
  const Ctor = SCHEMES[scheme] ?? SCHEMES[DEFAULT_SCHEME]
  const s = new Ctor(Hct.fromInt(argbFromHex(seedHex)), dark, 0)
  const roles: Roles = {}
  for (const m of ROLE_METHODS) {
    const color = (dynamicColors[m as RoleMethod] as () => DynamicColor).call(dynamicColors)
    roles[snake(m)] = hexFromArgb(color.getArgb(s))
  }
  return roles
}

/// The seed for a wallpaper: the first of its ranked source colours. Ranking
/// is the library's `Score`, done when the library was scanned, so there is
/// nothing left to choose here.
export function pickSeedColor(colors?: string[]): string {
  return colors?.find(Boolean) ?? THEME_SEED_FALLBACK
}

/// Roles from an external Material 3 palette (the read side of the Noctalia
/// bridge). It uses the same key names we export. Roles it does not carry are
/// generated from its own `primary`, so the window never paints with a gap.
export function rolesFromPalette(p: Record<string, string>): Roles | null {
  const primary = p['primary']
  const surface = p['surface']
  if (!primary || !surface) return null
  const dark = Hct.fromInt(argbFromHex(surface)).tone < 50
  return { ...buildRoles(primary, dark), ...p }
}

/// Paint `roles` onto the document as `--md-sys-color-*`, the token names the
/// M3 spec uses, for the few places styled outside MUI.
export function applyRoleVars(roles: Roles): void {
  const root = document.documentElement.style
  for (const [k, v] of Object.entries(roles)) {
    root.setProperty(`--md-sys-color-${k.replace(/_/g, '-')}`, v)
  }
}

export function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const n = argbFromHex(hex)
  return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 }
}
