//! Red peatonal: aristas entre nodos compartidos, en UTM, componente conexa mayor (ADR 014).
//!
//! Misma numeración que `build_network` de Python: nodos ordenados por id de OSM, aristas en el
//! orden en que salen de las vías. La caché es un formato binario propio (`network.wachi`).

use crate::config::Settings;
use crate::geo_utm::to_utm;
use crate::osm::{OsmData, RecintoLL, read_osm};
use anyhow::{Context, Result, bail};
use geo::{Coord, LineString, MapCoords, MultiPolygon, Polygon};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::time::Instant;

/// Subir cuando cambie cómo se construye la red o el formato de la caché.
pub const CACHE_VERSION: u32 = 1;
const MAGIC: &[u8; 8] = b"WACHINET";

pub struct Recinto {
    pub kind: String,
    pub name: String,
    pub geom: MultiPolygon<f64>, // UTM
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub vias_peatonales: usize,
    pub nodos: usize,
    pub aristas: usize,
    pub km_red: f64,
    pub recintos: usize,
    pub recintos_por_tipo: std::collections::BTreeMap<String, usize>,
    pub recintos_descartados: usize,
    pub pbf_sha256: String,
    pub cache_version: u32,
    pub segundos_construccion: f64,
}

pub struct Network {
    pub u: Vec<u32>,
    pub v: Vec<u32>,
    pub length: Vec<f64>,
    /// offsets en `xy` (len E+1)
    pub off: Vec<u64>,
    /// UTM de todas las geometrías concatenadas
    pub xy: Vec<[f64; 2]>,
    /// UTM de los nodos
    pub nxy: Vec<[f64; 2]>,
    pub hw: Vec<u16>,
    pub hw_names: Vec<String>,
    pub recintos: Vec<Recinto>,
    pub stats: Stats,
}

impl Network {
    pub fn n_nodes(&self) -> usize {
        self.nxy.len()
    }
    pub fn n_edges(&self) -> usize {
        self.u.len()
    }
    pub fn edge_xy(&self, e: usize) -> &[[f64; 2]] {
        &self.xy[self.off[e] as usize..self.off[e + 1] as usize]
    }
}

fn line_len(c: &[[f64; 2]]) -> f64 {
    // shapely.length: suma de segmentos en orden
    let mut s = 0.0;
    for p in c.windows(2) {
        s += ((p[1][0] - p[0][0]).powi(2) + (p[1][1] - p[0][1]).powi(2)).sqrt();
    }
    s
}

struct Dsu(Vec<u32>);
impl Dsu {
    fn find(&mut self, mut x: u32) -> u32 {
        while self.0[x as usize] != x {
            let p = self.0[self.0[x as usize] as usize];
            self.0[x as usize] = p;
            x = p;
        }
        x
    }
    fn union(&mut self, a: u32, b: u32) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            self.0[hi as usize] = lo;
        }
    }
}

pub fn build_network(osm: OsmData) -> Network {
    let OsmData {
        ways,
        recintos,
        recintos_descartados,
    } = osm;
    // nodos compartidos: aparecen ≥ 2 veces en el total de vías (np.unique con return_counts)
    let mut cnt: HashMap<i64, u32> = HashMap::new();
    for w in &ways {
        for id in &w.ids {
            *cnt.entry(*id).or_default() += 1;
        }
    }
    let mut hw_names: Vec<String> = Vec::new();
    let mut hw_idx: HashMap<String, u16> = HashMap::new();
    let (mut eu, mut ev, mut ehw) = (Vec::new(), Vec::new(), Vec::new());
    let mut coords: Vec<[f64; 2]> = Vec::new();
    let mut off: Vec<u64> = vec![0];
    for w in &ways {
        let ids = &w.ids;
        let mut cut = vec![0usize];
        cut.extend((1..ids.len().saturating_sub(1)).filter(|&i| cnt[&ids[i]] >= 2));
        cut.push(ids.len() - 1);
        let h = *hw_idx.entry(w.highway.clone()).or_insert_with(|| {
            hw_names.push(w.highway.clone());
            (hw_names.len() - 1) as u16
        });
        for p in cut.windows(2) {
            let (a, b) = (p[0], p[1]);
            if ids[a] == ids[b] && b - a < 2 {
                continue;
            }
            eu.push(ids[a]);
            ev.push(ids[b]);
            ehw.push(h);
            coords.extend(w.lonlat[a..=b].iter().map(|&(lo, la)| [lo, la]));
            off.push(off.last().unwrap() + (b - a + 1) as u64);
        }
    }
    let xy: Vec<[f64; 2]> = coords
        .par_iter()
        .map(|c| {
            let (x, y) = to_utm(c[0], c[1]);
            [x, y]
        })
        .collect();
    let ne = eu.len();
    let length: Vec<f64> = (0..ne)
        .into_par_iter()
        .map(|e| line_len(&xy[off[e] as usize..off[e + 1] as usize]))
        .collect();
    let keep0: Vec<bool> = (0..ne).map(|e| length[e] > 0.5 && eu[e] != ev[e]).collect();
    // np.unique(np.r_[eu, ev]) → índice denso ordenado por id de OSM
    let mut node_ids: Vec<i64> = eu.iter().chain(ev.iter()).copied().collect();
    node_ids.sort_unstable();
    node_ids.dedup();
    let pos: HashMap<i64, u32> = node_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i as u32))
        .collect();
    let ui: Vec<u32> = eu.iter().map(|id| pos[id]).collect();
    let vi: Vec<u32> = ev.iter().map(|id| pos[id]).collect();
    let mut nxy = vec![[0.0; 2]; node_ids.len()];
    for e in 0..ne {
        nxy[ui[e] as usize] = xy[off[e] as usize];
    }
    for e in 0..ne {
        nxy[vi[e] as usize] = xy[off[e + 1] as usize - 1];
    }
    // componentes conexas sobre las aristas que se quedan; la mayor (empate: la de menor etiqueta,
    // que en scipy es la que contiene el nodo de menor índice)
    let mut dsu = Dsu((0..node_ids.len() as u32).collect());
    for e in 0..ne {
        if keep0[e] {
            dsu.union(ui[e], vi[e]);
        }
    }
    let roots: Vec<u32> = (0..node_ids.len() as u32).map(|i| dsu.find(i)).collect();
    let mut size: HashMap<u32, usize> = HashMap::new();
    for &r in &roots {
        *size.entry(r).or_default() += 1;
    }
    // etiqueta scipy = orden de aparición de la raíz al recorrer nodos 0..N; argmax → primer máximo
    let mut best: Option<(usize, u32)> = None;
    let mut seen = std::collections::HashSet::new();
    for &r in &roots {
        if seen.insert(r) {
            let s = size[&r];
            if best.is_none_or(|(bs, _)| s > bs) {
                best = Some((s, r));
            }
        }
    }
    let big = best.map(|b| b.1).unwrap_or(0);
    let mut remap = vec![u32::MAX; node_ids.len()];
    let mut nxy_new = Vec::new();
    for (i, &r) in roots.iter().enumerate() {
        if r == big {
            remap[i] = nxy_new.len() as u32;
            nxy_new.push(nxy[i]);
        }
    }
    let (mut u, mut v, mut len2, mut hw2, mut xy2, mut off2) = (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![0u64],
    );
    for e in 0..ne {
        if keep0[e] && roots[ui[e] as usize] == big {
            u.push(remap[ui[e] as usize]);
            v.push(remap[vi[e] as usize]);
            len2.push(length[e]);
            hw2.push(ehw[e]);
            xy2.extend_from_slice(&xy[off[e] as usize..off[e + 1] as usize]);
            off2.push(xy2.len() as u64);
        }
    }
    let recintos: Vec<Recinto> = recintos
        .into_iter()
        .map(|RecintoLL { kind, name, geom }| Recinto {
            kind,
            name,
            geom: geom.map_coords(|c| {
                let (x, y) = to_utm(c.x, c.y);
                Coord { x, y }
            }),
        })
        .collect();
    let mut kinds = std::collections::BTreeMap::new();
    for r in &recintos {
        *kinds.entry(r.kind.clone()).or_default() += 1;
    }
    let stats = Stats {
        vias_peatonales: ways.len(),
        nodos: nxy_new.len(),
        aristas: u.len(),
        km_red: len2.iter().sum::<f64>() / 1000.0,
        recintos: recintos.len(),
        recintos_por_tipo: kinds,
        recintos_descartados,
        ..Default::default()
    };
    Network {
        u,
        v,
        length: len2,
        off: off2,
        xy: xy2,
        nxy: nxy_new,
        hw: hw2,
        hw_names,
        recintos,
        stats,
    }
}

// ---------- caché binaria ----------

fn put<W: Write>(w: &mut W, b: &[u8]) -> Result<()> {
    w.write_all(&(b.len() as u64).to_le_bytes())?;
    w.write_all(b)?;
    Ok(())
}

fn get<R: Read>(r: &mut R) -> Result<Vec<u8>> {
    let mut n = [0u8; 8];
    r.read_exact(&mut n)?;
    let n = u64::from_le_bytes(n) as usize;
    let mut b = vec![0u8; n];
    r.read_exact(&mut b)?;
    Ok(b)
}

fn cast_vec<T: bytemuck::Pod>(b: Vec<u8>) -> Vec<T> {
    bytemuck::pod_collect_to_vec(&b)
}

/// Anillo exterior y huecos de un polígono, en UTM.
type PolySer = (Vec<[f64; 2]>, Vec<Vec<[f64; 2]>>);

#[derive(Serialize, Deserialize)]
struct RecintoSer {
    kind: String,
    name: String,
    polys: Vec<PolySer>,
}

pub fn save_cache(net: &Network, path: &Path) -> Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = path.with_extension("tmp");
    let mut w = BufWriter::new(std::fs::File::create(&tmp)?);
    w.write_all(MAGIC)?;
    w.write_all(&CACHE_VERSION.to_le_bytes())?;
    put(&mut w, serde_json::to_string(&net.stats)?.as_bytes())?;
    put(&mut w, bytemuck::cast_slice(&net.u))?;
    put(&mut w, bytemuck::cast_slice(&net.v))?;
    put(&mut w, bytemuck::cast_slice(&net.length))?;
    put(&mut w, bytemuck::cast_slice(&net.off))?;
    put(&mut w, bytemuck::cast_slice(&net.xy))?;
    put(&mut w, bytemuck::cast_slice(&net.nxy))?;
    put(&mut w, bytemuck::cast_slice(&net.hw))?;
    put(&mut w, serde_json::to_string(&net.hw_names)?.as_bytes())?;
    let rec: Vec<RecintoSer> = net
        .recintos
        .iter()
        .map(|r| RecintoSer {
            kind: r.kind.clone(),
            name: r.name.clone(),
            polys: r
                .geom
                .0
                .iter()
                .map(|p| {
                    let ring =
                        |l: &LineString<f64>| l.0.iter().map(|c| [c.x, c.y]).collect::<Vec<_>>();
                    (ring(p.exterior()), p.interiors().iter().map(ring).collect())
                })
                .collect(),
        })
        .collect();
    put(&mut w, serde_json::to_string(&rec)?.as_bytes())?;
    w.flush()?;
    drop(w);
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn load_cache(path: &Path) -> Result<Network> {
    let mut r = BufReader::new(std::fs::File::open(path)?);
    let mut magic = [0u8; 8];
    r.read_exact(&mut magic)?;
    if &magic != MAGIC {
        bail!("{} no es una caché de red de wachi", path.display());
    }
    let mut ver = [0u8; 4];
    r.read_exact(&mut ver)?;
    let mut stats: Stats = serde_json::from_slice(&get(&mut r)?)?;
    stats.cache_version = u32::from_le_bytes(ver);
    let u = cast_vec(get(&mut r)?);
    let v = cast_vec(get(&mut r)?);
    let length = cast_vec(get(&mut r)?);
    let off = cast_vec(get(&mut r)?);
    let xy = cast_vec(get(&mut r)?);
    let nxy = cast_vec(get(&mut r)?);
    let hw = cast_vec(get(&mut r)?);
    let hw_names = serde_json::from_slice(&get(&mut r)?)?;
    let rec: Vec<RecintoSer> = serde_json::from_slice(&get(&mut r)?)?;
    let recintos = rec
        .into_iter()
        .map(|r| {
            let ls = |v: Vec<[f64; 2]>| {
                LineString::from(
                    v.into_iter()
                        .map(|[x, y]| Coord { x, y })
                        .collect::<Vec<_>>(),
                )
            };
            Recinto {
                kind: r.kind,
                name: r.name,
                geom: MultiPolygon(
                    r.polys
                        .into_iter()
                        .map(|(e, h)| Polygon::new(ls(e), h.into_iter().map(ls).collect()))
                        .collect(),
                ),
            }
        })
        .collect();
    Ok(Network {
        u,
        v,
        length,
        off,
        xy,
        nxy,
        hw,
        hw_names,
        recintos,
        stats,
    })
}

pub fn file_sha256(p: &Path) -> Result<String> {
    let mut f = BufReader::with_capacity(
        1 << 20,
        std::fs::File::open(p).with_context(|| format!("abriendo {}", p.display()))?,
    );
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// Por qué una caché no sirve para esta configuración, o `None` si sirve.
pub fn stale_reason(stats: &Stats, cfg: &Settings) -> Option<String> {
    if stats.cache_version != CACHE_VERSION {
        return Some(format!(
            "cache_version {} ≠ {CACHE_VERSION}",
            stats.cache_version
        ));
    }
    if !cfg.pbf_sha256.is_empty() && stats.pbf_sha256 != cfg.pbf_sha256 {
        return Some(format!(
            "pbf_sha256 de la caché {:.12}… ≠ fijado {:.12}…",
            stats.pbf_sha256, cfg.pbf_sha256
        ));
    }
    None
}

pub fn load_or_build(cfg: &Settings) -> Result<Network> {
    load_or_build_with(cfg, |c| read_osm(&c.pbf_path, c.bbox, &c.recinto_tags))
}

/// Como `load_or_build`, con el lector de OSM inyectable (los tests lo reemplazan).
pub fn load_or_build_with(
    cfg: &Settings,
    read: impl Fn(&Settings) -> Result<OsmData>,
) -> Result<Network> {
    if cfg.graph_cache.exists() {
        match load_cache(&cfg.graph_cache) {
            Ok(net) => match stale_reason(&net.stats, cfg) {
                None => return Ok(net),
                Some(why) => tracing::warn!("reconstruyendo {}: {why}", cfg.graph_cache.display()),
            },
            Err(e) => tracing::warn!(
                "reconstruyendo {}: no se pudo leer ({e})",
                cfg.graph_cache.display()
            ),
        }
    }
    let t0 = Instant::now();
    let digest = file_sha256(&cfg.pbf_path)?;
    if !cfg.pbf_sha256.is_empty() && digest != cfg.pbf_sha256 {
        bail!(
            "sha256 de {} no coincide: {:.12}… ≠ {:.12}…",
            cfg.pbf_path.display(),
            digest,
            cfg.pbf_sha256
        );
    }
    let osm = read(cfg)?;
    let mut net = build_network(osm);
    net.stats.pbf_sha256 = digest;
    net.stats.cache_version = CACHE_VERSION;
    net.stats.segundos_construccion = (t0.elapsed().as_secs_f64() * 10.0).round() / 10.0;
    save_cache(&net, &cfg.graph_cache)?;
    Ok(net)
}
