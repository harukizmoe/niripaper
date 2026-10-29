import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Box, Button, Fab, LinearProgress, Stack, ToggleButton, ToggleButtonGroup, Typography } from '@mui/material'
import ArrowUpwardRounded from '@mui/icons-material/ArrowUpwardRounded'
import type { WallpaperItem } from '../data/wallpapers'
import { hexToRgb } from '../theme/color'
import { EVERY_OUTPUT, type OutputInfo } from '../hooks/useOutputs'
import { LocalControls, type LocalFilter } from './LocalControls'
import { OutputSelect } from './OutputSelect'
import { WallhavenControls } from './WallhavenControls'
import { DEFAULT_FILTERS, type WallhavenFilters } from './wallhavenFilters'
import { MasonryGrid } from './MasonryGrid'
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
  /** Fetch the next page of the last search and append it. */
  onLoadMoreWallhaven: () => void
  onRefreshLocal: () => void
  /** A search asked for from elsewhere (a tag in the drawer); `key` makes a repeat count. */
  searchRequest?: { query: string; key: number } | null
  whLoading?: boolean
  /** There is another page after the last one loaded. */
  whHasMore?: boolean
  /** The last request failed; scrolling alone will not retry it. */
  whFailed?: boolean
  enableBlur?: boolean
  lazyLoad?: boolean
}

/// How far down the "back to top" button appears.
const TOP_BUTTON_AFTER = 1200

/// Quiet time after the last scroll event before cards react to the pointer again.
const SCROLL_IDLE_MS = 120

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
  onLoadMoreWallhaven,
  onRefreshLocal,
  searchRequest,
  whLoading = false,
  whHasMore = false,
  whFailed = false,
  enableBlur = true,
  lazyLoad = true,
}: WallpaperViewProps) {
  const [hoveredCardId, setHoveredCardId] = useState<string | null>(null)
  // `filters` is what the list on screen was searched with; `draft` is what is
  // typed in the box, which only becomes a search when it is submitted.
  const [filters, setFilters] = useState<WallhavenFilters>(DEFAULT_FILTERS)
  const [draft, setDraft] = useState('')
  const [localFilter, setLocalFilter] = useState<LocalFilter>('all')
  const [scrolledDown, setScrolledDown] = useState(false)

  const containerRef = useRef<HTMLDivElement>(null)
  const gridWrapRef = useRef<HTMLDivElement>(null)
  const scrollingRef = useRef(false)
  const scrollTimer = useRef<ReturnType<typeof setTimeout>>(undefined)

  // The monitor "fits the screen" is about: the chosen one, or the only one.
  const screen = outputs.find((o) => o.name === target) ?? (target === EVERY_OUTPUT ? outputs[0] : undefined)
  const screenSize = screen && screen.width > 0 && screen.height > 0 ? screen : undefined

  /// A new search: `patch` on top of the current filters and the typed text.
  const search = (patch: Partial<WallhavenFilters> = {}) => {
    const f = { ...filters, query: draft, ...patch }
    setFilters(f)
    setDraft(f.query)
    containerRef.current?.scrollTo({ top: 0, behavior: 'instant' })
    // Wallhaven reads three category and three purity flags as bit strings;
    // all off would match nothing, so fall back to the first flag.
    const categories = `${+f.general}${+f.anime}${+f.people}`
    const purity = `${+f.sfw}${+f.sketchy}0`
    const match = f.matchScreen && screenSize
    const divisor = match ? gcd(screenSize.width, screenSize.height) : 1
    onSearchWallhaven({
      q: f.query.trim(),
      categories: categories === '000' ? '100' : categories,
      purity: purity === '000' ? '100' : purity,
      sorting: f.sort,
      page: 1,
      ratios: match ? `${screenSize.width / divisor}x${screenSize.height / divisor}` : undefined,
      atleast: match ? `${screenSize.width}x${screenSize.height}` : undefined,
    })
  }

  useEffect(() => {
    if (searchRequest) search({ query: searchRequest.query })
    // Only a new request runs a search; `search` is rebuilt every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchRequest?.key])

  const filteredLocal = useMemo(() => filterLocal(localList, localFilter), [localList, localFilter])
  const list = currentSource === 'wallhaven' ? wallhavenList : filteredLocal
  const handleSelect = useCallback(
    (wp: WallpaperItem) => onSelectWallpaper(wp, currentSource),
    [onSelectWallpaper, currentSource],
  )
  const canLoadMore = currentSource === 'wallhaven' && whHasMore && !whLoading && !whFailed

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
          filters={{ ...filters, query: draft }}
          canMatchScreen={screenSize !== undefined}
          loading={whLoading}
          onQueryChange={setDraft}
          onSearch={(query) => search(query === undefined ? {} : { query })}
          onChange={(patch) => search(patch)}
          onReset={() => search(DEFAULT_FILTERS)}
        />
      ) : (
        <LocalControls
          currentFilter={localFilter}
          onFilterChange={setLocalFilter}
          onRefresh={onRefreshLocal}
        />
      )}

      {whLoading && currentSource === 'wallhaven' && <LinearProgress />}

      <Box
        className="gallery-viewport"
        ref={containerRef}
        // The grid positions its cards from offsets inside this box.
        sx={{ position: 'relative' }}
        onScroll={(e) => {
          setScrolledDown(e.currentTarget.scrollTop > TOP_BUTTON_AFTER)
          // Cards move under a still pointer while scrolling; letting each one
          // react (blur, tilt) would repaint the whole page on every frame.
          if (!scrollingRef.current) {
            scrollingRef.current = true
            gridWrapRef.current?.style.setProperty('pointer-events', 'none')
            setHoveredCardId(null)
          }
          clearTimeout(scrollTimer.current)
          scrollTimer.current = setTimeout(() => {
            scrollingRef.current = false
            gridWrapRef.current?.style.removeProperty('pointer-events')
          }, SCROLL_IDLE_MS)
        }}
      >
        {list.length === 0 && !whLoading && (
          <Typography sx={{ color: 'text.secondary', textAlign: 'center', py: 8 }}>
            {currentSource === 'local' ? '这里还没有壁纸。' : '没有找到壁纸，换个关键词或放宽筛选试试。'}
          </Typography>
        )}

        <Box ref={gridWrapRef}>
        <MasonryGrid
          items={list}
          scrollRef={containerRef}
          canLoadMore={canLoadMore}
          onLoadMore={onLoadMoreWallhaven}
          renderItem={(wp, lane) => (
            <WallpaperCard
              wallpaper={wp}
              isAnyHovered={hoveredCardId !== null}
              isHovered={hoveredCardId === wp.id}
              enableBlur={enableBlur}
              lazyLoad={lazyLoad}
              scrollRoot={containerRef}
              index={lane}
              onHover={setHoveredCardId}
              onSelect={handleSelect}
            />
          )}
        />
        </Box>

        <Stack sx={{ alignItems: 'center', pt: 4, minHeight: 56 }}>
          {currentSource === 'wallhaven' && whFailed ? (
            <Button onClick={onLoadMoreWallhaven}>加载失败，重试</Button>
          ) : (
            list.length > 0 &&
            !whLoading &&
            !canLoadMore && (
              <Typography variant="body2" sx={{ color: 'text.secondary' }}>
                已显示全部 {list.length} 张
              </Typography>
            )
          )}
        </Stack>

        {scrolledDown && (
          <Fab
            size="medium"
            aria-label="回到顶部"
            onClick={() => containerRef.current?.scrollTo({ top: 0, behavior: 'smooth' })}
            sx={{ position: 'sticky', float: 'right', bottom: 16, mr: 1 }}
          >
            <ArrowUpwardRounded />
          </Fab>
        )}
      </Box>
    </Box>
  )
}
