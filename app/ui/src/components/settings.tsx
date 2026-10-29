// Building blocks every settings page is made of: a page with a title, groups
// of rows, and a row that is a label + one line of help on the left and its
// control on the right. Pages compose these and nothing else, so spacing,
// type and the reset affordance are the same everywhere.

import type { ReactNode } from 'react'
import { Box, Divider, IconButton, Paper, Stack, Tooltip, Typography } from '@mui/material'
import RestartAltRounded from '@mui/icons-material/RestartAltRounded'

export function SettingsPage({
  title,
  actions,
  children,
}: {
  title: string
  /** Page-level controls, right of the title. */
  actions?: ReactNode
  children: ReactNode
}) {
  return (
    <Box sx={{ flex: 1, minHeight: 0, overflowY: 'auto' }}>
      <Box sx={{ maxWidth: 880, mx: 'auto', px: 4, pt: 4, pb: 6 }}>
        <Stack direction="row" sx={{ alignItems: 'center', mb: 3, minHeight: 40 }}>
          <Typography variant="h5" component="h1" sx={{ flex: 1 }}>
            {title}
          </Typography>
          {actions}
        </Stack>
        <Stack spacing={4}>{children}</Stack>
      </Box>
    </Box>
  )
}

export function SettingsGroup({
  title,
  description,
  children,
}: {
  title: string
  description?: ReactNode
  children: ReactNode
}) {
  // Dividers between rows, not around them: interleave instead of relying on
  // `Stack divider`, which would also need every child to be a row.
  const rows = (Array.isArray(children) ? children : [children]).filter(Boolean)
  return (
    <Box component="section">
      <Typography variant="subtitle2" component="h2" sx={{ color: 'primary.main', px: 2, mb: description ? 0.5 : 1 }}>
        {title}
      </Typography>
      {description && (
        <Typography variant="body2" sx={{ color: 'text.secondary', px: 2, mb: 1 }}>
          {description}
        </Typography>
      )}
      <Paper sx={{ bgcolor: 'md.surface_container_low', borderRadius: 4, overflow: 'hidden' }}>
        {rows.map((row, i) => (
          <Box key={i}>
            {i > 0 && <Divider sx={{ mx: 2 }} />}
            {row}
          </Box>
        ))}
      </Paper>
    </Box>
  )
}

export function SettingRow({
  label,
  description,
  control,
  onReset,
  modified = false,
  disabled = false,
  children,
}: {
  label: ReactNode
  description?: ReactNode
  /** The row's control, right-aligned. */
  control?: ReactNode
  /** Restores the default; the button only shows while `modified`. */
  onReset?: () => void
  modified?: boolean
  disabled?: boolean
  /** Full-width content under the label (a slider, a grid). */
  children?: ReactNode
}) {
  return (
    <Box sx={{ px: 2, py: 1.5 }}>
      <Stack direction="row" spacing={2} sx={{ alignItems: 'center', minHeight: 40 }}>
        {/* Only the text dims; a disabled control already draws itself so. */}
        <Box sx={{ flex: 1, minWidth: 0, opacity: disabled ? 0.38 : 1 }}>
          <Typography variant="subtitle1" component="div" sx={{ fontSize: 15, lineHeight: '22px' }}>
            {label}
          </Typography>
          {description && (
            <Typography variant="body1" sx={{ color: 'text.secondary', mt: 0.25 }}>
              {description}
            </Typography>
          )}
        </Box>
        {onReset && (
          // The slot is kept when hidden so the control does not jump sideways.
          <Box sx={{ width: 40, flexShrink: 0 }}>
            {modified && !disabled && (
              <Tooltip title="恢复默认值">
                <IconButton size="small" onClick={onReset} aria-label="恢复默认值">
                  <RestartAltRounded fontSize="small" />
                </IconButton>
              </Tooltip>
            )}
          </Box>
        )}
        {control && <Box sx={{ flexShrink: 0 }}>{control}</Box>}
      </Stack>
      {children && <Box sx={{ mt: 1.5 }}>{children}</Box>}
    </Box>
  )
}
