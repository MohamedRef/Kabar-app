#!/usr/bin/env python3
"""Draft stroke data for Aksara Jawa calligraphy (PLAN §18).

Renders each glyph (or syllable cluster) with Noto Sans Javanese, thins it
to a one-pixel skeleton, splits the skeleton into segments between ends and
junctions, and chains the segments into strokes written left to right,
continuing straight through junctions where the line carries on. The result
is a *draft*: content/aksara/strokes.json is reviewed by hand (and by a
native writer before `verified: true`), and content/aksara/order.yaml holds
the corrections this script applies on top.

Usage: python3 scripts/aksara/strokes.py FONT.ttf content/aksara/glyphs.yaml > content/aksara/strokes.json
Needs Pillow (with libraqm for mark positioning) and numpy.
"""

import json
import math
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFont

SIZE = 320  # px per em while tracing
ORIGIN = (60, 480)  # pen origin (left, baseline) in the raster
N8 = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)]


def raster(font, text):
    right = font.getbbox(text, anchor="ls")[2]
    im = Image.new("L", (ORIGIN[0] * 2 + max(right, SIZE), 900), 0)
    ImageDraw.Draw(im).text(ORIGIN, text, font=font, fill=255, anchor="ls")
    return (np.array(im) > 127).astype(np.uint8)


def thin(img):
    """Zhang–Suen thinning."""
    img = img.copy()
    changed = True
    while changed:
        changed = False
        for step in (0, 1):
            p = np.pad(img, 1)
            p2, p3, p4 = p[:-2, 1:-1], p[:-2, 2:], p[1:-1, 2:]
            p5, p6, p7 = p[2:, 2:], p[2:, 1:-1], p[2:, :-2]
            p8, p9 = p[1:-1, :-2], p[:-2, :-2]
            ring = [p2, p3, p4, p5, p6, p7, p8, p9, p2]
            b = sum(ring[:8])
            a = sum(((ring[i] == 0) & (ring[i + 1] == 1)).astype(int) for i in range(8))
            if step == 0:
                c = (p2 * p4 * p6 == 0) & (p4 * p6 * p8 == 0)
            else:
                c = (p2 * p4 * p8 == 0) & (p2 * p6 * p8 == 0)
            m = (img == 1) & (b >= 2) & (b <= 6) & (a == 1) & c
            if m.any():
                img[m] = 0
                changed = True
    return img


def nbrs(pts, p):
    return [(p[0] + dy, p[1] + dx) for dy, dx in N8 if (p[0] + dy, p[1] + dx) in pts]


def components(pts):
    seen, out = set(), []
    for p in sorted(pts):
        if p in seen:
            continue
        comp, stack = [], [p]
        seen.add(p)
        while stack:
            q = stack.pop()
            comp.append(q)
            for r in nbrs(pts, q):
                if r not in seen:
                    seen.add(r)
                    stack.append(r)
        out.append(comp)
    return out


def graph(skel):
    """Segments between nodes (line ends and junctions), as point paths.
    Touching junction pixels form one node at their centroid, so a curve
    meeting a bar meets it at one point."""
    pts = set(zip(*map(lambda a: a.tolist(), np.nonzero(skel))))
    deg = {p: len(nbrs(pts, p)) for p in pts}
    node_px = {p for p in pts if deg[p] != 2}
    node_of, centers = {}, {}
    for comp in components(node_px):
        cid = len(centers)
        centers[cid] = (sum(q[0] for q in comp) / len(comp), sum(q[1] for q in comp) / len(comp))
        for q in comp:
            node_of[q] = cid
    segs = []
    chains = components(pts - node_px)
    for comp in chains:
        cset = set(comp)
        ends = [q for q in comp if len([r for r in nbrs(pts, q) if r in cset]) <= 1]
        attach = lambda q: sorted({node_of[r] for r in nbrs(pts, q) if r in node_of})
        if not ends:  # a ring with no junction
            ordered = walk(comp[0], cset)
            segs.append({"a": None, "b": None, "path": ordered + [ordered[0]]})
            continue
        ordered = walk(ends[0], cset)
        a_nodes, b_nodes = attach(ordered[0]), attach(ordered[-1])
        a = a_nodes[0] if a_nodes else None
        b = b_nodes[-1] if b_nodes else None
        if len(ordered) == 1 and len(a_nodes) > 1:
            a, b = a_nodes[0], a_nodes[1]
        path = ([centers[a]] if a is not None else []) + ordered + ([centers[b]] if b is not None else [])
        segs.append({"a": a, "b": b, "path": path})
    # Two junction nodes can touch directly, with no chain between them.
    for p in node_px:
        for r in nbrs(pts, p):
            if r in node_of and node_of[r] != node_of[p] and node_of[p] < node_of[r]:
                if not any({s["a"], s["b"]} == {node_of[p], node_of[r]} for s in segs):
                    segs.append({"a": node_of[p], "b": node_of[r], "path": [centers[node_of[p]], centers[node_of[r]]]})
    # Free line ends are nodes too: give each its own id.
    for s in segs:
        for k, i in (("a", 0), ("b", -1)):
            if s[k] is None and s["path"]:
                s[k] = ("end", s["path"][i])
    return segs


def walk(start, cset):
    order, seen, cur = [start], {start}, start
    while True:
        nxt = [r for r in nbrs(cset, cur) if r not in seen]
        if not nxt:
            return order
        # Prefer 4-neighbours, so the walk doesn't skip a pixel on a stair.
        nxt.sort(key=lambda r: abs(r[0] - cur[0]) + abs(r[1] - cur[1]))
        cur = nxt[0]
        seen.add(cur)
        order.append(cur)


def merge_close(segs, w):
    """Thinning leaves clusters of junctions where a thick terminal or a tight
    curl meets a line: nodes closer than the pen is thick become one, and
    the tiny loops between them go."""
    pos = {}
    for s in segs:
        pos[s["a"]] = s["path"][0]
        pos[s["b"]] = s["path"][-1]
    parent = {n: n for n in pos}

    def find(n):
        while parent[n] != n:
            parent[n] = parent[parent[n]]
            n = parent[n]
        return n

    for s in segs:
        if s["a"] != s["b"] and length(s["path"]) < 0.6 * w:
            parent[find(s["a"])] = find(s["b"])
    out = []
    for s in segs:
        a, b = find(s["a"]), find(s["b"])
        if a == b and length(s["path"]) < 1.5 * w:
            continue
        out.append({"a": a, "b": b, "path": s["path"]})
    return out


def prune(segs, spur):
    """Drops short dead-end spurs left by thinning at corners: a segment with
    one free end whose other end is a real junction. One pass only, so a
    hook made of several short pieces survives."""
    count = {}
    for s in segs:
        count[s["a"]] = count.get(s["a"], 0) + 1
        count[s["b"]] = count.get(s["b"], 0) + 1
    def spurious(s):
        ends = sorted((count[s["a"]], count[s["b"]]))
        return length(s["path"]) < spur and ends[0] == 1 and ends[1] >= 3
    return [s for s in segs if not spurious(s)]


def heading(path, from_start, reach=14):
    pts = path if from_start else path[::-1]
    a, b = pts[0], pts[min(len(pts) - 1, reach)]
    return math.atan2(b[0] - a[0], b[1] - a[1])


def chain(segs):
    """Chains segments into strokes: start at the leftmost free end, keep
    going through each junction along the straightest unused segment."""
    left = list(segs)
    strokes = []
    while left:
        ends = []
        for s in left:
            for at_a in (True, False):
                node = s["a"] if at_a else s["b"]
                free = sum(1 for t in left if node in (t["a"], t["b"]))
                y, x = s["path"][0] if at_a else s["path"][-1]
                ends.append((free != 1, round(x / 20), y, at_a, s))
        ends.sort(key=lambda e: e[:3])
        _, _, _, at_a, s = ends[0]
        path = list(s["path"] if at_a else s["path"][::-1])
        node = s["b"] if at_a else s["a"]
        left.remove(s)
        while True:
            out_dir = heading(path, False) + math.pi
            best = None
            for t in left:
                for t_at_a in (True, False):
                    if (t["a"] if t_at_a else t["b"]) != node:
                        continue
                    turn = abs((heading(t["path"], t_at_a) - out_dir + math.pi) % (2 * math.pi) - math.pi)
                    if best is None or turn < best[0]:
                        best = (turn, t, t_at_a)
            if best is None or best[0] > math.radians(80):
                break
            _, t, t_at_a = best
            path += (t["path"] if t_at_a else t["path"][::-1])[1:]
            node = t["b"] if t_at_a else t["a"]
            left.remove(t)
        strokes.append(path)
    return strokes


def length(path):
    return sum(math.dist(a, b) for a, b in zip(path, path[1:]))


def absorb_short(strokes, w):
    """A stroke barely longer than the pen is thick is a thinning artifact
    or a tiny hook: append it to the stroke whose end it touches."""
    strokes = [list(s) for s in strokes]
    changed = True
    while changed:
        changed = False
        for s in sorted(strokes, key=length):
            if length(s) > 2.5 * w:
                break
            others = [t for t in strokes if t is not s]
            best = None
            for t in others:
                for t_end in (0, -1):
                    for s_end in (0, -1):
                        d = math.dist(t[t_end], s[s_end])
                        if d <= 1.5 * w and (best is None or d < best[0]):
                            best = (d, t, t_end, s_end)
            strokes.remove(s)
            if best is not None:
                _, t, t_end, s_end = best
                piece = s if s_end == 0 else s[::-1]
                if t_end == -1:
                    t.extend(piece)
                else:
                    t[:0] = piece[::-1]
            elif length(s) > w:
                strokes.append(s)
                continue
            changed = True
            break
    return strokes


def chamfer(img):
    """Distance to the nearest background pixel (3-4 chamfer, /3)."""
    ys, xs = np.nonzero(img)
    y0, x0 = max(0, ys.min() - 1), max(0, xs.min() - 1)
    y1, x1 = min(img.shape[0], ys.max() + 2), min(img.shape[1], xs.max() + 2)
    h, w = img.shape
    big = 10**6
    d = np.where(img > 0, big, 0).astype(np.int64).tolist()
    for y in range(y0, y1):
        for x in range(x0, x1):
            if d[y][x]:
                for dy, dx, c in ((-1, -1, 4), (-1, 0, 3), (-1, 1, 4), (0, -1, 3)):
                    yy, xx = y + dy, x + dx
                    if 0 <= yy < h and 0 <= xx < w:
                        d[y][x] = min(d[y][x], d[yy][xx] + c)
    for y in range(y1 - 1, y0 - 1, -1):
        for x in range(x1 - 1, x0 - 1, -1):
            if d[y][x]:
                for dy, dx, c in ((1, 1, 4), (1, 0, 3), (1, -1, 4), (0, 1, 3)):
                    yy, xx = y + dy, x + dx
                    if 0 <= yy < h and 0 <= xx < w:
                        d[y][x] = min(d[y][x], d[yy][xx] + c)
    return np.array(d) / 3.0


def center_path(dist, a, goal):
    """Cheapest path through ink from a to any pixel in goal, preferring the
    middle of the line (cost falls with the distance to the edge)."""
    import heapq

    a = (int(round(a[0])), int(round(a[1])))
    best = {a: 0.0}
    prev = {}
    heap = [(0.0, a)]
    while heap:
        c, p = heapq.heappop(heap)
        if p in goal:
            path = [p]
            while path[-1] in prev:
                path.append(prev[path[-1]])
            return path[::-1]
        if c > best.get(p, 1e18):
            continue
        for dy, dx in N8:
            q = (p[0] + dy, p[1] + dx)
            if not (0 <= q[0] < dist.shape[0] and 0 <= q[1] < dist.shape[1]) or dist[q] <= 0:
                continue
            step = math.hypot(dy, dx) * (1.0 + 4.0 / (dist[q] + 0.5) ** 2)
            if c + step < best.get(q, 1e18):
                best[q] = c + step
                prev[q] = p
                heapq.heappush(heap, (c + step, q))
    return None


def bridge(strokes, img, w):
    """Thinning loses tight U-turns (suku, pangkon) and curled tails (tha,
    taling), where the line folds back closer than it is thick. Ink no stroke
    covers is found and drawn: between the two stroke ends it joins, or out
    from the one end it touches to its far tip."""
    from PIL import Image as PImage, ImageDraw as PDraw

    dist = chamfer(img)
    for _ in range(6):
        cover = PImage.new("L", (img.shape[1], img.shape[0]), 0)
        dr = PDraw.Draw(cover)
        for st in strokes:
            dr.line([(p[1], p[0]) for p in st], fill=255, width=int(w * 1.6), joint="curve")
            for p in (st[0], st[-1]):
                dr.ellipse((p[1] - w * 0.8, p[0] - w * 0.8, p[1] + w * 0.8, p[0] + w * 0.8), fill=255)
        left = (img > 0) & (np.array(cover) == 0)
        blobs = [b for b in components(set(zip(*map(lambda a: a.tolist(), np.nonzero(left))))) if len(b) > w * w * 0.8]
        if not blobs:
            return strokes
        changed = False
        for blob in sorted(blobs, key=len, reverse=True):
            bset = set(blob)
            touch = []
            for i, st in enumerate(strokes):
                for end in (0, -1):
                    e = st[end]
                    if min(math.dist(e, q) for q in blob) <= w * 1.4:
                        touch.append((i, end))
            if not touch:
                continue
            i, end = touch[0]
            st = strokes[i] if end == -1 else strokes[i][::-1]
            others = [(j, e2) for j, e2 in touch[1:] if j != i]
            if others:
                j, e2 = others[0]
                tgt = strokes[j][e2]
                goal = {(int(round(tgt[0])) + dy, int(round(tgt[1])) + dx) for dy in range(-2, 3) for dx in range(-2, 3)}
                path = center_path(dist, st[-1], goal)
                if not path:
                    continue
                other = strokes[j] if e2 == 0 else strokes[j][::-1]
                joined = st + path[1:-1] + other
                at = min(i, j)
                strokes = [joined if k == at else s for k, s in enumerate(strokes) if k != max(i, j)]
            else:
                # Out to the blob's far tip: the pixel farthest along the ink.
                far = max(blob, key=lambda q: math.dist(q, st[-1]))
                path = center_path(dist, st[-1], {far})
                if not path:
                    continue
                strokes = [st + path[1:] if k == i else s for k, s in enumerate(strokes)]
            changed = True
            break
        if not changed:
            return strokes
    return strokes


def rdp(pts, eps):
    if len(pts) < 3:
        return pts
    a, b = np.array(pts[0], float), np.array(pts[-1], float)
    ab = b - a
    ll = float(ab @ ab)
    best, at = -1.0, 0
    for i, p in enumerate(pts[1:-1], 1):
        p = np.array(p, float)
        t = 0.0 if ll == 0 else max(0.0, min(1.0, float((p - a) @ ab) / ll))
        d = float(np.linalg.norm(p - (a + t * ab)))
        if d > best:
            best, at = d, i
    if best > eps:
        return rdp(pts[: at + 1], eps)[:-1] + rdp(pts[at:], eps)
    return [pts[0], pts[-1]]


def em(p):
    """(row, col) raster pixel -> [x, y] in 1/1000 em from the pen origin;
    y grows downwards, baseline at 0."""
    y, x = p
    return [round((x - ORIGIN[0]) * 1000 / SIZE), round((y - ORIGIN[1]) * 1000 / SIZE)]


def trace(font, text, order=None):
    img = raster(font, text)
    stroke_px = int(img.sum() / max(1, thin(img).sum()))
    segs = prune(merge_close(graph(thin(img)), stroke_px), spur=int(stroke_px * 1.6))
    strokes = bridge(absorb_short(chain(segs), stroke_px), img, stroke_px)
    out = [[em(p) for p in rdp(s, 1.2)] for s in strokes]
    out = [s for s in out if len(s) >= 2]
    # Descenders (suku, pangkon, wignyan, taling's tail) are written from the
    # top down: a stroke that starts below the baseline and ends above it is
    # turned round.
    out = [s[::-1] if s[0][1] > 120 and s[-1][1] < s[0][1] - 300 else s for s in out]
    if order:
        out = apply_order(out, order)
    return out, round(stroke_px * 1000 / SIZE)


def apply_order(strokes, order):
    """order.yaml entry: a list of stroke indices (as drafted, from 0), each
    optionally suffixed with r to reverse it; '3+4' joins two strokes."""
    out, used = [], set()
    for item in order:
        used.update(int(p.rstrip("r")) for p in str(item).split("+"))
    for item in order:
        parts = []
        for piece in str(item).split("+"):
            rev = piece.endswith("r")
            s = strokes[int(piece.rstrip("r"))]
            parts.append(s[::-1] if rev else s)
        joined = parts[0]
        for p in parts[1:]:
            joined = joined + p[1:] if joined[-1] == p[0] else joined + p
        out.append(joined)
    return out + [s for i, s in enumerate(strokes) if i not in used]


def load_yaml(path):
    import yaml  # PyYAML

    with open(path, encoding="utf-8") as f:
        return yaml.safe_load(f)


def main():
    font = ImageFont.truetype(sys.argv[1], SIZE, layout_engine=ImageFont.Layout.RAQM)
    spec = load_yaml(sys.argv[2])
    out = {"schema": "kabar.aksara.strokes/v1", "font": "Noto Sans Javanese", "units_per_em": 1000, "glyphs": []}
    for g in spec["glyphs"]:
        strokes, width = trace(font, g["text"], g.get("order"))
        out["glyphs"].append(
            {
                "id": g["id"],
                "text": g["text"],
                "latin": g["latin"],
                "kind": g["kind"],
                **({"sign": g["sign"]} if g.get("sign") else {}),
                "width": width,
                "strokes": strokes,
                "verified": False,
            }
        )
    json.dump(out, sys.stdout, ensure_ascii=False, indent=1)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
