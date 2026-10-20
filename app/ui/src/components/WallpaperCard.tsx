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
  // One state, not two. Splitting "the skeleton may go" (`loaded`) from "the
  // entrance may start" (`entering`) let a render land between them: the
  // skeleton was already removed while the image had not begun fading in, so
  // the card's own light background showed through for a frame — the white
  // flash. Everything here now advances in a single transition.
  //
  // `placed` is set only once the bitmap is decoded and actually paintable, so
  // the skeleton never leaves before there are pixels to replace it.
  const [requested, setRequested] = useState(!lazyLoad)
  const [placed, setPlaced] = useState(false)

  // Stagger by position within the page so a fresh page arrives as a wave.
  const staggerMs = lazyLoad ? Math.min(index, 12) * 45 : 0

  useEffect(() => {
    setImgSrc(toMediaSrc(wallpaper.thumb || wallpaper.path || ''))
    setPlaced(false)
  }, [wallpaper.thumb, wallpaper.path])

  // Turning the feature off has to reveal everything already on screen, not
  // wait for the next scroll.
  useEffect(() => {
    if (!lazyLoad) setRequested(true)
  }, [lazyLoad])

  // Wait for the decoded bitmap, not merely `load`.
  //
  // `load` means the bytes arrived; with `decoding="async"` the browser may
  // still not have produced a paintable frame, so starting the fade there
  // shows background through a partially decoded image. `decode()` resolves
  // when the frame is ready, and two rAFs guarantee it has been painted before
  // the skeleton is taken away.
  useEffect(() => {
    if (!requested || placed) return
    const img = cardRef.current?.querySelector('img')
    if (!img) return

    let cancelled = false
    let raf = 0
    const reveal = () => {
      if (cancelled) return
      // First frame: the decoded image is committed. Second frame: the browser
      // has actually drawn it, so removing the skeleton cannot expose a gap.
      raf = requestAnimationFrame(() => {
        raf = requestAnimationFrame(() => {
          if (!cancelled) setPlaced(true)
        })
      })
    }

    if (img.complete && img.naturalWidth > 0) {
      reveal()
    } else {
      const onLoad = () => reveal()
      img.addEventListener('load', onLoad, { once: true })
      return () => {
        cancelled = true
        img.removeEventListener('load', onLoad)
        if (raf) cancelAnimationFrame(raf)
      }
    }

    return () => {
      cancelled = true
      if (raf) cancelAnimationFrame(raf)
    }
  }, [requested, placed, imgSrc])

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
        {/* 骨架占位：图片解码完成前铺满卡片。它与图片的显隐由同一个 `placed`
            翻转驱动，因此不存在"骨架已撤、图片未现"的中间帧。 */}
        {lazyLoad && !placed && (
          <div
            aria-hidden
            data-skeleton
            style={{
              position: 'absolute',
              inset: 0,
              // A trough-then-highlight sweep, not a flat fill: a card that is
              // merely light reads as empty (and, at a glance, as a white
              // flash), while a moving sheen reads as "loading".
              background:
                'linear-gradient(100deg, transparent 20%, rgba(255,255,255,0.55) 45%, rgba(255,255,255,0.55) 55%, transparent 80%)',
              backgroundSize: '220% 100%',
              backgroundColor: 'var(--m3-surface-subtle)',
              animation:
                'card-placeholder-sweep 1.4s ease-in-out infinite',
            }}
          />
        )}

        {/* 全量采用轻量级高清预览缩略图，视频文件只在应用到壁纸时才由后台守护进程解码。
            图片在整个生命周期内始终挂载：卸载再挂载会重新解码，那一帧本身就是白屏。 */}
        <img
          // No `src` until the card is near the viewport. The element stays
          // mounted (so the reveal is a style change, not a re-mount that
          // would re-decode and flash), but an absent src issues no request.
          src={requested ? imgSrc : undefined}
          alt={wallpaper.name}
          // Our own observer already gates the fetch, so asking the browser to
          // defer again would only add latency.
          loading="eager"
          decoding="async"
          onError={() => {
            setImgSrc(FALLBACK_THUMB)
          }}
          // A keyframes animation, not a transition on toggled inline styles:
          // the transition form ran from a start value that was yanked in the
          // same frame it ended, so the last step jumped.
          className={lazyLoad && placed ? 'card-enter' : undefined}
          style={{
            // Hidden until decoded, so it cannot paint a blank or partially
            // decoded frame of its own over the skeleton.
            visibility: lazyLoad && !placed ? 'hidden' : undefined,
            width: '100%',
            height: '100%',
            objectFit: 'cover',
            display: 'block',
            borderRadius: 'inherit',
            // Locked to the animation's start frame during the stagger delay,
            // so the card cannot flash at full opacity first.
            animationDelay: lazyLoad ? `${staggerMs}ms` : undefined,
          }}
        />

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
