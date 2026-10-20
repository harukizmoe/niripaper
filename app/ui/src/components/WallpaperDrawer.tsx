import React, { useState, useEffect } from 'react'
import { convertFileSrc } from '@tauri-apps/api/core'
import type { WallpaperItem } from '../data/wallpapers'
import { PRESETS, type PresetKey, type FilterValues } from '../data/presets'

function toMediaSrc(pathOrUrl: string): string {
  if (!pathOrUrl) return ''
  if (
    pathOrUrl.startsWith('http://') ||
    pathOrUrl.startsWith('https://') ||
    pathOrUrl.startsWith('data:')
  ) {
    return pathOrUrl
  }
  try {
    return convertFileSrc(pathOrUrl)
  } catch {
    return pathOrUrl
  }
}
interface WallpaperDrawerProps {
  isOpen: boolean
  wallpaper: WallpaperItem | null
  source: 'wallhaven' | 'local'
  targetDisplay: string
  onClose: () => void
  onSetTargetDisplay: (target: string) => void
  onApply: (params: { fit: string; scale: number; filterPreset: string }) => void
  onSaveToLocal: (wallpaper: WallpaperItem) => void
  onAddToPlaylist: (wallpaper: WallpaperItem) => void
  onSearchTag: (tag: string) => void
}

export const WallpaperDrawer: React.FC<WallpaperDrawerProps> = ({
  isOpen,
  wallpaper,
  source,
  targetDisplay,
  onClose,
  onSetTargetDisplay,
  onApply,
  onSaveToLocal,
  onAddToPlaylist,
  onSearchTag,
}) => {
  const [fit, setFit] = useState('fill')
  const [scale, setScale] = useState(1.2)
  const [preset, setPreset] = useState<PresetKey>('none')
  const [filters, setFilters] = useState<FilterValues>(PRESETS.none)

  const [activeWp, setActiveWp] = useState<WallpaperItem | null>(wallpaper)

  useEffect(() => {
    if (wallpaper) {
      setActiveWp(wallpaper)
      setPreset('none')
      setFilters(PRESETS.none)
      setScale(1.2)
    }
  }, [wallpaper?.id])

  if (!activeWp) return null
  const displayWp = activeWp
  const handlePresetChange = (p: PresetKey) => {
    setPreset(p)
    setFilters(PRESETS[p])
  }

  const handleFilterChange = (key: keyof FilterValues, val: number) => {
    setFilters((prev) => ({ ...prev, [key]: val }))
  }

  const resetFilters = () => {
    setPreset('none')
    setFilters(PRESETS.none)
  }

  const calcTrackBg = (val: number, min: number, max: number) => {
    const pct = Math.max(0, Math.min(100, ((val - min) / (max - min)) * 100))
    return `linear-gradient(to right, var(--m3-accent-purple) 0%, var(--m3-accent-purple) ${pct}%, var(--m3-surface-subtle) ${pct}%, var(--m3-surface-subtle) 100%)`
  }

  const scalePct = ((scale - 1.0) / (1.5 - 1.0)) * 100
  const scaleTrackBg = `linear-gradient(to right, var(--m3-accent-purple) 0%, var(--m3-accent-purple) ${scalePct}%, var(--m3-surface-subtle) ${scalePct}%, var(--m3-surface-subtle) 100%)`

  const imgFilterStyle = {
    filter: `brightness(${filters.b}%) contrast(${filters.c}%) saturate(${filters.s}%) hue-rotate(${filters.h}deg)`,
  }

  return (
    <>
      {/* 遮罩 */}
      <div
        className={`m3-drawer-scrim ${isOpen ? 'show' : ''}`}
        onClick={onClose}
      />

      {/* 抽屉 */}
      <aside className={`m3-drawer ${isOpen ? 'open' : ''}`}>
        {/* 顶栏 */}
        <div className="drawer-header">
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <span style={{ fontSize: 15, fontWeight: 800 }}>壁纸设置与应用</span>
            <span
              className="wh-quick-tag"
              style={{
                background: 'var(--m3-surface-container)',
                color: 'var(--m3-accent-purple)',
                fontWeight: 800,
              }}
            >
              {source === 'wallhaven'
                ? `Wallhaven #${displayWp.id}`
                : '本地壁纸'}
            </span>
          </div>
          <button
            style={{
              border: 'none',
              background: 'var(--m3-surface-subtle)',
              width: 32,
              height: 32,
              borderRadius: '50%',
              cursor: 'pointer',
              color: 'var(--m3-fg)',
              fontSize: 14,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontWeight: 800,
            }}
            onClick={onClose}
          >
            ✕
          </button>
        </div>

        {/* 内容 */}
        <div className="drawer-body">
          {/* 大图预览 */}
          <div className="drawer-media-box">
            {displayWp.kind === 'video' ? (
              <video
                src={toMediaSrc(displayWp.path || displayWp.thumb)}
                controls
                autoPlay
                muted
                loop
                playsInline
                style={{
                  width: '100%',
                  height: '100%',
                  objectFit: 'cover',
                  display: 'block',
                }}
              />
            ) : (
              <img
                id="drawer-img"
                src={toMediaSrc(
                  displayWp.full_url || displayWp.path || displayWp.thumb
                )}
                alt={displayWp.name}
                style={imgFilterStyle}
              />
            )}
            {filters.v > 0 && (
              <div
                id="drawer-vignette-overlay"
                style={{
                  position: 'absolute',
                  inset: 0,
                  pointerEvents: 'none',
                  opacity: filters.v / 100,
                  background:
                    'radial-gradient(circle at center, transparent 40%, rgba(0,0,0,0.92) 100%)',
                }}
              />
            )}
            {filters.g > 0 && (
              <div
                id="drawer-grain-overlay"
                style={{
                  position: 'absolute',
                  inset: 0,
                  pointerEvents: 'none',
                  opacity: filters.g / 100,
                  backgroundImage:
                    'radial-gradient(rgba(255,255,255,0.4) 1px, transparent 0)',
                  backgroundSize: '3px 3px',
                }}
              />
            )}
            <span className="drawer-media-tag">
              {displayWp.res || '2560×1440'} · {displayWp.aspect || '16:9'}
            </span>
          </div>

          {/* 标题与信息 */}
          <div>
            <h3 style={{ fontSize: 16, fontWeight: 800, lineHeight: 1.3 }}>
              {displayWp.name}
            </h3>
            <div
              style={{
                fontSize: 12,
                fontWeight: 600,
                color: 'var(--m3-fg-muted)',
                marginTop: 4,
              }}
            >
              {source === 'wallhaven'
                ? `${displayWp.res} · ${displayWp.ratio} · ❤️ ${displayWp.favs} 收藏 · ${displayWp.category} · ${displayWp.purity}`
                : `${displayWp.res} · ${
                    displayWp.kind === 'video'
                      ? '动态视频壁纸'
                      : '静态图像壁纸'
                  }`}
            </div>
          </div>

          {/* Wallhaven 5 色主色调 */}
          {source === 'wallhaven' && displayWp.colors && (
            <div>
              <div
                style={{
                  fontSize: 11,
                  fontWeight: 700,
                  color: 'var(--m3-fg-muted)',
                  marginBottom: 8,
                  textTransform: 'uppercase',
                }}
              >
                Wallhaven 5 色主色调:
              </div>
              <div className="palette-swatches">
                {displayWp.colors.map((c, i) => (
                  <div
                    key={i}
                    className="swatch"
                    style={{ background: c }}
                    title={`按色值 ${c} 搜索`}
                    onClick={() => {
                      onClose()
                      onSearchTag(c)
                    }}
                  />
                ))}
              </div>
            </div>
          )}

          {/* 标签云 */}
          {source === 'wallhaven' && displayWp.tags && (
            <div>
              <div
                style={{
                  fontSize: 11,
                  fontWeight: 700,
                  color: 'var(--m3-fg-muted)',
                  marginBottom: 8,
                  textTransform: 'uppercase',
                }}
              >
                题材标签:
              </div>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
                {displayWp.tags.map((t, i) => (
                  <button
                    key={i}
                    className="wh-quick-tag"
                    onClick={() => {
                      onClose()
                      onSearchTag(t)
                    }}
                  >
                    #{t}
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* 本地路径 */}
          {source === 'local' && (
            <div>
              <div
                style={{
                  fontSize: 11,
                  fontWeight: 700,
                  color: 'var(--m3-fg-muted)',
                  marginBottom: 4,
                  textTransform: 'uppercase',
                }}
              >
                本地文件路径:
              </div>
              <div
                style={{
                  fontSize: 11,
                  fontFamily: 'var(--font-mono)',
                  color: 'var(--m3-fg)',
                  background: 'var(--m3-surface-subtle)',
                  padding: '8px 12px',
                  borderRadius: 'var(--m3-radius-sm)',
                  wordBreak: 'break-all',
                }}
              >
                {displayWp.path || `~/Pictures/Wallpapers/${displayWp.name}`}
              </div>
            </div>
          )}
          <hr
            style={{
              border: 'none',
              borderTop: '1px solid var(--m3-outline-variant)',
            }}
          />

          {/* 屏幕与渲染设置 */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
            <div
              style={{
                fontSize: 12,
                fontWeight: 800,
                color: 'var(--m3-fg)',
                display: 'flex',
                alignItems: 'center',
                gap: 6,
              }}
            >
              <span>⚙️ 屏幕与渲染参数设置</span>
            </div>

            {/* 目标显示器选择 */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              <span
                style={{
                  fontSize: 11,
                  fontWeight: 700,
                  color: 'var(--m3-fg-muted)',
                }}
              >
                目标显示器:
              </span>
              <div className="m3-seg-box" style={{ width: '100%' }}>
                <button
                  className={`m3-seg-btn ${
                    targetDisplay === 'DP-1' ? 'active' : ''
                  }`}
                  style={{ flex: 1 }}
                  onClick={() => onSetTargetDisplay('DP-1')}
                >
                  🖥️ DP-1 (主屏)
                </button>
                <button
                  className={`m3-seg-btn ${
                    targetDisplay === 'eDP-1' ? 'active' : ''
                  }`}
                  style={{ flex: 1 }}
                  onClick={() => onSetTargetDisplay('eDP-1')}
                >
                  💻 eDP-1
                </button>
                <button
                  className={`m3-seg-btn ${
                    targetDisplay === '*' ? 'active' : ''
                  }`}
                  style={{ flex: 1 }}
                  onClick={() => onSetTargetDisplay('*')}
                >
                  🌐 所有屏幕
                </button>
              </div>
            </div>

            {/* 填充方式与缩放 */}
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: '1.1fr 1fr',
                gap: 14,
              }}
            >
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                <span
                  style={{
                    fontSize: 11,
                    fontWeight: 700,
                    color: 'var(--m3-fg-muted)',
                  }}
                >
                  填充方式 (fit):
                </span>
                <div className="m3-select-wrapper">
                  <select
                    className="m3-select"
                    value={fit}
                    onChange={(e) => setFit(e.target.value)}
                  >
                    <option value="fill">fill (比例填满)</option>
                    <option value="fit">fit (保留加黑边)</option>
                    <option value="stretch">stretch (拉伸铺满)</option>
                    <option value="center">center (居中不缩放)</option>
                    <option value="tile">tile (平铺重复)</option>
                  </select>
                  <div className="m3-select-arrow">▼</div>
                </div>
              </div>

              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                <div
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    fontSize: 11,
                    fontWeight: 700,
                  }}
                >
                  <span style={{ color: 'var(--m3-fg-muted)' }}>
                    缩放 (scale):
                  </span>
                  <div className="m3-num-badge">
                    <input
                      type="number"
                      min="1.0"
                      max="2.0"
                      step="0.05"
                      value={scale.toFixed(2)}
                      onChange={(e) => {
                        const v = parseFloat(e.target.value)
                        if (!isNaN(v)) setScale(v)
                      }}
                    />
                    <span className="m3-num-badge-unit">x</span>
                  </div>
                </div>
                <input
                  type="range"
                  className="m3-slider"
                  min="1.0"
                  max="1.5"
                  step="0.05"
                  value={scale}
                  style={{ background: scaleTrackBg }}
                  onChange={(e) => setScale(parseFloat(e.target.value))}
                />
              </div>
            </div>
          </div>

          <hr
            style={{
              border: 'none',
              borderTop: '1px solid var(--m3-outline-variant)',
            }}
          />

          {/* 后期滤镜与色彩调优 */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
              }}
            >
              <div
                style={{
                  fontSize: 12,
                  fontWeight: 800,
                  color: 'var(--m3-fg)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                }}
              >
                <span>🎨 后期滤镜与色彩调优 (对标 WE)</span>
              </div>
              <button
                style={{
                  border: 'none',
                  background: 'transparent',
                  fontSize: 11,
                  fontWeight: 700,
                  color: 'var(--m3-accent-purple)',
                  cursor: 'pointer',
                }}
                onClick={resetFilters}
              >
                ↺ 重置滤镜
              </button>
            </div>

            {/* 预设下拉 */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              <span
                style={{
                  fontSize: 11,
                  fontWeight: 700,
                  color: 'var(--m3-fg-muted)',
                }}
              >
                选择滤镜预设库:
              </span>
              <div className="m3-select-wrapper">
                <select
                  className="m3-select"
                  value={preset}
                  onChange={(e) => handlePresetChange(e.target.value as PresetKey)}
                >
                  {(Object.keys(PRESETS) as PresetKey[]).map((k) => (
                    <option key={k} value={k}>
                      {PRESETS[k].label}
                    </option>
                  ))}
                </select>
                <div className="m3-select-arrow">▼</div>
              </div>
            </div>

            {/* 滑块清单 */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              {/* 亮度 */}
              <div className="m3-slider-tile">
                <div className="m3-tile-header">
                  <span className="m3-tile-label">☀️ 亮度 (Brightness)</span>
                  <div className="m3-num-badge">
                    <input
                      type="number"
                      min="50"
                      max="150"
                      value={filters.b}
                      onChange={(e) => handleFilterChange('b', parseFloat(e.target.value) || 100)}
                    />
                    <span className="m3-num-badge-unit">%</span>
                  </div>
                </div>
                <input
                  type="range"
                  className="m3-slider"
                  min="50"
                  max="150"
                  value={filters.b}
                  style={{ background: calcTrackBg(filters.b, 50, 150) }}
                  onChange={(e) => handleFilterChange('b', parseFloat(e.target.value))}
                />
              </div>

              {/* 对比度 */}
              <div className="m3-slider-tile">
                <div className="m3-tile-header">
                  <span className="m3-tile-label">◐ 对比度 (Contrast)</span>
                  <div className="m3-num-badge">
                    <input
                      type="number"
                      min="50"
                      max="160"
                      value={filters.c}
                      onChange={(e) => handleFilterChange('c', parseFloat(e.target.value) || 100)}
                    />
                    <span className="m3-num-badge-unit">%</span>
                  </div>
                </div>
                <input
                  type="range"
                  className="m3-slider"
                  min="50"
                  max="160"
                  value={filters.c}
                  style={{ background: calcTrackBg(filters.c, 50, 160) }}
                  onChange={(e) => handleFilterChange('c', parseFloat(e.target.value))}
                />
              </div>

              {/* 饱和度 */}
              <div className="m3-slider-tile">
                <div className="m3-tile-header">
                  <span className="m3-tile-label">🎨 饱和度 (Saturation)</span>
                  <div className="m3-num-badge">
                    <input
                      type="number"
                      min="0"
                      max="220"
                      value={filters.s}
                      onChange={(e) => handleFilterChange('s', parseFloat(e.target.value) || 100)}
                    />
                    <span className="m3-num-badge-unit">%</span>
                  </div>
                </div>
                <input
                  type="range"
                  className="m3-slider"
                  min="0"
                  max="220"
                  value={filters.s}
                  style={{ background: calcTrackBg(filters.s, 0, 220) }}
                  onChange={(e) => handleFilterChange('s', parseFloat(e.target.value))}
                />
              </div>

              {/* 色相偏移 */}
              <div className="m3-slider-tile">
                <div className="m3-tile-header">
                  <span className="m3-tile-label">🌈 色相偏移 (Hue Shift)</span>
                  <div className="m3-num-badge">
                    <input
                      type="number"
                      min="0"
                      max="360"
                      value={filters.h}
                      onChange={(e) => handleFilterChange('h', parseFloat(e.target.value) || 0)}
                    />
                    <span className="m3-num-badge-unit">°</span>
                  </div>
                </div>
                <input
                  type="range"
                  className="m3-slider"
                  min="0"
                  max="360"
                  value={filters.h}
                  style={{ background: calcTrackBg(filters.h, 0, 360) }}
                  onChange={(e) => handleFilterChange('h', parseFloat(e.target.value))}
                />
              </div>

              {/* 电影暗角 */}
              <div className="m3-slider-tile">
                <div className="m3-tile-header">
                  <span className="m3-tile-label">🔲 电影暗角 (Vignette)</span>
                  <div className="m3-num-badge">
                    <input
                      type="number"
                      min="0"
                      max="100"
                      value={filters.v}
                      onChange={(e) => handleFilterChange('v', parseFloat(e.target.value) || 0)}
                    />
                    <span className="m3-num-badge-unit">%</span>
                  </div>
                </div>
                <input
                  type="range"
                  className="m3-slider"
                  min="0"
                  max="100"
                  value={filters.v}
                  style={{ background: calcTrackBg(filters.v, 0, 100) }}
                  onChange={(e) => handleFilterChange('v', parseFloat(e.target.value))}
                />
              </div>

              {/* 胶片颗粒 */}
              <div className="m3-slider-tile">
                <div className="m3-tile-header">
                  <span className="m3-tile-label">🎞️ 胶片颗粒 (Film Grain)</span>
                  <div className="m3-num-badge">
                    <input
                      type="number"
                      min="0"
                      max="100"
                      value={filters.g}
                      onChange={(e) => handleFilterChange('g', parseFloat(e.target.value) || 0)}
                    />
                    <span className="m3-num-badge-unit">%</span>
                  </div>
                </div>
                <input
                  type="range"
                  className="m3-slider"
                  min="0"
                  max="100"
                  value={filters.g}
                  style={{ background: calcTrackBg(filters.g, 0, 100) }}
                  onChange={(e) => handleFilterChange('g', parseFloat(e.target.value))}
                />
              </div>
            </div>
          </div>
        </div>

        {/* 底部按钮 */}
        <div className="drawer-footer">
          <button
            className="m3-act-btn"
            style={{
              background: 'var(--m3-accent-lime)',
              color: '#1c192b',
              height: 50,
              fontSize: 14,
              fontWeight: 900,
              justifyContent: 'center',
              boxShadow: '0 6px 18px rgba(214, 248, 88, 0.45)',
              letterSpacing: '0.3px',
            }}
            onClick={() => onApply({ fit, scale, filterPreset: preset })}
          >
            ⚡ 立即设为壁纸 (应用到目标屏)
          </button>

          <div style={{ display: 'flex', gap: 10 }}>
            {source === 'wallhaven' && (
              <button
                className="m3-act-btn m3-act-tonal"
                style={{
                  flex: 1,
                  height: 42,
                  justifyContent: 'center',
                  fontSize: 13,
                  fontWeight: 700,
                  border: '1.5px solid var(--m3-outline-variant)',
                }}
                onClick={() => onSaveToLocal(displayWp)}
              >
                💾 保存到本地库
              </button>
            )}
            <button
              className="m3-act-btn m3-act-tonal"
              style={{
                flex: 1,
                height: 42,
                justifyContent: 'center',
                fontSize: 13,
                fontWeight: 700,
                border: '1.5px solid var(--m3-outline-variant)',
              }}
              onClick={() => onAddToPlaylist(displayWp)}
            >
              ➕ 加入轮播列表
            </button>
          </div>
        </div>
      </aside>
    </>
  )
}
