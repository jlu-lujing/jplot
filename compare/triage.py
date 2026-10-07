#!/usr/bin/env python3
"""triage.py — diff-region analysis for every comparison figure.

Buckets each figure's >16 diff pixels by region (left gutter / right legend /
bottom / top / inside panel) using the reference panel rect parsed from the
svglite SVG. Prints figures sorted by total diff with the dominant region, so
review starts from evidence (WHERE the diff lives) instead of the raw %.
"""
import os
import re
import sys

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
REFS, OURS, DIFFDIR = (os.path.join(HERE, d) for d in ("refs_png", "ours", "diff"))
SVGS = os.path.join(HERE, "refs")
THRESH = int(sys.argv[1]) if len(sys.argv) > 1 else 16


def panel_rect(svg_path):
    try:
        s = open(svg_path, encoding="utf-8", errors="ignore").read()
    except FileNotFoundError:
        return None
    m = re.search(
        r"<rect x='([\d.]+)' y='([\d.]+)' width='([\d.]+)' height='([\d.]+)'[^>]*fill: #EBEBEB", s
    ) or re.search(
        r'<rect x="([\d.]+)" y="5.48" width="([\d.]+)" height="([\d.]+)" fill="#ebebeb"', s
    )
    if not m:
        return None
    x, y, w, h = (float(m.group(i)) for i in range(1, 5))
    return x, y, x + w, y + h


def main():
    figs = sorted(f[:-4] for f in os.listdir(REFS) if f.endswith(".png"))
    rows = []
    for name in figs:
        rp, op = os.path.join(REFS, name + ".png"), os.path.join(OURS, name + ".png")
        if not (os.path.exists(rp) and os.path.exists(op)):
            continue
        r = np.asarray(Image.open(rp).convert("L")).astype(int)
        o = np.asarray(Image.open(op).convert("L")).astype(int)
        if r.shape != o.shape:
            rows.append((1e9, name, {"SIZE-MISMATCH": r.shape[1] - o.shape[1]}))
            continue
        mask = np.abs(r - o) > THRESH
        ys, xs = np.nonzero(mask)
        total = mask.size and int(mask.sum())
        pr = panel_rect(os.path.join(SVGS, name + ".svg"))
        regions = {}
        if pr:
            px0, py0, px1, py1 = pr
            regions["left-gutter"] = int(((xs < px0)).sum())
            regions["right-legend"] = int(((xs >= px1)).sum())
            inx = (xs >= px0) & (xs < px1)
            regions["top"] = int((inx & (ys < py0)).sum())
            regions["bottom"] = int((inx & (ys >= py1)).sum())
            regions["panel"] = int((inx & (ys >= py0) & (ys < py1)).sum())
        rows.append((total, name, regions))

    rows.sort(reverse=True)
    print(f"{'figure':22s} {'diff':>6s}  dominant regions")
    for total, name, reg in rows:
        if total == 0:
            continue
        parts = ", ".join(f"{k}={v}" for k, v in sorted(reg.items(), key=lambda kv: -kv[1]) if v)
        dom = max(reg.items(), key=lambda kv: kv[1])[0] if reg else "-"
        print(f"{name:22s} {total:6d}  [{dom}] {parts}")


if __name__ == "__main__":
    main()
