import { useState } from 'react'
import {
  Avatar,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  IconButton,
  List,
  ListItem,
  ListItemAvatar,
  ListItemText,
  MenuItem,
  Select,
  Stack,
  Switch,
  Tooltip,
  Typography,
} from '@mui/material'
import CloseRounded from '@mui/icons-material/CloseRounded'
import DeleteOutlineRounded from '@mui/icons-material/DeleteOutlineRounded'
import SkipNextRounded from '@mui/icons-material/SkipNextRounded'
import ShuffleRounded from '@mui/icons-material/ShuffleRounded'
import RepeatRounded from '@mui/icons-material/RepeatRounded'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useConfig } from '../hooks/useConfig'
import { invoke } from '../hooks/useDaemon'
import { NumberInput } from './fields'
import { SettingRow } from './settings'

interface PlaylistDialogProps {
  open: boolean
  onClose: () => void
  onNotify: (msg: string) => void
}

function toMediaSrc(path: string): string {
  if (!path) return ''
  try {
    return convertFileSrc(path)
  } catch {
    return path
  }
}

export function PlaylistDialog({ open, onClose, onNotify }: PlaylistDialogProps) {
  const config = useConfig(onNotify)
  const [switching, setSwitching] = useState(false)

  const enabled = config.value<boolean>('playlist.enabled') ?? false
  const intervalMins = config.value<number>('playlist.interval_mins') ?? 15
  const order = config.value<string>('playlist.order') ?? 'rotate'
  const items = config.value<string[]>('playlist.items') ?? []

  const handleNext = async () => {
    setSwitching(true)
    try {
      await invoke('daemon_next', {})
      onNotify('已切换到下一张壁纸')
    } catch (e) {
      onNotify(`切换失败：${e}`)
    } finally {
      setSwitching(false)
    }
  }

  const handleRemove = (pathToRemove: string) => {
    const nextItems = items.filter((p) => p !== pathToRemove)
    void config.set('playlist.items', nextItems)
  }

  const handleClear = () => {
    void config.set('playlist.items', [])
    onNotify('已清空轮播列表')
  }

  const filenameOf = (p: string) => p.split('/').pop() || p

  return (
    <Dialog open={open} onClose={onClose} maxWidth="sm" fullWidth>
      <DialogTitle sx={{ m: 0, p: 2.5, display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <Typography variant="h6" component="span" sx={{ fontWeight: 600 }}>
          壁纸定时轮播
        </Typography>
        <IconButton onClick={onClose} aria-label="关闭" size="small">
          <CloseRounded />
        </IconButton>
      </DialogTitle>

      <DialogContent dividers sx={{ p: 2.5 }}>
        <Stack spacing={2}>
          <SettingRow
            label="启用定时轮播"
            description="开启后守护进程将按设定间隔自动切换壁纸。"
            control={
              <Switch
                checked={enabled}
                onChange={() => void config.set('playlist.enabled', !enabled)}
              />
            }
          />

          <SettingRow
            label="轮播间隔"
            description="每隔多长时间切换一次壁纸。"
            disabled={!enabled}
            modified={config.isSet('playlist.interval_mins')}
            onReset={() => void config.reset('playlist.interval_mins')}
            control={
              <NumberInput
                value={intervalMins}
                min={1}
                max={1440}
                step={1}
                unit="分钟"
                disabled={!enabled}
                onCommit={(v) => void config.set('playlist.interval_mins', v)}
              />
            }
          />

          <SettingRow
            label="播放顺序"
            description="按列表顺序循环或随机挑换。"
            disabled={!enabled}
            modified={config.isSet('playlist.order')}
            onReset={() => void config.reset('playlist.order')}
            control={
              <Select
                size="small"
                value={order}
                disabled={!enabled}
                onChange={(e) => void config.set('playlist.order', e.target.value)}
                sx={{ minWidth: 140 }}
              >
                <MenuItem value="rotate">
                  <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
                    <RepeatRounded fontSize="small" />
                    <span>顺序循环</span>
                  </Stack>
                </MenuItem>
                <MenuItem value="random">
                  <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
                    <ShuffleRounded fontSize="small" />
                    <span>随机播放</span>
                  </Stack>
                </MenuItem>
              </Select>
            }
          />

          <Stack direction="row" spacing={2} sx={{ pt: 1, alignItems: 'center', justifyContent: 'space-between' }}>
            <Button
              variant="outlined"
              startIcon={<SkipNextRounded />}
              disabled={switching || items.length === 0}
              onClick={handleNext}
            >
              立即切换下一张
            </Button>
            {items.length > 0 && (
              <Button size="small" color="error" onClick={handleClear}>
                清空列表
              </Button>
            )}
          </Stack>

          <Divider sx={{ my: 1 }} />

          <Box>
            <Typography variant="subtitle2" sx={{ fontWeight: 600, mb: 1 }}>
              播放列表 ({items.length} 张)
            </Typography>

            {items.length === 0 ? (
              <Typography variant="body2" sx={{ color: 'text.secondary', py: 3, textAlign: 'center' }}>
                轮播列表中暂无壁纸。在壁纸库中打开任意本地壁纸抽屉，点击「加入轮播列表」即可添加。
              </Typography>
            ) : (
              <List disablePadding sx={{ maxHeight: 280, overflowY: 'auto' }}>
                {items.map((item) => (
                  <ListItem
                    key={item}
                    secondaryAction={
                      <Tooltip title="从列表中移除">
                        <IconButton edge="end" aria-label="移除" onClick={() => handleRemove(item)}>
                          <DeleteOutlineRounded />
                        </IconButton>
                      </Tooltip>
                    }
                    sx={{ px: 1, py: 0.75, borderRadius: 2, '&:hover': { bgcolor: 'action.hover' } }}
                  >
                    <ListItemAvatar sx={{ minWidth: 56 }}>
                      <Avatar
                        variant="rounded"
                        src={toMediaSrc(item)}
                        sx={{ width: 44, height: 44, borderRadius: 1.5, bgcolor: 'md.surface_container' }}
                      />
                    </ListItemAvatar>
                    <ListItemText
                      primary={
                        <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
                          {filenameOf(item)}
                        </Typography>
                      }
                      secondary={
                        <Typography variant="caption" noWrap sx={{ color: 'text.secondary', display: 'block' }}>
                          {item}
                        </Typography>
                      }
                    />
                  </ListItem>
                ))}
              </List>
            )}
          </Box>
        </Stack>
      </DialogContent>

      <DialogActions sx={{ px: 2.5, py: 1.5 }}>
        <Button onClick={onClose}>完成</Button>
      </DialogActions>
    </Dialog>
  )
}
