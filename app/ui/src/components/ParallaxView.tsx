import { Button, Switch, Typography } from '@mui/material'
import { useConfig, type Config } from '../hooks/useConfig'
import { AnimationField, NumberInput, SliderField, type AnimationValue } from './fields'
import { SettingRow, SettingsGroup, SettingsPage } from './settings'

// The parallax and overview settings, as `config.toml` states them. Every row
// edits one key and saves it at once; the daemon reloads the file itself.

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
  const parallax = c.value<boolean>('parallax') ?? true
  const scale = c.value<number>('scale')
  const overviewZoom = c.value<number>('animations.overview-open-close.zoom') ?? 0.96
  const overviewEnabled = overviewZoom < 0.999
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
      <SettingsGroup title="移动" description="在列和工作区之间切换时，壁纸朝同一方向平移。">
        <SettingRow
          label="跟随窗口移动壁纸"
          description="在列和工作区之间切换时，壁纸平移以呈现视差感。"
          modified={c.isSet('parallax')}
          onReset={() => void c.reset('parallax')}
          control={
            <Switch
              checked={parallax}
              onChange={() => void c.set('parallax', !parallax)}
            />
          }
        />
        <SettingRow
          label="移动幅度"
          description="壁纸放大的倍数。越大能移动得越远，四周裁掉的也越多。"
          disabled={!parallax}
          modified={c.isSet('scale')}
          onReset={() => void c.reset('scale')}
        >
          <SliderField
            value={scale}
            min={1.01}
            max={1.35}
            step={0.01}
            unit="×"
            disabled={!parallax}
            onCommit={(v) => void c.set('scale', v)}
          />
        </SettingRow>
        <SettingRow
          label="横向跨度"
          description="壁纸从最左移到最右对应的列数。按固定列数计算，开关窗口时壁纸不会跳动。"
          disabled={!parallax}
          modified={c.isSet('column_span')}
          onReset={() => void c.reset('column_span')}
          control={
            <NumberInput
              value={c.value<number>('column_span')}
              min={2}
              step={1}
              unit="列"
              disabled={!parallax}
              onCommit={(v) => void c.set('column_span', v)}
            />
          }
        />
        <SettingRow
          label="纵向跨度"
          description="壁纸从最上移到最下对应的工作区数。"
          disabled={!parallax}
          modified={c.isSet('workspace_span')}
          onReset={() => void c.reset('workspace_span')}
          control={
            <NumberInput
              value={c.value<number>('workspace_span')}
              min={2}
              step={1}
              unit="个"
              disabled={!parallax}
              onCommit={(v) => void c.set('workspace_span', v)}
            />
          }
        />
        <SettingRow
          label="移动动画"
          description="焦点或工作区变化时，壁纸移到新位置的方式。"
          disabled={!parallax}
          modified={c.isSet('animations.parallax')}
          onReset={() => void c.reset('animations.parallax')}
        >
          <AnimationField
            value={c.value<AnimationValue>('animations.parallax')}
            curves={curves('animations.parallax')}
            disabled={!parallax}
            onCommit={(v) => void c.set('animations.parallax', v)}
          />
        </SettingRow>
      </SettingsGroup>

      <SettingsGroup title="总览" description="打开 niri 的总览时，壁纸随工作区一起后撤缩小。">
        <SettingRow
          label="打开总览时缩放壁纸"
          description="在打开 niri 总览时，壁纸同步后撤缩小。"
          control={
            <Switch
              checked={overviewEnabled}
              onChange={() =>
                void (overviewEnabled
                  ? c.set('animations.overview-open-close.zoom', 1.0)
                  : c.reset('animations.overview-open-close.zoom'))
              }
            />
          }
        />
        <SettingRow
          label="缩放比例"
          description="打开总览时壁纸缩小到的比例，1 为不缩放。"
          disabled={!overviewEnabled}
          modified={c.isSet('animations.overview-open-close.zoom')}
          onReset={() => void c.reset('animations.overview-open-close.zoom')}
        >
          <SliderField
            value={overviewZoom}
            min={0.9}
            max={1.0}
            step={0.005}
            unit="×"
            disabled={!overviewEnabled}
            onCommit={(v) => void c.set('animations.overview-open-close.zoom', v)}
          />
        </SettingRow>
        {fromNiri('animations.overview-open-close') ? (
          <SettingRow
            label="总览动画"
            description="使用 niri 配置里的总览动画。"
            disabled={!overviewEnabled}
            control={takeOver('animations.overview-open-close')}
          />
        ) : (
          <SettingRow
            label="总览动画"
            description={
              followNiri ? '打开和关闭总览时缩放的方式。恢复后改用 niri 的设置。' : '打开和关闭总览时缩放的方式。'
            }
            disabled={!overviewEnabled}
            modified={c.isSet('animations.overview-open-close')}
            onReset={() => void c.reset('animations.overview-open-close')}
          >
            <AnimationField
              value={c.value<AnimationValue>('animations.overview-open-close')}
              curves={curves('animations.overview-open-close')}
              disabled={!overviewEnabled}
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
