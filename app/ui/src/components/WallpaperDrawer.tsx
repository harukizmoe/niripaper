import { useState } from 'react'
import {
  Box,
  Button,
  Chip,
  Divider,
  Drawer,
  IconButton,
  MenuItem,
  Select,
  Stack,
  Tooltip,
  Typography,
} from '@mui/material'
import CloseRounded from '@mui/icons-material/CloseRounded'
import { convertFileSrc } from '@tauri-apps/api/core'
import type { WallpaperItem } from '../data/wallpapers'
import { useConfig } from '../hooks/useConfig'
import type { OutputInfo } from '../hooks/useOutputs'
import { NumberInput, SliderField } from './fields'
import { OutputSelect } from './OutputSelect'
import { SettingRow } from './settings'

function toMediaSrc(pathOrUrl: string): string {
  if (!pathOrUrl) return ''
  if (/^(https?:|data:)/.test(pathOrUrl)) return pathOrUrl
  try {
    return convertFileSrc(pathOrUrl)
  } catch {
    return pathOrUrl
  }
}

const FIT_LABELS: Record<string, string> = {
  fill: '填满，裁掉多出的部分',
  fit: '完整显示，留出边',
  stretch: '拉伸铺满',
  center: '居中，不缩放',
  tile: '平铺',
}

interface WallpaperDrawerProps {
  isOpen: boolean
  wallpaper: WallpaperItem | null
  source: 'wallhaven' | 'local'
  outputs: OutputInfo[]
  target: string
  onTargetChange: (target: string) => void
  onClose: () => void
  onApply: (params: { fit: string }) => void
  onSaveToLocal: (wallpaper: WallpaperItem) => void
  onSearchTag: (tag: string) => void
  onNotify: (msg: string) => void
}

export function WallpaperDrawer({ isOpen, wallpaper, ...rest }: WallpaperDrawerProps) {
  // Keep showing the last wallpaper while the drawer slides shut.
  const [shown, setShown] = useState(wallpaper)
  if (wallpaper && wallpaper !== shown) setShown(wallpaper)

  return (
    <Drawer
      anchor="right"
      open={isOpen}
      onClose={rest.onClose}
      slotProps={{ paper: { sx: { width: 440, maxWidth: '100%' } } }}
    >
      {/* Keyed so the fit starts over for each wallpaper. */}
      {shown && <DrawerBody key={shown.id} wallpaper={shown} {...rest} />}
    </Drawer>
  )
}

const FILTER_KEYS = [
  'filter.brightness',
  'filter.contrast',
  'filter.saturation',
  'filter.vibrance',
  'filter.temperature',
  'filter.hue',
  'filter.vignette',
  'filter.grain',
  'filter.grain_size',
  'filter.aberration',
  'filter.tone_shadow',
  'filter.tone_highlight',
  'filter.tone_amount',
]

function DrawerBody({
  wallpaper: wp,
  source,
  outputs,
  target,
  onTargetChange,
  onClose,
  onApply,
  onSaveToLocal,
  onSearchTag,
  onNotify,
}: Omit<WallpaperDrawerProps, 'isOpen' | 'wallpaper'> & { wallpaper: WallpaperItem }) {
  const config = useConfig(onNotify)
  // Until the user picks one, the fit the daemon is already using.
  const [pickedFit, setPickedFit] = useState<string | null>(null)
  const fits = (() => {
    const kind = config.key('fit')?.kind
    return kind?.type === 'enum' ? kind.options : Object.keys(FIT_LABELS)
  })()
  const fit = pickedFit ?? (config.ready ? config.value<string>('fit') : 'fill')

  const num = (key: string) => config.value<number>(key) ?? (config.key(key)?.default as number)
  const brightness = num('filter.brightness')
  const contrast = num('filter.contrast')
  const saturation = num('filter.saturation')
  const vibrance = num('filter.vibrance')
  const temperature = num('filter.temperature')
  const hue = num('filter.hue')
  const vignette = num('filter.vignette')
  const grain = num('filter.grain')
  const grainSize = num('filter.grain_size')
  const aberration = num('filter.aberration')
  const toneShadow = num('filter.tone_shadow')
  const toneHighlight = num('filter.tone_highlight')
  const toneAmount = num('filter.tone_amount')

  const anyFilterSet = FILTER_KEYS.some((k) => config.isSet(k))
  const resetAllFilters = () => FILTER_KEYS.forEach((k) => void config.reset(k))

  // Same tolerance for every key: a preset matches when nothing is visibly off.
  const currentPreset =
    config.presets.find((p) =>
      FILTER_KEYS.every((k) => Math.abs((p.values[k] ?? 0) - num(k)) < 0.015 * Math.max(1, Math.abs(p.values[k] ?? 0))),
    )?.id ?? 'custom'

  const applyPreset = (presetId: string) => {
    const preset = config.presets.find((p) => p.id === presetId)
    if (!preset) return
    if (preset.id === 'none') {
      resetAllFilters()
      return
    }
    FILTER_KEYS.forEach((k) => void config.set(k, preset.values[k]))
  }

  const sepia = temperature > 0 ? temperature * 0.35 : 0
  const effectiveSat = Math.max(0, saturation * (1 + (vibrance - 1) * 0.5))
  const cssFilter = `brightness(${brightness}) contrast(${contrast}) saturate(${effectiveSat}) hue-rotate(${hue}deg) sepia(${sepia})`

  const playlistItems = config.value<string[]>('playlist.items') ?? []
  const currentPath = wp.path || ''
  const inPlaylist = currentPath ? playlistItems.includes(currentPath) : false

  const togglePlaylist = () => {
    if (!currentPath) {
      onNotify('无法加入轮播：当前壁纸无本地文件，请先保存到本地')
      return
    }
    const nextItems = inPlaylist
      ? playlistItems.filter((p) => p !== currentPath)
      : [...playlistItems, currentPath]
    void config.set('playlist.items', nextItems)
    onNotify(inPlaylist ? '已从轮播列表中移出' : '已加入轮播列表')
  }

  return (
    <Stack sx={{ height: '100%' }}>
      <Stack direction="row" sx={{ alignItems: 'center', px: 3, pt: 2.5, pb: 1.5 }}>
        <Typography variant="h6" component="h2" sx={{ flex: 1, minWidth: 0, wordBreak: 'break-word' }}>
          {wp.name}
        </Typography>
        <IconButton onClick={onClose} aria-label="关闭">
          <CloseRounded />
        </IconButton>
      </Stack>

      <Stack spacing={2.5} sx={{ flex: 1, minHeight: 0, overflowY: 'auto', px: 3, pb: 2 }}>
        <Box
          sx={{
            position: 'relative',
            borderRadius: 3,
            overflow: 'hidden',
            bgcolor: 'md.surface_container',
            aspectRatio: '16 / 9',
          }}
        >
          {wp.kind === 'video' ? (
            <Box
              component="video"
              src={toMediaSrc(wp.path || wp.thumb)}
              controls
              autoPlay
              muted
              loop
              playsInline
              sx={{
                width: '100%',
                height: '100%',
                objectFit: 'contain',
                display: 'block',
                filter: cssFilter,
              }}
            />
          ) : (
            <Box
              component="img"
              src={toMediaSrc(wp.full_url || wp.path || wp.thumb)}
              alt={wp.name}
              sx={{
                width: '100%',
                height: '100%',
                objectFit: 'contain',
                display: 'block',
                filter: cssFilter,
              }}
            />
          )}
          {vignette > 0 && (
            <Box
              sx={{
                position: 'absolute',
                inset: 0,
                pointerEvents: 'none',
                background: `radial-gradient(circle, transparent ${40 * (1 - vignette * 0.5)}%, rgba(0,0,0,${vignette * 0.85}) 100%)`,
              }}
            />
          )}
        </Box>

        <Box>
          <Typography variant="body2" sx={{ color: 'text.secondary' }}>
            {source === 'wallhaven'
              ? [wp.res, wp.ratio, wp.favs && `${wp.favs} 收藏`, wp.category, wp.purity].filter(Boolean).join(' · ')
              : [wp.res, wp.kind === 'video' ? '视频壁纸' : '图片壁纸'].filter(Boolean).join(' · ')}
          </Typography>
          {source === 'wallhaven' && (
            <Typography variant="body2" sx={{ color: 'text.secondary' }}>
              Wallhaven #{wp.id}
            </Typography>
          )}
        </Box>

        {source === 'wallhaven' && wp.colors && (
          <Stack direction="row" spacing={1}>
            {wp.colors.map((c) => (
              <Tooltip key={c} title={`搜索颜色 ${c}`}>
                <Box
                  component="button"
                  aria-label={`搜索颜色 ${c}`}
                  onClick={() => {
                    onClose()
                    onSearchTag(c)
                  }}
                  sx={{
                    width: 32,
                    height: 32,
                    p: 0,
                    borderRadius: '50%',
                    border: 1,
                    borderColor: 'divider',
                    bgcolor: c,
                    cursor: 'pointer',
                  }}
                />
              </Tooltip>
            ))}
          </Stack>
        )}

        {source === 'wallhaven' && wp.tags && wp.tags.length > 0 && (
          <Stack direction="row" sx={{ flexWrap: 'wrap', gap: 0.75 }}>
            {wp.tags.map((t) => (
              <Chip
                key={t}
                label={t}
                variant="outlined"
                onClick={() => {
                  onClose()
                  onSearchTag(t)
                }}
              />
            ))}
          </Stack>
        )}

        {source === 'local' && wp.path && (
          <Typography
            variant="body2"
            sx={{
              fontFamily: 'monospace',
              wordBreak: 'break-all',
              p: 1.5,
              borderRadius: 2,
              bgcolor: 'md.surface_container',
              color: 'text.secondary',
            }}
          >
            {wp.path}
          </Typography>
        )}

        <Divider />

        <Box>
          <SettingRow
            label="设置到"
            description="这张壁纸用在哪个显示器上。"
            control={<OutputSelect outputs={outputs} value={target} onChange={onTargetChange} />}
          />
          <SettingRow
            label="填充方式"
            description="壁纸与画布比例不同时的放置方式。"
            control={
              <Select
                size="small"
                value={fit}
                onChange={(e) => setPickedFit(e.target.value)}
                inputProps={{ 'aria-label': '填充方式' }}
                sx={{ minWidth: 180 }}
              >
                {fits.map((f) => (
                  <MenuItem key={f} value={f}>
                    {FIT_LABELS[f] ?? f}
                  </MenuItem>
                ))}
              </Select>
            }
          />
          {wp.kind === 'video' && config.ready && (
            <SettingRow
              label="视频帧率上限"
              description="0 表示使用视频自身的帧率。下次载入视频时生效，对所有视频壁纸有效。"
              modified={config.isSet('video_fps')}
              onReset={() => void config.reset('video_fps')}
              control={
                <NumberInput
                  value={config.value<number>('video_fps')}
                  min={0}
                  max={240}
                  step={1}
                  unit="fps"
                  onCommit={(v) => void config.set('video_fps', v)}
                />
              }
            />
          )}
        </Box>

        <Divider />

        <Box>
          <Stack direction="row" sx={{ alignItems: 'center', justifyContent: 'space-between', mb: 1 }}>
            <Typography variant="subtitle2" sx={{ fontWeight: 600 }}>
              后期滤镜
            </Typography>
            {anyFilterSet && (
              <Button size="small" variant="text" onClick={resetAllFilters}>
                恢复默认
              </Button>
            )}
          </Stack>

          <SettingRow
            label="滤镜预设"
            description="一键应用风格化色调，支持在下方微调。"
            control={
              <Select
                size="small"
                value={currentPreset}
                onChange={(e) => applyPreset(e.target.value)}
                sx={{ minWidth: 180 }}
              >
                {config.presets.map((p) => (
                  <MenuItem key={p.id} value={p.id}>
                    {p.label}
                  </MenuItem>
                ))}
                {currentPreset === 'custom' && (
                  <MenuItem value="custom" disabled>
                    自定义微调
                  </MenuItem>
                )}
              </Select>
            }
          />

          <SettingRow
            label="亮度"
            modified={config.isSet('filter.brightness')}
            onReset={() => void config.reset('filter.brightness')}
          >
            <SliderField
              value={brightness}
              min={0.5}
              max={1.5}
              step={0.01}
              unit="×"
              onCommit={(v) => void config.set('filter.brightness', v)}
            />
          </SettingRow>

          <SettingRow
            label="对比度"
            modified={config.isSet('filter.contrast')}
            onReset={() => void config.reset('filter.contrast')}
          >
            <SliderField
              value={contrast}
              min={0.5}
              max={1.6}
              step={0.01}
              unit="×"
              onCommit={(v) => void config.set('filter.contrast', v)}
            />
          </SettingRow>

          <SettingRow
            label="饱和度"
            modified={config.isSet('filter.saturation')}
            onReset={() => void config.reset('filter.saturation')}
          >
            <SliderField
              value={saturation}
              min={0.0}
              max={2.0}
              step={0.01}
              unit="×"
              onCommit={(v) => void config.set('filter.saturation', v)}
            />
          </SettingRow>

          <SettingRow
            label="自然饱和度"
            description="提高平淡颜色的鲜艳度，同时避免肤色和高光过饱和。"
            modified={config.isSet('filter.vibrance')}
            onReset={() => void config.reset('filter.vibrance')}
          >
            <SliderField
              value={vibrance}
              min={0.0}
              max={2.0}
              step={0.01}
              unit="×"
              onCommit={(v) => void config.set('filter.vibrance', v)}
            />
          </SettingRow>

          <SettingRow
            label="色温"
            description="偏负为清冷电影蓝绿，偏正为暖阳黄金落日。"
            modified={config.isSet('filter.temperature')}
            onReset={() => void config.reset('filter.temperature')}
          >
            <SliderField
              value={temperature}
              min={-1.0}
              max={1.0}
              step={0.02}
              onCommit={(v) => void config.set('filter.temperature', v)}
            />
          </SettingRow>

          <SettingRow
            label="色相偏移"
            modified={config.isSet('filter.hue')}
            onReset={() => void config.reset('filter.hue')}
          >
            <SliderField
              value={hue}
              min={0.0}
              max={360.0}
              step={1}
              unit="°"
              onCommit={(v) => void config.set('filter.hue', v)}
            />
          </SettingRow>

          <SettingRow
            label="电影暗角"
            modified={config.isSet('filter.vignette')}
            onReset={() => void config.reset('filter.vignette')}
          >
            <SliderField
              value={vignette}
              min={0.0}
              max={1.0}
              step={0.01}
              onCommit={(v) => void config.set('filter.vignette', v)}
            />
          </SettingRow>

          <SettingRow
            label="颗粒大小"
            modified={config.isSet('filter.grain_size')}
            onReset={() => void config.reset('filter.grain_size')}
          >
            <SliderField
              value={grainSize}
              min={1}
              max={4}
              step={0.1}
              unit="px"
              onCommit={(v) => void config.set('filter.grain_size', v)}
            />
          </SettingRow>

          <SettingRow
            label="分离色调强度"
            description="给暗部和亮部各染一点色，不改变整体亮度。"
            modified={config.isSet('filter.tone_amount')}
            onReset={() => void config.reset('filter.tone_amount')}
          >
            <SliderField
              value={toneAmount}
              min={0}
              max={1}
              step={0.01}
              onCommit={(v) => void config.set('filter.tone_amount', v)}
            />
          </SettingRow>

          <SettingRow
            label="暗部色相"
            modified={config.isSet('filter.tone_shadow')}
            onReset={() => void config.reset('filter.tone_shadow')}
          >
            <SliderField
              value={toneShadow}
              min={0}
              max={360}
              step={1}
              unit="°"
              disabled={toneAmount <= 0.001}
              onCommit={(v) => void config.set('filter.tone_shadow', v)}
            />
          </SettingRow>

          <SettingRow
            label="亮部色相"
            modified={config.isSet('filter.tone_highlight')}
            onReset={() => void config.reset('filter.tone_highlight')}
          >
            <SliderField
              value={toneHighlight}
              min={0}
              max={360}
              step={1}
              unit="°"
              disabled={toneAmount <= 0.001}
              onCommit={(v) => void config.set('filter.tone_highlight', v)}
            />
          </SettingRow>

          <SettingRow
            label="边缘色散"
            description="屏幕四周的光学棱镜色差，呈现电影镜头感。"
            modified={config.isSet('filter.aberration')}
            onReset={() => void config.reset('filter.aberration')}
          >
            <SliderField
              value={aberration}
              min={0.0}
              max={1.0}
              step={0.01}
              onCommit={(v) => void config.set('filter.aberration', v)}
            />
          </SettingRow>

          <SettingRow
            label="胶片颗粒"
            modified={config.isSet('filter.grain')}
            onReset={() => void config.reset('filter.grain')}
          >
            <SliderField
              value={grain}
              min={0.0}
              max={1.0}
              step={0.01}
              onCommit={(v) => void config.set('filter.grain', v)}
            />
          </SettingRow>
        </Box>
      </Stack>

      <Stack spacing={1.25} sx={{ px: 3, py: 2, borderTop: 1, borderColor: 'divider' }}>
        <Button variant="contained" size="large" onClick={() => onApply({ fit })}>
          设为壁纸
        </Button>
        {currentPath && (
          <Button variant="outlined" onClick={togglePlaylist}>
            {inPlaylist ? '从轮播列表中移出' : '加入轮播列表'}
          </Button>
        )}
        {source === 'wallhaven' && (
          <Button variant="contained" color="secondary" onClick={() => onSaveToLocal(wp)}>
            保存到本地库
          </Button>
        )}
      </Stack>
    </Stack>
  )
}
