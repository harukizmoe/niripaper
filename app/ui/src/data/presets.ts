export interface FilterValues {
  b: number;
  c: number;
  s: number;
  h: number;
  v: number;
  g: number;
}

export type PresetKey =
  | "none"
  | "vivid"
  | "film"
  | "mono"
  | "cyber"
  | "warm"
  | "cool"
  | "invert";

export const PRESETS: Record<PresetKey, FilterValues & { label: string }> = {
  none: { b: 100, c: 100, s: 100, h: 0, v: 0, g: 0, label: "原画 (None · 无后期调色)" },
  vivid: { b: 105, c: 120, s: 145, h: 0, v: 15, g: 0, label: "鲜艳 (Vivid · 增强对比高饱和)" },
  film: { b: 95, c: 110, s: 80, h: 10, v: 45, g: 45, label: "复古胶片 (Film · 胶片颗粒与暗角)" },
  mono: { b: 100, c: 130, s: 0, h: 0, v: 30, g: 25, label: "黑白单色 (Mono · 高反差黑白)" },
  cyber: { b: 110, c: 135, s: 170, h: 290, v: 35, g: 0, label: "赛博霓虹 (Cyber · 紫粉色相偏转)" },
  warm: { b: 105, c: 105, s: 125, h: 25, v: 20, g: 10, label: "暖调夕阳 (Warm · 温暖金暮)" },
  cool: { b: 100, c: 115, s: 110, h: 190, v: 30, g: 15, label: "冷冽电影 (Cool · 青蓝冷调)" },
  invert: { b: 100, c: 100, s: 100, h: 180, v: 0, g: 0, label: "负片反相 (Invert · 颠倒色彩)" },
};
