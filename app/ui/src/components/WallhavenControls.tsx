import {
  Button,
  Chip,
  IconButton,
  InputAdornment,
  MenuItem,
  Select,
  Stack,
  TextField,
  Typography,
} from '@mui/material'
import SearchRounded from '@mui/icons-material/SearchRounded'
import ClearRounded from '@mui/icons-material/ClearRounded'
import { ToggleChip } from './chips'
import type { WallhavenFilters } from './wallhavenFilters'

const QUICK_TAGS = ['Cyberpunk', 'Anime', 'Landscape', 'Pixel Art', 'Fantasy', 'Sci-Fi', 'Minimalism']

const SORTS = [
  { id: 'toplist', label: '排行榜' },
  { id: 'hot', label: '正在流行' },
  { id: 'random', label: '随机' },
  { id: 'favorites', label: '收藏最多' },
  { id: 'views', label: '浏览最多' },
  { id: 'date_added', label: '最新发布' },
]

export function WallhavenControls({
  filters,
  canMatchScreen,
  loading,
  onQueryChange,
  onSearch,
  onChange,
  onReset,
}: {
  filters: WallhavenFilters
  /** Whether the daemon told us how big the target monitor is. */
  canMatchScreen: boolean
  loading: boolean
  /** Typing only edits the text; `onSearch` runs it. */
  onQueryChange: (query: string) => void
  onSearch: (query?: string) => void
  /** A filter changed: search again with it. */
  onChange: (patch: Partial<WallhavenFilters>) => void
  onReset: () => void
}) {
  return (
    <Stack spacing={1.5} sx={{ px: 3.5, pb: 1.5 }}>
      <Stack direction="row" spacing={1.5}>
        <TextField
          fullWidth
          size="small"
          value={filters.query}
          placeholder="搜索标签，例如 anime、landscape、night"
          onChange={(e) => onQueryChange(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && onSearch()}
          slotProps={{
            input: {
              startAdornment: (
                <InputAdornment position="start">
                  <SearchRounded />
                </InputAdornment>
              ),
              endAdornment: filters.query && (
                <InputAdornment position="end">
                  <IconButton size="small" aria-label="清空" onClick={() => onSearch('')}>
                    <ClearRounded fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
            htmlInput: { 'aria-label': '搜索 Wallhaven' },
          }}
        />
        <Button variant="contained" disabled={loading} onClick={() => onSearch()} sx={{ flexShrink: 0 }}>
          {loading ? '检索中…' : '搜索'}
        </Button>
      </Stack>

      <Stack direction="row" spacing={1} sx={{ alignItems: 'center', flexWrap: 'wrap', rowGap: 1 }}>
        <Typography variant="body2" sx={{ color: 'text.secondary' }}>
          常用标签
        </Typography>
        {QUICK_TAGS.map((tag) => (
          <Chip key={tag} label={tag} variant="outlined" onClick={() => onSearch(tag)} />
        ))}
      </Stack>

      <Stack direction="row" spacing={1} sx={{ alignItems: 'center', flexWrap: 'wrap', rowGap: 1 }}>
        <Typography variant="body2" sx={{ color: 'text.secondary' }}>
          分类
        </Typography>
        <ToggleChip label="综合" selected={filters.general} onClick={() => onChange({ general: !filters.general })} />
        <ToggleChip label="动漫" selected={filters.anime} onClick={() => onChange({ anime: !filters.anime })} />
        <ToggleChip label="人物" selected={filters.people} onClick={() => onChange({ people: !filters.people })} />

        <Typography variant="body2" sx={{ color: 'text.secondary', pl: 1.5 }}>
          分级
        </Typography>
        <ToggleChip label="安全" selected={filters.sfw} onClick={() => onChange({ sfw: !filters.sfw })} />
        <ToggleChip label="敏感" selected={filters.sketchy} onClick={() => onChange({ sketchy: !filters.sketchy })} />

        <Select
          size="small"
          value={filters.sort}
          onChange={(e) => onChange({ sort: e.target.value })}
          inputProps={{ 'aria-label': '排序方式' }}
          sx={{ ml: 'auto !important', minWidth: 130 }}
        >
          {SORTS.map((s) => (
            <MenuItem key={s.id} value={s.id}>
              {s.label}
            </MenuItem>
          ))}
        </Select>
        {canMatchScreen && (
          <ToggleChip
            label="适合当前屏幕"
            selected={filters.matchScreen}
            onClick={() => onChange({ matchScreen: !filters.matchScreen })}
          />
        )}
        <Button onClick={onReset}>重置</Button>
      </Stack>
    </Stack>
  )
}
