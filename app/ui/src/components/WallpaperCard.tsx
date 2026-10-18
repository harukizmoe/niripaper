import React, { useState, useEffect, useRef } from 'react'
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
    const card = cardRef.current
    const glare = glareRef.current

    if (rafRef.current) cancelAnimationFrame(rafRef.current)

    rafRef.current = requestAnimationFrame(() => {
      const rect = card.getBoundingClientRect()
      const centerX = rect.left + rect.width / 2
      const centerY = rect.top + rect.height / 2
      const mouseX = e.clientX - centerX
      const mouseY = e.clientY - centerY

      const px = mouseX / (rect.width / 2)
      const py = mouseY / (rect.height / 2)

      // 3D 悬浮视差与随鼠标四向微翘（完全对齐 prototype-ui.html 的 attach3DParallax）
      const maxTilt = 12
      const rotateY = px * maxTilt
      const rotateX = -py * maxTilt

      card.style.transform = `perspective(1000px) translateY(-6px) scale(1.04) rotateX(${rotateX.toFixed(
        2
      )}deg) rotateY(${rotateY.toFixed(2)}deg)`

      if (glare) {
        const percentX = ((e.clientX - rect.left) / rect.width) * 100
        const percentY = ((e.clientY - rect.top) / rect.height) * 100
        glare.style.opacity = '1'
        glare.style.background = `radial-gradient(circle at ${percentX.toFixed(
          1
        )}% ${percentY.toFixed(1)}%, rgba(255, 255, 255, 0.35) 0%, transparent 65%)`
      }
    })
  }

  const handleMouseEnter = () => {
    if (cardRef.current) {
      cardRef.current.style.transition =
        'transform 0.12s ease-out, box-shadow 0.25s ease'
    }
  }

  const handleMouseLeave = () => {
    if (rafRef.current) cancelAnimationFrame(rafRef.current)
    if (cardRef.current) {
      cardRef.current.style.transition =
        'transform 0.45s cubic-bezier(0.16, 1, 0.3, 1), box-shadow 0.3s ease'
      cardRef.current.style.transform =
        'perspective(1000px) translateY(0) scale(1) rotateX(0deg) rotateY(0deg)'
    }
    if (glareRef.current) {
      glareRef.current.style.opacity = '0'
    }
  }

  return (
    <div
      ref={cardRef}
      className={`wallpaper-card ${wallpaper.activeOn ? 'active-target' : ''}`}
      onMouseMove={handleMouseMove}
      onMouseEnter={handleMouseEnter}
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
        <div ref={glareRef} className="card-glare"></div>
        {wallpaper.activeOn && (
          <span className="badge-active">✓ {wallpaper.activeOn}</span>
        )}
      </div>
    </div>
  )
}
