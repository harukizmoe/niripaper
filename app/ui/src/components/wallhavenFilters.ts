// What the Wallhaven search page filters on.

export interface WallhavenFilters {
  query: string
  general: boolean
  anime: boolean
  people: boolean
  sfw: boolean
  sketchy: boolean
  sort: string
  /** Only wallpapers that fit the target monitor's shape and size. */
  matchScreen: boolean
}

export const DEFAULT_FILTERS: WallhavenFilters = {
  query: '',
  general: true,
  anime: true,
  people: false,
  sfw: true,
  sketchy: false,
  sort: 'toplist',
  matchScreen: false,
}
