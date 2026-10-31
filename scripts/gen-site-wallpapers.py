#!/usr/bin/env python3
"""Generate the wallpapers used by the product page (site/assets/wall-*.webp).

Flat, ink-outlined street scenes drawn with Pillow: shop fronts with crowded
signboards, posters, awnings, power lines. They are original work, so the page
ships no third-party artwork and there is no licence to track. Fixed seeds: the
same run gives the same files. Needs Noto Sans CJK Black for the sign lettering
(the glyphs end up as pixels, so the font is not shipped).

    python3 -I scripts/gen-site-wallpapers.py site/assets
"""
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

W, H = 2560, 1440
SS = 2  # drawn at 2x and scaled down, which is what anti-aliases the edges
FONT = "/usr/share/fonts/noto-cjk/NotoSansCJK-Black.ttc"

PALETTES = {
    "noon": dict(
        seed=5, ink="#14151c", lit="#fff3c2", neon=None,
        sky=[(0, "#1fb7c4"), (0.55, "#92e4d3"), (1, "#ffd9a0")], sun="#fff6cf", sun_at=(0.74, 0.30),
        far="#76c3c4", mid=["#f0a05a", "#e8685a", "#3d4d63", "#f3d9a8", "#2aa7a0"],
        near=["#ff6a3d", "#2f3e52", "#ffd21f", "#18a7a0", "#f4f1e8"],
        road="#3a4252", walk="#8d97a8",
        signs=["RANDOM PLAY", "レコード", "録像店", "OPEN 24H", "ラーメン", "音像", "喫茶", "ゲーム", "CAFE", "電器"],
    ),
    "neon": dict(
        seed=17, ink="#07060f", lit="#ffd21f", neon=["#ff3d9a", "#18f0e0", "#d4ff3a"],
        sky=[(0, "#0b0a2c"), (0.55, "#3b1468"), (1, "#d8387f")], sun="#ffd1ef", sun_at=(0.28, 0.26),
        far="#2b2160", mid=["#1e1842", "#2d2264", "#3b2060", "#142c4e", "#2a1748"],
        near=["#241c4d", "#3a216e", "#17345a", "#4a1d5a", "#101a38"],
        road="#14122c", walk="#33305a",
        signs=["RANDOM PLAY", "ネオン", "録像店", "BAR 24H", "ゲーム", "酒場", "PLAY", "音像", "カラオケ", "CLUB"],
    ),
    "dusk": dict(
        seed=29, ink="#150d24", lit="#fff0a0", neon=["#d4ff3a", "#ff4b4b", "#2fe6d8"],
        sky=[(0, "#2a2f79"), (0.5, "#e0557f"), (1, "#ffb56b")], sun="#fff2c0", sun_at=(0.5, 0.40),
        far="#7b5a9c", mid=["#3b3574", "#c44c70", "#2b3c5a", "#ff9d4a", "#6b3f8f"],
        near=["#ffb12a", "#e2557f", "#2c3a5a", "#7b3fa0", "#17a89a"],
        road="#2a2440", walk="#6e6490",
        signs=["RANDOM PLAY", "レコード", "ビデオ", "OPEN", "喫茶", "ゲーム", "新艾利都", "音像", "CAFE", "電器"],
    ),
}


def rgb(h):
    return tuple(int(h[i : i + 2], 16) for i in (1, 3, 5))


def mix(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(rgb(a) if isinstance(a, str) else a, rgb(b) if isinstance(b, str) else b))


def s(v):
    return int(round(v * SS))


class Scene:
    def __init__(self, p):
        self.p = p
        self.rng = np.random.default_rng(p["seed"])
        self.ink = p["ink"]
        self.img = Image.new("RGB", (W * SS, H * SS))
        self.d = ImageDraw.Draw(self.img)
        self.fonts = {}

    def font(self, px):
        px = max(8, int(px))
        if px not in self.fonts:
            self.fonts[px] = ImageFont.truetype(FONT, px)
        return self.fonts[px]

    def rect(self, x0, y0, x1, y1, fill, outline=True, w=4):
        self.d.rectangle((s(x0), s(y0), s(x1), s(y1)), fill=fill, outline=self.ink if outline else None, width=s(w) if outline else 0)

    def poly(self, pts, fill, outline=True, w=4):
        self.d.polygon([(s(x), s(y)) for x, y in pts], fill=fill, outline=self.ink if outline else None, width=s(w))

    def text_fit(self, box, text, fill, vertical=False, pad=0.12):
        x0, y0, x1, y1 = (s(v) for v in box)
        bw, bh = x1 - x0, y1 - y0
        if vertical:
            cell = min(bw * (1 - pad * 2), bh * (1 - pad * 2) / max(1, len(text)))
            f = self.font(cell * 0.92)
            for i, ch in enumerate(text):
                cx, cy = (x0 + x1) / 2, y0 + bh * pad + cell * (i + 0.5)
                self.d.text((cx, cy), ch, font=f, fill=fill, anchor="mm")
            return
        size = bh * (1 - pad * 2)
        while size > 10:
            f = self.font(size)
            l, t, r, b = self.d.textbbox((0, 0), text, font=f, anchor="lt")
            if r - l <= bw * (1 - pad * 2):
                break
            size *= 0.92
        self.d.text(((x0 + x1) / 2, (y0 + y1) / 2), text, font=f, fill=fill, anchor="mm")

    # --- layers -------------------------------------------------------------
    def sky(self):
        p = self.p
        stops = p["sky"]
        ys = np.linspace(0, 1, H)
        cols = np.array([rgb(c) for _, c in stops], np.float32)
        col = np.stack([np.interp(ys, [q for q, _ in stops], cols[:, k]) for k in range(3)], 1)
        arr = np.broadcast_to(col[:, None, :], (H, W, 3)).copy()
        # a big halftone-ringed sun
        yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
        cx, cy = p["sun_at"][0] * W, p["sun_at"][1] * H
        dist = np.hypot(xx - cx, yy - cy)
        dots = (np.hypot((xx % 22) - 11, (yy % 22) - 11) < np.clip((420 - dist) / 40, 0, 9)) & (dist < 420) & (dist > 190)
        arr[dots] = arr[dots] * 0.55 + np.array(rgb(p["sun"]), np.float32) * 0.45
        arr[dist < 190] = rgb(p["sun"])
        self.img = Image.fromarray(np.clip(arr, 0, 255).astype(np.uint8)).resize((W * SS, H * SS), Image.NEAREST)
        self.d = ImageDraw.Draw(self.img)

    def skyline(self, base, hmin, hmax, color, windows):
        rng, p = self.rng, self.p
        x = -60
        while x < W:
            w, h = int(rng.integers(130, 300)), int(rng.integers(hmin, hmax))
            self.rect(x, base - h, x + w, base + 40, color, outline=False)
            if windows:
                for cx in range(int(x + 16), int(x + w - 24), 34):
                    for cy in range(int(base - h + 18), int(base - 20), 44):
                        if rng.random() < 0.28:
                            self.rect(cx, cy, cx + 14, cy + 22, mix(color, p["lit"], 0.65), outline=False)
            if rng.random() < 0.45:  # antenna
                ax = x + w * rng.uniform(0.2, 0.8)
                self.d.line((s(ax), s(base - h), s(ax), s(base - h - rng.integers(40, 110))), fill=rgb(self.ink), width=s(4))
            x += w - int(rng.integers(0, 40))

    def vsign(self, x, y, w, h, text):
        p, rng = self.p, self.rng
        bg = p["neon"][int(rng.integers(0, len(p["neon"])))] if p["neon"] else p["near"][int(rng.integers(0, len(p["near"])))]
        self.rect(x, y, x + w, y + h, bg)
        lum = sum(rgb(bg)) / 3
        self.text_fit((x, y, x + w, y + h), text, self.ink if lum > 110 else "#ffffff", vertical=True, pad=0.1)

    def mid_buildings(self, base):
        rng, p = self.rng, self.p
        x = -80
        while x < W:
            w, h = int(rng.integers(260, 440)), int(rng.integers(420, 820))
            color = p["mid"][int(rng.integers(0, len(p["mid"])))]
            self.rect(x, base - h, x + w, base + 20, color)
            # window grid
            for cx in range(int(x + 26), int(x + w - 60), 62):
                for cy in range(int(base - h + 30), int(base - 90), 78):
                    lit = rng.random() < 0.32
                    self.rect(cx, cy, cx + 38, cy + 50, p["lit"] if lit else mix(color, p["ink"], 0.55), w=3)
                    if rng.random() < 0.08:  # air conditioner
                        self.rect(cx + 6, cy + 52, cx + 40, cy + 74, "#c9ced8", w=3)
            # a vertical shop sign hanging off the face
            if rng.random() < 0.8:
                text = p["signs"][int(rng.integers(0, len(p["signs"])))]
                if len(text) <= 5 and text.isascii() is False or len(text) <= 3:
                    sw = 70
                    self.vsign(x + w - sw - 18, base - h + 140, sw, 70 * min(4, len(text)) + 30, text)
            x += w + int(rng.integers(-10, 30))

    def poster(self, x, y, w, h):
        p, rng = self.p, self.rng
        pal = p["near"] + (p["neon"] or [])
        bg = pal[int(rng.integers(0, len(pal)))]
        self.rect(x, y, x + w, y + h, bg, w=3)
        kind = int(rng.integers(0, 3))
        cx, cy = x + w / 2, y + h * 0.42
        fg = mix(bg, "#ffffff", 0.75) if sum(rgb(bg)) < 380 else self.ink
        if kind == 0:
            self.d.ellipse((s(cx - w * 0.3), s(cy - w * 0.3), s(cx + w * 0.3), s(cy + w * 0.3)), fill=fg)
        elif kind == 1:
            self.poly([(x + w * 0.15, y + h * 0.72), (cx, y + h * 0.1), (x + w * 0.85, y + h * 0.72)], fg, outline=False)
        else:
            for k in range(4):
                self.rect(x + w * 0.15, y + h * (0.12 + k * 0.1), x + w * (0.85 - k * 0.08), y + h * (0.17 + k * 0.1), fg, outline=False)
        self.rect(x + w * 0.14, y + h * 0.84, x + w * 0.86, y + h * 0.9, fg, outline=False)

    def shops(self, ground):
        rng, p = self.rng, self.p
        x = -30
        while x < W:
            w = int(rng.integers(380, 560))
            top = ground - int(rng.integers(440, 560))
            body = p["near"][int(rng.integers(0, len(p["near"])))]
            self.rect(x, top, x + w, ground, body, w=5)
            # signboard
            sh = 118
            sign_bg = p["neon"][int(rng.integers(0, len(p["neon"])))] if p["neon"] else p["near"][int(rng.integers(0, len(p["near"])))]
            if sign_bg == body:
                sign_bg = "#ffffff"
            self.rect(x + 14, top + 22, x + w - 14, top + 22 + sh, sign_bg, w=5)
            text = p["signs"][int(rng.integers(0, len(p["signs"])))]
            lum = sum(rgb(sign_bg)) / 3
            self.text_fit((x + 14, top + 22, x + w - 14, top + 22 + sh), text, self.ink if lum > 110 else "#ffffff")
            if p["neon"]:  # a neon outline over the sign
                col = p["neon"][int(rng.integers(0, len(p["neon"])))]
                self.d.rectangle((s(x + 24), s(top + 32), s(x + w - 24), s(top + 12 + sh)), outline=rgb(col), width=s(5))
            # upper-floor window strip with posters
            ay = top + 22 + sh + 20
            self.rect(x + 20, ay, x + w - 20, ay + 120, mix(body, p["ink"], 0.45), w=4)
            px = x + 32
            while px < x + w - 90:
                pw = int(rng.integers(52, 84))
                self.poster(px, ay + 10, pw, 100)
                px += pw + 12
            # awning
            aw_y = ay + 150
            stripes = 7
            sw_ = (w - 20) / stripes
            self.poly([(x + 6, aw_y), (x + w - 6, aw_y), (x + w + 10, aw_y + 66), (x - 10, aw_y + 66)], "#ffffff")
            for k in range(0, stripes, 2):
                col = p["neon"][k // 2 % len(p["neon"])] if p["neon"] else p["near"][(k // 2) % len(p["near"])]
                ax0 = x + 6 + sw_ * k
                self.poly([(ax0, aw_y), (ax0 + sw_, aw_y), (ax0 + sw_ + 3, aw_y + 66), (ax0 - 3 - (0 if k else 10), aw_y + 66)], col, outline=False)
            self.d.line((s(x - 10), s(aw_y + 66), s(x + w + 10), s(aw_y + 66)), fill=rgb(self.ink), width=s(5))
            # shop window + door
            wy = aw_y + 84
            self.rect(x + 28, wy, x + w * 0.62, ground - 14, mix(body, p["lit"], 0.35) if rng.random() < 0.6 else mix(body, p["ink"], 0.6), w=5)
            self.rect(x + w * 0.68, wy + 20, x + w - 34, ground, p["ink"], w=5)
            # vending machine / sandwich board on some fronts
            if rng.random() < 0.55:
                vx = x + w - 130
                col = ["#e63b3b", "#2fb7e8", "#ffd21f"][int(rng.integers(0, 3))]
                self.rect(vx, ground - 190, vx + 84, ground, col, w=4)
                self.rect(vx + 10, ground - 176, vx + 74, ground - 100, "#f4f7fb", w=3)
            x += w - 6

    def wires(self, ground):
        rng, p = self.rng, self.p
        xs = [int(v) for v in np.linspace(120, W - 120, 5) + rng.uniform(-60, 60, 5)]
        top = 150
        for x in xs:
            self.rect(x - 8, top, x + 8, ground, "#4b4f5e", w=3)
            self.rect(x - 70, top + 20, x + 70, top + 34, "#4b4f5e", w=3)
        for a, b in zip(xs, xs[1:]):
            for off in (26, 12, 40):
                pts = []
                for t in np.linspace(0, 1, 40):
                    pts.append((s(a + (b - a) * t), s(top + off + 46 * math.sin(math.pi * t))))
                self.d.line(pts, fill=rgb(self.ink), width=s(3))

    def ground(self, y):
        p = self.p
        self.rect(0, y, W, y + 56, p["walk"], w=5)
        self.rect(0, y + 56, W, H, p["road"], outline=False)
        self.d.line((0, s(y + 56), W * SS, s(y + 56)), fill=rgb(self.ink), width=s(6))
        for x in range(0, W, 240):  # dashed centre line
            self.rect(x, y + 190, x + 130, y + 208, "#f4f1e8", outline=False)
        self.rect(0, y + 270, W, y + 282, "#f4f1e8", outline=False)  # kerb paint


def render(name, p):
    sc = Scene(p)
    sc.sky()
    sc.skyline(930, 220, 520, p["far"], windows=False)
    sc.skyline(960, 160, 360, mix(p["far"], p["ink"], 0.22), windows=True)
    sc.mid_buildings(1010)
    ground = 1130
    sc.shops(ground)
    sc.wires(ground)
    sc.ground(ground)
    img = sc.img.resize((W, H), Image.LANCZOS)
    arr = np.asarray(img, np.float32)
    # vignette + grain, so the flat fills read as printed rather than rendered
    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
    v = 1 - 0.22 * (np.hypot((xx - W / 2) / (W / 2), (yy - H / 2) / (H / 2)) ** 2)
    arr *= v[..., None]
    arr += sc.rng.normal(0, 2.2, (H, W, 1))
    return Image.fromarray(np.clip(arr, 0, 255).astype(np.uint8))


def main():
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "site/assets")
    out.mkdir(parents=True, exist_ok=True)
    for name, p in PALETTES.items():
        path = out / f"wall-{name}.webp"
        render(name, p).save(path, quality=84, method=6)
        print(path, path.stat().st_size // 1024, "KiB")


if __name__ == "__main__":
    main()
