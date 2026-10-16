// Ported directly from tasks/prototype-ui.html with 100% fidelity

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
  tags?: string[];
  kind?: "static" | "video";
  fps?: string;
  activeOn?: string | null;
  path?: string;
}

export interface TransitionItem {
  id: string;
  name: string;
  desc: string;
}

export const whWallpapers: WallpaperItem[] = [
      {
        id: "wh-1",
        name: "Cyberpunk Geisha Cyber City",
        res: "3840×2160",
        ratio: "16:9",
        category: "Anime",
        purity: "SFW",
        favs: "3.8k",
        aspect: "16 / 9",
        colors: ["#0f172a", "#f43f5e", "#06b6d4", "#8b5cf6", "#e2e8f0"],
        thumb: "https://images.unsplash.com/photo-1578632767115-351597cf2477?w=700&auto=format&fit=crop&q=80",
        tags: ["Cyberpunk", "AnimeGirl", "Neon", "Sci-Fi", "Night"]
      },
      {
        id: "wh-2",
        name: "Rainy Shinjuku Neon Crossing",
        res: "2560×1440",
        ratio: "16:9",
        category: "General",
        purity: "SFW",
        favs: "2.1k",
        aspect: "9 / 14",
        colors: ["#18181b", "#3b82f6", "#e11d48", "#f59e0b", "#64748b"],
        thumb: "https://images.unsplash.com/photo-1503899036084-c55cdd92da26?w=700&auto=format&fit=crop&q=80",
        tags: ["Tokyo", "Rain", "Night", "Cityscape", "Japan"]
      },
      {
        id: "wh-3",
        name: "Ethereal Mountain Milky Way",
        res: "3840×2160",
        ratio: "16:9",
        category: "General",
        purity: "SFW",
        favs: "4.5k",
        aspect: "16 / 9",
        colors: ["#0f172a", "#38bdf8", "#818cf8", "#f472b6", "#f8fafc"],
        thumb: "https://images.unsplash.com/photo-1519681393784-d120267933ba?w=700&auto=format&fit=crop&q=80",
        tags: ["Landscape", "Stars", "Night", "Galaxy", "Peak"]
      },
      {
        id: "wh-4",
        name: "Raiden Shogun Electro Shrine",
        res: "3840×2160",
        ratio: "16:9",
        category: "Anime",
        purity: "SFW",
        favs: "6.2k",
        aspect: "4 / 3",
        colors: ["#2e1065", "#7e22ce", "#a855f7", "#e9d5ff", "#fbbf24"],
        thumb: "https://images.unsplash.com/photo-1534447677768-be436bb09401?w=700&auto=format&fit=crop&q=80",
        tags: ["Genshin", "Anime", "Raiden", "Purple", "Fantasy"]
      },
      {
        id: "wh-5",
        name: "Nordic Lake Cabin Sunset Glow",
        res: "2560×1440",
        ratio: "16:9",
        category: "General",
        purity: "SFW",
        favs: "1.8k",
        aspect: "16 / 10",
        colors: ["#1e293b", "#ea580c", "#f97316", "#38bdf8", "#f1f5f9"],
        thumb: "https://images.unsplash.com/photo-1506744038136-46273834b3fb?w=700&auto=format&fit=crop&q=80",
        tags: ["Sunset", "Lake", "Cabin", "Nature", "Calm"]
      },
      {
        id: "wh-6",
        name: "Retro Synthwave Outrun Sunset",
        res: "3840×2160",
        ratio: "16:9",
        category: "General",
        purity: "SFW",
        favs: "5.1k",
        aspect: "16 / 9",
        colors: ["#180026", "#ff007f", "#7928ca", "#00dfd8", "#ffffff"],
        thumb: "https://images.unsplash.com/photo-1508739773434-c26b3d09e071?w=700&auto=format&fit=crop&q=80",
        tags: ["Synthwave", "Retro", "Outrun", "Grid", "80s"]
      },
      {
        id: "wh-7",
        name: "Dark Knight Bloodmoon Armor",
        res: "2560×1440",
        ratio: "16:9",
        category: "People",
        purity: "SFW",
        favs: "2.9k",
        aspect: "4 / 5",
        colors: ["#09090b", "#71717a", "#b91c1c", "#f43f5e", "#fafafa"],
        thumb: "https://images.unsplash.com/photo-1579783900882-c0d3dad7b119?w=700&auto=format&fit=crop&q=80",
        tags: ["Knight", "Armor", "DarkFantasy", "Sword"]
      },
      {
        id: "wh-8",
        name: "Minimalist Fluid Aurora 5K",
        res: "5120×2880",
        ratio: "16:9",
        category: "General",
        purity: "SFW",
        favs: "7.4k",
        aspect: "16 / 10",
        colors: ["#020617", "#06b6d4", "#3b82f6", "#6366f1", "#f8fafc"],
        thumb: "https://images.unsplash.com/photo-1557683316-973673baf926?w=700&auto=format&fit=crop&q=80",
        tags: ["Abstract", "Fluid", "Aurora", "5K", "Minimalism"]
      },
      {
        id: "wh-9",
        name: "Anime Samurai Sakura Falling",
        res: "3840×2160",
        ratio: "16:9",
        category: "Anime",
        purity: "SFW",
        favs: "3.3k",
        aspect: "16 / 9",
        colors: ["#1c1917", "#f43f5e", "#fda4af", "#78716c", "#fff1f2"],
        thumb: "https://images.unsplash.com/photo-1518709268805-4e9042af9f23?w=700&auto=format&fit=crop&q=80",
        tags: ["Samurai", "CherryBlossom", "Anime", "Japan"]
      },
      {
        id: "wh-10",
        name: "Cozy Lofi Pixel Study Rain",
        res: "2560×1440",
        ratio: "16:9",
        category: "General",
        purity: "SFW",
        favs: "4.1k",
        aspect: "1 / 1",
        colors: ["#1e1b4b", "#4338ca", "#a5b4fc", "#fde047", "#f5f3ff"],
        thumb: "https://images.unsplash.com/photo-1550684848-fac1c5b4e853?w=700&auto=format&fit=crop&q=80",
        tags: ["PixelArt", "Cozy", "Lofi", "Rain", "Room"]
      },
      {
        id: "wh-11",
        name: "Fantasy Floating Castle Islands",
        res: "3840×2160",
        ratio: "16:9",
        category: "General",
        purity: "SFW",
        favs: "5.8k",
        aspect: "3 / 4",
        colors: ["#082f49", "#0284c7", "#38bdf8", "#facc15", "#f0f9ff"],
        thumb: "https://images.unsplash.com/photo-1464822759023-fed622ff2c3b?w=700&auto=format&fit=crop&q=80",
        tags: ["Fantasy", "Castle", "Sky", "Islands"]
      },
      {
        id: "wh-12",
        name: "Minimalist Red Torii Gate Fog",
        res: "3840×2160",
        ratio: "16:9",
        category: "General",
        purity: "SFW",
        favs: "3.6k",
        aspect: "16 / 9",
        colors: ["#18181b", "#dc2626", "#ef4444", "#a1a1aa", "#ffffff"],
        thumb: "https://images.unsplash.com/photo-1493976040374-85c8e12f0c0e?w=700&auto=format&fit=crop&q=80",
        tags: ["Torii", "Japan", "Fog", "Minimalism", "Red"]
      }
    ];

export const localWallpapers: WallpaperItem[] = [
      { id: "loc-1", name: "kobeni_gun.webp", res: "2560×1440", kind: "static", activeOn: "DP-1", aspect: "4/3", colors: ["#3b2a63", "#8b5cf6", "#d8b4fe", "#f0abfc", "#1e1b4b"], thumb: "https://images.unsplash.com/photo-1579783900882-c0d3dad7b119?w=600&auto=format&fit=crop&q=80" },
      { id: "loc-2", name: "蔚蓝档案_白洲梓无模糊版2.mp4", res: "3072×1728", kind: "video", fps: "25 FPS", aspect: "16/9", colors: ["#1e3a5f", "#4a7dd6", "#93c5fd", "#e0f2fe", "#0f172a"], thumb: "https://images.unsplash.com/photo-1618005182384-a83a8bd57fbe?w=600&auto=format&fit=crop&q=80" },
      { id: "loc-3", name: "anime_car_sunset.webp", res: "3840×2160", kind: "static", aspect: "9/14", colors: ["#7c2d12", "#e8734a", "#fb923c", "#fcd34d", "#fed7aa"], thumb: "https://images.unsplash.com/photo-1509198397868-475647b2a1e5?w=600&auto=format&fit=crop&q=80" },
      { id: "loc-4", name: "c45动态壁纸循环.mp4", res: "1920×1080", kind: "video", fps: "30 FPS", activeOn: "eDP-1", aspect: "1/1", colors: ["#0c4a6e", "#2d6a8f", "#38bdf8", "#7dd3fc", "#082f49"], thumb: "https://images.unsplash.com/photo-1550684848-fac1c5b4e853?w=600&auto=format&fit=crop&q=80" },
      { id: "loc-5", name: "cyberpunk_city_rain.mp4", res: "2560×1440", kind: "video", fps: "24 FPS", aspect: "4/5", colors: ["#2e1065", "#7c3aed", "#22d3ee", "#f472b6", "#18181b"], thumb: "https://images.unsplash.com/photo-1518709268805-4e9042af9f23?w=600&auto=format&fit=crop&q=80" },
      { id: "loc-6", name: "minimalist_mountain.webp", res: "2560×1440", kind: "static", aspect: "3/4", colors: ["#334155", "#6b7f95", "#94a3b8", "#e2e8f0", "#1e293b"], thumb: "https://images.unsplash.com/photo-1464822759023-fed622ff2c3b?w=600&auto=format&fit=crop&q=80" },
      { id: "loc-7", name: "nordic_deep_forest.webp", res: "2560×1440", kind: "static", aspect: "4/5", colors: ["#14532d", "#86efac", "#22c55e", "#166534", "#052e16"], thumb: "https://images.unsplash.com/photo-1448375240586-882707db888b?w=600&auto=format&fit=crop&q=80" },
      { id: "loc-8", name: "retro_synthwave_grid.mp4", res: "1920×1080", kind: "video", fps: "60 FPS", aspect: "16/9", colors: ["#180026", "#ff007f", "#e879f9", "#7000ff", "#2b0040"], thumb: "https://images.unsplash.com/photo-1509198397868-475647b2a1e5?w=600&auto=format&fit=crop&q=80" },
      { id: "loc-9", name: "sakura_torii_fog.webp", res: "3840×2160", kind: "static", aspect: "16/10", colors: ["#18181b", "#fca5a5", "#dc2626", "#f87171", "#7f1d1d"], thumb: "https://images.unsplash.com/photo-1528164344705-475426879c0d?w=600&auto=format&fit=crop&q=80" },
      { id: "loc-10", name: "lofi_study_room.mp4", res: "1920×1080", kind: "video", fps: "30 FPS", aspect: "1/1", colors: ["#1e1b4b", "#fde047", "#6366f1", "#4338ca", "#312e81"], thumb: "https://images.unsplash.com/photo-1519681393784-d120267933ba?w=600&auto=format&fit=crop&q=80" },
      { id: "loc-11", name: "aurora_fluid_5k.webp", res: "5120×2880", kind: "static", aspect: "16/10", colors: ["#020617", "#22d3ee", "#3b82f6", "#06b6d4", "#0f172a"], thumb: "https://images.unsplash.com/photo-1579033461380-adb47c3eb938?w=600&auto=format&fit=crop&q=80" },
      { id: "loc-12", name: "neon_alley_night.mp4", res: "2560×1440", kind: "video", fps: "24 FPS", aspect: "9/16", colors: ["#0f172a", "#f472b6", "#38bdf8", "#ec4899", "#1e293b"], thumb: "https://images.unsplash.com/photo-1514565131-fce0801e5785?w=600&auto=format&fit=crop&q=80" }
    ];

export const transitions: TransitionItem[] = [
      { id: "portal", name: "Portal 传送门", desc: "空间推挤折叠，伴随景深收缩，默认配置 0.55 push。" },
      { id: "iris", name: "Iris 圆环光圈", desc: "以画面焦点展开圆形视野，start_radius 与 center 可配。" },
      { id: "zoom", name: "Zoom 径向变焦", desc: "画面从纵深疾速拉近覆盖，经典电影级转场。" },
      { id: "slide", name: "Slide 平滑推移", desc: "跟随桌面平铺方向水平滑入，极其克制工整。" },
      { id: "fade", name: "Fade 柔和淡入", desc: "最稳健优雅的淡入淡出，零侵略感。" },
      { id: "dissolve", name: "Dissolve 溶解粒子", desc: "像素矩阵轻微解离渐变，带胶片质感。" },
      { id: "wipe", name: "Wipe 线性擦除", desc: "单向扫光换图，斜角擦除。" },
      { id: "stripes", name: "Stripes 百叶窗", desc: "多条等距切带并行展开。" },
      { id: "push", name: "Push 弹性推挤", desc: "带轻微 spring 物理惯性的位移覆盖。" }
    ];
