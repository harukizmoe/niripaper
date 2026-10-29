import { Chip } from '@mui/material'
import CheckRounded from '@mui/icons-material/CheckRounded'

/// A filter chip: outlined while off, tonal with a check while on.
export function ToggleChip({
  label,
  selected,
  disabled,
  onClick,
}: {
  label: string
  selected: boolean
  disabled?: boolean
  onClick: () => void
}) {
  return (
    <Chip
      label={label}
      disabled={disabled}
      onClick={onClick}
      variant={selected ? 'filled' : 'outlined'}
      color={selected ? 'secondary' : 'default'}
      icon={selected ? <CheckRounded /> : undefined}
      aria-pressed={selected}
    />
  )
}
