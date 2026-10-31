#!/usr/bin/env python3
"""Cut Noto Sans CJK Black (SC face) down to the CJK characters the page sets in
display type, and write site/assets/fonts/ZoneCJK.woff2. Re-run after editing the
headings or the decorative words in site/*.html or site/zh-CN/index.html.

Display type means h1/h2/h3, <summary> and .deco; body text uses the visitor's own
CJK font, so the subset stays small. Needs fontTools with brotli:

    python3 -m venv /tmp/fvenv && /tmp/fvenv/bin/pip install fonttools brotli
    /tmp/fvenv/bin/python -I scripts/gen-site-fonts.py
"""
from html.parser import HTMLParser
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTCollection

SRC = "/usr/share/fonts/noto-cjk/NotoSansCJK-Black.ttc"
PAGES = ["site/index.html", "site/zh-CN/index.html"]
OUT = "site/assets/fonts/ZoneCJK.woff2"
VOID = {"meta", "link", "img", "input", "br", "source"}


class Display(HTMLParser):
    def __init__(self):
        super().__init__()
        self.stack, self.chars = [], set()

    def handle_starttag(self, tag, attrs):
        if tag in VOID:
            return
        cls = (dict(attrs).get("class") or "").split()
        self.stack.append(tag in ("h1", "h2", "h3", "summary") or "deco" in cls)

    def handle_endtag(self, tag):
        if tag not in VOID and self.stack:
            self.stack.pop()

    def handle_data(self, data):
        if any(self.stack):
            self.chars.update(c for c in data if ord(c) >= 0x2E80)


def main():
    p = Display()
    for page in PAGES:
        p.feed(Path(page).read_text())
    font = next(f for f in TTCollection(SRC).fonts if "SC" in f["name"].getBestFamilyName())
    opts = subset.Options()
    opts.flavor = "woff2"
    opts.layout_features = []
    ss = subset.Subsetter(opts)
    ss.populate(text="".join(sorted(p.chars)))
    ss.subset(font)
    subset.save_font(font, OUT, opts)
    print(OUT, len(p.chars), "chars", Path(OUT).stat().st_size // 1024, "KiB")


if __name__ == "__main__":
    main()
