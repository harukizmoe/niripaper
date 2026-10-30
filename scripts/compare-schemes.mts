// Compare our schemes against `noctalia theme`, so agreement with the desktop
// palette is measured rather than assumed.
//
//   node --experimental-strip-types scripts/compare-schemes.mts <image> [image...]
//
// Both sides start from the same seed — the `source_color` noctalia reports
// for `m3-tonal-spot` — so this checks the scheme step alone. (Seed extraction
// is the Rust side's `extract_colors`; it uses the same library's quantiser
// and Score.) Reports, per scheme and image, the hue/chroma/lightness distance
// of `primary`. It is a report, not a gate.
import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { register } from 'node:module'

// material-color-utilities 0.4.0 ships ESM with extensionless relative imports,
// which bundlers accept and Node's resolver does not. Add the `.js` here
// rather than patch the package.
register(
  'data:text/javascript,' +
    encodeURIComponent(`
      export async function resolve(spec, ctx, next) {
        try { return await next(spec, ctx) } catch (e) {
          if (spec.startsWith('.') && !spec.endsWith('.js')) return next(spec + '.js', ctx)
          throw e
        }
      }`),
)

const { buildRoles, SCHEME_NAMES } = await import('../app/ui/src/theme/color.ts')

function oklch(hex: string): { L: number; C: number; h: number } {
  const n = parseInt(hex.replace('#', ''), 16)
  const lin = (c: number) => {
    const v = c / 255
    return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4)
  }
  const R = lin((n >> 16) & 255)
  const G = lin((n >> 8) & 255)
  const B = lin(n & 255)
  const l = Math.cbrt(0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B)
  const m = Math.cbrt(0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B)
  const s = Math.cbrt(0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B)
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s
  const a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s
  const bb = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s
  return { L, C: Math.hypot(a, bb), h: ((Math.atan2(bb, a) * 180) / Math.PI + 360) % 360 }
}

function hueDelta(a: number, b: number): number {
  const d = Math.abs(a - b) % 360
  return d > 180 ? 360 - d : d
}

function noctalia(img: string, scheme: string): { primary: string; source_color: string } | null {
  try {
    return JSON.parse(
      execFileSync('noctalia', ['theme', img, '--scheme', scheme, '--dark'], {
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'ignore'],
      }),
    )
  } catch {
    return null
  }
}

const images = process.argv.slice(2).filter((p) => existsSync(p))
if (!images.length) {
  console.error('usage: node --experimental-strip-types scripts/compare-schemes.mts <image>...')
  process.exit(2)
}

console.log('scheme             image                            ours     noctalia  hueΔ  chromaΔ  LΔ')
for (const img of images) {
  const seed = noctalia(img, 'm3-tonal-spot')?.source_color
  const file = img.split('/').pop()!.slice(0, 30).padEnd(32)
  if (!seed) {
    console.log(`(noctalia unavailable for ${file})`)
    continue
  }
  for (const name of SCHEME_NAMES) {
    const ours = buildRoles(seed, true, name).primary
    const ref = noctalia(img, name)
    if (!ref) {
      console.log(`${name.padEnd(18)} ${file} ${ours}  (no such scheme in noctalia)`)
      continue
    }
    const a = oklch(ref.primary)
    const b = oklch(ours)
    console.log(
      `${name.padEnd(18)} ${file} ${ours}  ${ref.primary}  ${hueDelta(a.h, b.h).toFixed(0).padStart(4)}  ${Math.abs(a.C - b.C).toFixed(3).padStart(7)}  ${Math.abs(a.L - b.L).toFixed(3)}`,
    )
  }
}
