// Compare our scheme parameters against `noctalia theme`, so the approximations
// in theme/schemes.ts are checked rather than assumed.
//
//   node --experimental-strip-types scripts/compare-schemes.mts <image> [image...]
//
// Reports, per scheme and per image, the hue/chroma/lightness distance of
// `primary` between the two implementations. It is a report, not a gate: the
// goal is "same character", and a scheme whose error is large should be
// explained in docs/SPEC.md rather than silently tolerated.
import { execFileSync } from 'node:child_process'
import { readFileSync, existsSync } from 'node:fs'
import { SCHEMES, SCHEME_NAMES, type SchemeName } from '../app/ui/src/theme/schemes.ts'

type Rgb = [number, number, number]

function hexToRgb(hex: string): Rgb {
  const h = hex.replace('#', '')
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ]
}

const lin = (c: number) => {
  const v = c / 255
  return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4)
}

function oklch(hex: string): { L: number; C: number; h: number } {
  const [r, g, b] = hexToRgb(hex)
  const R = lin(r)
  const G = lin(g)
  const B = lin(b)
  const l = Math.cbrt(0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B)
  const m = Math.cbrt(0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B)
  const s = Math.cbrt(0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B)
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s
  const a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s
  const bb = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s
  return {
    L,
    C: Math.hypot(a, bb),
    h: ((Math.atan2(bb, a) * 180) / Math.PI + 360) % 360,
  }
}

function hueDelta(a: number, b: number): number {
  const d = Math.abs(a - b) % 360
  return d > 180 ? 360 - d : d
}

/** Our primary for a seed, mirroring buildM3Scheme's arithmetic. */
function ourPrimary(seedHex: string, scheme: SchemeName, dark: boolean): string {
  const p = SCHEMES[scheme]
  const seed = oklch(seedHex)
  const hue = (seed.h + p.hueShift + 360) % 360
  const [cMin, cMax] = p.chromaClamp
  const C = p.monochrome
    ? 0
    : Math.min(Math.max(seed.C * p.chromaScale, cMin), cMax)
  // Dark primary sits around L 0.80, light around L 0.46 (before any shift).
  const baseL = dark ? 0.795 : 0.455
  const L = Math.min(0.99, Math.max(0.15, baseL + p.lightnessShift))
  return oklchToHex(L, C, hue)
}

function oklchToHex(L: number, C: number, hDeg: number): string {
  const h = (hDeg * Math.PI) / 180
  let c = C
  for (let i = 0; i < 32; i++) {
    const a = c * Math.cos(h)
    const b = c * Math.sin(h)
    const l_ = Math.pow(L + 0.3963377774 * a + 0.2158037573 * b, 3)
    const m_ = Math.pow(L - 0.1055613458 * a - 0.0638541728 * b, 3)
    const s_ = Math.pow(L - 0.0894841775 * a - 1.291485548 * b, 3)
    const toS = (v: number) =>
      (v <= 0.0031308 ? v * 12.92 : 1.055 * Math.pow(v, 1 / 2.4) - 0.055) * 255
    const rgb = [
      toS(4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_),
      toS(-1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_),
      toS(-0.0041960863 * l_ - 0.7034186147 * m_ + 1.707614701 * s_),
    ]
    if (rgb.every((v) => v >= -0.6 && v <= 255.6)) {
      return (
        '#' +
        rgb.map((v) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, '0')).join('')
      )
    }
    c *= 0.93
  }
  return '#808080'
}

// A common seed for every scheme on the same image: the first non-self
// `source_color` reported. Schemes like `dysfunctional` replace it, so it
// cannot be read per-scheme.
const SEED_OF: Record<string, string> = {}

const images = process.argv.slice(2).filter((p) => existsSync(p))
if (!images.length) {
  console.error('usage: node --experimental-strip-types scripts/compare-schemes.mts <image>...')
  process.exit(2)
}

console.log('scheme              image                          hueΔ   chromaΔ   LΔ    note')
for (const img of images) {
  for (const name of SCHEME_NAMES) {
    let ref: { primary: string; source_color: string }
    try {
      ref = JSON.parse(
        execFileSync('noctalia', ['theme', img, '--scheme', name, '--dark'], {
          encoding: 'utf8',
        })
      )
      // The wallpaper's own colour is what `m3-tonal-spot` reports as its
      // source; it is stable, unlike the per-scheme values.
      if (!SEED_OF[img] || name === 'm3-tonal-spot') SEED_OF[img] = ref.source_color
    } catch {
      console.log(`${name}  ${img}  (noctalia unavailable, skipped)`)
      continue
    }
    // `source_color` is NOT a constant across schemes: `dysfunctional` reports
    // the transformed colour here (source == primary), so feeding it back in
    // measures the wrong thing entirely. Compare against the seed we would
    // actually use — the wallpaper's own colour — and skip the schemes that
    // self-reference.
    if (ref.source_color.toLowerCase() === ref.primary.toLowerCase()) {
      console.log(
        `${name.padEnd(18)} ${img.split('/').pop()!.slice(0, 28).padEnd(30)}    -        -      -   source==primary, skipped`
      )
      continue
    }
    const seedHex = SEED_OF[img] ?? ref.source_color
    const a = oklch(ref.primary)
    const b = oklch(ourPrimary(seedHex, name, true))
    const dh = hueDelta(a.h, b.h)
    const dc = Math.abs(a.C - b.C)
    const dl = Math.abs(a.L - b.L)
    const note =
      dh > 30 ? 'hue far off' : dc > 0.06 ? 'chroma far off' : dl > 0.12 ? 'lightness far off' : 'ok'
    console.log(
      `${name.padEnd(18)} ${img.split('/').pop()!.slice(0, 28).padEnd(30)} ${dh.toFixed(0).padStart(4)}   ${dc.toFixed(3).padStart(7)}  ${dl.toFixed(3)}   ${note}`
    )
  }
}
