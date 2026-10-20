// ============================================================================
// 壁纸取色方案 (schemes)
//
// 每个方案是一组"如何从源色派生成主色"的参数：色相偏移、色度缩放、亮度
// 偏移。它们由 `noctalia theme --scheme <name>` 的实测输出反推，用的是四张
// 差异很大的壁纸交叉验证（见 scripts/compare-schemes.mts），不是官方公式。
//
// 只保留六个方案。Noctalia 还有 vibrant / soft / dysfunctional / muted 四个，
// 实测发现它们**会从图像里换一个不同的源色**（同一张图上 tonal-spot 取
// #5a748b，muted 取 #9f592d，色相相差 161°），属于"重新挑色"而非"对源色
// 变形"。不复刻它的调色板提取就拿不到正确结果，与其给出名不副实的选项，
// 不如不提供 —— 这六个方案在四张图上色相误差都在 11° 以内。
//
// 同时实测确认：surface 的亮度阶梯在各方案间几乎重合（L 0.16→0.35），所以
// 方案的差异只在色相/色度，界面骨架不随之变化。
// ============================================================================

export type SchemeName =
  | 'm3-tonal-spot'
  | 'm3-content'
  | 'm3-fruit-salad'
  | 'm3-rainbow'
  | 'm3-monochrome'
  | 'faithful'

export interface SchemeParams {
  /** 相对源色相的偏移（度）。 */
  hueShift: number
  /** 相对源色度的倍数。 */
  chromaScale: number
  /** 输出色度区间。 */
  chromaClamp: [number, number]
  /** 主色亮度整体偏移（正值更亮）。 */
  lightnessShift: number
  /**
   * `boost`: the shift grows with the seed's own lightness, for schemes whose
   * primary rides just under the source (m3-content).
   */
  lightnessFrom?: 'boost'
  /** 完全去色。 */
  monochrome: boolean
}

export const SCHEMES: Record<SchemeName, SchemeParams> = {
  'm3-tonal-spot': {
    hueShift: 0,
    chromaScale: 1,
    chromaClamp: [0.088, 0.155],
    lightnessShift: 0,
    monochrome: false,
  },
  'm3-content': {
    // Measured: primary sits near L 0.83 across images, except on a very
    // bright seed (L 0.88) where it reaches 0.98 — so the shift grows with the
    // source instead of being a constant. Chroma tracks the seed at roughly
    // 0.7-1.2x; the earlier 0.10 ceiling crushed a vivid seed into grey.
    hueShift: -3,
    chromaScale: 0.75,
    chromaClamp: [0.04, 0.16],
    lightnessShift: 0.0,
    monochrome: false,
    lightnessFrom: 'boost',
  },
  'm3-fruit-salad': {
    hueShift: -48,
    chromaScale: 1.4,
    chromaClamp: [0.09, 0.16],
    lightnessShift: 0.02,
    monochrome: false,
  },
  'm3-rainbow': {
    hueShift: 1,
    chromaScale: 1.4,
    chromaClamp: [0.1, 0.18],
    lightnessShift: 0,
    monochrome: false,
  },
  'm3-monochrome': {
    hueShift: 0,
    chromaScale: 0,
    chromaClamp: [0, 0],
    // The reference tool returns pure #ffffff here: monochrome takes the top
    // of the ramp rather than the accent lightness the other schemes use.
    lightnessShift: 0.205,
    monochrome: true,
  },

  faithful: {
    // Measured: chroma lands at 0.11–0.17 even when the seed is nearly grey
    // (C 0.03), so it is a floor, not a multiple of the seed.
    hueShift: 3,
    chromaScale: 1.0,
    chromaClamp: [0.11, 0.19],
    lightnessShift: 0.0,
    monochrome: false,
  },



}

export const SCHEME_LABELS: Record<SchemeName, string> = {
  'm3-tonal-spot': 'M3 Tonal Spot（标准）',
  'm3-content': 'M3 Content（明亮低彩）',
  'm3-fruit-salad': 'M3 Fruit Salad（跳色）',
  'm3-rainbow': 'M3 Rainbow（高彩）',
  'm3-monochrome': 'M3 Monochrome（去色）',
  faithful: 'Faithful（贴近原图）',
}

export const SCHEME_NAMES = Object.keys(SCHEMES) as SchemeName[]

export const DEFAULT_SCHEME: SchemeName = 'm3-tonal-spot'

export function isSchemeName(value: string): value is SchemeName {
  return value in SCHEMES
}
