import React, { useState, useEffect, useRef } from 'react'
import { convertFileSrc } from '@tauri-apps/api/core'
import { Chip } from '@mui/material'
import CheckRounded from '@mui/icons-material/CheckRounded'
import PlayArrowRounded from '@mui/icons-material/PlayArrowRounded'
import ImageNotSupportedRounded from '@mui/icons-material/ImageNotSupportedRounded'
import type { WallpaperItem } from '../data/wallpapers'
import { outputLabel } from '../hooks/useOutputs'

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
  /** The card under the pointer, or null when it leaves. */
  onHover: (id: string | null) => void
  onSelect: (wallpaper: WallpaperItem) => void
}

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
  onHover,
  onSelect,
}) => {
  const cardRef = useRef<HTMLDivElement>(null)
  const glareRef = useRef<HTMLDivElement>(null)
  const rafRef = useRef<number | null>(null)

  // 优先加载 thumb（静态缩略图或视频提取帧），视频源路径仅用于壁纸应用
  const mediaSource = toMediaSrc(wallpaper.thumb || wallpaper.path || '')
  const [imgSrc, setImgSrc] = useState(mediaSource)
  const [broken, setBroken] = useState(false)
  // One state, not two. Splitting "the skeleton may go" (`loaded`) from "the
  // entrance may start" (`entering`) let a render land between them: the
  // skeleton was already removed while the image had not begun fading in, so
  // the card's own light background showed through for a frame — the white
  // flash. Everything here now advances in a single transition.
  //
  // `placed` is set only once the bitmap is decoded and actually paintable, so
  // the skeleton never leaves before there are pixels to replace it.
  const [requested, setRequested] = useState(!lazyLoad)
  // Decoded and paintable: the entrance may start.
  const [placed, setPlaced] = useState(false)

  // Stagger by position within the page so a fresh page arrives as a wave.
  const staggerMs = lazyLoad ? Math.min(index, 12) * 45 : 0

  useEffect(() => {
    setImgSrc(toMediaSrc(wallpaper.thumb || wallpaper.path || ''))
    setPlaced(false)
    setBroken(false)
  }, [wallpaper.thumb, wallpaper.path])

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
        glare.style.setProperty('--glare-x', `${percentX.toFixed(1)}%`)
        glare.style.setProperty('--glare-y', `${percentY.toFixed(1)}%`)
      }
    })
  }

  const handleMouseEnterCard = () => {
    onHover(wallpaper.id)
    if (cardRef.current) {
      cardRef.current.style.transition =
        'transform 0.12s ease-out, box-shadow 0.25s ease, filter 0.3s ease, opacity 0.3s ease'
    }
  }

  const handleMouseLeaveCard = () => {
    onHover(null)
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
      onClick={() => onSelect(wallpaper)}
    >
      <div
        className="card-media"
        style={{
          aspectRatio: wallpaper.aspect || '16 / 9',
          // Never a fill of its own: the card surface is pale in the light
          // theme, and any gap between it and the image reads as a flash.
          background: 'transparent',
        }}
      >
        {/* 骨架占位：图片解码完成前铺满卡片。它与图片的显隐由同一个 `placed`
            翻转驱动，因此不存在"骨架已撤、图片未现"的中间帧。 */}

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
          onError={() => setBroken(true)}
          // The entrance is scale only. `opacity` is used here purely as an
          // on/off gate for "decoded yet" — it is 0 or 1, never animated, so
          // the image is never shown blended with the card behind it.
          className={
            lazyLoad && placed ? 'card-enter' : undefined
          }
          style={{
            position: 'relative',
            zIndex: 1,
            width: '100%',
            height: '100%',
            objectFit: 'cover',
            display: 'block',
            borderRadius: 'inherit',
            opacity: lazyLoad && !placed ? 0 : 1,
            animationDelay: lazyLoad ? `${staggerMs}ms` : undefined,
          }}
        />

        <div ref={glareRef} className="card-glare"></div>

        {broken && (
          <ImageNotSupportedRounded
            sx={{ position: 'absolute', inset: 0, m: 'auto', fontSize: 40, color: 'text.secondary', zIndex: 2 }}
          />
        )}

        {wallpaper.kind === 'video' && (
          <Chip
            size="small"
            icon={<PlayArrowRounded />}
            label={wallpaper.fps ? `视频 · ${wallpaper.fps}` : '视频'}
            sx={{ position: 'absolute', bottom: 8, left: 8, zIndex: 6, bgcolor: 'md.inverse_surface', color: 'md.inverse_on_surface', '& .MuiChip-icon': { color: 'inherit' } }}
          />
        )}

        {wallpaper.activeOn && (
          <Chip
            size="small"
            color="primary"
            icon={<CheckRounded />}
            label={outputLabel(wallpaper.activeOn)}
            sx={{ position: 'absolute', top: 10, right: 10, zIndex: 6 }}
          />
        )}
      </div>
    </div>
  )
}

export const WallpaperCard = React.memo(WallpaperCardComponent)
