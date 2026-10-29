// ============================================================================
// Material 3 on MUI.
//
// MUI implements Material Design 2 and has no M3 (mui/material-ui#29345 is on
// hold), so M3 here is a theme: the colour roles from `color.ts`, the M3 type
// scale and shape scale, and per-component overrides that give MUI's widgets
// M3 geometry. Component styling lives here and only here — pages compose
// MUI components and do not restyle them.
//
// The theme is rebuilt from real hex values whenever the roles change (new
// wallpaper, scheme, light/dark). That is rare, and it lets MUI compute its
// own derived shades instead of depending on `color-mix()` at runtime.
// ============================================================================

import { alpha, createTheme, type Theme } from '@mui/material/styles'
import type { Roles } from './color'

declare module '@mui/material/styles' {
  interface Palette {
    /** Every M3 colour role, by its snake_case name. */
    md: Roles
  }
  interface PaletteOptions {
    md?: Roles
  }
}

/// M3 shape scale (px).
export const shape = {
  xs: 4,
  sm: 8,
  md: 12,
  lg: 16,
  xl: 28,
  full: 9999,
} as const

/// M3 state-layer opacities.
const state = { hover: 0.08, focus: 0.1, pressed: 0.1, dragged: 0.16 }

const FONT = [
  '"Roboto Flex"',
  'Roboto',
  '"Noto Sans CJK SC"',
  '"Source Han Sans SC"',
  '"Noto Sans SC"',
  'sans-serif',
].join(',')

export function buildTheme(r: Roles, dark: boolean): Theme {
  return createTheme({
    palette: {
      mode: dark ? 'dark' : 'light',
      md: r,
      primary: { main: r.primary, contrastText: r.on_primary },
      secondary: { main: r.secondary, contrastText: r.on_secondary },
      error: { main: r.error, contrastText: r.on_error },
      background: { default: r.surface_container, paper: r.surface },
      text: {
        primary: r.on_surface,
        secondary: r.on_surface_variant,
        disabled: alpha(r.on_surface, 0.38),
      },
      divider: r.outline_variant,
      action: {
        active: r.on_surface_variant,
        hover: alpha(r.on_surface, state.hover),
        hoverOpacity: state.hover,
        selected: r.secondary_container,
        focus: alpha(r.on_surface, state.focus),
        disabled: alpha(r.on_surface, 0.38),
        disabledBackground: alpha(r.on_surface, 0.12),
      },
    },
    shape: { borderRadius: shape.md },
    // M3 type scale. MUI's variant names are reused rather than augmented:
    // h5 headline-small, h6 title-large, subtitle1/2 title-medium/small,
    // body1/2 body-medium/small (desktop density), caption/overline labels.
    typography: {
      fontFamily: FONT,
      h5: { fontSize: 24, lineHeight: '32px', fontWeight: 400 },
      h6: { fontSize: 22, lineHeight: '28px', fontWeight: 400 },
      subtitle1: { fontSize: 16, lineHeight: '24px', fontWeight: 500 },
      subtitle2: { fontSize: 14, lineHeight: '20px', fontWeight: 500 },
      body1: { fontSize: 14, lineHeight: '20px', letterSpacing: 0.25 },
      body2: { fontSize: 12, lineHeight: '16px', letterSpacing: 0.4 },
      button: { fontSize: 14, lineHeight: '20px', fontWeight: 500, textTransform: 'none', letterSpacing: 0.1 },
      caption: { fontSize: 12, lineHeight: '16px', fontWeight: 500, letterSpacing: 0.5 },
      overline: { fontSize: 11, lineHeight: '16px', fontWeight: 500, letterSpacing: 0.5, textTransform: 'none' },
    },
    transitions: {
      // M3 standard easing.
      easing: { easeInOut: 'cubic-bezier(0.2, 0, 0, 1)' },
    },
    components: {
      MuiCssBaseline: {
        styleOverrides: {
          body: { backgroundColor: r.surface_container, color: r.on_surface },
          '::selection': { backgroundColor: r.primary_container, color: r.on_primary_container },
        },
      },
      MuiButtonBase: { defaultProps: { disableRipple: false } },
      MuiButton: {
        defaultProps: { disableElevation: true },
        styleOverrides: {
          root: { borderRadius: shape.full, height: 40, paddingInline: 24 },
          sizeSmall: { height: 32, paddingInline: 16 },
          outlined: { borderColor: r.outline, color: r.primary },
          text: { paddingInline: 12 },
        },
        variants: [
          {
            // M3 "tonal" button: the secondary-container fill.
            props: { variant: 'contained', color: 'secondary' },
            style: {
              backgroundColor: r.secondary_container,
              color: r.on_secondary_container,
              '&:hover': { backgroundColor: r.secondary_container, boxShadow: 'none' },
            },
          },
        ],
      },
      MuiIconButton: {
        styleOverrides: { root: { color: r.on_surface_variant } },
      },
      MuiPaper: {
        defaultProps: { elevation: 0 },
        styleOverrides: { root: { backgroundImage: 'none' } },
      },
      MuiCard: {
        defaultProps: { elevation: 0 },
        styleOverrides: { root: { borderRadius: shape.md, backgroundColor: r.surface_container_low } },
      },
      MuiSwitch: {
        // M3 switch: 52×32 track, a 16px thumb that grows to 24px when on,
        // an outlined track when off.
        styleOverrides: {
          root: { width: 52, height: 32, padding: 0, overflow: 'visible' },
          switchBase: {
            padding: 0,
            margin: 8,
            '&.Mui-checked': {
              transform: 'translateX(20px)',
              margin: 4,
              '& + .MuiSwitch-track': { backgroundColor: r.primary, borderColor: r.primary, opacity: 1 },
              '& .MuiSwitch-thumb': { width: 24, height: 24, backgroundColor: r.on_primary },
            },
            '&.Mui-disabled + .MuiSwitch-track': { opacity: 0.12 },
            '&.Mui-disabled .MuiSwitch-thumb': { opacity: 0.38 },
          },
          thumb: {
            width: 16,
            height: 16,
            boxShadow: 'none',
            backgroundColor: r.outline,
            transition: 'all 150ms cubic-bezier(0.2, 0, 0, 1)',
          },
          track: {
            borderRadius: shape.full,
            border: `2px solid ${r.outline}`,
            backgroundColor: r.surface_container_highest,
            opacity: 1,
            boxSizing: 'border-box',
          },
        },
      },
      MuiSlider: {
        styleOverrides: {
          root: { height: 4 },
          rail: { backgroundColor: r.secondary_container, opacity: 1 },
          thumb: {
            width: 20,
            height: 20,
            boxShadow: 'none',
            '&:hover, &.Mui-focusVisible': { boxShadow: `0 0 0 10px ${alpha(r.primary, state.hover)}` },
            '&.Mui-active': { boxShadow: `0 0 0 10px ${alpha(r.primary, state.pressed)}` },
          },
          valueLabel: {
            backgroundColor: r.inverse_surface,
            color: r.inverse_on_surface,
            borderRadius: shape.full,
            fontSize: 12,
          },
        },
      },
      MuiToggleButtonGroup: {
        // M3 segmented button.
        styleOverrides: {
          root: { borderRadius: shape.full },
          grouped: {
            borderColor: r.outline,
            '&:first-of-type': { borderTopLeftRadius: shape.full, borderBottomLeftRadius: shape.full },
            '&:last-of-type': { borderTopRightRadius: shape.full, borderBottomRightRadius: shape.full },
          },
        },
      },
      MuiToggleButton: {
        styleOverrides: {
          root: {
            height: 40,
            paddingInline: 16,
            textTransform: 'none',
            fontWeight: 500,
            color: r.on_surface,
            '&.Mui-selected, &.Mui-selected:hover': {
              backgroundColor: r.secondary_container,
              color: r.on_secondary_container,
            },
          },
          sizeSmall: { height: 32, paddingInline: 12 },
        },
      },
      MuiOutlinedInput: {
        styleOverrides: {
          root: {
            borderRadius: shape.xs,
            '& .MuiOutlinedInput-notchedOutline': { borderColor: r.outline },
          },
        },
      },
      MuiMenu: {
        styleOverrides: {
          paper: { backgroundColor: r.surface_container, borderRadius: shape.xs },
        },
      },
      MuiMenuItem: {
        styleOverrides: {
          root: {
            minHeight: 48,
            '&.Mui-selected': { backgroundColor: alpha(r.on_surface, state.focus) },
          },
        },
      },
      MuiTooltip: {
        defaultProps: { arrow: false, enterDelay: 400 },
        styleOverrides: {
          tooltip: {
            backgroundColor: r.inverse_surface,
            color: r.inverse_on_surface,
            borderRadius: shape.xs,
            fontSize: 12,
            padding: '4px 8px',
          },
        },
      },
      MuiAlert: {
        styleOverrides: {
          root: { borderRadius: shape.md, padding: '4px 16px', fontSize: 14, alignItems: 'center' },
        },
        variants: [
          {
            props: { severity: 'info', variant: 'filled' },
            style: {
              backgroundColor: r.surface_container_highest,
              color: r.on_surface,
              '& .MuiAlert-icon': { color: r.primary },
            },
          },
          {
            props: { severity: 'error', variant: 'filled' },
            style: {
              backgroundColor: r.error_container,
              color: r.on_error_container,
              '& .MuiAlert-icon': { color: r.error },
            },
          },
        ],
      },
      MuiChip: {
        styleOverrides: {
          root: { borderRadius: shape.sm, height: 32, fontWeight: 500 },
          outlined: { borderColor: r.outline_variant },
        },
      },
      MuiDrawer: {
        styleOverrides: {
          paper: { backgroundColor: r.surface_container_low, border: 'none' },
        },
      },
      MuiDialog: {
        styleOverrides: {
          paper: { backgroundColor: r.surface_container_high, borderRadius: shape.xl },
        },
      },
      MuiDivider: {
        styleOverrides: { root: { borderColor: r.outline_variant } },
      },
      MuiListItemButton: {
        styleOverrides: {
          root: {
            borderRadius: shape.full,
            '&.Mui-selected, &.Mui-selected:hover': {
              backgroundColor: r.secondary_container,
              color: r.on_secondary_container,
            },
          },
        },
      },
    },
  })
}
