//! Relief : ombrage des pentes calculé à partir d'un modèle d'altitude libre
//! (tuiles « Terrarium » de l'Amazon Open Data, zoom 10, ~110 m par pixel en France).
//!
//! Sortie : petites images en niveaux de gris (128 px par tuile, plat = blanc), regroupées en paquets.
//! La page les pose sur la carte en mode « produit » : seules les ombres assombrissent le fond.
//! Grossier exprès : au-delà du zoom 11, la page agrandit l'image en douceur.
//!
//! Usage :
//!   carto relief-liste <zones.min.json>              -> liste « 10/x/y » des tuiles d'altitude à télécharger
//!   carto relief <dossier_terrarium> <dossier_sortie> -> <sortie>/r/*.wasm + <sortie>/r/index.json

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

const SRC_Z: u32 = 10; // zoom des tuiles d'altitude téléchargées
const ZOUT_MIN: u32 = 5;
const ZOUT_MAX: u32 = 11;
const PX: usize = 128; // taille d'une tuile d'ombrage
const N: usize = 256; // taille d'une tuile d'altitude

/// Zones à couvrir : métropole + zones d'outre-mer et étranger (boîtes du fichier zones).
pub fn roi(zones: &Path) -> Vec<[f64; 4]> {
    let mut r = vec![[-5.5, 41.2, 9.8, 51.3]];
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(zones).expect("zones")).unwrap();
    for b in v["boxes"].as_array().unwrap() {
        let b: Vec<f64> = b.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        r.push([b[0], b[1], b[2], b[3]]);
    }
    r
}

fn tile_range(z: u32, b: [f64; 4]) -> (i64, i64, i64, i64) {
    let n = (1u64 << z) as f64;
    let tx = |lon: f64| ((lon + 180.0) / 360.0 * n).floor() as i64;
    let ty = |lat: f64| {
        let r = lat.to_radians();
        ((1.0 - (r.tan() + 1.0 / r.cos()).ln() / std::f64::consts::PI) / 2.0 * n).floor() as i64
    };
    (tx(b[0]), tx(b[2]), ty(b[3]), ty(b[1]))
}

pub fn liste(args: &[String]) {
    let zones = Path::new(args.first().expect("zones.min.json"));
    let mut seen = std::collections::BTreeSet::new();
    for b in roi(zones) {
        let (x0, x1, y0, y1) = tile_range(SRC_Z, b);
        for x in x0..=x1 {
            for y in y0..=y1 {
                if seen.insert((x, y)) {
                    println!("{SRC_Z}/{x}/{y}");
                }
            }
        }
    }
}

/// Altitudes d'une tuile (mètres, i16). Terrarium : altitude = R*256 + G + B/256 - 32768.
fn read_terrarium(path: &Path) -> Option<Vec<i16>> {
    let dec = png::Decoder::new(std::io::BufReader::new(fs::File::open(path).ok()?));
    let mut rd = dec.read_info().ok()?;
    let mut buf = vec![0; rd.output_buffer_size()?];
    let info = rd.next_frame(&mut buf).ok()?;
    let ch = info.color_type.samples();
    if info.width as usize != N || info.height as usize != N || ch < 3 {
        return None;
    }
    Some(
        (0..N * N)
            .map(|i| {
                let p = &buf[i * ch..i * ch + 3];
                let e = p[0] as f32 * 256.0 + p[1] as f32 + p[2] as f32 / 256.0 - 32768.0;
                e.clamp(-500.0, 9000.0) as i16
            })
            .collect(),
    )
}

type Level = HashMap<(i64, i64), Vec<i16>>;

/// Niveau inférieur (zoom - 1) par moyenne 2x2 des tuiles enfants.
fn downsample(lv: &Level) -> Level {
    let mut parents: BTreeMap<(i64, i64), ()> = BTreeMap::new();
    for (x, y) in lv.keys() {
        parents.insert((x >> 1, y >> 1), ());
    }
    let mut out = Level::new();
    for (px, py) in parents.keys() {
        let mut t = vec![0i16; N * N];
        for qy in 0..2 {
            for qx in 0..2 {
                let Some(c) = lv.get(&(px * 2 + qx, py * 2 + qy)) else { continue };
                for j in 0..N / 2 {
                    for i in 0..N / 2 {
                        let s = c[2 * j * N + 2 * i] as i32 + c[2 * j * N + 2 * i + 1] as i32 + c[(2 * j + 1) * N + 2 * i] as i32 + c[(2 * j + 1) * N + 2 * i + 1] as i32;
                        t[(qy as usize * N / 2 + j) * N + qx as usize * N / 2 + i] = (s / 4) as i16;
                    }
                }
            }
        }
        out.insert((*px, *py), t);
    }
    out
}

fn elev(lv: &Level, gx: i64, gy: i64) -> f32 {
    let (tx, ty) = (gx.div_euclid(N as i64), gy.div_euclid(N as i64));
    match lv.get(&(tx, ty)) {
        Some(t) => t[(gy.rem_euclid(N as i64) as usize) * N + gx.rem_euclid(N as i64) as usize].max(0) as f32,
        None => 0.0,
    }
}

/// Tuile d'ombrage (zoom zo) calculée sur le niveau d'altitude zs = zo - 1 (même résolution).
fn shade_tile(lv: &Level, zs: u32, zo: u32, x: i64, y: i64) -> Option<Vec<u8>> {
    let n = (1u64 << zs) as f64;
    let (ox, oy) = (x * PX as i64, y * PX as i64); // pixel global au niveau zs
    // Taille d'un pixel au sol (mètres), à la latitude de la tuile
    let lat = (std::f64::consts::PI * (1.0 - 2.0 * (oy as f64 + PX as f64 / 2.0) / (n * N as f64))).sinh().atan();
    let res = 40_075_016.686 * lat.cos() / (n * N as f64);
    // Exagération plus forte quand on voit de loin, pour que les massifs restent lisibles
    let zf = 1.6 + (ZOUT_MAX - zo) as f64 * 0.55;
    // Lumière venant du nord-ouest (315°), 45° au-dessus de l'horizon ; angle converti en repère mathématique (135°)
    let (az, alt) = (135f64.to_radians(), 45f64.to_radians());
    let mut img = vec![255u8; PX * PX];
    let mut any = false;
    for j in 0..PX as i64 {
        for i in 0..PX as i64 {
            let (gx, gy) = (ox + i, oy + j);
            let e = |dx: i64, dy: i64| elev(lv, gx + dx, gy + dy) as f64;
            // Pente de Horn (3x3)
            let dzdx = ((e(1, -1) + 2.0 * e(1, 0) + e(1, 1)) - (e(-1, -1) + 2.0 * e(-1, 0) + e(-1, 1))) / (8.0 * res) * zf;
            let dzdy = ((e(-1, 1) + 2.0 * e(0, 1) + e(1, 1)) - (e(-1, -1) + 2.0 * e(0, -1) + e(1, -1))) / (8.0 * res) * zf;
            if dzdx == 0.0 && dzdy == 0.0 {
                continue;
            }
            let slope = (dzdx * dzdx + dzdy * dzdy).sqrt().atan();
            let aspect = dzdy.atan2(-dzdx);
            let s = alt.sin() * slope.cos() + alt.cos() * slope.sin() * (az - aspect).cos();
            // Plat = blanc ; seules les pentes à l'ombre s'assombrissent
            let shade = (s / alt.sin()).clamp(0.0, 1.0);
            let v = 255.0 - (1.0 - shade) * 175.0;
            // 32 niveaux : invisible à l'œil, et l'image se compresse bien mieux
            let q = ((v / 8.0).round() * 8.0).clamp(0.0, 255.0) as u8;
            if q < 248 {
                any = true;
            }
            img[j as usize * PX + i as usize] = q;
        }
    }
    if !any {
        return None; // tuile toute plate (mer, plaine) : pas d'image, la page n'ombre rien
    }
    let mut png_bytes = vec![];
    {
        let mut enc = png::Encoder::new(&mut png_bytes, PX as u32, PX as u32);
        enc.set_color(png::ColorType::Grayscale);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::High);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&img).unwrap();
    }
    Some(png_bytes)
}

pub fn run(args: &[String]) {
    let src = Path::new(args.first().expect("dossier terrarium"));
    let out = Path::new(args.get(1).expect("dossier sortie")).join("r");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).unwrap();
    // Chargement du zoom 10
    let mut levels: BTreeMap<u32, Level> = BTreeMap::new();
    let mut lv = Level::new();
    for xd in fs::read_dir(src.join(SRC_Z.to_string())).expect("dossier 10/") {
        let xd = xd.unwrap();
        let Ok(x) = xd.file_name().to_string_lossy().parse::<i64>() else { continue };
        for f in fs::read_dir(xd.path()).unwrap() {
            let f = f.unwrap().path();
            let Some(y) = f.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<i64>().ok()) else { continue };
            if let Some(t) = read_terrarium(&f) {
                lv.insert((x, y), t);
            }
        }
    }
    eprintln!("relief : {} tuiles d'altitude chargées", lv.len());
    levels.insert(SRC_Z, lv);
    for z in (ZOUT_MIN - 1..SRC_Z).rev() {
        let d = downsample(&levels[&(z + 1)]);
        levels.insert(z, d);
    }
    let mut index: BTreeMap<u32, Vec<[i64; 2]>> = BTreeMap::new();
    let mut total = 0;
    for zo in ZOUT_MIN..=ZOUT_MAX {
        let zs = zo - 1;
        let lv = &levels[&zs];
        // Tuiles de sortie : les quatre quarts de chaque tuile d'altitude
        let mut tiles = vec![];
        for (tx, ty) in lv.keys() {
            for q in 0..4 {
                tiles.push((tx * 2 + (q & 1), ty * 2 + (q >> 1)));
            }
        }
        tiles.sort();
        let shift = 3.min(zo); // paquets de 8x8 tuiles
        let mut bundles: BTreeMap<(i64, i64), Vec<((i64, i64), Vec<u8>)>> = BTreeMap::new();
        for (x, y) in tiles {
            if let Some(png) = shade_tile(lv, zs, zo, x, y) {
                bundles.entry((x >> shift, y >> shift)).or_default().push(((x, y), png));
            }
        }
        let mut zb = 0;
        let mut nt = 0;
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
            nt += list.len();
            zb += head.len();
            fs::write(out.join(format!("{zo}_{gx}_{gy}.wasm")), &head).unwrap();
            index.entry(zo).or_default().push([gx, gy]);
        }
        total += zb;
        eprintln!("relief z{zo}: {nt} tuiles, {} Ko", zb / 1024);
    }
    let idx = serde_json::json!({ "px": PX, "zmin": ZOUT_MIN, "zmax": ZOUT_MAX, "groups": index });
    fs::write(out.join("index.json"), idx.to_string()).unwrap();
    eprintln!("relief total {} Ko", total / 1024);
}
