import React, { useState, useEffect, useRef } from 'react'
import { convertFileSrc } from '@tauri-apps/api/core'
import type { WallpaperItem } from '../data/wallpapers'

interface WallpaperCardProps {
  wallpaper: WallpaperItem
  isAnyHovered: boolean
  isHovered: boolean
  onMouseEnter: () => void
  onMouseLeave: () => void
  onClick: () => void
}

const FALLBACK_THUMB =
  'https://images.unsplash.com/photo-1506744038136-46273834b3fb?w=600&auto=format&fit=crop&q=80'

function toMediaSrc(pathOrUrl: string): string {
  if (!pathOrUrl) return ''
  if (
    pathOrUrl.startsWith('http://') ||
    pathOrUrl.startsWith('https://') ||
    pathOrUrl.startsWith('data:')
  ) {
    return pathOrUrl
  }
  try {
    return convertFileSrc(pathOrUrl)
  } catch {
    return pathOrUrl
  }
}

const WallpaperCardComponent: React.FC<WallpaperCardProps> = ({
  wallpaper,
  isAnyHovered,
  isHovered,
  onMouseEnter,
  onMouseLeave,
  onClick,
}) => {
  const cardRef = useRef<HTMLDivElement>(null)
  const glareRef = useRef<HTMLDivElement>(null)
  const rafRef = useRef<number | null>(null)

  const mediaSource = toMediaSrc(wallpaper.path || wallpaper.thumb)
  const [imgSrc, setImgSrc] = useState(mediaSource)
  useEffect(() => {
    setImgSrc(toMediaSrc(wallpaper.path || wallpaper.thumb))
  }, [wallpaper.path, wallpaper.thumb])

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

  const handleMouseEnterCard = () => {
    onMouseEnter()
    if (cardRef.current) {
      cardRef.current.style.transition =
        'transform 0.12s ease-out, box-shadow 0.25s ease, filter 0.3s ease, opacity 0.3s ease'
    }
  }

  const handleMouseLeaveCard = () => {
    onMouseLeave()
    if (rafRef.current) cancelAnimationFrame(rafRef.current)
    if (cardRef.current) {
      cardRef.current.style.transition =
        'transform 0.45s cubic-bezier(0.16, 1, 0.3, 1), box-shadow 0.3s ease, filter 0.3s ease, opacity 0.3s ease'
      cardRef.current.style.transform =
        'perspective(1000px) translateY(0) scale(1) rotateX(0deg) rotateY(0deg)'
    }
    if (glareRef.current) {
      glareRef.current.style.opacity = '0'
    }
  }

  const isBlurred = isAnyHovered && !isHovered
  const cardFilter = isBlurred ? 'blur(3.5px) brightness(0.82)' : 'none'
  const cardOpacity = isBlurred ? 0.78 : 1

  return (
    <div
      ref={cardRef}
      className={`wallpaper-card ${wallpaper.activeOn ? 'active-target' : ''}`}
      style={{
        filter: cardFilter,
        opacity: cardOpacity,
        zIndex: isHovered ? 25 : 1,
        transition:
          'transform 0.15s ease-out, box-shadow 0.25s ease, filter 0.3s cubic-bezier(0.2, 0, 0, 1), opacity 0.3s ease',
      }}
      onMouseMove={handleMouseMove}
      onMouseEnter={handleMouseEnterCard}
      onMouseLeave={handleMouseLeaveCard}
      onClick={onClick}
    >
      <div
        className="card-media"
        style={{
          aspectRatio: wallpaper.aspect || '16 / 9',
          background: 'var(--m3-surface-subtle)',
        }}
      >
        {/* 视频壁纸优化：默认呈现轻量海报卡片，悬停时才激活真实视频解码循环，避免17路解码器并行卡死滚动 */}
        {wallpaper.kind === 'video' ? (
          <div style={{ position: 'relative', width: '100%', height: '100%' }}>
            {isHovered ? (
              <video
                src={mediaSource}
                muted
                loop
                autoPlay
                playsInline
                style={{
                  width: '100%',
                  height: '100%',
                  objectFit: 'cover',
                  display: 'block',
                  borderRadius: 'inherit',
                }}
              />
            ) : (
              <div
                style={{
                  width: '100%',
                  height: '100%',
                  background:
                    'linear-gradient(135deg, #1e1b4b 0%, #312e81 60%, #4338ca 100%)',
                  display: 'flex',
                  flexDirection: 'column',
                  alignItems: 'center',
                  justifyContent: 'center',
                  gap: 8,
                  color: '#fff',
                  userSelect: 'none',
                }}
              >
                <span style={{ fontSize: 32, opacity: 0.9 }}>🎬</span>
                <span
                  style={{
                    fontSize: 11,
                    fontWeight: 800,
                    opacity: 0.85,
                    maxWidth: '85%',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                    fontFamily: 'var(--font-mono)',
                  }}
                >
                  {wallpaper.name}
                </span>
                <span
                  style={{
                    fontSize: 10,
                    opacity: 0.6,
                    background: 'rgba(255, 255, 255, 0.12)',
                    padding: '2px 8px',
                    borderRadius: 9999,
                  }}
                >
                  悬停以播放动效
                </span>
              </div>
            )}
          </div>
        ) : (
          <img
            src={imgSrc}
            alt={wallpaper.name}
            loading="lazy"
            decoding="async"
            onError={() => {
              setImgSrc(FALLBACK_THUMB)
            }}
            style={{
              width: '100%',
              height: '100%',
              objectFit: 'cover',
              display: 'block',
              borderRadius: 'inherit',
            }}
          />
        )}
        <div ref={glareRef} className="card-glare"></div>

        {/* 动态视频标记 */}
        {wallpaper.kind === 'video' && (
          <span
            style={{
              position: 'absolute',
              bottom: 8,
              left: 8,
              background: 'rgba(0, 0, 0, 0.75)',
              color: '#fff',
              fontSize: 10,
              fontWeight: 800,
              padding: '2px 8px',
              borderRadius: 'var(--m3-radius-full)',
              zIndex: 6,
              display: 'flex',
              alignItems: 'center',
              gap: 4,
            }}
          >
            ▶ 动态视频 {wallpaper.fps || ''}
          </span>
        )}

        {/* 生效中指示器 */}
        {wallpaper.activeOn && (
          <span className="badge-active">✓ {wallpaper.activeOn}</span>
        )}
      </div>
    </div>
  )
}

export const WallpaperCard = React.memo(WallpaperCardComponent)
