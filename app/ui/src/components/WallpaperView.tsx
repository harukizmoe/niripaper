import { useEffect, useMemo, useRef, useState } from 'react'
import { Box, Button, LinearProgress, Pagination, Stack, ToggleButton, ToggleButtonGroup, Typography } from '@mui/material'
import CasinoRounded from '@mui/icons-material/CasinoRounded'
import type { WallpaperItem } from '../data/wallpapers'
import { hexToRgb } from '../theme/color'
import { EVERY_OUTPUT, type OutputInfo } from '../hooks/useOutputs'
import { LocalControls, type LocalFilter } from './LocalControls'
import { OutputSelect } from './OutputSelect'
import { WallhavenControls } from './WallhavenControls'
import { DEFAULT_FILTERS, type WallhavenFilters } from './wallhavenFilters'
import { WallpaperCard } from './WallpaperCard'

export interface WallhavenQuery {
  q?: string
  categories?: string
  purity?: string
  sorting?: string
  order?: string
  page?: number
  ratios?: string
  atleast?: string
}

interface WallpaperViewProps {
  wallhavenList: WallpaperItem[]
  localList: WallpaperItem[]
  currentSource: 'wallhaven' | 'local'
  outputs: OutputInfo[]
  target: string
  onTargetChange: (target: string) => void
  onSourceChange: (source: 'wallhaven' | 'local') => void
  onSelectWallpaper: (wp: WallpaperItem, source: 'wallhaven' | 'local') => void
  onSearchWallhaven: (params: WallhavenQuery) => void
  onRefreshLocal: () => void
  /** A search asked for from elsewhere (a tag in the drawer); `key` makes a repeat count. */
  searchRequest?: { query: string; key: number } | null
  whLoading?: boolean
  whTotal?: number
  enableBlur?: boolean
  lazyLoad?: boolean
}

const WH_PAGE_SIZE = 24
const LOCAL_PAGE_SIZE = 24

const gcd = (a: number, b: number): number => (b === 0 ? a : gcd(b, a % b))

/// 0.299/0.587/0.114 luma of the wallpaper's first source colour, 0..255.
const lumaOf = (w: WallpaperItem) => {
  const c = hexToRgb(w.colors![0])
  return 0.299 * c.r + 0.587 * c.g + 0.114 * c.b
}

const DARK_BELOW = 65

function filterLocal(list: WallpaperItem[], filter: LocalFilter) {
  switch (filter) {
    case 'static':
    case 'video':
      return list.filter((w) => w.kind === filter)
    case 'dark':
      return list.filter((w) => w.colors?.length && lumaOf(w) < DARK_BELOW)
    case 'light':
      return list.filter((w) => w.colors?.length && lumaOf(w) >= DARK_BELOW)
    default:
      return list
  }
}

export function WallpaperView({
  wallhavenList,
  localList,
  currentSource,
  outputs,
  target,
  onTargetChange,
  onSourceChange,
  onSelectWallpaper,
  onSearchWallhaven,
  onRefreshLocal,
  searchRequest,
  whLoading = false,
  whTotal = 26116,
  enableBlur = true,
  lazyLoad = true,
}: WallpaperViewProps) {
  const [hoveredCardId, setHoveredCardId] = useState<string | null>(null)
  const [filters, setFilters] = useState<WallhavenFilters>(DEFAULT_FILTERS)
  const [whPage, setWhPage] = useState(1)
  const [localFilter, setLocalFilter] = useState<LocalFilter>('all')
  const [localPage, setLocalPage] = useState(1)

  const containerRef = useRef<HTMLDivElement>(null)
  const [colCount, setColCount] = useState(4)

  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const updateCols = () => {
      const w = el.clientWidth
      setColCount(w < 720 ? 2 : w < 1120 ? 3 : 4)
    }
    updateCols()
    const observer = new ResizeObserver(updateCols)
    observer.observe(el)
    return () => observer.disconnect()
  }, [])

  // The monitor "fits the screen" is about: the chosen one, or the only one.
  const screen = outputs.find((o) => o.name === target) ?? (target === EVERY_OUTPUT ? outputs[0] : undefined)
  const screenSize = screen && screen.width > 0 && screen.height > 0 ? screen : undefined

  /// Search with `patch` applied on top of the current filters.
  const search = (patch: Partial<WallhavenFilters> = {}, page = 1) => {
    const f = { ...filters, ...patch }
    setFilters(f)
    setWhPage(page)
    // Wallhaven reads three category and three purity flags as bit strings;
    // all off would match nothing, so fall back to the defaults' first flag.
    const categories = `${+f.general}${+f.anime}${+f.people}`
    const purity = `${+f.sfw}${+f.sketchy}0`
    const match = f.matchScreen && screenSize
    const divisor = match ? gcd(screenSize.width, screenSize.height) : 1
    onSearchWallhaven({
      q: f.query.trim(),
      categories: categories === '000' ? '100' : categories,
      purity: purity === '000' ? '100' : purity,
      sorting: f.sort,
      page,
      ratios: match ? `${screenSize.width / divisor}x${screenSize.height / divisor}` : undefined,
      atleast: match ? `${screenSize.width}x${screenSize.height}` : undefined,
    })
  }

  useEffect(() => {
    if (searchRequest) search({ query: searchRequest.query })
    // Only a new request runs a search; `search` is rebuilt every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchRequest?.key])

  const whPages = Math.max(1, Math.ceil(whTotal / WH_PAGE_SIZE))

  const filteredLocal = useMemo(() => filterLocal(localList, localFilter), [localList, localFilter])
  const localPages = Math.max(1, Math.ceil(filteredLocal.length / LOCAL_PAGE_SIZE))
  const pagedLocal = filteredLocal.slice((localPage - 1) * LOCAL_PAGE_SIZE, localPage * LOCAL_PAGE_SIZE)

  const displayList = currentSource === 'wallhaven' ? wallhavenList : pagedLocal

  const columns = useMemo(() => {
    const cols: WallpaperItem[][] = Array.from({ length: colCount }, () => [])
    displayList.forEach((item, index) => cols[index % colCount].push(item))
    return cols
  }, [displayList, colCount])

  return (
    <Box sx={{ flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column' }}>
      <Stack direction="row" sx={{ alignItems: 'center', px: 3.5, py: 2 }}>
        <ToggleButtonGroup
          exclusive
          size="small"
          value={currentSource}
          onChange={(_, v) => v && onSourceChange(v)}
        >
          <ToggleButton value="local">本地 · {localList.length}</ToggleButton>
          <ToggleButton value="wallhaven">Wallhaven</ToggleButton>
        </ToggleButtonGroup>
        <Box sx={{ ml: 'auto' }}>
          <OutputSelect outputs={outputs} value={target} onChange={onTargetChange} />
        </Box>
      </Stack>

      {currentSource === 'wallhaven' ? (
        <WallhavenControls
          filters={filters}
          canMatchScreen={screenSize !== undefined}
          loading={whLoading}
          onQueryChange={(query) => setFilters((f) => ({ ...f, query }))}
          onSearch={(query) => search(query === undefined ? {} : { query })}
          onChange={(patch) => search(patch)}
          onReset={() => search(DEFAULT_FILTERS)}
        />
      ) : (
        <LocalControls
          currentFilter={localFilter}
          onFilterChange={(f) => {
            setLocalFilter(f)
            setLocalPage(1)
          }}
          onRefresh={onRefreshLocal}
        />
      )}

      <Box className="gallery-viewport" ref={containerRef}>
        {whLoading && currentSource === 'wallhaven' && <LinearProgress sx={{ mb: 2, borderRadius: 1 }} />}

        {displayList.length === 0 && !whLoading && (
          <Typography sx={{ color: 'text.secondary', textAlign: 'center', py: 8 }}>
            {currentSource === 'local' ? '这里还没有壁纸。' : '没有找到壁纸，换个关键词或放宽筛选试试。'}
          </Typography>
        )}

        <Box sx={{ display: 'grid', gridTemplateColumns: `repeat(${colCount}, minmax(0, 1fr))`, gap: 2.5, alignItems: 'start' }}>
          {columns.map((col, colIdx) => (
            <Box key={colIdx} sx={{ display: 'flex', flexDirection: 'column', gap: 2.5 }}>
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
            </Box>
          ))}
        </Box>

        {currentSource === 'wallhaven' && (
          <Stack direction="row" spacing={2} sx={{ justifyContent: 'center', alignItems: 'center', pt: 4 }}>
            <Pagination
              page={whPage}
              count={whPages}
              shape="rounded"
              color="primary"
              disabled={whLoading}
              onChange={(_, p) => search({}, p)}
            />
            <Button
              variant="contained"
              color="secondary"
              startIcon={<CasinoRounded />}
              disabled={whLoading}
              // Past the first hundred pages the results are too old to be worth a roll.
              onClick={() => search({}, Math.floor(Math.random() * Math.min(100, whPages)) + 1)}
            >
              随机一页
            </Button>
          </Stack>
        )}

        {currentSource === 'local' && localPages > 1 && (
          <Stack sx={{ alignItems: 'center', pt: 4 }}>
            <Pagination page={localPage} count={localPages} shape="rounded" color="primary" onChange={(_, p) => setLocalPage(p)} />
          </Stack>
        )}
      </Box>
    </Box>
  )
}
