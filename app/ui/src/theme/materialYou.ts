// ============================================================================
// Material You 动态配色引擎 (OKLCH 近似)
// 移植自 tasks/prototype-ui.html，完全保证色彩与主题流转效果与原型 100% 一致。
// ============================================================================

import { SCHEMES, DEFAULT_SCHEME, type SchemeName } from "./schemes";

export const THEME_SEED_FALLBACK = "#624b89";

export interface RGB {
  r: number;
  g: number;
  b: number;
}

export function hexToRgb(hex: string): RGB {
  const h = String(hex).replace("#", "");
  const full = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
  const n = parseInt(full, 16);
  return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
}

function srgbToLinear(c: number): number {
  c /= 255;
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

function linearToSrgb(c: number): number {
  return c <= 0.0031308 ? c * 12.92 : 1.055 * Math.pow(c, 1 / 2.4) - 0.055;
}

function rgbToOklab(r: number, g: number, b: number) {
  const R = srgbToLinear(r);
  const G = srgbToLinear(g);
  const B = srgbToLinear(b);
  const l = Math.cbrt(0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B);
  const m = Math.cbrt(0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B);
  const s = Math.cbrt(0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B);
  return {
    L: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    a: 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    b: 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  };
}

function oklabToRgb(L: number, a: number, b: number): RGB {
  const l = Math.pow(L + 0.3963377774 * a + 0.2158037573 * b, 3);
  const m = Math.pow(L - 0.1055613458 * a - 0.0638541728 * b, 3);
  const s = Math.pow(L - 0.0894841775 * a - 1.291485548 * b, 3);
  return {
    r: linearToSrgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s) * 255,
    g: linearToSrgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s) * 255,
    b: linearToSrgb(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s) * 255,
  };
}

function rgbToHex(rgb: RGB): string {
  const f = (v: number) =>
    Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, "0");
  return "#" + f(rgb.r) + f(rgb.g) + f(rgb.b);
}

export function oklchToHex(L: number, C: number, hDeg: number): string {
  const h = (hDeg * Math.PI) / 180;
  let c = C;
  for (let i = 0; i < 32; i++) {
    const rgb = oklabToRgb(L, c * Math.cos(h), c * Math.sin(h));
    if (
      rgb.r >= -0.6 &&
      rgb.r <= 255.6 &&
      rgb.g >= -0.6 &&
      rgb.g <= 255.6 &&
      rgb.b >= -0.6 &&
      rgb.b <= 255.6
    ) {
      return rgbToHex(rgb);
    }
    c *= 0.93;
  }
  return rgbToHex(oklabToRgb(L, 0, 0));
}

export function seedToOklch(hex: string) {
  const { r, g, b } = hexToRgb(hex);
  const lab = rgbToOklab(r, g, b);
  return {
    L: lab.L,
    C: Math.hypot(lab.a, lab.b),
    h: ((Math.atan2(lab.b, lab.a) * 180) / Math.PI + 360) % 360,
  };
}

export function buildM3Scheme(
  seedHex: string,
  dark: boolean,
  scheme: SchemeName = DEFAULT_SCHEME
): Record<string, string> {
  const seed = seedToOklch(seedHex);
  const p = SCHEMES[scheme] ?? SCHEMES[DEFAULT_SCHEME];

  // A scheme reshapes the seed; it does not replace the neutral ramp, which
  // the measurements show is nearly identical across every scheme.
  const hue = (seed.h + p.hueShift + 360) % 360;
  const [cMin, cMax] = p.chromaClamp;
  const primaryC = p.monochrome
    ? 0
    : Math.min(Math.max(seed.C * p.chromaScale, cMin), cMax);
  const tertiaryHue = (hue + 62) % 360;
  const tertiaryC = p.monochrome
    ? 0
    : Math.min(Math.max(seed.C * 0.85 * p.chromaScale, cMin * 0.9), cMax * 0.94);
  const neutralC = p.monochrome
    ? 0
    : Math.min(seed.C * 0.1, 0.022) + 0.006;

  // `content`/`dysfunctional` are lighter overall; the shift is applied to the
  // primary only, so the surface ladder the layout depends on stays put.
  // `content` saturates near L 0.83 for ordinary seeds and climbs further only
  // once the seed itself is very bright. Fitted against four wallpapers
  // (0.388→0.828, 0.547→0.827, 0.569→0.838, 0.878→0.984), max error 0.015.
  const contentL = Math.min(0.99, 0.83 + Math.max(0, seed.L - 0.55) * 1.2);
  const primaryL = (base: number) =>
    p.lightnessFrom === 'boost'
      ? contentL
      : Math.min(0.99, Math.max(0.15, base + p.lightnessShift));

  const s: Record<string, string> = {};
  if (dark) {
    s["--m3-canvas"]            = oklchToHex(0.155, neutralC, hue);
    s["--m3-surface"]           = oklchToHex(0.215, neutralC, hue);
    s["--m3-surface-subtle"]    = oklchToHex(0.275, neutralC, hue);
    s["--m3-surface-container"] = oklchToHex(0.335, neutralC * 1.6, hue);
    s["--m3-surface-high"]      = oklchToHex(0.395, neutralC * 1.6, hue);
    s["--m3-bg"]                = oklchToHex(0.115, neutralC, hue);

    s["--m3-fg"]                = oklchToHex(0.945, neutralC * 0.5, hue);
    s["--m3-fg-muted"]          = oklchToHex(0.735, neutralC * 0.8, hue);
    s["--m3-outline"]           = oklchToHex(0.545, neutralC, hue);

    s["--m3-accent-purple"]     = oklchToHex(primaryL(0.795), primaryC, hue);
    s["--m3-accent-lavender"]   = oklchToHex(0.375, primaryC * 0.75, hue);
    s["--m3-accent-lime"]       = oklchToHex(primaryL(0.885), tertiaryC, tertiaryHue);
    s["--m3-accent-mint"]       = oklchToHex(primaryL(0.815), tertiaryC * 0.5, (hue + 150) % 360);
    s["--m3-accent-coral"]      = oklchToHex(primaryL(0.8), 0.115, (hue + 205) % 360);
    s["--m3-rail-bg"]           = oklchToHex(0.145, neutralC * 2.2, hue);
    s["--m3-rail-text"]         = oklchToHex(0.76, neutralC, hue);
    s["--m3-rail-text-active"]  = oklchToHex(0.205, primaryC, hue);
  } else {
    s["--m3-canvas"]            = oklchToHex(0.965, neutralC, hue);
    s["--m3-surface"]           = oklchToHex(0.995, neutralC * 0.4, hue);
    s["--m3-surface-subtle"]    = oklchToHex(0.93, neutralC, hue);
    s["--m3-surface-container"] = oklchToHex(0.895, neutralC * 1.8, hue);
    s["--m3-surface-high"]      = oklchToHex(1.0, 0, hue);
    s["--m3-bg"]                = oklchToHex(0.935, neutralC, hue);

    s["--m3-fg"]                = oklchToHex(0.245, neutralC * 1.2, hue);
    s["--m3-fg-muted"]          = oklchToHex(0.52, neutralC * 1.4, hue);
    s["--m3-outline"]           = oklchToHex(0.79, neutralC, hue);

    s["--m3-accent-purple"]     = oklchToHex(primaryL(0.455), primaryC, hue);
    s["--m3-accent-lavender"]   = oklchToHex(0.89, primaryC * 0.55, hue);
    s["--m3-accent-lime"]       = oklchToHex(primaryL(0.88), tertiaryC, tertiaryHue);
    s["--m3-accent-mint"]       = oklchToHex(primaryL(0.88), tertiaryC * 0.45, (hue + 150) % 360);
    s["--m3-accent-coral"]      = oklchToHex(primaryL(0.76), 0.105, (hue + 205) % 360);
    s["--m3-rail-bg"]           = oklchToHex(0.235, neutralC * 2.4, hue);
    s["--m3-rail-text"]         = oklchToHex(0.8, neutralC, hue);
    s["--m3-rail-text-active"]  = oklchToHex(0.205, primaryC, hue);
  }
  guardContrast(s);
  return s;
}

// ---------------------------------------------------------------------------
// Contrast guard
//
// The ten schemes come from measurements, not from an official formula, so a
// scheme can put `primary` on the same lightness as the surface it sits on —
// `monochrome` and `dysfunctional` are the obvious offenders. Rather than let
// a chosen style produce unreadable buttons, every foreground/background pair
// the UI actually relies on is checked, and the foreground is pushed away from
// the background until it clears WCAG AA (4.5:1). This is a safety net, not
// part of the colour design: on a normal scheme it changes nothing.
// ---------------------------------------------------------------------------

const AA = 4.5

function relLuminance(hex: string): number {
  const { r, g, b } = hexToRgb(hex)
  const ch = (v: number) => {
    const c = v / 255
    return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4)
  }
  return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)
}

export function contrastRatio(a: string, b: string): number {
  const la = relLuminance(a)
  const lb = relLuminance(b)
  const hi = Math.max(la, lb)
  const lo = Math.min(la, lb)
  return (hi + 0.05) / (lo + 0.05)
}

/// Nudge `fg` along the lightness axis until it clears AA against `bg`.
/// Direction is chosen once, from which side has more room.
function ensureContrast(fg: string, bg: string): string {
  if (contrastRatio(fg, bg) >= AA) return fg
  const { L, C, h } = seedToOklch(fg)
  const bgL = seedToOklch(bg).L
  const goLighter = bgL < 0.5

  // Move along lightness only, and keep the chroma. The earlier version faded
  // chroma in step with the distance travelled, so a pale accent dragged far
  // enough to clear the background ended up grey — `m3-content`'s whole point
  // is a tinted pale accent, and it came out as #686774 instead of #dcd8ff.
  // Gamut clipping in `oklchToHex` already reduces chroma where sRGB cannot
  // hold it, which is the only fading that is actually needed.
  for (let step = 1; step <= 50; step++) {
    const cur = L + (goLighter ? step : -step) * 0.02
    if (cur > 0.995 || cur < 0.06) break
    const candidate = oklchToHex(cur, C, h)
    if (contrastRatio(candidate, bg) >= AA) return candidate
  }
  // Nothing with this hue clears the bar; fall back to the neutral extreme.
  return goLighter ? '#ffffff' : '#000000'
}

/// Every pair the layout depends on, corrected in place.
function guardContrast(s: Record<string, string>): void {
  const pairs: [string, string][] = [
    ['--m3-fg', '--m3-surface'],
    ['--m3-fg', '--m3-canvas'],
    ['--m3-fg', '--m3-bg'],
    ['--m3-fg', '--m3-surface-subtle'],
    ['--m3-fg', '--m3-surface-container'],
    ['--m3-fg', '--m3-surface-high'],
    ['--m3-fg-muted', '--m3-surface'],
    ['--m3-fg-muted', '--m3-canvas'],
    ['--m3-rail-text', '--m3-rail-bg'],
    // The rail's active pill: rail-text-active sits on accent-lime.
    ['--m3-rail-text-active', '--m3-accent-lime'],
    ['--m3-accent-purple', '--m3-surface'],
    ['--m3-accent-purple', '--m3-bg'],
  ]
  for (const [f, b] of pairs) {
    if (s[f] && s[b]) s[f] = ensureContrast(s[f], s[b])
  }
}

export function pickSeedColor(colors?: string[]): string {
  const list = (colors || []).filter(Boolean);
  if (!list.length) return THEME_SEED_FALLBACK;
  let best = list[0];
  let bestC = -1;
  list.forEach((c) => {
    const o = seedToOklch(c);
    if (o.C > bestC) {
      bestC = o.C;
      best = c;
    }
  });
  return best;
}

/// Map the internal `--m3-*` tokens onto standard Material 3 names, which is
/// what the exported palette file uses. Anything without a close M3 analogue is
/// dropped rather than guessed at.
const M3_NAMES: Record<string, string> = {
  '--m3-surface': 'surface',
  '--m3-surface-subtle': 'surface_container',
  '--m3-surface-container': 'surface_container_high',
  '--m3-surface-high': 'surface_container_highest',
  '--m3-bg': 'background',
  '--m3-canvas': 'surface_dim',
  '--m3-fg': 'on_surface',
  '--m3-fg-muted': 'on_surface_variant',
  '--m3-outline': 'outline',
  '--m3-accent-purple': 'primary',
  '--m3-accent-lime': 'tertiary',
  '--m3-accent-mint': 'tertiary_container',
  '--m3-accent-coral': 'error',
  // `--m3-rail-bg` is the dark navigation rail, not an M3 surface step: mapping
  // it to `surface_container_low` exported a near-black value in light mode.
  // `--m3-canvas` already covers `surface_dim`, so the rail has no honest M3
  // equivalent and is simply not exported.
}

export function toM3Export(scheme: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [internal, m3] of Object.entries(M3_NAMES)) {
    const value = scheme[internal];
    if (value) out[m3] = value;
  }
  return out;
}

export function applyThemeTokens(
  seedHex: string,
  dark: boolean,
  schemeName: SchemeName = DEFAULT_SCHEME
) {
  const scheme = buildM3Scheme(seedHex, dark, schemeName);
  const root = document.documentElement.style;
  Object.keys(scheme).forEach((k) => root.setProperty(k, scheme[k]));
}
