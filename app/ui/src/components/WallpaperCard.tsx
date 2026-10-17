import React, { useRef, useState, useEffect } from 'react'
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
  const cardRef = useRef<HTMLDivElement>(null)
  const glareRef = useRef<HTMLDivElement>(null)
  const rafRef = useRef<number | null>(null)

  const [imgSrc, setImgSrc] = useState(wallpaper.thumb)
  const [loaded, setLoaded] = useState(false)

  useEffect(() => {
    setImgSrc(wallpaper.thumb)
    setLoaded(false)
  }, [wallpaper.thumb])

  const handleMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!cardRef.current) return
    if (rafRef.current) cancelAnimationFrame(rafRef.current)

    rafRef.current = requestAnimationFrame(() => {
      if (!cardRef.current) return
      const rect = cardRef.current.getBoundingClientRect()
      const centerX = rect.left + rect.width / 2
      const centerY = rect.top + rect.height / 2
      const mouseX = e.clientX - centerX
      const mouseY = e.clientY - centerY

      const px = mouseX / (rect.width / 2)
      const py = mouseY / (rect.height / 2)

      // 微视差轻翘：克制的倾斜角度，避免大比例放大导致命中测试抖动和闪烁
      const maxTilt = 7
      const rotateY = px * maxTilt
      const rotateX = -py * maxTilt

      cardRef.current.style.transform = `perspective(800px) translateY(-5px) rotateX(${rotateX.toFixed(
        1
      )}deg) rotateY(${rotateY.toFixed(1)}deg)`

      if (glareRef.current) {
        const percentX = ((e.clientX - rect.left) / rect.width) * 100
        const percentY = ((e.clientY - rect.top) / rect.height) * 100
        glareRef.current.style.background = `radial-gradient(circle at ${percentX.toFixed(
          1
        )}% ${percentY.toFixed(1)}%, rgba(255, 255, 255, 0.3) 0%, transparent 60%)`
      }
    })
  }

  const handleMouseLeave = () => {
    if (rafRef.current) cancelAnimationFrame(rafRef.current)
    if (cardRef.current) {
      cardRef.current.style.transform =
        'perspective(800px) translateY(0) rotateX(0deg) rotateY(0deg)'
    }
  }

  return (
    <div
      ref={cardRef}
      className={`wallpaper-card ${wallpaper.activeOn ? 'active-target' : ''}`}
      onMouseMove={handleMouseMove}
      onMouseLeave={handleMouseLeave}
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
          onLoad={() => setLoaded(true)}
          onError={() => {
            setImgSrc(FALLBACK_THUMB)
            setLoaded(true)
          }}
          style={{
            opacity: loaded ? 1 : 0.4,
            transition: 'opacity 0.25s ease, transform 0.4s cubic-bezier(0.2, 0, 0, 1)',
          }}
        />
        <div ref={glareRef} className="card-glare"></div>
        {wallpaper.activeOn && (
          <span className="badge-active">✓ {wallpaper.activeOn}</span>
        )}
      </div>
    </div>
  )
}
