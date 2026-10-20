#!/usr/bin/env python3
"""Emit Tidewall terrain.svg.

All shapes, symbols, colors and layer structure below are hand-written SVG. The only thing
computed is *placement*: decorations are dropped onto the hand-authored top-edge curves by
sampling them, so tufts and flowers sit exactly on the surface.
"""
import math, random
from pathlib import Path

HERE = Path(__file__).resolve().parent

# Top edges: (start, [(c1, c2, end), ...]). Hand-authored.
TOPS = {
    "A": ((-10, 402), [((30, 394), (70, 378), (118, 382)), ((160, 386), (186, 404), (230, 399)), ((262, 395), (284, 378), (314, 386))]),
    "B": ((350, 336), [((372, 320), (408, 314), (440, 320)), ((466, 325), (490, 316), (508, 330))]),
    "C": ((540, 418), [((566, 404), (600, 396), (640, 402)), ((680, 408), (702, 392), (736, 385)), ((764, 380), (790, 388), (812, 384))]),
}
# Remainder of each landmass outline after its top edge. Hand-authored.
RESTS = {
    "A": "C 324,392 330,412 326,436 C 322,462 306,486 300,520 C 296,548 300,580 296,640 L -10,640 Z",
    "B": "C 518,340 512,356 500,370 C 488,386 474,400 462,430 C 456,446 450,470 444,494 C 436,470 428,450 418,432 C 404,410 384,394 368,380 C 352,366 342,350 350,336 Z",
    "C": "L 812,640 L 558,640 C 552,580 548,540 538,500 C 530,470 524,440 540,418 Z",
}


def bumpy_top(name, per_seg=90):
    """Hand-authored top curve, roughened along its normal so silhouettes read as rock, not
    vector blobs. Endpoints are pinned so the hand-authored sides still join cleanly."""
    pts = sample(name, per_seg * len(TOPS[name][1]), raw=True)
    n = len(pts)
    out = []
    for i, (x, y, ang) in enumerate(pts):
        pin = min(i, n - 1 - i) / 12
        k = min(1.0, pin)
        off = k * (1.5 * math.sin(x / 7.3 + 1.1) + 1.1 * math.sin(x / 3.1 + 0.4) + 1.8 * max(0.0, math.sin(x / 23.0 + 2.0)) ** 3)
        r = math.radians(ang)
        out.append((x + off * math.sin(r), y - off * math.cos(r), ang))
    return out


def top_d(name):
    (sx, sy), _ = TOPS[name]
    pts = bumpy_top(name)
    return f"M {sx},{sy} " + " ".join(f"L {x:.2f},{y:.2f}" for x, y, _ in pts[1:]) + f" L {TOPS[name][1][-1][2][0]},{TOPS[name][1][-1][2][1]}"


def land_d():
    return " ".join(top_d(n) + " " + RESTS[n] for n in TOPS)


def sample(name, n, raw=False):
    """n points (x, y, slope_deg) spread along a top edge (raw=True: n evenly spaced in t)."""
    (sx, sy), segs = TOPS[name]
    pts, p0 = [], (sx, sy)
    for c1, c2, p3 in segs:
        for i in range(120):
            t = i / 120
            u = 1 - t
            x = u**3 * p0[0] + 3 * u * u * t * c1[0] + 3 * u * t * t * c2[0] + t**3 * p3[0]
            y = u**3 * p0[1] + 3 * u * u * t * c1[1] + 3 * u * t * t * c2[1] + t**3 * p3[1]
            dx = 3 * u * u * (c1[0] - p0[0]) + 6 * u * t * (c2[0] - c1[0]) + 3 * t * t * (p3[0] - c2[0])
            dy = 3 * u * u * (c1[1] - p0[1]) + 6 * u * t * (c2[1] - c1[1]) + 3 * t * t * (p3[1] - c2[1])
            pts.append((x, y, math.degrees(math.atan2(dy, dx))))
        p0 = p3
    if raw:
        res = 120 * len(segs)
        return [pts[min(res - 1, int(i * res / n))] for i in range(n)]
    step = len(pts) / n
    return [pts[int(i * step + step / 2)] for i in range(n)]


# 96x72 tile, three courses of irregular sandstone blocks. Hand-authored polygons.
STONES = [
    [(2, 3), (27, 2), (29, 12), (26, 22), (3, 21), (0, 12)],
    [(31, 2), (55, 4), (57, 21), (33, 22), (30, 12)],
    [(59, 3), (78, 2), (82, 11), (79, 21), (60, 21)],
    [(84, 3), (94, 2), (95, 12), (94, 21), (84, 22)],
    [(-14, 25), (12, 24), (15, 35), (12, 46), (-12, 46), (-16, 36)],
    [(18, 25), (44, 24), (47, 36), (44, 46), (19, 46), (17, 35)],
    [(49, 24), (78, 25), (79, 35), (77, 46), (50, 45), (48, 35)],
    [(17, 48), (40, 48), (42, 59), (40, 70), (17, 70), (15.5, 59)],
    [(44, 48), (64, 49), (66, 59), (64, 70), (45, 70), (43, 59)],
    [(68, 48), (86, 48), (88, 59), (86, 70), (68, 70), (66, 59)],
    [(90, 48), (108, 49), (110, 59), (108, 70), (90, 70), (88, 59)],
]
TINTS = ["#d68e5a", "#cf8455", "#dc9a64", "#d48a58", "#c97f52", "#d99360", "#d08757", "#dc9762", "#cc8254", "#d68e5a", "#d18a59"]


def stones_tile():
    out = []
    for i, pts in enumerate(STONES):
        xs = [x for x, _ in pts]
        shifts = [0] + ([96] if min(xs) < 0 else []) + ([-96] if max(xs) > 96 else [])
        for dx in shifts:
            d = "M " + " L ".join(f"{x + dx},{y}" for x, y in pts) + " Z"
            (x0, y0), (x1, y1) = pts[0], pts[1]
            out.append(f'<path d="{d}" transform="translate(1,1.6)" fill="#9c5638" stroke="#9c5638" stroke-width="2.4" stroke-linejoin="round"/>')
            out.append(f'<path d="{d}" fill="{TINTS[i]}" stroke="{TINTS[i]}" stroke-width="2.4" stroke-linejoin="round"/>')
            out.append(f'<path d="M {x0 + dx + 1.5},{y0 + 1.2} L {x1 + dx - 1.5},{y1 + 1.2}" stroke="#f3bb84" stroke-width="1.6" stroke-linecap="round"/>')
    return "\n        ".join(out)


def decorations():
    rnd = random.Random(4)
    out = []
    for name, count in (("A", 26), ("B", 13), ("C", 24)):
        rough = bumpy_top(name)
        for i in range(count):
            x, y, ang = rough[int((i + 0.5) * len(rough) / count)]
            if not -5 < x < 805:
                continue
            k = rnd.random()
            sym = "tuftL" if k < 0.45 else ("tuftS" if k < 0.8 else "flower")
            s = 0.8 + rnd.random() * 0.45
            flip = -1 if rnd.random() < 0.5 else 1
            out.append(f'<use href="#{sym}" transform="translate({x + rnd.uniform(-3, 3):.1f} {y + 1.5:.1f}) rotate({ang:.1f}) scale({flip * s:.2f} {s:.2f})"/>')
    return "\n    ".join(out)


SVG = '''<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600" viewBox="0 0 800 600">
  <!-- Tidewall terrain foreground. Opaque = solid ground (the sim's carve mask), transparent = sky.
       Light from upper-left. Generated by build_terrain.py; edit there. -->
  <defs>
    <path id="land" d="{LAND}"/>
    <clipPath id="landClip"><use href="#land"/></clipPath>

    <!-- sandstone blocks, 96x72 tile -->
    <pattern id="stones" width="96" height="72" patternUnits="userSpaceOnUse">
      <rect width="96" height="72" fill="#6e3436"/>
      <g>
        {STONES}
      </g>
      <g fill="none" stroke="#9c5638" stroke-width="0.9" stroke-linecap="round">
        <path d="M 10,6 l 3,3 l 1,3 M 66,8 l -2,4 M 28,30 l 4,2 l 1,3 M 56,52 l 3,3 M 40,7 l -2,3 M 80,30 l 2,4"/>
      </g>
      <g fill="#e9a974"><circle cx="20" cy="9" r="1"/><circle cx="70" cy="58" r="0.9"/><circle cx="34" cy="38" r="1"/><circle cx="8" cy="40" r="0.8"/></g>
    </pattern>
    <linearGradient id="depth" gradientUnits="userSpaceOnUse" x1="0" y1="330" x2="0" y2="600">
      <stop offset="0" stop-color="#3a1838" stop-opacity="0"/>
      <stop offset="0.35" stop-color="#3a1838" stop-opacity="0.15"/>
      <stop offset="1" stop-color="#3a1838" stop-opacity="0.6"/>
    </linearGradient>
    <filter id="mottle" x="0" y="0" width="100%" height="100%">
      <feTurbulence type="fractalNoise" baseFrequency="0.012 0.03" numOctaves="3" seed="21" result="n"/>
      <feColorMatrix in="n" type="matrix" values="0 0 0 0 0.35  0 0 0 0 0.12  0 0 0 0 0.25  0 0 0 2.4 -1.1"/>
    </filter>
    <!-- cap = land minus land shifted down; drippy lower edge from displacement -->
    <filter id="drip" x="-5%" y="-5%" width="110%" height="110%">
      <feTurbulence type="fractalNoise" baseFrequency="0.07 0.01" numOctaves="2" seed="5" result="n"/>
      <!-- R := 0.5 so displacement is vertical only; otherwise the cap leaks down walls -->
      <feColorMatrix in="n" type="matrix" values="0 0 0 0 0.5  0 1.8 0 0 -0.4  0 0 0 0 0  0 0 0 0 1" result="ny"/>
      <feDisplacementMap in="SourceGraphic" in2="ny" scale="6" xChannelSelector="R" yChannelSelector="G" result="d"/>
      <!-- blur + alpha threshold rounds the fringe into scallops instead of streaks -->
      <feGaussianBlur in="d" stdDeviation="1.3" result="b"/>
      <feComponentTransfer in="b"><feFuncA type="linear" slope="8" intercept="-3.5"/></feComponentTransfer>
    </filter>
    <mask id="capMask" maskUnits="userSpaceOnUse" x="0" y="0" width="800" height="600">
      <rect width="800" height="600" fill="#fff"/>
      <use href="#land" transform="translate(0,7)" fill="#000"/>
    </mask>
    <mask id="fringeMask" maskUnits="userSpaceOnUse" x="0" y="0" width="800" height="600">
      <rect width="800" height="600" fill="#fff"/>
      <use href="#land" transform="translate(0,11)" fill="#000" filter="url(#drip)"/>
    </mask>
    <mask id="capLitMask" maskUnits="userSpaceOnUse" x="0" y="0" width="800" height="600">
      <rect width="800" height="600" fill="#fff"/>
      <use href="#land" transform="translate(0,3)" fill="#000"/>
    </mask>
    <mask id="capShadeMask" maskUnits="userSpaceOnUse" x="0" y="0" width="800" height="600">
      <rect width="800" height="600" fill="#fff"/>
      <use href="#land" transform="translate(0,15)" fill="#000" filter="url(#drip)"/>
    </mask>

    <filter id="feather" x="-5%" y="-10%" width="110%" height="120%"><feGaussianBlur stdDeviation="9"/></filter>
    <mask id="topLightMask" maskUnits="userSpaceOnUse" x="0" y="0" width="800" height="600">
      <rect width="800" height="600" fill="#fff"/>
      <use href="#land" transform="translate(0,34)" fill="#000" filter="url(#feather)"/>
    </mask>
    <g id="boulder">
      <path d="M -20,8 C -24,-2 -18,-14 -4,-16 C 10,-18 22,-10 22,0 C 22,10 12,15 0,15 C -10,15 -17,13 -20,8 Z" transform="translate(1.5,2.5)" fill="#7e3f3a"/>
      <path d="M -20,8 C -24,-2 -18,-14 -4,-16 C 10,-18 22,-10 22,0 C 22,10 12,15 0,15 C -10,15 -17,13 -20,8 Z" fill="#c9865a" stroke="#6e3436" stroke-width="1.6"/>
      <path d="M -16,-4 C -14,-10 -8,-13 0,-13.5" fill="none" stroke="#f3bb84" stroke-width="2.2" stroke-linecap="round"/>
      <path d="M 4,12 C 12,11 18,7 19,1" fill="none" stroke="#9c5638" stroke-width="2.4" stroke-linecap="round"/>
      <path d="M -2,-6 l 4,4 l -1,5 M 8,-2 l 3,3" fill="none" stroke="#8a4a34" stroke-width="1" stroke-linecap="round"/>
    </g>

    <!-- decoration symbols (origin = attach point) -->
    <g id="tuftL" stroke="#2a1a2e" stroke-width="0.9" stroke-linejoin="round">
      <path d="M -6,1 C -6,-4 -8,-7 -10,-9 C -5,-8 -3,-5 -2,-2 C -2,-7 0,-11 2,-13 C 2,-9 3,-5 2,-1 C 4,-5 7,-7 10,-8 C 8,-5 6,-2 6,1 Z" fill="#8ad64e"/>
      <path d="M -2,-2 C -2,-7 0,-11 2,-13 C 1.4,-9 1.4,-5 1,-1.5 Z" fill="#c4f27c" stroke="none"/>
    </g>
    <g id="tuftS" stroke="#2a1a2e" stroke-width="0.9" stroke-linejoin="round">
      <path d="M -4,1 C -4,-2 -5,-4 -7,-6 C -3,-5 -1.5,-3 -1,-1 C 0,-4 2,-6 4,-7 C 3,-4 3,-2 4,1 Z" fill="#74c443"/>
    </g>
    <g id="flower" stroke="#2a1a2e" stroke-linejoin="round">
      <path d="M 0,1 C 0,-3 0.5,-6 1,-9" fill="none" stroke-width="2.4"/>
      <path d="M 0,1 C 0,-3 0.5,-6 1,-9" fill="none" stroke="#5aa83a" stroke-width="1"/>
      <circle cx="1" cy="-10.5" r="3" fill="#ff7fae" stroke-width="0.9"/>
      <circle cx="0.2" cy="-11.4" r="1.1" fill="#ffd0e2" stroke="none"/>
      <path d="M -5,1 C -5,-2 -6,-3 -7.5,-4.5 C -4,-4 -2,-2 -1.5,0 Z" fill="#74c443" stroke-width="0.8"/>
    </g>
    <g id="fossil">
      <circle r="7.2" fill="#f6e0b4" stroke="#7a3d3c" stroke-width="1.2"/>
      <path d="M 1,0 A 1 1 0 0 0 -1,0 A 2 2 0 0 0 3,0 A 3 3 0 0 0 -3,0 A 4 4 0 0 0 5,0 A 5 5 0 0 0 -5,0 A 6 6 0 0 0 7,0" fill="none" stroke="#b07a52" stroke-width="1"/>
      <path d="M -4.8,-3.4 A 6 6 0 0 1 -0.6,-6.4" fill="none" stroke="#fffaf0" stroke-width="1.2" stroke-linecap="round"/>
    </g>
    <g id="star" stroke="#2a1a2e" stroke-width="0.9" stroke-linejoin="round">
      <path d="M 0,-8 L 2.2,-2.6 L 7.6,-2.4 L 3.4,1.2 L 4.8,6.8 L 0,3.6 L -4.8,6.8 L -3.4,1.2 L -7.6,-2.4 L -2.2,-2.6 Z" fill="#ff8a5c"/>
      <g fill="#ffd2a0" stroke="none"><circle cy="-4.2" r="0.7"/><circle cx="3.8" cy="-1.4" r="0.6"/><circle cx="-3.8" cy="-1.4" r="0.6"/><circle cx="2.2" cy="3.4" r="0.6"/><circle cx="-2.2" cy="3.4" r="0.6"/><circle r="0.8"/></g>
    </g>
    <g id="barnacles" stroke="#2a1a2e" stroke-width="0.8" stroke-linejoin="round">
      <path d="M -6,2 L -4.6,-2.6 L -2,-2.6 L -0.8,2 Z" fill="#efe4cf"/><ellipse cx="-3.3" cy="-2.6" rx="1.3" ry="0.6" fill="#6a5a6a"/>
      <path d="M 0,2 L 1.6,-4 L 4.6,-4 L 6,2 Z" fill="#f6ecd8"/><ellipse cx="3.1" cy="-4" rx="1.5" ry="0.7" fill="#6a5a6a"/>
      <path d="M 5,3 L 6,0 L 8,0 L 9,3 Z" fill="#e6d8c0"/>
    </g>
    <g id="coral" stroke="#2a1a2e" stroke-width="0.9" stroke-linejoin="round" stroke-linecap="round">
      <path d="M 0,2 C 0,-4 -1,-8 -5,-12 M 0,-3 C 2,-7 5,-9 8,-14 M -3,-9 C -6,-10 -8,-12 -9,-16 M 4,-9 C 3,-12 3,-14 4,-17" fill="none" stroke-width="4.4"/>
      <path d="M 0,2 C 0,-4 -1,-8 -5,-12 M 0,-3 C 2,-7 5,-9 8,-14 M -3,-9 C -6,-10 -8,-12 -9,-16 M 4,-9 C 3,-12 3,-14 4,-17" fill="none" stroke="#ff6f91" stroke-width="2.6"/>
      <path d="M -0.6,0 C -0.6,-4 -1.4,-7 -4.6,-10.6 M 1,-4 C 2.6,-7 5,-9 7,-12.6" fill="none" stroke="#ffb3c6" stroke-width="0.8"/>
    </g>
    <g id="kelp" stroke-linecap="round" fill="none">
      <path d="M 0,0 C 4,8 -4,16 1,24 C 5,31 -2,38 2,46" stroke="#2a1a2e" stroke-width="7"/>
      <path d="M 0,0 C 4,8 -4,16 1,24 C 5,31 -2,38 2,46" stroke="#2f8070" stroke-width="4.4"/>
      <path d="M 1,8 C 2,12 -2,16 0,20 M 2,30 C 3,34 0,38 1,42" stroke="#1f5e56" stroke-width="1.2"/>
      <path d="M 0.6,2 C 3.6,8 -2.4,14 1.4,21" stroke="#6cc0a8" stroke-width="0.9"/>
    </g>
  </defs>

  <!-- hanging kelp behind the rock lips -->
  <g>
    <use href="#kelp" transform="translate(420,425) scale(0.9)"/>
    <use href="#kelp" transform="translate(452,440) scale(1.1 0.8)"/>
    <use href="#kelp" transform="translate(302,505) scale(0.8)"/>
  </g>

  <g clip-path="url(#landClip)">
    <rect width="800" height="600" fill="#d98f5c"/>
    <rect width="800" height="600" fill="url(#stones)"/>
    <rect width="800" height="600" filter="url(#mottle)" opacity="0.55"/>
    <rect width="800" height="600" fill="url(#depth)"/>

    <!-- sediment strata: break up the tile repeat -->
    <g fill="none" stroke-linecap="round">
      <path d="M -10,452 C 60,444 120,458 200,450 C 250,445 290,452 330,446" stroke="#5a2a3a" stroke-width="5" opacity="0.35"/>
      <path d="M -10,456 C 60,448 120,462 200,454 C 250,449 290,456 330,450" stroke="#f3bb84" stroke-width="1.2" opacity="0.5"/>
      <path d="M -10,540 C 80,530 160,548 240,536 C 270,532 290,538 310,534" stroke="#4a2034" stroke-width="7" opacity="0.35"/>
      <path d="M 530,470 C 600,460 660,476 730,466 C 770,461 790,466 812,462" stroke="#5a2a3a" stroke-width="5" opacity="0.35"/>
      <path d="M 530,474 C 600,464 660,480 730,470 C 770,465 790,470 812,466" stroke="#f3bb84" stroke-width="1.2" opacity="0.5"/>
      <path d="M 540,556 C 620,546 700,560 812,548" stroke="#4a2034" stroke-width="7" opacity="0.35"/>
      <path d="M 360,368 C 400,360 450,372 506,362" stroke="#5a2a3a" stroke-width="4" opacity="0.3"/>
    </g>

    <rect width="800" height="600" fill="#ffd9a0" opacity="0.28" mask="url(#topLightMask)"/>

    <!-- boulders and cracks break the block grid -->
    <use href="#boulder" transform="translate(36,436) rotate(-8) scale(0.9)"/>
    <use href="#boulder" transform="translate(292,520) rotate(-12) scale(0.8)"/>
    <use href="#boulder" transform="translate(470,412) rotate(20) scale(0.6)"/>
    <use href="#boulder" transform="translate(762,452) rotate(8) scale(0.95)"/>
    <use href="#boulder" transform="translate(584,584) rotate(-6) scale(1.1)"/>
    <g fill="none" stroke="#4a2034" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" opacity="0.7">
      <path d="M 196,410 l 6,14 l -4,12 l 8,16 M 202,424 l 10,4"/>
      <path d="M 640,420 l -4,16 l 6,10 l -2,18 M 636,436 l -10,6"/>
      <path d="M 410,345 l 6,12 l -2,10"/>
    </g>

    <!-- carved ornament -->
    <g transform="translate(150,470)">
      <circle r="24" fill="#c47a4c" stroke="#7a3d3c" stroke-width="2"/>
      <g transform="scale(3)"><use href="#fossil"/></g>
      <circle r="24" fill="none" stroke="#f2b67e" stroke-width="1.2" stroke-dasharray="3 4"/>
    </g>
    <use href="#fossil" transform="translate(62,488) rotate(30) scale(1.4)"/>
    <use href="#fossil" transform="translate(258,478) rotate(-20) scale(1.2)"/>
    <use href="#fossil" transform="translate(612,520) rotate(80) scale(1.5)"/>
    <use href="#fossil" transform="translate(430,390) scale(1.1)"/>
    <use href="#star" transform="translate(216,515) rotate(14) scale(1.5)"/>
    <use href="#star" transform="translate(588,446) rotate(-20) scale(1.3)"/>
    <use href="#star" transform="translate(772,512) rotate(40) scale(1.5)"/>
    <use href="#barnacles" transform="translate(30,560) scale(1.4)"/>
    <use href="#barnacles" transform="translate(470,372) scale(1.2)"/>
    <use href="#barnacles" transform="translate(668,566) scale(1.4)"/>

    <!-- arched window with lantern glow on the right cliff -->
    <g transform="translate(700,500)">
      <path d="M -20,30 L -20,-6 A 20 20 0 0 1 20,-6 L 20,30 Z" fill="#f2b67e" stroke="#7a3d3c" stroke-width="2"/>
      <path d="M -14,30 L -14,-6 A 14 14 0 0 1 14,-6 L 14,30 Z" fill="#2c1630" stroke="#2a1a2e" stroke-width="1.5"/>
      <circle cy="6" r="14" fill="#ffb84a" opacity="0.25"/>
      <path d="M 0,-20 V -2" stroke="#2a1a2e" stroke-width="1"/>
      <path d="M -3.6,-2 H 3.6 L 3,8 H -3 Z" fill="#ffd36b" stroke="#2a1a2e" stroke-width="1"/>
      <path d="M -2,0 H 2 V 6 H -2 Z" fill="#fff6c0"/>
      <path d="M -4,-23 L 0,-27 L 4,-23 L 2.6,-19 H -2.6 Z" fill="#f6e0b4" stroke="#7a3d3c" stroke-width="1.2"/>
    </g>

    <!-- inner edge occlusion -->
    <use href="#land" fill="none" stroke="#3a1838" stroke-width="14" opacity="0.18"/>

    <!-- moss cap: shade band, base, lit lip -->
    <rect width="800" height="600" fill="#3a1838" mask="url(#capShadeMask)" opacity="0.35"/>
    <rect width="800" height="600" fill="#3f8f3c" mask="url(#fringeMask)"/>
    <rect width="800" height="600" fill="#6cc247" mask="url(#capMask)"/>
    <rect width="800" height="600" fill="#b6ec6e" mask="url(#capLitMask)"/>
  </g>

  <!-- silhouette outline -->
  <use href="#land" fill="none" stroke="#2a1a2e" stroke-width="4.6" stroke-linejoin="round"/>

  <!-- edge growth: coral on cliffs, tufts and flowers along the tops -->
  <use href="#coral" transform="translate(318,450) rotate(70)"/>
  <use href="#coral" transform="translate(533,452) rotate(-80) scale(0.9)"/>
  <use href="#coral" transform="translate(506,362) rotate(60) scale(0.8)"/>
  <g>
    {DECOR}
  </g>
</svg>
'''

MASK = """<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600" viewBox="0 0 800 600">
  <!-- Tidewall collision mask: land path only (no tufts/coral/kelp). Generated by build_terrain.py. -->
  <path d="{LAND}" fill="#000"/>
</svg>
"""

if __name__ == "__main__":
    (HERE / "terrain.svg").write_text(SVG.replace("{LAND}", land_d()).replace("{STONES}", stones_tile()).replace("{DECOR}", decorations()))
    (HERE / "terrain_mask.svg").write_text(MASK.replace("{LAND}", land_d()))
