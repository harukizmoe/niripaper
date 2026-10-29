import React, { useState, useMemo, useRef, useEffect } from 'react'
import type { WallpaperItem } from '../data/wallpapers'
import { WallhavenControls } from './WallhavenControls'
import { LocalControls, type LocalFilter } from './LocalControls'
import { WallpaperCard } from './WallpaperCard'
import { hexToRgb } from '../theme/color'

interface WallpaperViewProps {
  wallhavenList: WallpaperItem[]
  localList: WallpaperItem[]
  currentSource: 'wallhaven' | 'local'
  currentDisplay: string
  onSourceChange: (source: 'wallhaven' | 'local') => void
  onCycleDisplay: () => void
  onSelectWallpaper: (wp: WallpaperItem, source: 'wallhaven' | 'local') => void
  onNotify: (msg: string) => void
  onSearchWallhaven: (params: {
    q?: string
    categories?: string
    purity?: string
    sorting?: string
    order?: string
    page?: number
    ratios?: string
    atleast?: string
  }) => void
  onRefreshLocal: () => void
  whLoading?: boolean
  whTotal?: number
  enableBlur?: boolean
  lazyLoad?: boolean
}

export const WallpaperView: React.FC<WallpaperViewProps> = ({
  wallhavenList,
  localList,
  currentSource,
  currentDisplay,
  onSourceChange,
  onCycleDisplay,
  onSelectWallpaper,
  onNotify,
  onSearchWallhaven,
  onRefreshLocal,
  whLoading = false,
  whTotal = 26116,
  enableBlur = true,
  lazyLoad = true,
}) => {
  const [hoveredCardId, setHoveredCardId] = useState<string | null>(null)

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

  const buildCategories = (
    gen = catGeneral,
    ani = catAnime,
    peo = catPeople
  ) => {
    const cat = `${gen ? '1' : '0'}${ani ? '1' : '0'}${peo ? '1' : '0'}`
    return cat === '000' ? '100' : cat
  }

  const buildPurity = (sfw = puritySfw, sk = puritySketchy) => {
    const p = `${sfw ? '1' : '0'}${sk ? '1' : '0'}0`
    return p === '000' ? '100' : p
  }

  // 触发实时 Wallhaven API 查询
  const executeSearch = (
    queryText?: string,
    pageVal?: number,
    ratiosVal?: string,
    atleastVal?: string,
    sortVal?: string,
    genVal?: boolean,
    aniVal?: boolean,
    peoVal?: boolean,
    sfwVal?: boolean,
    skVal?: boolean
  ) => {
    const q = queryText !== undefined ? queryText : whQuery
    const p = pageVal !== undefined ? pageVal : whPage
    const s = sortVal !== undefined ? sortVal : sortOption
    const categories = buildCategories(genVal, aniVal, peoVal)
    const purity = buildPurity(sfwVal, skVal)

    onSearchWallhaven({
      q: q.trim(),
      categories,
      purity,
      sorting: s,
      page: p,
      ratios: ratiosVal,
      atleast: atleastVal,
    })
  }

  const handleWhSearchSubmit = () => {
    setWhPage(1)
    executeSearch(whQuery, 1)
    onNotify(`🌐 正在从 Wallhaven 检索：「${whQuery || '热门 Toplist'}」`)
  }

  const handleQuickTag = (tag: string) => {
    setWhQuery(tag)
    setWhPage(1)
    executeSearch(tag, 1)
    onNotify(`🌐 按灵感标签「${tag}」检索 Wallhaven`)
  }

  const handleMatchScreen = () => {
    setWhQuery('')
    setWhPage(1)
    executeSearch('', 1, '16x9', '2560x1440')
    onNotify('🌐 已开启屏幕匹配检索：16:9 比例 · ≥2K 分辨率')
  }

  const handleResetWhFilters = () => {
    setWhQuery('')
    setWhPage(1)
    setCatGeneral(true)
    setCatAnime(true)
    setCatPeople(false)
    setPuritySfw(true)
    setPuritySketchy(false)
    setSortOption('toplist')
    executeSearch('', 1, undefined, undefined, 'toplist', true, true, false, true, false)
    onNotify('已重置所有 Wallhaven 过滤条件并刷新')
  }

  const handleSortChange = (newSort: string) => {
    setSortOption(newSort)
    setWhPage(1)
    executeSearch(undefined, 1, undefined, undefined, newSort)
    onNotify(`切换排序方式为：${newSort}`)
  }

  const handleToggleCatGeneral = () => {
    const next = !catGeneral
    setCatGeneral(next)
    executeSearch(undefined, 1, undefined, undefined, undefined, next)
  }

  const handleToggleCatAnime = () => {
    const next = !catAnime
    setCatAnime(next)
    executeSearch(undefined, 1, undefined, undefined, undefined, undefined, next)
  }

  const handleToggleCatPeople = () => {
    const next = !catPeople
    setCatPeople(next)
    executeSearch(undefined, 1, undefined, undefined, undefined, undefined, undefined, next)
  }

  const handleTogglePuritySfw = () => {
    const next = !puritySfw
    setPuritySfw(next)
    executeSearch(undefined, 1, undefined, undefined, undefined, undefined, undefined, undefined, next)
  }

  const handleTogglePuritySketchy = () => {
    const next = !puritySketchy
    setPuritySketchy(next)
    executeSearch(undefined, 1, undefined, undefined, undefined, undefined, undefined, undefined, undefined, next)
  }

  const handleLocalFilterChange = (filter: LocalFilter) => {
    setLocalFilter(filter)
    onNotify(`筛选本地壁纸：${filter}`)
  }

  const handlePageChange = (delta: number) => {
    const totalPages = Math.max(1, Math.ceil(whTotal / 24))
    const next = Math.max(1, Math.min(totalPages, whPage + delta))
    if (next === whPage) return
    setWhPage(next)
    executeSearch(undefined, next)
    onNotify(`加载 Wallhaven 第 ${next} 页`)
  }

  const [localPage, setLocalPage] = useState(1)
  const LOCAL_PAGE_SIZE = 24
  const filteredLocal = useMemo(() => getFilteredLocal(), [localList, localFilter])
  const localTotalPages = Math.max(1, Math.ceil(filteredLocal.length / LOCAL_PAGE_SIZE))

  useEffect(() => {
    setLocalPage(1)
  }, [localFilter])

  const paginatedLocal = useMemo(() => {
    const start = (localPage - 1) * LOCAL_PAGE_SIZE
    return filteredLocal.slice(start, start + LOCAL_PAGE_SIZE)
  }, [filteredLocal, localPage])

  const displayList =
    currentSource === 'wallhaven' ? wallhavenList : paginatedLocal

  const columns = useMemo(() => {
    const cols: WallpaperItem[][] = Array.from({ length: colCount }, () => [])
    displayList.forEach((item, index) => {
      cols[index % colCount].push(item)
    })
    return cols
  }, [displayList, colCount])

  const totalPages = Math.max(1, Math.ceil(whTotal / 24))

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

        <button className="m3-display-capsule" onClick={onCycleDisplay}>
          <span>🖥️</span>
          <span>{currentDisplay}</span>
          <span style={{ fontSize: 11, opacity: 0.6 }}>▼</span>
        </button>
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
          onToggleCategoryGeneral={handleToggleCatGeneral}
          categoryAnime={catAnime}
          onToggleCategoryAnime={handleToggleCatAnime}
          categoryPeople={catPeople}
          onToggleCategoryPeople={handleToggleCatPeople}
          puritySfw={puritySfw}
          onTogglePuritySfw={handleTogglePuritySfw}
          puritySketchy={puritySketchy}
          onTogglePuritySketchy={handleTogglePuritySketchy}
          sortOption={sortOption}
          onSortChange={handleSortChange}
          loading={whLoading}
        />
      )}

      {/* 模式 B：本地壁纸专属工具条 */}
      {currentSource === 'local' && (
        <LocalControls
          currentFilter={localFilter}
          count={getFilteredLocal().length}
          onFilterChange={handleLocalFilterChange}
          onRefresh={onRefreshLocal}
        />
      )}

      {/* 纯视觉大瀑布流 */}
      <div className="gallery-viewport" ref={containerRef}>
        {whLoading && currentSource === 'wallhaven' && (
          <div
            style={{
              padding: '16px 0',
              textAlign: 'center',
              color: 'var(--m3-accent-purple)',
              fontWeight: 700,
              fontSize: 13,
            }}
          >
            ⏳ 正在从 Wallhaven 服务器获取高清壁纸…
          </div>
        )}

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
              {col.map((wp, rowIdx) => (
                <WallpaperCard
                  key={wp.id}
                  wallpaper={wp}
                  isAnyHovered={hoveredCardId !== null}
                  isHovered={hoveredCardId === wp.id}
                  enableBlur={enableBlur}
                  lazyLoad={lazyLoad}
                  scrollRoot={containerRef}
                  index={rowIdx * colCount + colIdx}
                  onMouseEnter={() => setHoveredCardId(wp.id)}
                  onMouseLeave={() => setHoveredCardId(null)}
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
                disabled={whPage <= 1}
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
                页 / 共 {totalPages.toLocaleString()} 页
              </span>
              <button
                className="wh-page-nav-btn"
                onClick={() => handlePageChange(1)}
                disabled={whPage >= totalPages}
              >
                ▶
              </button>
            </div>
            <button
              className="wh-toggle-pill"
              style={{ height: 40, padding: '0 18px' }}
              onClick={() => {
                const randomPage = Math.floor(Math.random() * Math.min(100, totalPages)) + 1
                setWhPage(randomPage)
                executeSearch(undefined, randomPage)
                onNotify(`🎲 随机跳转至 Wallhaven 第 ${randomPage} 页`)
              }}
            >
              🎲 换一批随机
            </button>
          </div>
        )}

        {/* 本地壁纸分页条 */}
        {currentSource === 'local' && localTotalPages > 1 && (
          <div className="wh-pagination-bar" id="local-pagination">
            <div className="wh-page-pill">
              <button
                className="wh-page-nav-btn"
                onClick={() => setLocalPage((p) => Math.max(1, p - 1))}
                disabled={localPage <= 1}
              >
                ◀
              </button>
              <span>
                第{' '}
                <strong style={{ color: 'var(--m3-accent-purple)' }}>
                  {localPage}
                </strong>{' '}
                页 / 共 {localTotalPages} 页 (本地共 {filteredLocal.length} 张)
              </span>
              <button
                className="wh-page-nav-btn"
                onClick={() => setLocalPage((p) => Math.min(localTotalPages, p + 1))}
                disabled={localPage >= localTotalPages}
              >
                ▶
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
