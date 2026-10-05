# niripaper

[![CI](https://github.com/harukizmoe/niripaper/actions/workflows/ci.yml/badge.svg)](https://github.com/harukizmoe/niripaper/actions/workflows/ci.yml)

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

Version 0.2.0, early.

Working today: static images (PNG, JPEG, WebP), **video wallpapers** (hardware-decoded),
parallax, the overview transition, the wallpaper transition (nine effects), `fit` for
sources that do not match the canvas, configuration with a `config.d` directory and hot
reload, per-output settings, `schema`/`state` for a panel to build its UI from, and
changing the wallpaper of a running daemon.

Not here yet: the Noctalia panel (a separate project — the daemon side of the interface
is `schema`, `state` and `config.d`, all of which are here), an AUR package, and prebuilt
binaries.

## Requirements

- **niri.** Developed against 26.04.
- **libmpv** — the video path links it. On Arch, that is the `mpv` package.
- **Rust** (a recent stable toolchain) if you are building it yourself.

## Install

Build from source. `libmpv`, `libEGL`, `libGL`, `libgbm` and `libwayland-client` have
to be installed — the daemon links them by hand, so their development files are needed
to build (`mpv`, `libglvnd`, `mesa`, `wayland` on Arch):

```bash
git clone https://github.com/harukizmoe/niripaper.git
cd niripaper
cargo install --path . --locked --bin niripaper
```

**`--bin niripaper` is not optional**: without it the diagnostic probes in `src/bin/`
get installed alongside the daemon.

There is a `PKGBUILD` in `packaging/aur/` for the AUR (not published yet — the AUR is not
accepting new accounts, see `packaging/aur/README.md`).

Releases carry a binary built on **Ubuntu 22.04** for `x86_64`. It is not a generic Linux
binary and the name says so: a daemon that links `libmpv`, `libEGL`, `libgbm` and
`libwayland-client` is tied to the ABI of the distribution that built it — the Ubuntu build
wants `libmpv.so.1` (mpv 0.34) while Arch ships `libmpv.so.2` (mpv 0.41). It runs on Ubuntu
22.04 and newer; anywhere else, build from source.

## Usage

```bash
niripaper daemon                          # draw the wallpaper
niripaper daemon --wallpaper ~/wall.webp  # …with a specific image
niripaper watch                           # print what the parallax would do, without drawing
```

`watch` is a debugging aid: it prints the target position as niri's layout changes,
which helps tell whether a problem is in niri's events or in the rendering.

### Changing the wallpaper of a running daemon

The daemon listens on a small control socket, one per output
(`$XDG_RUNTIME_DIR/niripaper-DP-1.sock`):

```bash
niripaper set ~/Pictures/wall.webp    # switch it, image or video
niripaper query                       # what is on screen right now
niripaper schema                      # every config key, as JSON
niripaper state                       # what the daemon is doing, as JSON
niripaper kill                        # shut it down
```

`set` loads the new wallpaper *before* swapping it in, so a bad path reports an error
and leaves the current one on screen instead of blanking it.

`set` is a **temporary override**: it changes what is on screen, not the configuration,
and any reload of `config.d` or `config.toml` puts the configured wallpaper back. To keep
a wallpaper, put it in the configuration.

`schema` and `state` are what a panel (the Noctalia plugin, or anything else) talks to.
`schema` lists every configuration key with its type, bounds, default, unit and whether
a reload picks it up; `state` reports the effective values under the same names, plus
the output, the canvas, the wallpaper actually on screen and the parallax position. A
panel asks for the schema once to build its widgets, and afterwards only needs `state` —
so adding a key here never requires a panel change.

`state`'s `config.values` is what the configuration says, while `wallpaper.path` is what
is *actually* on screen: `set` swaps the media without touching the configuration, so
the two can legitimately differ.

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
| `--socket PATH` | control socket to use (default: `$XDG_RUNTIME_DIR/niripaper-<output>.sock`) |
| `--output NAME` | which output's daemon to talk to; needed only when several are running |
| `--trace` | log every frame (position, zoom, timing) — for debugging |

## Configuration

The config file lives at `~/.config/niripaper/config.toml`. It is optional: with no
file at all, the built-in defaults are used. A `config.d/` directory next to it is
read too — every `*.toml` in it, in filename order, merged over the main file
(later files win). That is where a tool keeps its own settings without touching
yours: delete the file and it is gone. A fragment has to spell out its own table
headers — `[transition]` and the like — since it is merged, not textually
spliced; a key at the wrong level is an unknown field, which is a hard error. Edits are picked up while the daemon runs;
a file that fails to parse is reported and the running configuration is kept. Values are resolved as
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

[transition]
selection = "rotate"                  # fixed | rotate | random
effects = ["portal", "iris", "dissolve"]
duration_ms = 1500
curve = "ease-out-cubic"
softness = 0.3                        # 0 is a hard edge, 1 is very soft
on_start = true                       # play one when the daemon starts

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
| `fit` | how a source that does not match the canvas aspect is placed: `fill` (default), `fit`, `stretch`, `center`, `tile` |
| `namespace` | layer-shell namespace (default `niripaper`) |
| `[animations] follow_niri` | take the overview transition's settings from niri (default `true`) |
| `[animations] slowdown` | stretch every animation's timeline |
| `[animations.parallax]` | how the wallpaper moves when you move |
| `[animations.overview-open-close]` | the overview transition: a `zoom` plus a spring or a curve |
| `[transition] selection` | how the effect is chosen each time: `fixed`, `rotate` or `random` |
| `[transition] effect`, `effects` | the effect used when `fixed`, and the pool `rotate` and `random` pick from |
| `[transition] duration_ms`, `curve` | how long it takes and how it is eased |
| `[transition] cubic_bezier` | control points for `curve = "cubic-bezier"`, as in CSS |
| `[transition] softness` | how wide the moving edge is (`0` hard, `1` very soft) |
| `[transition] center` | where radial effects start: `[x, y]` as fractions of the screen, `[0, 0]` being the top-left |
| `[transition] direction` | which way wipes, stripes and slides go |
| `[transition] stripes` | how many bands the `stripes` effect breaks the edge into |
| `[transition] push` | how far `portal` pushes the old frame outward; `0` makes it an `iris` |
| `[transition] start_radius` | how wide the hole already is at the start (a fraction of the screen height) |
| `[transition] hold_ms` | how long the first frame is held before the transition moves (part of `duration_ms`) |
| `[transition] allow_overshoot`, `on_start` | let the curve bounce; play one at startup |
| `[outputs."NAME"]` | per-output overrides of any of the above |

Animations use niri's own vocabulary: each of them is either `off`, an easing
(`duration_ms` plus a `curve`), or a `spring` (`damping_ratio`, `stiffness`,
`epsilon`). The `curve` names are niri's: `linear`, `ease-out-quad`,
`ease-out-cubic`, `ease-out-expo`, `cubic-bezier`.

The wallpaper transition is a table of its own rather than part of `[animations]`,
because that table mirrors niri's vocabulary and niri has no wallpaper-change
animation to mirror. The effects are `portal`, `iris`, `dissolve`, `wipe`, `stripes`, `zoom`, `slide`,
`fade` and `none`.

One constraint shapes all of them: the old side is a **frozen snapshot** and the new
side is live, so only one decoder ever runs. That is what lets a video arrive through
a portal that is already moving, and it is why every effect costs one fragment pass
however elaborate it looks. `softness` widens the moving edge — except in `slide`,
where the two frames are exactly adjacent and the seam is a step.

`iris` and `portal` are the same circle: `portal` additionally pushes the old frame
outward as the hole opens, so you move *through* it rather than watch it get cut away.
That is the whole difference, and `push` is how far it goes.

The circle itself is tunable: `center` is where it opens — `[x, y]` as fractions of the
screen, read from the **top-left**, so it matches how a screenshot is read — and
`start_radius` is how wide it already is when the transition begins. Left at `0` it
grows from a point; set to something like `0.08` it begins as a small **complete
circle**. However it is placed, it grows until it has covered the farthest corner from
`center`.

`hold_ms` holds the first frame still for a moment before anything moves, which is
**a stall, not a pause**: the radius stops dead and the transition reads as two pieces.
For a small circle that should still register, a slow-starting curve is the better
answer — the circle keeps moving, it just moves slowly at first. Reach for `hold_ms`
only when you really do want a beat. It is part of `duration_ms`, not extra.

niri's curves are all fast at the start and slow at the end, which is the opposite of
what a reveal wants. For slow-fast-slow, use a bezier — `curve = "cubic-bezier"` with
`cubic_bezier = [0.42, 0, 0.58, 1]`, which is CSS's `ease-in-out` (solved the same way,
`x(u) = t`).

`fit` decides what happens when a wallpaper's aspect ratio is not the canvas's, and the
names are **Wallpaper Engine's** (Windows' before it): `fill` (the default) scales until
the canvas is full and crops the overflow; `fit` shows the whole source and pads the rest
with black, which is worth having for film-shaped sources — a 2.35:1 picture loses about a
quarter of its width to `fill`; `stretch` distorts; `center` does not scale at all, so the
source sits at its own pixel size in the middle; `tile` repeats it at that size from the
top-left. (Windows' `span` is left out: spreading a picture across monitors cannot mean
anything to a daemon that draws on one output.)

The names are borrowed on purpose: anyone arriving from Wallpaper Engine already knows
what each one does, and a config that spells them differently is one they have to learn
twice. `tile` is the one mode a video cannot honour — mpv scales a video into the frame
and does not repeat it — so a video with `tile` falls back to `fill`, which `state`
reports under `wallpaper.fit` next to the configured value.

It is per output, and both the still and the video path use it, so a wallpaper cannot
change shape depending on whether it happens to be a picture or a film.

`niripaper schema` lists every key above, with its type, bounds and whether editing
the file while the daemon runs takes effect.

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
