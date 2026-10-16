import React, { useRef } from 'react'
import type { WallpaperItem } from '../data/wallpapers'

interface WallpaperCardProps {
  wallpaper: WallpaperItem
  onClick: () => void
}

export const WallpaperCard: React.FC<WallpaperCardProps> = ({
  wallpaper,
  onClick,
}) => {
  const cardRef = useRef<HTMLDivElement>(null)
  const glareRef = useRef<HTMLDivElement>(null)
  const rafRef = useRef<number | null>(null)

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

      const maxTilt = 14
      const rotateY = px * maxTilt
      const rotateX = -py * maxTilt

      cardRef.current.style.transform = `perspective(1000px) translateY(-8px) scale(1.08) rotateX(${rotateX.toFixed(
        2
      )}deg) rotateY(${rotateY.toFixed(2)}deg)`

      if (glareRef.current) {
        const percentX = ((e.clientX - rect.left) / rect.width) * 100
        const percentY = ((e.clientY - rect.top) / rect.height) * 100
        glareRef.current.style.background = `radial-gradient(circle at ${percentX.toFixed(
          1
        )}% ${percentY.toFixed(1)}%, rgba(255, 255, 255, 0.35) 0%, transparent 65%)`
      }
    })
  }

  const handleMouseEnter = () => {
    if (cardRef.current) {
      cardRef.current.style.transition = 'transform 0.12s ease-out, box-shadow 0.25s ease'
    }
  }

  const handleMouseLeave = () => {
    if (rafRef.current) cancelAnimationFrame(rafRef.current)
    if (cardRef.current) {
      cardRef.current.style.transition =
        'transform 0.45s cubic-bezier(0.16, 1, 0.3, 1), box-shadow 0.3s ease, filter 0.35s ease'
      cardRef.current.style.transform =
        'perspective(1000px) translateY(0) scale(1) rotateX(0deg) rotateY(0deg)'
      setTimeout(() => {
        if (cardRef.current) {
          cardRef.current.style.transition =
            'transform 0.12s ease-out, box-shadow 0.25s ease, filter 0.35s ease'
        }
      }, 450)
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
      <div className="card-media" style={{ aspectRatio: wallpaper.aspect }}>
        <img src={wallpaper.thumb} alt={wallpaper.name} loading="lazy" />
        <div ref={glareRef} className="card-glare"></div>
        {wallpaper.activeOn && (
          <span className="badge-active">✓ {wallpaper.activeOn}</span>
        )}
      </div>
    </div>
  )
}
