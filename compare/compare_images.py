#!/usr/bin/env python3
"""compare_images.py — pixel-diff jplot renders vs ggplot2 reference PNGs.

Outputs compare/diff/<name>_diff.png (heatmap) and <name>_side.png, prints a
metrics table: %pixels differing (threshold 16/channel), panel-bbox drift,
mean abs error. Run from repo root: python3 compare/compare_images.py
"""
import os, sys
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
REFS = os.path.join(HERE, "refs")
OURS = os.path.join(HERE, "ours")
DIFF = os.path.join(HERE, "diff")
os.makedirs(DIFF, exist_ok=True)

def load(p):
    return np.asarray(Image.open(p).convert("RGB"), dtype=np.int16)

def panel_bbox(img):
    """locate the grey panel (ggplot2 grey92 = 0.9216*255 = 235) as the region
    where pixels are ~uniform 230..239 across all channels, largest run."""
    g = np.abs(img[:, :, 0].astype(int) - img[:, :, 1].astype(int)) < 3
    grey = (np.abs(img[:, :, 0].astype(int) - 235) <= 2) & g
    rows = np.where(grey.sum(axis=1) > 50)[0]
    cols = np.where(grey.sum(axis=0) > 50)[0]
    if len(rows) == 0 or len(cols) == 0:
        return None
    return (cols.min(), cols.max(), rows.min(), rows.max())

def main():
    names = sorted(os.path.splitext(f)[0] for f in os.listdir(REFS) if f.endswith(".png"))
    rows = []
    for n in names:
        rp, op = os.path.join(REFS, n + ".png"), os.path.join(OURS, n + ".png")
        if not os.path.exists(op):
            print(f"{n:24s} MISSING render"); continue
        r, o = load(rp), load(op)
        h = min(r.shape[0], o.shape[0]); w = min(r.shape[1], o.shape[1])
        r, o = r[:h, :w], o[:h, :w]
        d = np.abs(r - o).max(axis=2)
        pct = float((d > 16).mean() * 100)
        pb_r, pb_o = panel_bbox(r), panel_bbox(o)
        drift = ""
        if pb_r and pb_o:
            dx0, dx1 = pb_o[0]-pb_r[0], pb_o[1]-pb_r[1]
            dy0, dy1 = pb_o[2]-pb_r[2], pb_o[3]-pb_r[3]
            drift = f"panel Δx=({dx0:+d},{dx1:+d}) Δy=({dy0:+d},{dy1:+d})"
        elif pb_r:
            drift = "panel NOT FOUND in ours"
        elif pb_o:
            drift = "panel NOT FOUND in ref"
        # side + diff viz
        sep = np.full((h, 6, 3), 255, np.uint8)
        Image.fromarray(np.hstack([r.astype(np.uint8), sep, o.astype(np.uint8)]).astype(np.uint8)).save(os.path.join(DIFF, n + "_side.png"))
        heat = (np.clip(d, 0, 60) * 4).astype(np.uint8)
        heat = np.stack([heat, np.zeros_like(heat), 60 - heat * 0 // 1 + 0 * heat], axis=2)
        heat[:, :, 0] = np.clip(d, 0, 255).astype(np.uint8)
        heat[:, :, 1] = np.zeros((h, w), np.uint8)
        heat[:, :, 2] = np.clip(255 - d * 4, 0, 255).astype(np.uint8)
        Image.fromarray(heat).save(os.path.join(DIFF, n + "_diff.png"))
        rows.append((n, pct, drift, f"{int(d.mean())}", f"{int(d.max())}"))
    print(f"{'figure':24s} {'%diff>16':>9s} {'meanΔ':>6s} {'maxΔ':>5s}  panel-drift")
    for n, pct, drift, mean, mx in rows:
        print(f"{n:24s} {pct:8.2f}% {mean:>6s} {mx:>5s}  {drift}")
    total = np.mean([r[1] for r in rows]) if rows else 0
    print(f"{'TOTAL mean':24s} {total:8.2f}%")

if __name__ == "__main__":
    sys.exit(main())
