//! Fond topographique : convertit les tuiles OpenStreetMap (.mbtiles produit par Planetiler,
//! schéma OpenMapTiles) dans notre format compact.
//!
//! On ne garde que la géométrie et un type par couche (forêt, eau, autoroute, chemin...) :
//! les noms (souvent en 60 langues) et les attributs inutiles disparaissent.
//! Chaque paquet est compressé (gzip) ; la page le décompresse avec DecompressionStream.
//!
//! Usage : carto topo <dossier_sortie> <a.mbtiles> [b.mbtiles ...]  (zones sans recouvrement)
//! Sortie : <sortie>/o/{z}_{gx}_{gy}.wasm, <sortie>/o/index.json (paquets existants), <sortie>/o/sommets.json

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

/// Unités par tuile en sortie (4096 en entrée) : 2048 = 1/8 de pixel, assez pour agrandir 3 crans.
pub const OUT_EXT: i32 = 2048;
pub const ZMIN: u32 = 7;
pub const ZMAX: u32 = 14;

// Couches du fond topo (numéros partagés avec la page)
pub const FOREST: u8 = 20;
pub const WETLAND: u8 = 21;
pub const ROCK: u8 = 22;
pub const ICE: u8 = 23;
pub const SAND: u8 = 24;
pub const URBAN: u8 = 25;
pub const INDUS: u8 = 26;
pub const MILITARY: u8 = 27;
pub const WATER: u8 = 30;
pub const RIVER: u8 = 31;
pub const STREAM: u8 = 32;
pub const MOTORWAY: u8 = 40;
pub const TRUNK: u8 = 41;
pub const PRIMARY: u8 = 42;
pub const SECONDARY: u8 = 43;
pub const TERTIARY: u8 = 44;
pub const MINOR: u8 = 45;
pub const SERVICE: u8 = 46;
pub const TRACK: u8 = 47;
pub const PATH: u8 = 48;
pub const RAIL: u8 = 49;

// ---------- Lecture protobuf minimale (format MVT) ----------

struct Pb<'a> {
    b: &'a [u8],
    i: usize,
}
impl<'a> Pb<'a> {
    fn new(b: &'a [u8]) -> Self {
        Pb { b, i: 0 }
    }
    fn more(&self) -> bool {
        self.i < self.b.len()
    }
    fn varint(&mut self) -> u64 {
        let (mut v, mut s) = (0u64, 0);
        loop {
            let c = self.b[self.i];
            self.i += 1;
            v |= ((c & 0x7f) as u64) << s;
            if c & 0x80 == 0 {
                return v;
            }
            s += 7;
        }
    }
    fn key(&mut self) -> (u64, u64) {
        let k = self.varint();
        (k >> 3, k & 7)
    }
    fn bytes(&mut self) -> &'a [u8] {
        let n = self.varint() as usize;
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        s
    }
    fn skip(&mut self, wt: u64) {
        match wt {
            0 => {
                self.varint();
            }
            1 => self.i += 8,
            2 => {
                self.bytes();
            }
            5 => self.i += 4,
            _ => panic!("type protobuf inconnu {wt}"),
        }
    }
}

#[derive(Clone, Debug)]
enum Val {
    S(String),
    N(f64),
    B(bool),
}

fn read_value(b: &[u8]) -> Val {
    let mut p = Pb::new(b);
    let mut v = Val::B(false);
    while p.more() {
        let (f, wt) = p.key();
        v = match (f, wt) {
            (1, 2) => Val::S(String::from_utf8_lossy(p.bytes()).into_owned()),
            (2, 5) => {
                let x = f32::from_le_bytes(p.b[p.i..p.i + 4].try_into().unwrap());
                p.i += 4;
                Val::N(x as f64)
            }
            (3, 1) => {
                let x = f64::from_le_bytes(p.b[p.i..p.i + 8].try_into().unwrap());
                p.i += 8;
                Val::N(x)
            }
            (4, 0) | (5, 0) => Val::N(p.varint() as i64 as f64),
            (6, 0) => {
                let x = p.varint();
                Val::N(((x >> 1) as i64 ^ -((x & 1) as i64)) as f64)
            }
            (7, 0) => Val::B(p.varint() != 0),
            _ => {
                p.skip(wt);
                continue;
            }
        };
    }
    v
}

struct Feature {
    kind: u64, // 1 point, 2 ligne, 3 surface
    props: HashMap<String, Val>,
    geom: Vec<Vec<[i32; 2]>>,
}

/// Décode les commandes de géométrie MVT (MoveTo, LineTo, ClosePath).
fn decode_geom(cmds: &[u32]) -> Vec<Vec<[i32; 2]>> {
    let (mut x, mut y) = (0i32, 0i32);
    let mut parts: Vec<Vec<[i32; 2]>> = vec![];
    let mut i = 0;
    let zz = |v: u32| ((v >> 1) as i32) ^ -((v & 1) as i32);
    while i < cmds.len() {
        let (id, n) = (cmds[i] & 7, (cmds[i] >> 3) as usize);
        i += 1;
        match id {
            1 | 2 => {
                for _ in 0..n {
                    x += zz(cmds[i]);
                    y += zz(cmds[i + 1]);
                    i += 2;
                    if id == 1 {
                        parts.push(vec![[x, y]]);
                    } else if let Some(p) = parts.last_mut() {
                        p.push([x, y]);
                    }
                }
            }
            7 => {} // fermeture implicite : nos surfaces sont refermées à l'affichage
            _ => break,
        }
    }
    parts
}

fn read_layer(b: &[u8]) -> (String, u32, Vec<Feature>) {
    let mut p = Pb::new(b);
    let (mut name, mut extent) = (String::new(), 4096u32);
    let (mut keys, mut vals, mut raw) = (vec![], vec![], vec![]);
    while p.more() {
        let (f, wt) = p.key();
        match (f, wt) {
            (1, 2) => name = String::from_utf8_lossy(p.bytes()).into_owned(),
            (2, 2) => raw.push(p.bytes()),
            (3, 2) => keys.push(String::from_utf8_lossy(p.bytes()).into_owned()),
            (4, 2) => vals.push(read_value(p.bytes())),
            (5, 0) => extent = p.varint() as u32,
            _ => p.skip(wt),
        }
    }
    let feats = raw
        .into_iter()
        .map(|fb| {
            let mut q = Pb::new(fb);
            let (mut kind, mut tags, mut geom) = (0, vec![], vec![]);
            while q.more() {
                let (f, wt) = q.key();
                match (f, wt) {
                    (2, 2) => {
                        let mut r = Pb::new(q.bytes());
                        while r.more() {
                            tags.push(r.varint() as usize);
                        }
                    }
                    (3, 0) => kind = q.varint(),
                    (4, 2) => {
                        let mut r = Pb::new(q.bytes());
                        while r.more() {
                            geom.push(r.varint() as u32);
                        }
                    }
                    _ => q.skip(wt),
                }
            }
            let mut props = HashMap::new();
            for t in tags.chunks(2) {
                if t.len() == 2 && t[0] < keys.len() && t[1] < vals.len() {
                    props.insert(keys[t[0]].clone(), vals[t[1]].clone());
                }
            }
            Feature { kind, props, geom: decode_geom(&geom) }
        })
        .collect();
    (name, extent, feats)
}

fn s<'a>(f: &'a Feature, k: &str) -> &'a str {
    match f.props.get(k) {
        Some(Val::S(v)) => v.as_str(),
        _ => "",
    }
}

/// Quelle couche de sortie pour cet objet ? None = on l'abandonne.
fn classify(layer: &str, f: &Feature, z: u32) -> Option<u8> {
    let class = s(f, "class");
    Some(match layer {
        "landcover" => match class {
            "wood" => FOREST,
            "wetland" => WETLAND,
            "rock" => ROCK,
            "ice" => ICE,
            "sand" => SAND,
            _ => return None, // prairies et cultures : non dessinées (comme sur une carte topo)
        },
        "landuse" => match class {
            "residential" | "suburb" | "quarter" | "neighbourhood" => URBAN,
            "industrial" | "commercial" | "retail" | "railway" => INDUS,
            "military" => MILITARY,
            _ => return None,
        },
        "water" => match class {
            "swimming_pool" => return None,
            _ => WATER,
        },
        "waterway" => match class {
            "river" | "canal" => RIVER,
            "stream" if z >= 13 => STREAM,
            _ => return None,
        },
        "transportation" => {
            if s(f, "brunnel") == "tunnel" {
                return None;
            }
            match class {
                "motorway" => MOTORWAY,
                "trunk" => TRUNK,
                "primary" => PRIMARY,
                "secondary" => SECONDARY,
                "tertiary" => TERTIARY,
                // Petites rues dès le zoom 13
                "minor" if z >= 13 => MINOR,
                // Voies de service au dernier zoom seulement ; allées de parking, accès privés, drive : supprimés
                "service" if z >= 14 => match s(f, "service") {
                    "driveway" | "parking_aisle" | "drive-through" => return None,
                    _ => SERVICE,
                },
                "track" => TRACK,
                // Sentiers au dernier zoom ; trottoirs, escaliers, couloirs, quais : supprimés
                "path" if z >= 14 => match s(f, "subclass") {
                    "path" | "bridleway" | "" => PATH,
                    _ => return None,
                },
                "rail" => match s(f, "subclass") {
                    "rail" | "narrow_gauge" | "" => RAIL,
                    _ => return None,
                },
                _ => return None,
            }
        }
        _ => return None,
    })
}

fn ring_area(r: &[[i32; 2]]) -> i64 {
    let mut a = 0i64;
    for i in 0..r.len() {
        let (p, q) = (r[i], r[(i + 1) % r.len()]);
        a += p[0] as i64 * q[1] as i64 - q[0] as i64 * p[1] as i64;
    }
    a / 2
}

/// Convertit une tuile MVT en tuile au format de la page. Renvoie aussi les sommets trouvés.
fn convert(data: &[u8], z: u32, peaks: &mut Vec<(String, i64, i32, i32)>) -> crate::Tile {
    let mut raw = vec![];
    let buf: &[u8] = if data.starts_with(&[0x1f, 0x8b]) {
        GzDecoder::new(data).read_to_end(&mut raw).unwrap();
        &raw
    } else {
        data
    };
    let mut tile = crate::Tile::new();
    let mut p = Pb::new(buf);
    while p.more() {
        let (f, wt) = p.key();
        if (f, wt) != (3, 2) {
            p.skip(wt);
            continue;
        }
        let (name, extent, feats) = read_layer(p.bytes());
        let scale = OUT_EXT as f64 / extent as f64;
        for ft in &feats {
            if name == "mountain_peak" {
                if s(ft, "class") == "peak" && !s(ft, "name").is_empty() {
                    if let (Some(Val::N(ele)), Some(pt)) = (ft.props.get("ele"), ft.geom.first().and_then(|g| g.first())) {
                        peaks.push((s(ft, "name").to_string(), *ele as i64, (pt[0] as f64 * scale) as i32, (pt[1] as f64 * scale) as i32));
                    }
                }
                continue;
            }
            let Some(id) = classify(&name, ft, z) else { continue };
            for part in &ft.geom {
                // Changement d'échelle et suppression des points en double
                let mut q: Vec<[i32; 2]> = Vec::with_capacity(part.len());
                for pt in part {
                    let r = [(pt[0] as f64 * scale).round() as i32, (pt[1] as f64 * scale).round() as i32];
                    if q.last() != Some(&r) {
                        q.push(r);
                    }
                }
                if ft.kind == 3 {
                    // Surfaces de moins d'un pixel : invisibles, on les jette
                    if q.len() < 3 || ring_area(&q).abs() < (OUT_EXT as i64 / 256).pow(2) {
                        continue;
                    }
                } else if q.len() < 2 {
                    continue;
                }
                tile.entry(id).or_default().push(q);
            }
        }
    }
    tile
}

/// Paquet de tuiles : 8x8 tuiles maximum par fichier (regroupement 3 niveaux plus haut).
pub fn group_zoom(z: u32) -> u32 {
    z - 3
}

pub fn run(args: &[String]) {
    let out = Path::new(args.first().expect("dossier sortie")).join("o");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).unwrap();
    let dbs: Vec<rusqlite::Connection> = args[1..].iter().map(|p| rusqlite::Connection::open(p).expect("ouverture mbtiles")).collect();
    let mut index: BTreeMap<u32, Vec<[i64; 2]>> = BTreeMap::new();
    let mut peaks_all: BTreeMap<(i64, i64), (String, i64, f64, f64)> = BTreeMap::new();
    let mut total = 0usize;
    let mut layer_bytes: BTreeMap<(u32, u8), usize> = BTreeMap::new();
    for z in ZMIN..=ZMAX {
        let mut rows = vec![];
        for db in &dbs {
            let mut stmt = db.prepare("select tile_column, tile_row, tile_data from tiles where zoom_level = ?1").unwrap();
            rows.extend(stmt.query_map([z], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, Vec<u8>>(2)?))).unwrap().map(|r| r.unwrap()));
        }
        let n = 1i64 << z;
        let gz = group_zoom(z);
        let shift = z - gz;
        let mut bundles: BTreeMap<(i64, i64), Vec<((i64, i64), Vec<u8>)>> = BTreeMap::new();
        let (mut ntiles, mut raw_in) = (0usize, 0usize);
        for (x, tms_y, data) in rows {
            let y = n - 1 - tms_y;
            raw_in += data.len();
            let mut peaks = vec![];
            let tile = convert(&data, z, &mut peaks);
            for (name, ele, px, py) in peaks {
                // Position du sommet en lon/lat, puis dédoublonnage (~100 m)
                let fx = x as f64 + px as f64 / OUT_EXT as f64;
                let fy = y as f64 + py as f64 / OUT_EXT as f64;
                let lon = fx / n as f64 * 360.0 - 180.0;
                let lat = (std::f64::consts::PI * (1.0 - 2.0 * fy / n as f64)).sinh().atan().to_degrees();
                peaks_all.entry(((lat * 1000.0).round() as i64, (lon * 1000.0).round() as i64)).or_insert((name, ele, lat, lon));
            }
            // Tuile vide, ou pleine mer (un seul carré d'eau) : rien à dessiner de plus que le fond
            let pleine_mer = tile.len() == 1 && tile.get(&WATER).is_some_and(|p| p.len() == 1 && p[0].len() <= 5);
            if tile.is_empty() || pleine_mer {
                continue;
            }
            if std::env::var("CARTO_STATS").is_ok() {
                for (id, parts) in &tile {
                    let mut one = crate::Tile::new();
                    one.insert(*id, parts.clone());
                    *layer_bytes.entry((z, *id)).or_insert(0) += crate::encode_tile(&one).len();
                }
            }
            ntiles += 1;
            bundles.entry((x >> shift, y >> shift)).or_default().push(((x, y), crate::encode_tile(&tile)));
        }
        let mut zbytes = 0;
        for ((gx, gy), list) in bundles {
            let (bx, by) = (gx << shift, gy << shift);
            let mut head = vec![];
            crate::varint(&mut head, list.len() as u64);
            for ((x, y), body) in &list {
                crate::varint(&mut head, (x - bx) as u64);
                crate::varint(&mut head, (y - by) as u64);
                crate::varint(&mut head, body.len() as u64);
            }
            for (_, body) in &list {
                head.extend_from_slice(body);
            }
            let mut enc = GzEncoder::new(Vec::new(), Compression::best());
            enc.write_all(&head).unwrap();
            let gzd = enc.finish().unwrap();
            zbytes += gzd.len();
            fs::write(out.join(format!("{z}_{gx}_{gy}.wasm")), &gzd).unwrap();
            index.entry(z).or_default().push([gx, gy]);
        }
        total += zbytes;
        eprintln!("topo z{z}: {ntiles} tuiles, entrée {} Ko -> sortie {} Ko", raw_in / 1024, zbytes / 1024);
    }
    let idx = serde_json::json!({ "ext": OUT_EXT, "zmin": ZMIN, "zmax": ZMAX, "groups": index });
    fs::write(out.join("index.json"), idx.to_string()).unwrap();
    let peaks: Vec<_> = peaks_all
        .values()
        .map(|(n, e, la, lo)| serde_json::json!([n, *e, (la * 1e4).round() / 1e4, (lo * 1e4).round() / 1e4]))
        .collect();
    fs::write(out.join("sommets.json"), serde_json::to_string(&peaks).unwrap()).unwrap();
    eprintln!("topo total {} Ko, {} sommets", total / 1024, peaks.len());
    for z in 12..=ZMAX {
        let t: usize = layer_bytes.iter().filter(|((zz, _), _)| *zz == z).map(|(_, b)| b).sum();
        let mut v: Vec<_> = layer_bytes.iter().filter(|((zz, _), _)| *zz == z).collect();
        v.sort_by(|a, b| b.1.cmp(a.1));
        if t > 0 {
            eprintln!("z{z} (brut) : {}", v.iter().map(|((_, id), b)| format!("{id}:{}%", 100 * **b / t)).collect::<Vec<_>>().join(" "));
        }
    }
}
