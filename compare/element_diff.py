#!/usr/bin/env python3
"""element_diff.py — element-level coordinate comparison: ref svglite SVG vs
ours. Parses circles/rects/lines/polylines/text (handles svglite single-quotes
and jplot double-quotes), pairs by kind+order, reports per-figure max pointwise
|Δ| px. Geometric bugs → large Δ on specific elements; AA noise → <0.6 px.
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
THRESH = float(os.environ.get("THRESH", "0.6"))


def attrs(tag):
    """extract attribute dict from a raw tag string, robust to quote style."""
    d = {}
    for m in re.finditer(r"""(\w+)=(?:"([^"]*)"|'([^']*)')""", tag):
        d[m.group(1)] = m.group(2) if m.group(2) is not None else m.group(3)
    return d


def F(a, k, default=None):
    v = a.get(k)
    if v is None:
        return default
    try:
        return float(re.sub(r"[^0-9.eE+-]", "", v))
    except (ValueError, re.error):
        return default


def elems(svg):
    svg = re.sub(r"<defs>.*?</defs>", "", svg, flags=re.S)
    out = {"circle": [], "rect": [], "poly": [], "text": []}
    for m in re.finditer(r"<(circle|rect|polyline|polygon|line|text)\b[^>]*>", svg):
        tag = m.group(0)
        kind = m.group(1)
        a = attrs(tag)
        if kind == "circle":
            out["circle"].append((F(a, "cx"), F(a, "cy"), F(a, "r")))
        elif kind == "rect":
            out["rect"].append((F(a, "x"), F(a, "y"), F(a, "width"), F(a, "height")))
        elif kind == "line":
            out["poly"].append(
                ((F(a, "x1"), F(a, "y1")), (F(a, "x2"), F(a, "y2")))
            )
        elif kind in ("polyline", "polygon"):
            pts = tuple(
                tuple(float(v) for v in p.split(","))
                for p in a.get("points", "").split()
                if "," in p
            )
            if pts:
                out["poly"].append(pts)
        elif kind == "text":
            content = re.search(r">([^<]*)<", tag)
            inner = re.search(r"<text\b[^>]*>([^<]*)</text>", svg[svg.find(tag):svg.find(tag) + 300])
            out["text"].append((F(a, "x"), F(a, "y"), F(a, "textLength"), None))
    # text content is outside the tag; refetch properly
    out["text"] = []
    for m in re.finditer(r"<text\b([^>]*)>([^<]*)</text>", svg):
        a = attrs(m.group(1))
        out["text"].append((F(a, "x"), F(a, "y"), F(a, "textLength"), m.group(2)))
    return out


def match_report(r, o):
    rep = []
    for kind in ("circle", "rect"):
        if not (r[kind] and o[kind]):
            continue
        if len(r[kind]) != len(o[kind]):
            rep.append(f"{kind}: count {len(r[kind])} vs {len(o[kind])}")
            continue
        worst, at = 0.0, ""
        for a, b in zip(r[kind], o[kind]):
            d = max(abs(x - y) for x, y in zip(a, b) if x is not None and y is not None)
            if d > worst:
                worst, at = d, f"{[round(v,1) if v else v for v in a]} vs {[round(v,1) if v else v for v in b]}"
        if worst > THRESH:
            rep.append(f"{kind}: maxΔ {worst:.2f}px @ {at}")
    if r["poly"] and o["poly"]:
        # pair polylines BY POINT COUNT (svg emission order differs: ours
        # interleaves data/grid; ref groups them). Only counts that exist on
        # both sides are compared, so invisible border paths don't misalign.
        worst, at = 0.0, ""
        rp = {}
        for p in r["poly"]:
            rp.setdefault(len(p), []).append(p)
        op = {}
        for p in o["poly"]:
            op.setdefault(len(p), []).append(p)
        if sorted(rp) != sorted(op):
            rep.append(f"poly: count-kinds {sorted(rp)} vs {sorted(op)}")
        for n in sorted(set(rp) & set(op)):
            for a, b in zip(sorted(rp[n]), sorted(op[n])):
                for (x1, y1), (x2, y2) in zip(a, b):
                    d = max(abs(x1 - x2), abs(y1 - y2))
                    if d > worst:
                        worst, at = d, f"n{n} {(round(x1),round(y1))}→{(round(x2),round(y2))}"
        if worst > THRESH:
            rep.append(f"poly: maxΔ {worst:.2f}px @ {at}")
    # text: pair by CONTENT only (coords may legitimately differ slightly);
    # sort occurrences by y so identical repeated labels align deterministically
    def group(ts):
        d = {}
        for x, y, tl, t in ts:
            d.setdefault(t, []).append((y if y is not None else 0.0, tl, x))
        for v in d.values():
            v.sort(key=lambda z: z[0])
        return d
    rt, ot = group(r["text"]), group(o["text"])
    for t in sorted(set(rt) & set(ot)):
        if len(rt[t]) != len(ot[t]):
            rep.append(f"text '{t}': count {len(rt[t])} vs {len(ot[t])}")
            continue
        for (y1, tl1, x1), (y2, tl2, x2) in zip(rt[t], ot[t]):
            dx = abs(x1 - x2) if x1 is not None and x2 is not None else 0
            dy = abs(y1 - y2)
            dtl = abs(tl1 - tl2) if (tl1 and tl2) else None
            if max(dx, dy) > THRESH or (dtl is not None and dtl > 0.4):
                rep.append(f"text '{t}': Δ({dx:.2f},{dy:.2f})px Δlen {dtl if dtl is not None else 'n/a'}")
    return rep


def main():
    names = sys.argv[1:] or sorted(
        f[:-4] for f in os.listdir(os.path.join(HERE, "specs")) if f.endswith(".json")
    )
    nbad = 0
    for name in names:
        rp = os.path.join(HERE, "refs", name + ".svg")
        op = os.path.join(HERE, "ours_svg", name + ".svg")
        if not (os.path.exists(rp) and os.path.exists(op)):
            continue
        rep = match_report(elems(open(rp).read()), elems(open(op).read()))
        if rep:
            nbad += 1
        print(f"[{'DIFF' if rep else 'OK '}] {name}")
        for line in rep:
            print("   ", line)
    print(f"\n{len(names)} figs, {nbad} with element deltas > {THRESH}px")


if __name__ == "__main__":
    main()
