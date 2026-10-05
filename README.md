# niripaper

**English** · [简体中文](README.zh-CN.md)

**A wallpaper that moves with your layout.**

niripaper is a parallax wallpaper daemon for the
[niri](https://github.com/niri-wm/niri) compositor. As you move between windows and
workspaces, it shifts the wallpaper by a few dozen pixels in the same direction, so
the wallpaper stays behind your windows instead of sitting still.

It draws the wallpaper itself, as a layer-shell background. The overview transition
takes its timing from niri's own animation settings, so the wallpaper moves in step
with the workspaces.

## Features

- **Parallax.** Moving between windows and workspaces shifts the wallpaper a few dozen
  pixels in the same direction.
- **It draws the wallpaper itself.** No mpv, no shell scripts, no helper processes:
  a single binary puts a layer-shell surface on the background layer and renders
  into it.
- **Video wallpapers.** mp4, webm, mkv, mov, m4v and avi, hardware-decoded.
  [mpv](https://mpv.io) is used as a *library* — no `mpv` process, no window, no IPC,
  no config file of its own — and it draws the decoded frame into a texture this
  daemon owns, so a video is sampled with the parallax offset exactly like a still.
- **The overview transition follows niri.** With `follow_niri = true` (the default),
  its spring or easing curve is read from niri's config, so the wallpaper moves in
  step with the workspaces.
- **Overview zoom.** Opening niri's overview zooms the wallpaper out with it.
- **Per-output settings.** Scale, spans and wallpaper can all differ per output.
- **Low resource usage.** While nothing is moving it draws no frames and uses no
  measurable CPU.

## Status

Version 0.1.0, early.

Working today: static images (PNG, JPEG, WebP), **video wallpapers** (hardware-decoded),
parallax, the overview transition, configuration, per-output settings, and changing the
wallpaper of a running daemon.

Not here yet: a cross-fade when the wallpaper changes.

## Requirements

- **niri.** Developed against 26.04.
- **libmpv** — the video path links it. On Arch, that is the `mpv` package.
- **Rust** (a recent stable toolchain) if you are building it yourself.

## Install

Build from source:

```bash
git clone https://github.com/harukizmoe/niripaper.git
cd niripaper
cargo build --release
install -Dm755 target/release/niripaper ~/.local/bin/niripaper
```

## Usage

```bash
niripaper daemon                          # draw the wallpaper
niripaper daemon --wallpaper ~/wall.webp  # …with a specific image
niripaper watch                           # print what the parallax would do, without drawing
```

`watch` is a debugging aid: it prints the target position as niri's layout changes,
which helps tell whether a problem is in niri's events or in the rendering.

### Changing the wallpaper of a running daemon

The daemon listens on a small control socket (`$XDG_RUNTIME_DIR/niripaper.sock`):

```bash
niripaper set ~/Pictures/wall.webp    # switch it, image or video
niripaper query                       # what is on screen right now
niripaper kill                        # shut it down
```

`set` loads the new wallpaper *before* swapping it in, so a bad path reports an error
and leaves the current one on screen instead of blanking it.

### Starting it with niri

Add this to `~/.config/niri/config.kdl`:

```kdl
spawn-at-startup "niripaper" "daemon"
```

The binary has to be on `PATH` for that to work — hence the `install` line above.

### Flags

| Flag | Meaning |
| --- | --- |
| `--output NAME` | which output to draw on (default: the first one niri reports) |
| `--config PATH` | config file to read (default: `~/.config/niripaper/config.toml`) |
| `--namespace NAME` | layer-shell namespace (default: `niripaper`) |
| `--wallpaper PATH` | image to draw |
| `--scale F` | canvas enlargement, `1.0`–`1.35` |
| `--column-span N`, `--workspace-span N` | how many steps the parallax spreads over |
| `--pattern blocks\|bands` | the built-in test pattern, used when no wallpaper is given |
| `--socket PATH` | control socket to use (default: `$XDG_RUNTIME_DIR/niripaper.sock`) |
| `--trace` | log every frame (position, zoom, timing) — for debugging |

## Configuration

The config file lives at `~/.config/niripaper/config.toml`. It is optional: with no
file at all, the built-in defaults are used. Values are resolved as
**command line → config file → built-in default**, so the file only needs to state
what differs.

```toml
wallpaper = "~/Pictures/wall.webp"   # an image, or a video (routed by extension)
video_fps = 25                       # frame-rate cap for video wallpapers (0 = source)

scale = 1.1           # canvas enlargement; larger values leave more room to move in
column_span = 6       # how many columns the horizontal parallax spans
workspace_span = 6    # how many workspaces the vertical parallax spans
namespace = "niripaper"

[animations]
follow_niri = true    # take the overview transition's settings from niri

[animations.parallax]
duration_ms = 600
curve = "ease-out-cubic"

[animations.overview-open-close]
zoom = 0.96           # 1.0 turns the effect off

# Per-output overrides — anything not listed here is inherited from above.
[outputs."DP-1"]
scale = 1.2
```

### Keys

| Key | Meaning |
| --- | --- |
| `wallpaper` | image or video to draw (PNG/JPEG/WebP, or mp4/webm/mkv/mov/m4v/avi) |
| `video_fps` | frame-rate cap for video wallpapers (default `0`, i.e. the source's) |
| `scale` | canvas enlargement (default `1.1`, max `1.35`) |
| `column_span` | fixed column span for the horizontal parallax (default `6`, min `2`) |
| `workspace_span` | fixed workspace span for the vertical parallax (default `6`, min `2`) |
| `namespace` | layer-shell namespace (default `niripaper`) |
| `[animations] follow_niri` | take the overview transition's settings from niri (default `true`) |
| `[animations] slowdown` | stretch every animation's timeline |
| `[animations.parallax]` | how the wallpaper moves when you move |
| `[animations.overview-open-close]` | the overview transition: a `zoom` plus a spring or a curve |
| `[outputs."NAME"]` | per-output overrides of any of the above |

Animations use niri's own vocabulary: each of them is either `off`, an easing
(`duration_ms` plus a `curve`), or a `spring` (`damping_ratio`, `stiffness`,
`epsilon`). The `curve` names are niri's: `linear`, `ease-out-quad`,
`ease-out-cubic`, `ease-out-expo`, `cubic-bezier`.

With `follow_niri = true`, the overview transition takes its parameters from niri's
config, so changing niri's settings changes the wallpaper's motion as well. Anything
written in this file wins over what niri says.

A video is decoded in hardware and scaled to the canvas — the output size times
`scale` — never to the source resolution. `video_fps` caps how often a frame is
presented: niri hands layer surfaces frame callbacks at 60 Hz, so a 60 fps source sits
exactly on that edge. Measured here, 25 is smooth and drops no frames; raising it is
worth trying on faster hardware.

## Troubleshooting

- **Nothing appears.** The output name has to match niri's; check `niri msg outputs`
  and pass `--output NAME`.
- **It exits right away.** The config did not parse — the error names the offending
  key and line.
- **The wallpaper does not move.** If niri's config turns animations off globally
  (`animations { off }`), the overview transition is off here too. The parallax is
  unaffected — it is not one of niri's animations.
- **Your layer rules match a different name.** Layer rules are matched by namespace,
  so set `namespace` to whatever name your existing rules already use.

## License

GPL-3.0-or-later © 2026 harukizmoe. See [LICENSE](LICENSE).

## Acknowledgements

- **[niri](https://github.com/niri-wm/niri)** — the compositor this is written for,
  and the source of the event stream it follows.
