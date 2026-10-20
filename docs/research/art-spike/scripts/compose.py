#!/usr/bin/env python3
"""Compose lobshot art-spike previews from the cutout rig.

Reads rig.ron (the single source of truth for pivots/offsets) and emits SVGs that
reference the part files via <image>, so the parts themselves are never duplicated.
"""
import math, re, struct, subprocess, sys, zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "out"

NUM = r"(-?\d+(?:\.\d+)?)"


def load_rig(path: Path) -> dict:
    src = re.sub(r"//[^\n]*", "", path.read_text())
    parts = []
    for m in re.finditer(
        r'\(id:\s*"(\w+)",\s*file:\s*"([^"]+)",\s*at:\s*\(' + NUM + r",\s*" + NUM + r"\),\s*z:\s*(\d+)([^)]*)\)",
        src,
    ):
        flags = m.group(6)
        parts.append(dict(id=m.group(1), file=path.parent / m.group(2), at=(float(m.group(3)), float(m.group(4))),
                          z=int(m.group(5)), spin="spin: true" in flags, aim="aim: true" in flags))
    cannon_part = re.search(r'cannon:\s*\(\s*part:\s*"(\w+)"', src).group(1)
    mz = re.search(r"muzzle:\s*\(" + NUM + r",\s*" + NUM + r"\)", src)
    rs = re.search(r"rider_socket:\s*\(at:\s*\(" + NUM + r",\s*" + NUM + r"\),\s*z:\s*(\d+)", src)
    hb = re.search(r"hitbox:\s*\(x:\s*" + NUM + r",\s*y:\s*" + NUM + r",\s*w:\s*" + NUM + r",\s*h:\s*" + NUM, src)
    pivot = next(p["at"] for p in parts if p["id"] == cannon_part)
    return dict(parts=parts, pivot=pivot, muzzle=(float(mz.group(1)), float(mz.group(2))),
                socket=(float(rs.group(1)), float(rs.group(2))), socket_z=int(rs.group(3)),
                hitbox=tuple(float(hb.group(i)) for i in range(1, 5)))


def viewbox(path: Path):
    vb = re.search(r'viewBox="([^"]+)"', path.read_text()).group(1)
    return tuple(float(v) for v in vb.split())


def image(path: Path, rel_to: Path) -> str:
    x, y, w, h = viewbox(path)
    href = Path(*[".."] * len(rel_to.relative_to(ROOT).parts), path.relative_to(ROOT)) if rel_to != ROOT else path.relative_to(ROOT)
    return f'<image x="{x}" y="{y}" width="{w}" height="{h}" href="{href}"/>'


def alpha_mask(svg: Path):
    """Rasterize the terrain at 1x and return (w, h, alpha bytes): the sim's carve mask."""
    png = OUT / (svg.stem + "_mask.png")
    subprocess.run([str(Path.home() / ".cargo/bin/resvg"), str(svg), str(png)], check=True)
    data = png.read_bytes()
    pos, idat, w = 8, b"", 0
    while pos < len(data):
        n, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + n]
        if kind == b"IHDR":
            w, h = struct.unpack(">II", body[:8])
        elif kind == b"IDAT":
            idat += body
        pos += 12 + n
    raw, stride, rows, prev = zlib.decompress(idat), w * 4, [], bytearray(w * 4)
    for y in range(h):
        f, line = raw[y * (stride + 1)], bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - 4] if i >= 4 else 0
            b = prev[i]
            c = prev[i - 4] if i >= 4 else 0
            if f == 1: line[i] = (line[i] + a) & 255
            elif f == 2: line[i] = (line[i] + b) & 255
            elif f == 3: line[i] = (line[i] + (a + b) // 2) & 255
            elif f == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else (b if pb <= pc else c))) & 255
        rows.append(bytes(line[3::4]))
        prev = line
    return w, h, rows


def ground_y(mask, x, y_from=0):
    w, h, rows = mask
    xi = max(0, min(w - 1, int(round(x))))
    for y in range(int(y_from), h):
        if rows[y][xi] > 128:
            return float(y)
    return float(h)


def place_on_ground(rig_path: Path, mask, x):
    """Body origin + tilt from the rig's ground probes, like the sim would fit it."""
    src = rig_path.read_text()
    probes = [float(v) for v in re.findall(NUM, re.search(r"ground_probes:\s*\[([^\]]*)\]", src).group(1))][0::2]
    ys = [ground_y(mask, x + px) for px in probes]
    tilt = math.degrees(math.atan2(ys[-1] - ys[0], probes[-1] - probes[0]))
    return x, sum(ys) / len(ys), tilt


def muzzle_world(rig, x, y, facing, aim):
    a = math.radians(aim)
    mx, my = rig["muzzle"]
    px, py = rig["pivot"]
    bx = px + mx * math.cos(a) + my * math.sin(a)
    by = py - mx * math.sin(a) + my * math.cos(a)
    return x + facing * bx, y + by


def mobile(rig, rider: Path, x, y, facing=1, aim=45.0, spin=0.0, tilt=0.0, out_dir=OUT, debug=False) -> str:
    layers = []
    for p in rig["parts"]:
        ax, ay = p["at"]
        rot = -aim if p["aim"] else (spin if p["spin"] else 0.0)
        layers.append((p["z"], f'<g transform="translate({ax} {ay}) rotate({rot})">{image(p["file"], out_dir)}</g>'))
    sx, sy = rig["socket"]
    layers.append((rig["socket_z"], f'<g transform="translate({sx} {sy})">{image(rider, out_dir)}</g>'))
    body = "\n    ".join(s for _, s in sorted(layers, key=lambda t: t[0]))
    if debug:
        hx, hy, hw, hh = rig["hitbox"]
        px, py = rig["pivot"]
        a = math.radians(aim)
        mx = px + rig["muzzle"][0] * math.cos(a)
        my = py - rig["muzzle"][0] * math.sin(a)
        body += (f'\n    <rect x="{hx}" y="{hy}" width="{hw}" height="{hh}" fill="none" stroke="#ff2d55" stroke-width="0.5" stroke-dasharray="2 1"/>'
                 f'\n    <circle cx="{px}" cy="{py}" r="1.2" fill="#00e0ff"/><circle cx="{mx:.2f}" cy="{my:.2f}" r="1.2" fill="#ff2d55"/>'
                 f'\n    <circle cx="{sx}" cy="{sy}" r="1.2" fill="#7cff4f"/><circle cx="0" cy="0" r="1.2" fill="#ffffff"/>')
    return f'<g transform="translate({x} {y}) rotate({tilt}) scale({facing} 1)">\n    {body}\n  </g>'


def preview_mobile(rig_path: Path, rider: Path, name: str):
    rig = load_rig(rig_path)
    w, h = 300, 110
    poses = [(55, 95, 0.0, False), (150, 95, 45.0, False), (245, 95, 80.0, True)]
    groups = "\n  ".join(mobile(rig, rider, x, y, aim=a, debug=d) for x, y, a, d in poses)
    svg = f'''<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">
  <rect width="{w}" height="{h}" fill="#d9e6ef"/>
  <rect y="95" width="{w}" height="15" fill="#b9c7d2"/>
  {groups}
</svg>
'''
    (OUT / name).write_text(svg)


def parts_sheet(rig_path: Path, rider: Path, name: str):
    """Exploded cutout rig: every part on its own, pivot marked, at its native viewBox size."""
    rig = load_rig(rig_path)
    seen, cells, x = set(), [], 6.0
    files = [p["file"] for p in rig["parts"]] + [rider]
    for f in files:
        if f in seen:
            continue
        seen.add(f)
        vx, vy, vw, vh = viewbox(f)
        cells.append(f'<g transform="translate({x - vx} {60 - vy - vh / 2})">{image(f, OUT)}'
                     f'<rect x="{vx}" y="{vy}" width="{vw}" height="{vh}" fill="none" stroke="#8aa0b4" stroke-width="0.4" stroke-dasharray="1.5 1"/>'
                     f'<path d="M -3,0 H 3 M 0,-3 V 3" stroke="#ff2d55" stroke-width="0.6"/></g>')
        x += vw + 8
    svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="{x:.0f}" height="120" viewBox="0 0 {x:.0f} 120"><rect width="{x:.0f}" height="120" fill="#e6edf3"/>' + "".join(cells) + "</svg>\n"
    (OUT / name).write_text(svg)


def composite(rig_path: Path, rider: Path, map_dir: Path, name: str, px, aim, enemy_x=None, enemy_aim=50.0):
    rig = load_rig(rig_path)
    mask = alpha_mask(map_dir / "terrain_mask.svg")
    x, y, tilt = place_on_ground(rig_path, mask, px)
    mx, my = muzzle_world(rig, x, y, 1, aim + tilt)  # body aim = world aim + tilt
    # muzzle_world ignores tilt; rotate the body-relative muzzle by tilt about the origin
    dx, dy = mx - x, my - y
    t = math.radians(tilt)
    mx, my = x + dx * math.cos(t) - dy * math.sin(t), y + dx * math.sin(t) + dy * math.cos(t)
    parts = [image(map_dir / "background.svg", OUT), image(map_dir / "terrain.svg", OUT)]
    if enemy_x is not None:
        ex, ey, etilt = place_on_ground(rig_path, mask, enemy_x)
        parts.append(mobile(rig, rider, ex, ey, facing=-1, aim=enemy_aim, tilt=etilt))
    parts.append(mobile(rig, rider, x, y, aim=aim + tilt, tilt=tilt))
    parts.append(projectile_svg((mx, my), aim))
    svg = '<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600" viewBox="0 0 800 600">\n  ' + "\n  ".join(parts) + "\n</svg>\n"
    (OUT / name).write_text(svg)


def projectile_svg(muzzle, aim, v=11.0, g=0.16, wind=0.012, t_now=38, tail=26):
    """Ballistic arc from the muzzle; the shell is drawn at t_now with a tapered glowing trail."""
    a = math.radians(aim)
    vx, vy = v * math.cos(a), -v * math.sin(a)
    pts = [(muzzle[0] + vx * t + 0.5 * wind * t * t, muzzle[1] + vy * t + 0.5 * g * t * t) for t in range(t_now + 1)]
    tr = pts[max(0, t_now - tail):]
    d = "M " + " L ".join(f"{x:.1f},{y:.1f}" for x, y in tr)
    (x0, y0), (hx, hy) = tr[0], tr[-1]
    grad = (f'<linearGradient id="trail" gradientUnits="userSpaceOnUse" x1="{x0:.1f}" y1="{y0:.1f}" x2="{hx:.1f}" y2="{hy:.1f}">'
            '<stop offset="0" stop-color="#fff6d8" stop-opacity="0"/><stop offset="1" stop-color="#fff6d8" stop-opacity="0.9"/></linearGradient>')
    puffs = "".join(f'<circle cx="{pts[t][0]:.1f}" cy="{pts[t][1]:.1f}" r="{1.6 + 0.12 * (t_now - t):.1f}" fill="#ffffff" opacity="{0.5 * (1 - (t_now - t) / tail):.2f}"/>'
                    for t in range(t_now - tail + 2, t_now - 1, 5))
    dx, dy = pts[t_now][0] - pts[t_now - 1][0], pts[t_now][1] - pts[t_now - 1][1]
    ang = math.degrees(math.atan2(dy, dx))
    shell = (f'<g transform="translate({hx:.1f} {hy:.1f}) rotate({ang:.1f})">'
             f'<circle r="8" fill="#ffd36b" opacity="0.3"/>'
             f'<path d="M -4.5,-3.2 L 1.5,-3.2 C 4.5,-3.2 6,-1.6 6,0 C 6,1.6 4.5,3.2 1.5,3.2 L -4.5,3.2 Z" fill="#e9b949" stroke="#2a1a2e" stroke-width="1.4" stroke-linejoin="round"/>'
             f'<path d="M -4.5,-3.2 L -7,-4.8 L -6,0 L -7,4.8 L -4.5,3.2 Z" fill="#2bb3a3" stroke="#2a1a2e" stroke-width="1.2" stroke-linejoin="round"/>'
             f'<path d="M -3,-1.8 H 2.5" stroke="#fff6c0" stroke-width="1.1" stroke-linecap="round"/></g>')
    return (f'<g><defs>{grad}</defs>{puffs}'
            f'<path d="{d}" fill="none" stroke="url(#trail)" stroke-width="6" stroke-linecap="round" opacity="0.45"/>'
            f'<path d="{d}" fill="none" stroke="url(#trail)" stroke-width="2" stroke-linecap="round"/>{shell}</g>')


if __name__ == "__main__":
    OUT.mkdir(exist_ok=True)
    rig = ROOT / "mobile/coilback/rig.ron"
    rider = ROOT / "riders/pip.svg"
    what = sys.argv[1] if len(sys.argv) > 1 else "preview"
    if what == "preview":
        preview_mobile(rig, rider, "preview_mobile.svg")
        parts_sheet(rig, rider, "parts_sheet.svg")
    elif what == "composite":
        x, aim = float(sys.argv[2]), float(sys.argv[3])
        ex = float(sys.argv[4]) if len(sys.argv) > 4 else None
        composite(rig, rider, ROOT / "maps/tidewall", "composite.svg", x, aim, ex)
