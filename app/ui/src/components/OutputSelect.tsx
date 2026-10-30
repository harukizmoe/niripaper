import { MenuItem, Select } from '@mui/material'
import MonitorRounded from '@mui/icons-material/MonitorRounded'
import { EVERY_OUTPUT, outputLabel, type OutputInfo } from '../hooks/useOutputs'

/// Which monitor a wallpaper goes to: one of the daemon's outputs, or all.
export function OutputSelect({
  outputs,
  value,
  onChange,
}: {
  outputs: OutputInfo[]
  value: string
  onChange: (target: string) => void
}) {
  // A target the daemon no longer reports must still show, not render blank.
  const names = [...new Set([...outputs.map((o) => o.name), value].filter((n) => n !== EVERY_OUTPUT))]
  return (
    <Select
      size="small"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      startAdornment={<MonitorRounded fontSize="small" sx={{ mr: 1, color: 'text.secondary' }} />}
      inputProps={{ 'aria-label': '目标显示器' }}
      sx={{ minWidth: 180 }}
    >
      <MenuItem value={EVERY_OUTPUT}>{outputLabel(EVERY_OUTPUT)}</MenuItem>
      {names.map((n) => (
        <MenuItem key={n} value={n}>
          {n}
        </MenuItem>
      ))}
    </Select>
  )
}
