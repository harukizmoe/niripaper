// One wallpaper as the library and Wallhaven search report it
// (app/src/commands.rs: LocalWallpaperItem, the Wallhaven result type).

export interface WallpaperItem {
  id: string;
  name: string;
  res?: string;
  ratio?: string;
  category?: string;
  purity?: string;
  favs?: string;
  aspect: string;
  colors?: string[];
  thumb: string;
  full_url?: string;
  file_type?: string;
  tags?: string[];
  kind?: "static" | "video";
  fps?: string;
  activeOn?: string | null;
  path?: string;
}
