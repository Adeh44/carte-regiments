//! carto — prépare le fond de la Carte des régiments.
//!
//! Lit des GeoJSON (IGN, Natural Earth), puis pour chaque niveau de zoom :
//! projette (Web Mercator), simplifie au demi-pixel, découpe en tuiles 256 px
//! et encode chaque tuile en binaire compact (varints + deltas).
//! Les tuiles sont regroupées en paquets : un fichier par zoom jusqu'à z5,
//! puis un fichier par "carré z5" au-delà. La page ne télécharge que ce qu'elle affiche.
//!
//! Enfin, assemble la page : gabarit HTML + index des paquets + villes.
//!
//! Usage : carto <src/geo> <dossier_sortie> <gabarit.html>
//! (src/geo contient zones, routes et villes ; src/geo/sources les fonds IGN et Natural Earth téléchargés par build.py)

mod relief;
mod topo;

use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Unités par tuile : 4096 = 1/16 de pixel, assez fin pour zoomer 3 crans au-delà des données.
const EXT: f64 = 4096.0;
/// Marge autour de chaque tuile (évite les coutures au bord des traits épais).
const BUF: f64 = 128.0;
const ZMIN: u32 = 2;
const ZMAX: u32 = 10;
/// Jusqu'à ce zoom, on produit toutes les tuiles du monde.
const ZWORLD: u32 = 5;
/// Paquets : un par tuile de ce zoom (au-dessus, un seul paquet par zoom).
const ZGROUP: u32 = 3;

/// Couches, dans l'ordre de dessin. La page connaît les mêmes numéros.
#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
enum Lyr {
    Land = 0,
    France = 1,
    // 2 : réservé (ancien tracé des frontières)
    DepFill = 3,
    // 4 : réservé
    SeaBox = 5,
    ZoneFr = 6,
    ZoneLand = 7,
    ZoneCoast = 8,
    ZoneAdm = 9,
    Lakes = 10,
    Road2 = 11,
    Road1 = 12,
    Rivers = 13,
    RiversBig = 14,
    Regions = 15,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Fill,
    Line,
}

/// Une géométrie projetée (coordonnées Mercator dans [0,1]), prête à être tuilée.
struct Feat {
    lyr: Lyr,
    kind: Kind,
    zmin: u32,
    parts: Vec<Vec<[f64; 2]>>,
    bbox: [f64; 4],
}

fn merc(lon: f64, lat: f64) -> [f64; 2] {
    let lat = lat.clamp(-85.0511, 85.0511).to_radians();
    let x = (lon + 180.0) / 360.0;
    let y = (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / std::f64::consts::PI) / 2.0;
    [x, y]
}

fn ring_of(v: &Value) -> Vec<[f64; 2]> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    let p = p.as_array()?;
                    Some(merc(p.first()?.as_f64()?, p.get(1)?.as_f64()?))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Toutes les lignes (ou anneaux) d'une géométrie GeoJSON.
fn parts_of(g: &Value) -> Vec<Vec<[f64; 2]>> {
    let c = &g["coordinates"];
    match g["type"].as_str().unwrap_or("") {
        "LineString" => vec![ring_of(c)],
        "MultiLineString" | "Polygon" => c.as_array().map(|a| a.iter().map(ring_of).collect()).unwrap_or_default(),
        "MultiPolygon" => c
            .as_array()
            .map(|a| a.iter().flat_map(|p| p.as_array().cloned().unwrap_or_default()).map(|r| ring_of(&r)).collect())
            .unwrap_or_default(),
        "GeometryCollection" => g["geometries"].as_array().map(|a| a.iter().flat_map(parts_of).collect()).unwrap_or_default(),
        _ => vec![],
    }
}

fn bbox_of(parts: &[Vec<[f64; 2]>]) -> [f64; 4] {
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in parts.iter().flatten() {
        b[0] = b[0].min(p[0]);
        b[1] = b[1].min(p[1]);
        b[2] = b[2].max(p[0]);
        b[3] = b[3].max(p[1]);
    }
    b
}

fn feat(lyr: Lyr, kind: Kind, zmin: u32, parts: Vec<Vec<[f64; 2]>>) -> Option<Feat> {
    let parts: Vec<_> = parts.into_iter().filter(|p| p.len() >= 2).collect();
    if parts.is_empty() {
        return None;
    }
    let bbox = bbox_of(&parts);
    Some(Feat { lyr, kind, zmin, parts, bbox })
}

fn load(path: &Path) -> Value {
    let s = fs::read_to_string(path).unwrap_or_else(|e| panic!("lecture {}: {e}", path.display()));
    serde_json::from_str(&s).unwrap_or_else(|e| panic!("JSON {}: {e}", path.display()))
}

fn features(v: &Value) -> Vec<Value> {
    match v["type"].as_str() {
        Some("FeatureCollection") => v["features"].as_array().cloned().unwrap_or_default(),
        Some("Feature") => vec![v.clone()],
        _ => vec![],
    }
}

/// Garde une géométrie seulement si elle touche la boîte (lon/lat).
fn in_box(g: &Value, b: [f64; 4]) -> bool {
    let (w, s) = (merc(b[0], b[3]), merc(b[2], b[1]));
    let bb = bbox_of(&parts_of(g));
    !(bb[2] < w[0] || bb[0] > s[0] || bb[3] < w[1] || bb[1] > s[1])
}

// ---------- Simplification (Douglas-Peucker, itératif) ----------

fn seg_d2(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let (ex, ey) = (a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
    ex * ex + ey * ey
}

fn simplify(pts: &[[f64; 2]], tol: f64) -> Vec<[f64; 2]> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let t2 = tol * tol;
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    while let Some((a, b)) = stack.pop() {
        let (mut best, mut bi) = (0.0, 0);
        for i in a + 1..b {
            let d = seg_d2(pts[i], pts[a], pts[b]);
            if d > best {
                best = d;
                bi = i;
            }
        }
        if best > t2 {
            keep[bi] = true;
            stack.push((a, bi));
            stack.push((bi, b));
        }
    }
    pts.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

fn area(r: &[[f64; 2]]) -> f64 {
    let mut s = 0.0;
    for i in 0..r.len() {
        let (a, b) = (r[i], r[(i + 1) % r.len()]);
        s += a[0] * b[1] - b[0] * a[1];
    }
    s / 2.0
}

// ---------- Découpe ----------

/// Sutherland-Hodgman : découpe un anneau contre le rectangle [lo, hi]².
fn clip_ring(r: &[[f64; 2]], lo: f64, hi: f64) -> Vec<[f64; 2]> {
    let mut out = r.to_vec();
    for edge in 0..4 {
        if out.is_empty() {
            break;
        }
        let inside = |p: &[f64; 2]| match edge {
            0 => p[0] >= lo,
            1 => p[0] <= hi,
            2 => p[1] >= lo,
            _ => p[1] <= hi,
        };
        let cross = |a: [f64; 2], b: [f64; 2]| {
            let (k, v) = match edge {
                0 => (0, lo),
                1 => (0, hi),
                2 => (1, lo),
                _ => (1, hi),
            };
            let t = (v - a[k]) / (b[k] - a[k]);
            [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]
        };
        let input = std::mem::take(&mut out);
        let mut prev = *input.last().unwrap();
        for &cur in &input {
            match (inside(&cur), inside(&prev)) {
                (true, true) => out.push(cur),
                (true, false) => {
                    out.push(cross(prev, cur));
                    out.push(cur);
                }
                (false, true) => out.push(cross(prev, cur)),
                _ => {}
            }
            prev = cur;
        }
    }
    out
}

/// Liang-Barsky segment par segment : une polyligne devient 0..n morceaux dans le rectangle.
fn clip_line(l: &[[f64; 2]], lo: f64, hi: f64) -> Vec<Vec<[f64; 2]>> {
    let mut res = vec![];
    let mut cur: Vec<[f64; 2]> = vec![];
    for w in l.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        let mut ok = true;
        for (p, q) in [(-dx, a[0] - lo), (dx, hi - a[0]), (-dy, a[1] - lo), (dy, hi - a[1])] {
            if p == 0.0 {
                if q < 0.0 {
                    ok = false;
                    break;
                }
            } else {
                let r = q / p;
                if p < 0.0 {
                    if r > t1 {
                        ok = false;
                        break;
                    }
                    t0 = t0.max(r);
                } else {
                    if r < t0 {
                        ok = false;
                        break;
                    }
                    t1 = t1.min(r);
                }
            }
        }
        if !ok {
            if cur.len() >= 2 {
                res.push(std::mem::take(&mut cur));
            }
            cur.clear();
            continue;
        }
        let pa = [a[0] + t0 * dx, a[1] + t0 * dy];
        let pb = [a[0] + t1 * dx, a[1] + t1 * dy];
        if cur.is_empty() {
            cur.push(pa);
        } else if t0 > 0.0 {
            // le segment ré-entre : nouveau morceau
            if cur.len() >= 2 {
                res.push(std::mem::take(&mut cur));
            }
            cur = vec![pa];
        }
        cur.push(pb);
        if t1 < 1.0 {
            if cur.len() >= 2 {
                res.push(std::mem::take(&mut cur));
            }
            cur.clear();
        }
    }
    if cur.len() >= 2 {
        res.push(cur);
    }
    res
}

// ---------- Encodage ----------

fn varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            break;
        }
        out.push(b | 0x80);
    }
}
fn zz(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

/// Contenu d'une tuile : par couche, une liste de morceaux en coordonnées tuile entières.
type Tile = BTreeMap<u8, Vec<Vec<[i32; 2]>>>;

fn encode_tile(t: &Tile) -> Vec<u8> {
    let mut o = vec![];
    varint(&mut o, t.len() as u64);
    let (mut px, mut py) = (0i64, 0i64);
    for (id, parts) in t {
        o.push(*id);
        varint(&mut o, parts.len() as u64);
        for p in parts {
            varint(&mut o, p.len() as u64);
            for q in p {
                varint(&mut o, zz(q[0] as i64 - px));
                varint(&mut o, zz(q[1] as i64 - py));
                px = q[0] as i64;
                py = q[1] as i64;
            }
        }
    }
    o
}

/// Arrondit en entiers et retire les doublons consécutifs.
fn quant(p: &[[f64; 2]], ox: f64, oy: f64) -> Vec<[i32; 2]> {
    let mut v: Vec<[i32; 2]> = Vec::with_capacity(p.len());
    for q in p {
        let r = [(q[0] - ox).round() as i32, (q[1] - oy).round() as i32];
        if v.last() != Some(&r) {
            v.push(r);
        }
    }
    v
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|a| a.as_str()) {
        Some("topo") => return topo::run(&args[2..]),
        Some("relief") => return relief::run(&args[2..]),
        Some("relief-liste") => return relief::liste(&args[2..]),
        _ => {}
    }
    let src = Path::new(args.get(1).expect("dossier src/geo"));
    let big = src.join("sources");
    let out = Path::new(args.get(2).expect("dossier sortie"));
    fs::create_dir_all(out.join("t")).unwrap();

    let metro_box = [-7.0, 40.5, 11.5, 52.2];
    let mut feats: Vec<Feat> = vec![];
    let fr_codes = ["FRA", "NCL", "PYF", "WLF", "SPM", "BLM", "MAF", "ATF"];

    // Monde : Natural Earth 1:10M (pays), France et territoires à part.
    for f in features(&load(&big.join("ne_10m_admin_0_countries.geojson"))) {
        let a3 = f["properties"]["ADM0_A3"].as_str().unwrap_or("");
        let parts = parts_of(&f["geometry"]);
        let lyr = if fr_codes.contains(&a3) { Lyr::France } else { Lyr::Land };
        // Les contours sont tracés depuis les surfaces : les bords de découpe tombent dans la marge, hors écran.
        feats.extend(feat(lyr, Kind::Fill, 0, parts));
    }
    // Départements et régions : IGN (via france-geojson), bien plus précis que l'ancien fond.
    let mut labels = vec![];
    for f in features(&load(&big.join("departements.geojson"))) {
        let parts = parts_of(&f["geometry"]);
        // Étiquette : barycentre du plus grand morceau
        if let Some(big) = parts.iter().max_by(|a, b| area(a).abs().total_cmp(&area(b).abs())) {
            let (mut cx, mut cy, mut s) = (0.0, 0.0, 0.0);
            for i in 0..big.len() {
                let (p, q) = (big[i], big[(i + 1) % big.len()]);
                let c = p[0] * q[1] - q[0] * p[1];
                cx += (p[0] + q[0]) * c;
                cy += (p[1] + q[1]) * c;
                s += c;
            }
            let (x, y) = (cx / (3.0 * s), cy / (3.0 * s));
            let lon = x * 360.0 - 180.0;
            let lat = (std::f64::consts::PI * (1.0 - 2.0 * y)).sinh().atan().to_degrees();
            labels.push(serde_json::json!([f["properties"]["nom"], (lat * 1000.0).round() / 1000.0, (lon * 1000.0).round() / 1000.0]));
        }
        feats.extend(feat(Lyr::DepFill, Kind::Fill, 0, parts));
    }
    for f in features(&load(&big.join("regions.geojson"))) {
        feats.extend(feat(Lyr::Regions, Kind::Line, 4, parts_of(&f["geometry"])));
    }
    // Outre-mer et étranger : fond détaillé de la version précédente (boîtes de mer + terres + côtes).
    let zone = load(&src.join("zones.min.json"));
    for b in zone["boxes"].as_array().unwrap() {
        let b: Vec<f64> = b.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let r = vec![merc(b[0], b[1]), merc(b[2], b[1]), merc(b[2], b[3]), merc(b[0], b[3])];
        feats.extend(feat(Lyr::SeaBox, Kind::Fill, 0, vec![r]));
    }
    for f in features(&zone["land"]) {
        let fr = f["properties"]["fr"].as_i64().unwrap_or(0) == 1;
        feats.extend(feat(if fr { Lyr::ZoneFr } else { Lyr::ZoneLand }, Kind::Fill, 0, parts_of(&f["geometry"])));
    }
    for f in features(&zone["coast"]) {
        feats.extend(feat(Lyr::ZoneCoast, Kind::Line, 0, parts_of(&f["geometry"])));
    }
    for f in features(&zone["adm"]) {
        feats.extend(feat(Lyr::ZoneAdm, Kind::Line, 0, parts_of(&f["geometry"])));
    }
    // Lacs et rivières : Natural Earth 1:10M + supplément Europe, autour de la France.
    let near = [-9.0, 38.0, 16.0, 55.0];
    for name in ["ne_10m_lakes.geojson", "ne_10m_lakes_europe.geojson"] {
        for f in features(&load(&big.join(name))) {
            if !in_box(&f["geometry"], near) {
                continue;
            }
            let mz = f["properties"]["min_zoom"].as_f64().unwrap_or(5.0);
            feats.extend(feat(Lyr::Lakes, Kind::Fill, (mz.floor() as u32).saturating_sub(1).max(4), parts_of(&f["geometry"])));
        }
    }
    for name in ["ne_10m_rivers_lake_centerlines.geojson", "ne_10m_rivers_europe.geojson"] {
        for f in features(&load(&big.join(name))) {
            if !in_box(&f["geometry"], near) {
                continue;
            }
            let p = &f["properties"];
            let mz = p["min_zoom"].as_f64().unwrap_or(7.0);
            let rank = p["scalerank"].as_f64().unwrap_or(10.0);
            let lyr = if rank <= 6.0 { Lyr::RiversBig } else { Lyr::Rivers };
            feats.extend(feat(lyr, Kind::Line, (mz.floor() as u32).saturating_sub(1).max(4), parts_of(&f["geometry"])));
        }
    }
    // Routes : on garde celles de la version précédente (autoroutes dès z6, secondaires dès z7).
    for f in features(&load(&src.join("roads.min.json"))) {
        let t = f["properties"]["t"].as_i64().unwrap_or(2);
        let (lyr, z) = if t == 1 { (Lyr::Road1, 6) } else { (Lyr::Road2, 7) };
        feats.extend(feat(lyr, Kind::Line, z, parts_of(&f["geometry"])));
    }

    // Zones où l'on produit des tuiles détaillées au-delà de z5.
    let mut roi: Vec<[f64; 4]> = vec![metro_box];
    for b in zone["boxes"].as_array().unwrap() {
        let b: Vec<f64> = b.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        roi.push([b[0] - 0.3, b[1] - 0.3, b[2] + 0.3, b[3] + 0.3]);
    }
    let roi_merc: Vec<[f64; 4]> = roi
        .iter()
        .map(|b| {
            let (a, c) = (merc(b[0], b[3]), merc(b[2], b[1]));
            [a[0], a[1], c[0], c[1]]
        })
        .collect();

    let pts_in: usize = feats.iter().map(|f| f.parts.iter().map(|p| p.len()).sum::<usize>()).sum();
    eprintln!("{} géométries, {} points en entrée", feats.len(), pts_in);

    let mut manifest = vec![];
    let mut total = 0usize;
    for z in ZMIN..=ZMAX {
        let n = (1u64 << z) as f64;
        let scale = n * EXT;
        // Demi-pixel de tolérance (1 px = EXT/256 unités)
        let tol = EXT / 256.0 * if z <= 4 { 0.6 } else { 0.45 };
        let min_area = (EXT / 256.0 * 1.2).powi(2);
        let tile_ok = |tx: i64, ty: i64| -> bool {
            if z <= ZWORLD {
                return true;
            }
            let (x0, y0, x1, y1) = (tx as f64 / n, ty as f64 / n, (tx + 1) as f64 / n, (ty + 1) as f64 / n);
            roi_merc.iter().any(|b| !(x1 < b[0] || x0 > b[2] || y1 < b[1] || y0 > b[3]))
        };
        let mut tiles: BTreeMap<(i64, i64), Tile> = BTreeMap::new();
        // Tuiles présentes même vides : sinon la page croirait qu'il faut agrandir un niveau inférieur.
        let nn = 1i64 << z;
        if z <= ZWORLD {
            for x in 0..nn {
                for y in 0..nn {
                    tiles.insert((x, y), Tile::new());
                }
            }
        } else {
            for b in &roi_merc {
                for x in (b[0] * n).floor() as i64..=(b[2] * n).floor() as i64 {
                    for y in (b[1] * n).floor() as i64..=(b[3] * n).floor() as i64 {
                        tiles.entry((x, y)).or_default();
                    }
                }
            }
        }
        for f in feats.iter().filter(|f| z >= f.zmin) {
            // Projection à ce zoom + simplification
            let mut parts: Vec<Vec<[f64; 2]>> = vec![];
            for p in &f.parts {
                let pp: Vec<[f64; 2]> = p.iter().map(|q| [q[0] * scale, q[1] * scale]).collect();
                let s = simplify(&pp, tol);
                if f.kind == Kind::Fill {
                    if s.len() < 4 || area(&s).abs() < min_area {
                        continue;
                    }
                } else if s.len() < 2 {
                    continue;
                }
                parts.push(s);
            }
            if parts.is_empty() {
                continue;
            }
            let m = BUF / EXT;
            let tx0 = ((f.bbox[0] * n) - m).floor().max(0.0) as i64;
            let ty0 = ((f.bbox[1] * n) - m).floor().max(0.0) as i64;
            let tx1 = ((f.bbox[2] * n) + m).floor().min(n - 1.0) as i64;
            let ty1 = ((f.bbox[3] * n) + m).floor().min(n - 1.0) as i64;
            for tx in tx0..=tx1 {
                for ty in ty0..=ty1 {
                    if !tile_ok(tx, ty) {
                        continue;
                    }
                    let (ox, oy) = (tx as f64 * EXT, ty as f64 * EXT);
                    let mut got: Vec<Vec<[i32; 2]>> = vec![];
                    for p in &parts {
                        let loc: Vec<[f64; 2]> = p.iter().map(|q| [q[0] - ox, q[1] - oy]).collect();
                        // Rejet rapide si hors tuile
                        let bb = bbox_of(std::slice::from_ref(&loc));
                        if bb[2] < -BUF || bb[0] > EXT + BUF || bb[3] < -BUF || bb[1] > EXT + BUF {
                            continue;
                        }
                        match f.kind {
                            Kind::Fill => {
                                let c = clip_ring(&loc, -BUF, EXT + BUF);
                                let q = quant(&c, 0.0, 0.0);
                                if q.len() >= 3 {
                                    got.push(q);
                                }
                            }
                            Kind::Line => {
                                for c in clip_line(&loc, -BUF, EXT + BUF) {
                                    let q = quant(&c, 0.0, 0.0);
                                    if q.len() >= 2 {
                                        got.push(q);
                                    }
                                }
                            }
                        }
                    }
                    if !got.is_empty() {
                        tiles.entry((tx, ty)).or_default().entry(f.lyr as u8).or_default().extend(got);
                    }
                }
            }
        }
        // Regroupement en paquets
        // Paquets plus fins aux grands zooms : ~16x16 tuiles maximum par fichier.
        let gz = if z <= ZGROUP { 0 } else { ZGROUP.max(z - 4) };
        let shift = z - gz;
        let mut bundles: BTreeMap<(i64, i64), Vec<((i64, i64), Vec<u8>)>> = BTreeMap::new();
        for ((x, y), t) in &tiles {
            let key = (x >> shift, y >> shift);
            bundles.entry(key).or_default().push(((*x, *y), encode_tile(t)));
        }
        let mut zbytes = 0;
        for ((gx, gy), list) in bundles {
            // Paquet entièrement vide (mer) : on ne l'écrit pas, la page dessinera du vide.
            if list.iter().all(|(_, b)| b.len() == 1) {
                continue;
            }
            let (bx, by) = (gx << shift, gy << shift);
            let mut head = vec![];
            varint(&mut head, list.len() as u64);
            for ((x, y), body) in &list {
                varint(&mut head, (x - bx) as u64);
                varint(&mut head, (y - by) as u64);
                varint(&mut head, body.len() as u64);
            }
            for (_, body) in &list {
                head.extend_from_slice(body);
            }
            let name = format!("{z}_{gx}_{gy}");
            fs::write(out.join("t").join(format!("{name}.wasm")), &head).unwrap();
            zbytes += head.len();
            manifest.push(name);
        }
        total += zbytes;
        eprintln!("z{z}: {} tuiles, {} Ko", tiles.len(), zbytes / 1024);
    }
    let meta = serde_json::json!({ "bundles": manifest, "zmax": ZMAX, "zgroup": ZGROUP, "gdepth": 4, "ext": EXT, "deps": labels });
    eprintln!("total {} Ko", total / 1024);

    // Villes : les étrangères sous 200 000 habitants ne s'affichent jamais, inutile de les embarquer.
    let cities: Vec<Value> = load(&src.join("cities.min.json"))
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c[4].as_i64() == Some(1) || c[3].as_f64().unwrap_or(0.0) >= 200000.0)
        .cloned()
        .collect();
    eprintln!("{} villes gardées", cities.len());
    if let Some(tpl) = args.get(3) {
        let html = fs::read_to_string(tpl).expect("gabarit")
            .replacen("/*CITIES*/", &serde_json::to_string(&cities).unwrap(), 1)
            .replacen("/*META*/", &meta.to_string(), 1);
        fs::write(out.join("index.html"), &html).unwrap();
        eprintln!("page : {} Ko", html.len() / 1024);
    }
}
