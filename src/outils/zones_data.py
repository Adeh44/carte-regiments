import json, sys
G = sys.argv[1]
ZONES = {
 "antilles": (-62.0, 14.2, -60.6, 16.7), "guyane": (-55.0, 1.8, -51.2, 6.2),
 "reunion": (55.1, -21.5, 55.95, -20.75), "mayotte": (44.85, -13.15, 45.45, -12.45),
 "nc": (163.4, -23.0, 168.3, -19.4), "tahiti": (-150.1, -18.0, -149.0, -17.3),
 "djib": (41.5, 10.7, 43.7, 13.0), "eau": (51.3, 22.4, 56.6, 26.4)}
P = 3
def rnd(pts): 
    out = []
    for x, y in pts:
        q = [round(x, P), round(y, P)]
        if not out or out[-1] != q: out.append(q)
    return out
def clip_ring(ring, b):
    w, s, e, n = b
    def clip(pts, inside, inter):
        out = []
        for i in range(len(pts)):
            cur, prev = pts[i], pts[i - 1]
            if inside(cur):
                if not inside(prev): out.append(inter(prev, cur))
                out.append(cur)
            elif inside(prev): out.append(inter(prev, cur))
        return out
    def ix(a, c, x): t = (x - a[0]) / (c[0] - a[0]); return [x, a[1] + t * (c[1] - a[1])]
    def iy(a, c, y): t = (y - a[1]) / (c[1] - a[1]); return [a[0] + t * (c[0] - a[0]), y]
    pts = [list(p[:2]) for p in ring]
    for inside, inter in ((lambda p: p[0] >= w, lambda a, c: ix(a, c, w)), (lambda p: p[0] <= e, lambda a, c: ix(a, c, e)),
                          (lambda p: p[1] >= s, lambda a, c: iy(a, c, s)), (lambda p: p[1] <= n, lambda a, c: iy(a, c, n))):
        if not pts: break
        pts = clip(pts, inside, inter)
    return pts
def clip_line(line, b):
    w, s, e, n = b; parts, cur = [], []
    def inb(p): return w <= p[0] <= e and s <= p[1] <= n
    for p in line:
        if inb(p): cur.append(p)
        else:
            if len(cur) > 1: parts.append(cur)
            cur = []
    if len(cur) > 1: parts.append(cur)
    return parts
def polys(g): return [g["coordinates"]] if g["type"] == "Polygon" else g["coordinates"]
def lines(g): return [g["coordinates"]] if g["type"] == "LineString" else g["coordinates"]
def bbox_hit(coords, b):
    xs = [p[0] for p in coords]; ys = [p[1] for p in coords]
    return not (max(xs) < b[0] or min(xs) > b[2] or max(ys) < b[1] or min(ys) > b[3])

land = []
for f in json.load(open(G + "/ne_10m_admin_0_countries.geojson"))["features"]:
    a3 = f["properties"]["ADM0_A3"]; fr = a3 in ("FRA", "NCL", "PYF")
    for poly in polys(f["geometry"]):
        for b in ZONES.values():
            if not bbox_hit(poly[0], b): continue
            rings = [rnd(clip_ring(r, b)) for r in poly]
            rings = [r for r in rings if len(r) >= 4]
            if rings: land.append({"type": "Feature", "properties": {"fr": 1 if fr else 0}, "geometry": {"type": "Polygon", "coordinates": rings}})
def lines_in(path, keep, props):
    out = []
    for f in json.load(open(path))["features"]:
        if not f["geometry"] or not keep(f["properties"]): continue
        for l in lines(f["geometry"]):
            for b in ZONES.values():
                if not bbox_hit(l, b): continue
                for part in clip_line(l, b):
                    part = rnd(part)
                    if len(part) > 1: out.append({"type": "Feature", "properties": props(f["properties"]), "geometry": {"type": "LineString", "coordinates": part}})
    return out
adm = lines_in(G + "/ne_10m_admin_1_states_provinces_lines.geojson", lambda p: True, lambda p: {})
roads = lines_in(G + "/ne_10m_roads.geojson", lambda p: p.get("type") != "Ferry Route", lambda p: {"t": 1 if p.get("type") == "Major Highway" else 2})
rivers = lines_in(G + "/ne_10m_rivers_lake_centerlines.geojson", lambda p: True, lambda p: {"n": p.get("name")})
fc = lambda fs: {"type": "FeatureCollection", "features": fs}
out = {"boxes": list(ZONES.values()), "land": fc(land), "adm": fc(adm)}
s = json.dumps(out, separators=(",", ":"), ensure_ascii=False); open(G + "/zones.min.json", "w").write(s)
print("land", len(land), "adm", len(adm), "roads", len(roads), "rivers", len(rivers), "zones.json", len(s))
# Routes et rivières des zones : ajoutées aux calques existants
for name, extra in (("roads.min.json", roads), ("rivers.min.json", rivers)):
    base = json.load(open(G + "/" + name.replace(".min", ".metro.min") if False else G + "/" + name))
    base["features"] = [f for f in base["features"] if not f.get("properties", {}).get("z")]
    for f in extra: f["properties"]["z"] = 1
    base["features"] += extra
    t = json.dumps(base, separators=(",", ":"), ensure_ascii=False); open(G + "/" + name, "w").write(t); print(name, len(t))
