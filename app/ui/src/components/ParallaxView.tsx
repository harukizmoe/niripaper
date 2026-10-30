import { Button, Switch, Typography } from '@mui/material'
import { useConfig, type Config } from '../hooks/useConfig'
import { AnimationField, NumberInput, SliderField, type AnimationValue } from './fields'
import { SettingRow, SettingsGroup, SettingsPage } from './settings'

// The parallax, as `config.toml` states it. Every row edits one key and saves
// it at once; the daemon reloads the file itself.

/// Below this the canvas has no room to move, which is what "off" means: a
/// scale of 1 is the daemon's own way to disable the parallax.
const STILL = 1.001

export function ParallaxView({ onNotify }: { onNotify: (msg: string) => void }) {
  const config = useConfig(onNotify)

  return (
    <SettingsPage title="视差">
      {config.ready ? (
        <Settings config={config} />
      ) : (
        <Typography sx={{ color: 'text.secondary', px: 2 }}>
          {config.loadError ? `读取配置失败：${config.loadError}` : '正在读取配置…'}
        </Typography>
      )}
    </SettingsPage>
  )
}

function Settings({ config: c }: { config: Config }) {
  const scale = c.value<number>('scale')
  const moving = scale > STILL
  const followNiri = c.value<boolean>('animations.follow_niri')
  const curves = (key: string) => {
    const kind = c.key(key)?.kind
    return kind?.type === 'animation' ? kind.curves : []
  }
  // While this file leaves them out, these keys come from niri: their real
  // values are niri's, which this page cannot see, so it shows no numbers and
  // offers to set them here instead.
  const fromNiri = (key: string) => followNiri && !c.isSet(key)
  const takeOver = (key: string) => (
    <Button variant="outlined" size="small" onClick={() => void c.set(key, c.key(key)?.default ?? null)}>
      在这里设置
    </Button>
  )

  return (
    <>
      <SettingsGroup title="移动" description="在列和工作区之间切换时，壁纸朝同一方向移动。">
        <SettingRow
          label="跟随窗口移动壁纸"
          control={
            <Switch
              checked={moving}
              // Off is scale 1; on goes back to the default enlargement.
              onChange={() => void (moving ? c.set('scale', 1.0) : c.reset('scale'))}
            />
          }
        />
        <SettingRow
          label="移动幅度"
          description="壁纸放大的倍数。越大能移动得越远，四周裁掉的也越多。"
          disabled={!moving}
          modified={c.isSet('scale')}
          onReset={() => void c.reset('scale')}
        >
          <SliderField
            value={scale}
            min={1.01}
            max={1.35}
            step={0.01}
            unit="×"
            disabled={!moving}
            onCommit={(v) => void c.set('scale', v)}
          />
        </SettingRow>
        <SettingRow
          label="横向跨度"
          description="壁纸从最左移到最右对应的列数。按固定列数计算，开关窗口时壁纸不会跳动。"
          disabled={!moving}
          modified={c.isSet('column_span')}
          onReset={() => void c.reset('column_span')}
          control={
            <NumberInput
              value={c.value<number>('column_span')}
              min={2}
              step={1}
              unit="列"
              disabled={!moving}
              onCommit={(v) => void c.set('column_span', v)}
            />
          }
        />
        <SettingRow
          label="纵向跨度"
          description="壁纸从最上移到最下对应的工作区数。"
          disabled={!moving}
          modified={c.isSet('workspace_span')}
          onReset={() => void c.reset('workspace_span')}
          control={
            <NumberInput
              value={c.value<number>('workspace_span')}
              min={2}
              step={1}
              unit="个"
              disabled={!moving}
              onCommit={(v) => void c.set('workspace_span', v)}
            />
          }
        />
        <SettingRow
          label="移动动画"
          description="焦点或工作区变化时，壁纸移到新位置的方式。"
          disabled={!moving}
          modified={c.isSet('animations.parallax')}
          onReset={() => void c.reset('animations.parallax')}
        >
          <AnimationField
            value={c.value<AnimationValue>('animations.parallax')}
            curves={curves('animations.parallax')}
            disabled={!moving}
            onCommit={(v) => void c.set('animations.parallax', v)}
          />
        </SettingRow>
      </SettingsGroup>

      <SettingsGroup title="总览" description="打开 niri 的总览时，壁纸随工作区一起缩小。">
        <SettingRow
          label="缩放比例"
          description="打开总览时壁纸缩小到的比例，1 为不缩放。"
          modified={c.isSet('animations.overview-open-close.zoom')}
          onReset={() => void c.reset('animations.overview-open-close.zoom')}
        >
          <SliderField
            value={c.value<number>('animations.overview-open-close.zoom')}
            min={0.9}
            max={1}
            step={0.005}
            unit="×"
            onCommit={(v) => void c.set('animations.overview-open-close.zoom', v)}
          />
        </SettingRow>
        {fromNiri('animations.overview-open-close') ? (
          <SettingRow
            label="总览动画"
            description="使用 niri 配置里的总览动画。"
            control={takeOver('animations.overview-open-close')}
          />
        ) : (
          <SettingRow
            label="总览动画"
            description={
              followNiri ? '打开和关闭总览时缩放的方式。恢复后改用 niri 的设置。' : '打开和关闭总览时缩放的方式。'
            }
            modified={c.isSet('animations.overview-open-close')}
            onReset={() => void c.reset('animations.overview-open-close')}
          >
            <AnimationField
              value={c.value<AnimationValue>('animations.overview-open-close')}
              curves={curves('animations.overview-open-close')}
              onCommit={(v) => void c.set('animations.overview-open-close', v)}
            />
          </SettingRow>
        )}
      </SettingsGroup>

      <SettingsGroup title="所有动画">
        <SettingRow
          label="使用 niri 的动画设置"
          description="总览动画和动画速度没有在这里单独设置时，使用 niri 配置里的值。"
          control={
            <Switch
              checked={followNiri}
              onChange={() => void c.set('animations.follow_niri', !followNiri)}
            />
          }
        />
        {fromNiri('animations.slowdown') ? (
          <SettingRow
            label="动画速度"
            description="使用 niri 配置里的值。"
            control={takeOver('animations.slowdown')}
          />
        ) : (
          <SettingRow
            label="动画速度"
            description="所有动画的时长乘以这个倍数，大于 1 变慢。"
            modified={c.isSet('animations.slowdown')}
            onReset={() => void c.reset('animations.slowdown')}
          >
            <SliderField
              value={c.value<number>('animations.slowdown')}
              min={0.25}
              max={4}
              step={0.05}
              inputMax={100}
              unit="×"
              onCommit={(v) => void c.set('animations.slowdown', v)}
            />
          </SettingRow>
        )}
        <SettingRow
          label="关闭所有动画"
          description="视差、总览和换壁纸的过渡都立即完成。"
          control={
            <Switch
              checked={c.value<boolean>('animations.off')}
              onChange={() => void c.set('animations.off', !c.value<boolean>('animations.off'))}
            />
          }
        />
      </SettingsGroup>
    </>
  )
}
