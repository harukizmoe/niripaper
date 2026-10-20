import React, { useState, useEffect, useRef } from 'react'
import { convertFileSrc } from '@tauri-apps/api/core'
import type { WallpaperItem } from '../data/wallpapers'

interface WallpaperCardProps {
  wallpaper: WallpaperItem
  isAnyHovered: boolean
  isHovered: boolean
  enableBlur?: boolean
  /// Defer loading the preview until the card approaches the viewport, and
  /// fade it in when it arrives. Off means eager load with no animation.
  lazyLoad?: boolean
  /// Position within its column, used only to stagger the entrance animation.
  index?: number
  /// The scrolling ancestor. The observer must be rooted here, not in the
  /// window: with `root: null` the browser measures against the viewport and
  /// treats content clipped by an inner scroller as visible, so every card in
  /// the page loads at once and the feature does nothing.
  scrollRoot?: React.RefObject<HTMLElement | null>
  onMouseEnter: () => void
  onMouseLeave: () => void
  onClick: () => void
}

const FALLBACK_THUMB =
  'https://images.unsplash.com/photo-1506744038136-46273834b3fb?w=600&auto=format&fit=crop&q=80'

/// How far outside the scroll viewport a card starts loading. Scrolling one
/// row takes far more than this, so the next row is always ready before it
/// arrives, while the rest of the page stays unloaded.
const PRELOAD_MARGIN = '200px 0px'

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
  enableBlur = true,
  lazyLoad = true,
  index = 0,
  scrollRoot,
  onMouseEnter,
  onMouseLeave,
  onClick,
}) => {
  const cardRef = useRef<HTMLDivElement>(null)
  const glareRef = useRef<HTMLDivElement>(null)
  const rafRef = useRef<number | null>(null)

  // 优先加载 thumb（静态缩略图或视频提取帧），视频源路径仅用于壁纸应用
  const mediaSource = toMediaSrc(wallpaper.thumb || wallpaper.path || '')
  const [imgSrc, setImgSrc] = useState(mediaSource)
  // `requested` means "the browser may fetch this now"; `loaded` means the
  // pixels are there and the entrance may start.
  const [requested, setRequested] = useState(!lazyLoad)
  const [loaded, setLoaded] = useState(false)
  // Runs the one-shot entrance. Kept as state rather than derived from
  // `loaded` so the class can be dropped once the animation is over: a
  // lingering `animation` would keep overriding the hover zoom.
  const [entering, setEntering] = useState(false)

  // Stagger by position within the page so a fresh page arrives as a wave.
  const staggerMs = lazyLoad ? Math.min(index, 12) * 45 : 0

  useEffect(() => {
    setImgSrc(toMediaSrc(wallpaper.thumb || wallpaper.path || ''))
    setLoaded(false)
    setEntering(false)
  }, [wallpaper.thumb, wallpaper.path])

  // Start the entrance only when pixels exist. Beginning it at mount made the
  // image fade in while still blank, so the card showed flat background for
  // the first part of the animation — the flash this replaced.
  useEffect(() => {
    if (lazyLoad && loaded) setEntering(true)
  }, [lazyLoad, loaded])

  // `onLoad` does not fire when the image is already complete (a cache hit
  // that happens between render and the listener attaching), which would leave
  // the skeleton up forever. `complete` is the authoritative check.
  useEffect(() => {
    const img = cardRef.current?.querySelector('img')
    if (img?.complete && img.naturalWidth > 0) setLoaded(true)
  }, [requested, imgSrc])

  // `animationend` is the normal exit; this is the fallback for when no
  // animation runs at all (reduced motion) and the class would otherwise stick
  // around forever.
  useEffect(() => {
    if (!entering) return
    const timer = window.setTimeout(() => setEntering(false), staggerMs + 800)
    return () => window.clearTimeout(timer)
  }, [entering, staggerMs])

  // Turning the feature off has to reveal everything already on screen, not
  // wait for the next scroll.
  useEffect(() => {
    if (!lazyLoad) setRequested(true)
  }, [lazyLoad])

  // Defer the fetch until the card is near the viewport. The root has to be
  // the actual scrolling element: with `root: null` the browser measures
  // against the viewport and counts content clipped by an inner scroller as
  // visible, so the whole page loads at once and the feature does nothing.
  useEffect(() => {
    if (!lazyLoad || requested) return
    const el = cardRef.current
    if (!el) {
      setRequested(true)
      return
    }
    if (typeof IntersectionObserver === 'undefined') {
      setRequested(true)
      return
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setRequested(true)
          observer.disconnect()
        }
      },
      { root: scrollRoot?.current ?? null, rootMargin: PRELOAD_MARGIN }
    )
    observer.observe(el)
    return () => observer.disconnect()
  }, [lazyLoad, requested, scrollRoot])

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

  const isBlurred = enableBlur && isAnyHovered && !isHovered
  const cardFilter = isBlurred ? 'blur(3.5px) brightness(0.82)' : 'none'
  const cardOpacity = isBlurred ? 0.78 : 1

  // The entrance: nothing until the card is near the viewport, then a short
  // rise-and-settle. Staggered by column position so a freshly loaded page
  // arrives as a wave rather than a single flash. Disabled entirely when the
  // setting is off — the feature is the whole deferral *and* the animation.
  const showPlaceholder = lazyLoad && !requested

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
        {/* 骨架占位：图片未就绪时铺满卡片，图片一到达就与它交叉淡出。
            不能在图片出现后继续留在底下 —— 两张图叠着会显出扫光条纹的重影。 */}
        {lazyLoad && !loaded && (
          <div
            aria-hidden
            data-skeleton
            style={{
              position: 'absolute',
              inset: 0,
              background:
                'linear-gradient(100deg, var(--m3-surface-subtle) 30%, rgba(255,255,255,0.06) 50%, var(--m3-surface-subtle) 70%)',
              backgroundSize: '220% 100%',
              animation:
                'card-placeholder-sweep 1.5s ease-in-out infinite',
            }}
          />
        )}

        {/* 全量采用轻量级高清预览缩略图，视频文件只在应用到壁纸时才由后台守护进程解码 */}
        {!showPlaceholder && (
          <img
            src={imgSrc}
            alt={wallpaper.name}
            // With our own observer already gating the fetch, asking the
            // browser to defer again would only add latency. Without the
            // feature, native lazy loading is the better default.
            loading={lazyLoad ? 'eager' : 'lazy'}
            decoding="async"
            onLoad={() => setLoaded(true)}
            onError={() => {
              setImgSrc(FALLBACK_THUMB)
              setLoaded(true)
            }}
            // A keyframes animation, not a transition on toggled inline
            // styles: the transition form ran from a start value that was
            // yanked in the same frame it ended, so the last step jumped. An
            // animation simply ends at its final keyframe, and the class is
            // dropped afterwards so the hover zoom gets the element back.
            className={entering ? 'card-enter' : undefined}
            style={{
              width: '100%',
              height: '100%',
              objectFit: 'cover',
              display: 'block',
              borderRadius: 'inherit',
              // Held back until called for, so the image cannot flash at full
              // opacity before the animation's start frame is painted.
              animationDelay: entering ? `${staggerMs}ms` : undefined,
            }}
            onAnimationEnd={() => setEntering(false)}
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
