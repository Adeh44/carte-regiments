# Statistiques d'un .mbtiles : poids par zoom, poids par couche (sur un échantillon), et extrait de test.
# Usage : python3 stats_mbtiles.py entree.mbtiles echantillon.mbtiles
import gzip, math, random, sqlite3, sys, collections
import mapbox_vector_tile as mvt
src, out = sys.argv[1], sys.argv[2]
db = sqlite3.connect(src)
print("== Poids par zoom (gzip tel que stocké) ==")
tot = 0
for z, n, s in db.execute("select zoom_level, count(*), sum(length(tile_data)) from tiles group by zoom_level"):
    tot += s; print(f"z{z:2d} {n:8d} tuiles {s/1e6:9.1f} Mo")
print(f"total {tot/1e6:.1f} Mo")
# Poids par couche, décompressé, sur un échantillon de 300 tuiles par zoom (z10-14)
print("== Part de chaque couche (échantillon, octets décompressés) ==")
for z in range(10, 15):
    rows = db.execute("select tile_data from tiles where zoom_level=? order by random() limit 300", (z,)).fetchall()
    per = collections.Counter(); feats = collections.Counter()
    for (d,) in rows:
        raw = gzip.decompress(d) if d[:2] == b'\x1f\x8b' else d
        # Découpe grossière par couche : on décode et on ré-encode chaque couche seule
        dec = mvt.decode(raw)
        for name, layer in dec.items():
            feats[name] += len(layer["features"])
            per[name] += len(mvt.encode([{"name": name, "features": layer["features"]}]))
    t = sum(per.values()) or 1
    print(f"z{z}: " + ", ".join(f"{k} {100*v/t:.0f}% ({feats[k]} obj.)" for k, v in per.most_common()))
    # Détail des classes de transport à z14
    if z == 14:
        cls = collections.Counter()
        for (d,) in rows:
            raw = gzip.decompress(d) if d[:2] == b'\x1f\x8b' else d
            for f in mvt.decode(raw).get("transportation", {}).get("features", []):
                cls[f["properties"].get("class")] += 1
        print("  classes transport z14 :", dict(cls.most_common()))
# Extrait de test : Paris, Grenoble-Varces, Toulouse, Mourmelon (toutes zooms)
def tiles_bbox(z, w, s, e, n):
    def tx(lon): return int((lon + 180) / 360 * 2**z)
    def ty(lat):
        r = math.radians(lat); return int((1 - math.log(math.tan(r) + 1/math.cos(r)) / math.pi) / 2 * 2**z)
    return tx(w), tx(e), ty(n), ty(s)
boxes = [(2.25, 48.8, 2.42, 48.9), (5.6, 45.05, 5.78, 45.2), (1.35, 43.55, 1.5, 43.65), (4.2, 49.05, 4.5, 49.2)]
o = sqlite3.connect(out)
o.execute("create table metadata (name text, value text)"); o.execute("create table tiles (zoom_level int, tile_column int, tile_row int, tile_data blob)")
for r in db.execute("select name, value from metadata"): o.execute("insert into metadata values (?,?)", r)
n = 0
for z in range(0, 15):
    for b in boxes:
        x0, x1, y0, y1 = tiles_bbox(z, *b)
        for x in range(x0, x1 + 1):
            for y in range(y0, y1 + 1):
                tms = 2**z - 1 - y
                for r in db.execute("select tile_data from tiles where zoom_level=? and tile_column=? and tile_row=?", (z, x, tms)):
                    o.execute("insert into tiles values (?,?,?,?)", (z, x, tms, r[0])); n += 1
o.commit(); print("échantillon :", n, "tuiles")
