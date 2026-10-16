import Paper from '@mui/material/Paper'
import Typography from '@mui/material/Typography'

// US-006/US-009: the library grid. The two daemon verbs it needs (`list` and
// `thumbnail`) are the only pending daemon work (PRD 技术考虑), so this page
// ships as the empty state it will honestly show until they land.
export function LibrarySection() {
  return (
    <Paper
      elevation={0}
      sx={{
        padding: 6,
        borderRadius: 'var(--m3-radius-lg)',
        background: 'var(--m3-surface)',
        boxShadow: 'var(--m3-shadow-sm)',
        textAlign: 'center',
      }}
    >
      <Typography variant="h6" sx={{ fontWeight: 700, marginBottom: 1 }}>
        库
      </Typography>
      <Typography sx={{ color: 'var(--m3-fg-muted)' }}>
        库的默认位置是 ~/Pictures/Wallpapers。缩略图由守护进程渲染（`list` / `thumbnail`
        两个动词落地后，这里会出现网格）。
      </Typography>
    </Paper>
  )
}
