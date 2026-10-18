import React, { useState, useMemo, useRef, useEffect } from 'react'
import type { WallpaperItem } from '../data/wallpapers'
import { WallhavenControls } from './WallhavenControls'
import { LocalControls, type LocalFilter } from './LocalControls'
import { WallpaperCard } from './WallpaperCard'
import { hexToRgb } from '../theme/materialYou'
import { ThemeToggle } from './ThemeToggle'

interface WallpaperViewProps {
  wallhavenList: WallpaperItem[]
  localList: WallpaperItem[]
  currentSource: 'wallhaven' | 'local'
  currentDisplay: string
  isDark: boolean
  onToggleTheme: () => void
  onSourceChange: (source: 'wallhaven' | 'local') => void
  onCycleDisplay: () => void
  onSelectWallpaper: (wp: WallpaperItem, source: 'wallhaven' | 'local') => void
  onNotify: (msg: string) => void
}

export const WallpaperView: React.FC<WallpaperViewProps> = ({
  wallhavenList,
  localList,
  currentSource,
  currentDisplay,
  isDark,
  onToggleTheme,
  onSourceChange,
  onCycleDisplay,
  onSelectWallpaper,
  onNotify,
}) => {
  // Wallhaven filters & search
  const [whQuery, setWhQuery] = useState('')
  const [whPage, setWhPage] = useState(1)
  const [catGeneral, setCatGeneral] = useState(true)
  const [catAnime, setCatAnime] = useState(true)
  const [catPeople, setCatPeople] = useState(false)
  const [puritySfw, setPuritySfw] = useState(true)
  const [puritySketchy, setPuritySketchy] = useState(false)
  const [sortOption, setSortOption] = useState('toplist')

  // Local filter
  const [localFilter, setLocalFilter] = useState<LocalFilter>('all')

  const containerRef = useRef<HTMLDivElement>(null)
  const [colCount, setColCount] = useState(4)

  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const updateCols = () => {
      const w = el.clientWidth
      if (w < 720) setColCount(2)
      else if (w < 1120) setColCount(3)
      else setColCount(4)
    }
    updateCols()
    const observer = new ResizeObserver(updateCols)
    observer.observe(el)
    return () => observer.disconnect()
  }, [])

  // Local filtering calculation
  const getFilteredLocal = () => {
    if (localFilter === 'static') {
      return localList.filter((w) => w.kind === 'static')
    }
    if (localFilter === 'video') {
      return localList.filter((w) => w.kind === 'video')
    }
    if (localFilter === 'dark') {
      return localList.filter((w) => {
        if (!w.colors || !w.colors.length) return false
        const c = hexToRgb(w.colors[0])
        return 0.299 * c.r + 0.587 * c.g + 0.114 * c.b < 65
      })
    }
    if (localFilter === 'light') {
      return localList.filter((w) => {
        if (!w.colors || !w.colors.length) return false
        const c = hexToRgb(w.colors[0])
        return 0.299 * c.r + 0.587 * c.g + 0.114 * c.b >= 65
      })
    }
    return localList
  }

  // Wallhaven search calculation
  const getFilteredWh = () => {
    let list = wallhavenList
    const q = whQuery.trim().toLowerCase()
    if (q) {
      list = list.filter(
        (w) =>
          w.name.toLowerCase().includes(q) ||
          (w.tags && w.tags.some((t) => t.toLowerCase().includes(q))) ||
          (w.category && w.category.toLowerCase().includes(q))
      )
    }
    return list
  }

  const handleWhSearchSubmit = () => {
    const q = whQuery.trim().toLowerCase()
    if (!q) {
      onNotify('重置为 Wallhaven 热门壁纸')
      return
    }
    const filtered = getFilteredWh()
    onNotify(`搜索「${q}」：找到 ${filtered.length} 张壁纸`)
  }

  const handleQuickTag = (tag: string) => {
    setWhQuery(tag)
    onNotify(`搜索标签「${tag}」`)
  }

  const handleMatchScreen = () => {
    setWhQuery('')
    onNotify('已开启屏幕专属过滤：16:9 比例 · ≥2K 分辨率 (完美适配 DP-1)')
  }

  const handleResetWhFilters = () => {
    setWhQuery('')
    setCatGeneral(true)
    setCatAnime(true)
    setCatPeople(false)
    setPuritySfw(true)
    setPuritySketchy(false)
    onNotify('已重置所有 Wallhaven 过滤条件')
  }

  const handleLocalFilterChange = (filter: LocalFilter) => {
    setLocalFilter(filter)
    const filtered = getFilteredLocal()
    onNotify(`筛选本地壁纸：找到 ${filtered.length} 张`)
  }

  const handlePageChange = (delta: number) => {
    const next = Math.max(1, whPage + delta)
    setWhPage(next)
    onNotify(`加载 Wallhaven 第 ${next} 页`)
  }

  const displayList =
    currentSource === 'wallhaven' ? getFilteredWh() : getFilteredLocal()

  const columns = useMemo(() => {
    const cols: WallpaperItem[][] = Array.from({ length: colCount }, () => [])
    displayList.forEach((item, index) => {
      cols[index % colCount].push(item)
    })
    return cols
  }, [displayList, colCount])
  return (
    <div className="view-page active" id="view-wallpapers">
      {/* 顶栏：壁纸来源切换 [本地壁纸 | 🌐 Wallhaven 公共源] + 显示器下拉 */}
      <header className="wallpaper-top-bar">
        <div className="source-segmented-switch">
          <button
            className={`source-seg-btn ${
              currentSource === 'wallhaven' ? 'active' : ''
            }`}
            onClick={() => onSourceChange('wallhaven')}
          >
            <span>🌐 Wallhaven 公共源</span>
          </button>
          <button
            className={`source-seg-btn ${
              currentSource === 'local' ? 'active' : ''
            }`}
            onClick={() => onSourceChange('local')}
          >
            <span>📁 本地壁纸库 ({localList.length})</span>
          </button>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <button className="m3-display-capsule" onClick={onCycleDisplay}>
            <span>🖥️</span>
            <span>{currentDisplay}</span>
            <span style={{ fontSize: 11, opacity: 0.6 }}>▼</span>
          </button>
          <ThemeToggle isDark={isDark} onToggle={onToggleTheme} />
        </div>
      </header>

      {/* 模式 A：Wallhaven 在线控制区 */}
      {currentSource === 'wallhaven' && (
        <WallhavenControls
          searchQuery={whQuery}
          onSearchChange={setWhQuery}
          onSearchSubmit={handleWhSearchSubmit}
          onQuickTag={handleQuickTag}
          onMatchScreen={handleMatchScreen}
          onResetFilters={handleResetWhFilters}
          categoryGeneral={catGeneral}
          onToggleCategoryGeneral={() => setCatGeneral((v) => !v)}
          categoryAnime={catAnime}
          onToggleCategoryAnime={() => setCatAnime((v) => !v)}
          categoryPeople={catPeople}
          onToggleCategoryPeople={() => setCatPeople((v) => !v)}
          puritySfw={puritySfw}
          onTogglePuritySfw={() => setPuritySfw((v) => !v)}
          puritySketchy={puritySketchy}
          onTogglePuritySketchy={() => setPuritySketchy((v) => !v)}
          sortOption={sortOption}
          onSortChange={setSortOption}
        />
      )}

      {/* 模式 B：本地壁纸专属工具条 */}
      {currentSource === 'local' && (
        <LocalControls
          currentFilter={localFilter}
          count={localList.length}
          onFilterChange={handleLocalFilterChange}
        />
      )}

      {/* 纯视觉大瀑布流 */}
      <div className="gallery-viewport" ref={containerRef}>
        <div
          className="gallery-grid-layout"
          id="wallpaper-grid"
          style={{
            display: 'grid',
            gridTemplateColumns: `repeat(${colCount}, minmax(0, 1fr))`,
            gap: 20,
            alignItems: 'start',
          }}
        >
          {columns.map((col, colIdx) => (
            <div
              key={colIdx}
              className="gallery-column"
              style={{ display: 'flex', flexDirection: 'column', gap: 20 }}
            >
              {col.map((wp) => (
                <WallpaperCard
                  key={wp.id}
                  wallpaper={wp}
                  onClick={() => onSelectWallpaper(wp, currentSource)}
                />
              ))}
            </div>
          ))}
        </div>

        {/* 分页条 */}
        {currentSource === 'wallhaven' && (
          <div className="wh-pagination-bar" id="wh-pagination">
            <div className="wh-page-pill">
              <button
                className="wh-page-nav-btn"
                onClick={() => handlePageChange(-1)}
              >
                ◀
              </button>
              <span>
                第{' '}
                <strong
                  style={{ color: 'var(--m3-accent-purple)' }}
                  id="wh-current-page"
                >
                  {whPage}
                </strong>{' '}
                页 / 共 26,116 页
              </span>
              <button
                className="wh-page-nav-btn"
                onClick={() => handlePageChange(1)}
              >
                ▶
              </button>
            </div>
            <button
              className="wh-toggle-pill"
              style={{ height: 40, padding: '0 18px' }}
              onClick={() => handlePageChange(1)}
            >
              🎲 换一批随机
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
