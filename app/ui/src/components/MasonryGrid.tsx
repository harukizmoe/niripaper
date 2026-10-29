import { useEffect, useRef, useState, type ReactNode, type RefObject } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'

// A waterfall of cards that only keeps the ones near the viewport mounted, so
// the page costs the same whether the list holds twenty wallpapers or twenty
// thousand. Heights are known up front (every item carries its aspect ratio
// and a card is exactly its picture), which is what makes laying out items
// that are not on screen possible at all.

const GAP = 20

/// Columns by available width.
const columnsFor = (width: number) => (width < 720 ? 2 : width < 1120 ? 3 : 4)

/// "1920 / 1080" (or "16:9") as a number; a malformed one is treated as 16:9.
function aspectOf(aspect: string): number {
  const m = aspect.match(/([\d.]+)\s*[/:]\s*([\d.]+)/)
  const ratio = m ? Number(m[1]) / Number(m[2]) : NaN
  return Number.isFinite(ratio) && ratio > 0 ? ratio : 16 / 9
}

export function MasonryGrid<T extends { id: string; aspect: string }>({
  items,
  scrollRef,
  renderItem,
  canLoadMore,
  onLoadMore,
}: {
  items: T[]
  /** The element that scrolls; the grid measures and listens to it. */
  scrollRef: RefObject<HTMLElement | null>
  /** `lane` is the column the item landed in. */
  renderItem: (item: T, lane: number) => ReactNode
  /** More can be fetched right now (there is more, and nothing is in flight). */
  canLoadMore?: boolean
  onLoadMore?: () => void
}) {
  const gridRef = useRef<HTMLDivElement>(null)
  const [box, setBox] = useState({ width: 0, top: 0 })

  useEffect(() => {
    const el = gridRef.current
    if (!el) return
    const measure = () => setBox({ width: el.clientWidth, top: el.offsetTop })
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(el)
    return () => observer.disconnect()
  }, [])

  const lanes = columnsFor(box.width)
  const colWidth = Math.max(0, (box.width - GAP * (lanes - 1)) / lanes)

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) => colWidth / aspectOf(items[i].aspect),
    getItemKey: (i) => items[i].id,
    lanes,
    gap: GAP,
    overscan: 8,
    scrollMargin: box.top,
  })

  // The sizes are estimates from the width; a new width makes them all stale.
  useEffect(() => virtualizer.measure(), [virtualizer, colWidth, lanes])

  const virtualItems = virtualizer.getVirtualItems()
  const lastIndex = virtualItems.length > 0 ? virtualItems[virtualItems.length - 1].index : -1
  // Ask for more while the last few rows are in (or near) view. The caller
  // turns `canLoadMore` off while it fetches, which is what stops this from
  // asking again before the answer is in.
  useEffect(() => {
    if (canLoadMore && lastIndex >= items.length - lanes * 3) onLoadMore?.()
  }, [canLoadMore, lastIndex, items.length, lanes, onLoadMore])

  return (
    <div ref={gridRef} style={{ position: 'relative', width: '100%', height: virtualizer.getTotalSize() }}>
      {box.width > 0 &&
        virtualItems.map((vi) => (
          <div
            key={vi.key}
            style={{
              position: 'absolute',
              // Not a transform: that would make this a stacking context and
              // trap a hovered card underneath its later neighbours.
              top: vi.start - box.top,
              left: vi.lane * (colWidth + GAP),
              width: colWidth,
            }}
          >
            {renderItem(items[vi.index], vi.lane)}
          </div>
        ))}
    </div>
  )
}
