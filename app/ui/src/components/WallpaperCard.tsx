import React, { useState, useEffect } from 'react'
import type { WallpaperItem } from '../data/wallpapers'

interface WallpaperCardProps {
  wallpaper: WallpaperItem
  onClick: () => void
}

const FALLBACK_THUMB =
  'https://images.unsplash.com/photo-1506744038136-46273834b3fb?w=600&auto=format&fit=crop&q=80'

export const WallpaperCard: React.FC<WallpaperCardProps> = ({
  wallpaper,
  onClick,
}) => {
  const [imgSrc, setImgSrc] = useState(wallpaper.thumb)
  const [loaded, setLoaded] = useState(false)

  useEffect(() => {
    setImgSrc(wallpaper.thumb)
    setLoaded(false)
  }, [wallpaper.thumb])

  return (
    <div
      className={`wallpaper-card ${wallpaper.activeOn ? 'active-target' : ''}`}
      onClick={onClick}
    >
      <div
        className="card-media"
        style={{
          aspectRatio: wallpaper.aspect,
          background: 'var(--m3-surface-subtle)',
        }}
      >
        <img
          src={imgSrc}
          alt={wallpaper.name}
          loading="lazy"
          decoding="async"
          onLoad={() => setLoaded(true)}
          onError={() => {
            setImgSrc(FALLBACK_THUMB)
            setLoaded(true)
          }}
          style={{
            opacity: loaded ? 1 : 0.4,
            transition:
              'opacity 0.25s ease, transform 0.4s cubic-bezier(0.2, 0, 0, 1)',
          }}
        />
        <div className="card-glare"></div>
        {wallpaper.activeOn && (
          <span className="badge-active">✓ {wallpaper.activeOn}</span>
        )}
      </div>
    </div>
  )
}
