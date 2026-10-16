import { createTheme } from '@mui/material/styles'

// M3 Expressive tokens (index.css) mapped into MUI's palette. The mapping is
// the single place where the design system meets the component library: change
// the tokens in index.css, and MUI follows; add a component here, and it stays
// on-token.
//
// Light theme; `dark` below mirrors it from the `html.dark` block of the
// prototype (tasks/prototype-ui.html).
export const lightTheme = createTheme({
  palette: {
    mode: 'light',
    primary: { main: '#624b89', contrastText: '#ffffff' },
    secondary: { main: '#624b89' },
    background: { default: '#ece8f6', paper: '#ffffff' },
    text: { primary: '#1d192b', secondary: '#6e6783' },
    divider: '#ccc6de',
    success: { main: '#3f9d6d' },
    error: { main: '#c96f68' },
  },
  shape: {
    borderRadius: 16, // --m3-radius-md
  },
  typography: {
    fontFamily:
      "'Plus Jakarta Sans', 'Roboto', -apple-system, sans-serif",
    button: { textTransform: 'none', fontWeight: 600 },
  },
  components: {
    MuiPaper: {
      styleOverrides: {
        root: { backgroundImage: 'none' },
      },
    },
    MuiCard: {
      styleOverrides: {
        root: { borderRadius: 'var(--m3-radius-lg)' },
      },
    },
    MuiChip: {
      styleOverrides: {
        root: { borderRadius: 'var(--m3-radius-full)' },
      },
    },
  },
})

export const darkTheme = createTheme({
  ...lightTheme,
  palette: {
    mode: 'dark',
    primary: { main: '#d1b8fc', contrastText: '#1c192b' },
    secondary: { main: '#d1b8fc' },
    background: { default: '#0f0c18', paper: '#221c33' },
    text: { primary: '#f4effa', secondary: '#aca3c5' },
    divider: '#463c5e',
    success: { main: '#9ad7b2' },
    error: { main: '#ffb4ab' },
  },
})
