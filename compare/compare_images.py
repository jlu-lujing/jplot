#!/usr/bin/env python3
"""compare_images.py — pixel-diff jplot vs ggplot2 under an identical rasteriser.

Pipeline (run after compare/export_specs.R):
  1. ggplot2 exports each figure as SVG via svglite (text kept as <text>,
     pt→px normalised in the R script).
  2. BOTH svglite SVGs and jplot SVGs are rasterised by jplot's own
     resvg/tiny-skia with one shared fontdb (Arial-first). This removes
     cairo-vs-resvg antialiasing differences that would otherwise dominate
     the diff and make geometry comparison meaningless.
  3. Diff per figure with thresholds 16/32/64; dump side-by-side + heatmaps.

Run: python3 compare/compare_images.py [--cairo-refs]   (default refs_png/)
"""
import os, sys
import numpy as np
from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
REF_DIR = sys.argv[1] if len(sys.argv) > 1 and not sys.argv[1].startswith("-") else "refs_png"
REFS = os.path.join(HERE, REF_DIR)
OURS = os.path.join(HERE, "ours")
DIFF = os.path.join(HERE, "diff")
os.makedirs(DIFF, exist_ok=True)

def load(p):
    return np.asarray(Image.open(p).convert("RGB"), dtype=np.int16)

def panel_bbox(img):
    grey = (np.abs(img[:,:,0].astype(int)-235)<=2) & (np.abs(img[:,:,1].astype(int)-235)<=2)
    cols = np.where(grey.sum(axis=0)>200)[0]; rows = np.where(grey.sum(axis=1)>200)[0]
    if len(cols)==0 or len(rows)==0: return None
    return (int(cols.min()), int(cols.max()), int(rows.min()), int(rows.max()))

def main():
    names = sorted(f[:-4] for f in os.listdir(REFS) if f.endswith(".png"))
    rows=[]
    for n in names:
        op = os.path.join(OURS, n + ".png")
        if not os.path.exists(op):
            print(f"{n:24s} MISSING render"); continue
        r, o = load(os.path.join(REFS, n+".png")), load(op)
        h = min(r.shape[0], o.shape[0]); w = min(r.shape[1], o.shape[1])
        r, o = r[:h,:w], o[:h,:w]
        d = np.abs(r - o).max(axis=2)
        p16, p32, p64 = [(d>t).mean()*100 for t in (16,32,64)]
        pb_r, pb_o = panel_bbox(r), panel_bbox(o)
        drift = "" if (pb_r and pb_o and abs(pb_r[0]-pb_o[0])<=1 and abs(pb_r[1]-pb_o[1])<=1) else f" PANEL-MISMATCH ref{pb_r} ours{pb_o}"
        sep = np.full((h,6,3),255,np.uint8)
        Image.fromarray(np.hstack([r.astype(np.uint8),sep,o.astype(np.uint8)]).astype(np.uint8)).save(os.path.join(DIFF,n+"_side.png"))
        heat = np.zeros((h,w,3),np.uint8)
        heat[:,:,0]=np.clip(d,0,255).astype(np.uint8)
        heat[:,:,2]=np.clip(255-d*4,0,255).astype(np.uint8)
        Image.fromarray(heat).save(os.path.join(DIFF,n+"_diff.png"))
        rows.append((n,p16,p32,p64,drift))
    print(f"{'figure':22s}{'>16':>8}{'>32':>8}{'>64':>8}")
    for n,a,b,c,d_ in rows: print(f"{n:22s}{a:7.2f}%{b:7.3f}%{c:7.3f}%{d_}")
    m=np.mean([[t[1],t[2],t[3]] for t in rows],axis=0) if rows else [0,0,0]
    print("MEAN %6.2f%% %6.3f%% %6.3f%%"%tuple(m))
    # overview montage
    cw,ch=170,120
    if rows:
        W=cw*len(rows)+4*(len(rows)+1); H=ch*2+16
        canvas=Image.new("RGB",(W,H),(255,255,255)); d=ImageDraw.Draw(canvas)
        for i,n in enumerate([t[0] for t in rows]):
            x=4+i*(cw+4)
            canvas.paste(Image.open(os.path.join(REFS,n+".png")).resize((cw,ch)),(x,4))
            canvas.paste(Image.open(os.path.join(OURS,n+".png")).resize((cw,ch)),(x,ch+8))
            d.text((x,ch*2+1),n.split("_")[0],fill=(0,0,0))
        canvas.save(os.path.join(HERE,"overview.png"))
    return 0

if __name__=="__main__":
    sys.exit(main())
