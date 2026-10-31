#!/bin/sh
# Render the product page's transition clips (site/assets/tr/<effect>.mp4 + .webp poster)
# with the real shader, via `trprobe --sequence`. Needs a render node and ffmpeg.
#
#   cargo build --release --bin trprobe
#   python3 -I scripts/gen-site-wallpapers.py site/assets     # once; the clips go noon -> neon
#   scripts/gen-site-transitions.sh [RENDER_NODE]
set -eu

node=${1:-/dev/dri/renderD128}
out=site/assets/tr
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$out"
target/release/trprobe "$node" "$tmp" --sequence site/assets/wall-noon.webp site/assets/wall-neon.webp 24 >/dev/null

for first in "$tmp"/*-000.png; do
  name=$(basename "$first" -000.png)
  # 24 frames at 24 fps, then the last one held for 0.4 s.
  ffmpeg -v error -y -framerate 24 -i "$tmp/$name-%03d.png" \
    -vf "scale=800:450,tpad=stop_mode=clone:stop_duration=0.4,format=yuv420p" \
    -c:v libx264 -crf 27 -preset slow -movflags +faststart -an "$out/$name.mp4"
  # The poster is the mid-point of the move, the frame that tells effects apart.
  ffmpeg -v error -y -i "$tmp/$name-008.png" -vf scale=800:450 -quality 80 "$out/$name.webp"
  echo "$out/$name"
done
du -sh "$out"
