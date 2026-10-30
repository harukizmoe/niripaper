import { IconButton, Stack, Tooltip } from '@mui/material'
import RefreshRounded from '@mui/icons-material/RefreshRounded'
import { ToggleChip } from './chips'

export type LocalFilter = 'all' | 'static' | 'video' | 'dark' | 'light' | 'playlist'

export function LocalControls({
  currentFilter,
  playlistCount = 0,
  onFilterChange,
  onRefresh,
}: {
  currentFilter: LocalFilter
  playlistCount?: number
  onFilterChange: (filter: LocalFilter) => void
  onRefresh: () => void
}) {
  const filters: { id: LocalFilter; label: string }[] = [
    { id: 'all', label: '全部' },
    { id: 'static', label: '图片' },
    { id: 'video', label: '视频' },
    { id: 'dark', label: '暗色' },
    { id: 'light', label: '亮色' },
    { id: 'playlist', label: playlistCount > 0 ? `轮播 (${playlistCount})` : '轮播' },
  ]

  return (
    <Stack direction="row" spacing={1} sx={{ px: 3.5, pb: 1.5, alignItems: 'center' }}>
      {filters.map((f) => (
        <ToggleChip key={f.id} label={f.label} selected={currentFilter === f.id} onClick={() => onFilterChange(f.id)} />
      ))}
      <Tooltip title="重新扫描壁纸库">
        <IconButton onClick={onRefresh} aria-label="重新扫描壁纸库" sx={{ ml: 'auto !important' }}>
          <RefreshRounded />
        </IconButton>
      </Tooltip>
    </Stack>
  )
}
