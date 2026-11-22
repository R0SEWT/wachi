//! Lectura del pbf de OSM: vías peatonales y recintos (ADR 013, 014).
//!
//! Equivale a `read_osm` de `api/wachi/graph.py` (pyosmium + `FileProcessor.with_areas`):
//! - las vías se devuelven en el orden del archivo, porque ese orden numera las aristas;
//! - una vía entra si pasa `walk_ok` y su primer o último nodo cae en el bbox;
//! - los recintos salen de vías cerradas y de relaciones multipolígono con alguna de las etiquetas.
//!
//! Se lee el archivo en tres pasadas: relaciones y nodos del bbox (en paralelo), vías (en orden) y
//! coordenadas de los nodos que se usan (en paralelo). Así no hace falta tener en memoria las
//! coordenadas de todo el Perú.

use anyhow::{Context, Result};
use geo::{Coord, LineString, MultiPolygon, Polygon};
use osmpbf::{Element, ElementReader, RelMemberType};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Una vía peatonal: ids de nodo con coordenada válida y sus (lon, lat) en grados.
pub struct Way {
    pub ids: Vec<i64>,
    pub lonlat: Vec<(f64, f64)>,
    pub highway: String,
}

/// Recinto en grados (se proyecta después).
pub struct RecintoLL {
    pub kind: String,
    pub name: String,
    pub geom: MultiPolygon<f64>,
}

const EXCL_HW: &[&str] = &[
    "abandoned",
    "bus_guideway",
    "busway",
    "bus_stop",
    "construction",
    "cycleway",
    "motor",
    "no",
    "planned",
    "platform",
    "proposed",
    "raceway",
    "razed",
];
const SIDEWALK_KEYS: &[&str] = &[
    "sidewalk",
    "sidewalk:both",
    "sidewalk:left",
    "sidewalk:right",
];
const FOOT_OK: &[&str] = &["yes", "designated", "permissive"];

/// Filtro tipo osmnx `network_type='walk'` + trunk sin acera y access=no (igual que Python).
pub fn walk_ok(tags: &HashMap<&str, &str>) -> bool {
    let Some(hw) = tags.get("highway") else {
        return false;
    };
    // `_EXCL_HW.search(hw)`: basta con que el patrón aparezca dentro del valor
    if tags.get("area") == Some(&"yes") || EXCL_HW.iter().any(|p| hw.contains(p)) {
        return false;
    }
    let foot = tags.get("foot").copied().unwrap_or("");
    if foot == "no" || tags.get("service") == Some(&"private") {
        return false;
    }
    if SIDEWALK_KEYS
        .iter()
        .any(|k| tags.get(k) == Some(&"separate"))
    {
        return false;
    }
    if matches!(tags.get("access"), Some(&"private") | Some(&"no")) && !FOOT_OK.contains(&foot) {
        return false;
    }
    if *hw == "trunk" || *hw == "trunk_link" {
        let has_sw = SIDEWALK_KEYS.iter().any(|k| {
            matches!(
                tags.get(k),
                Some(&"yes") | Some(&"both") | Some(&"left") | Some(&"right")
            )
        });
        if !has_sw && !FOOT_OK.contains(&foot) {
            return false;
        }
    }
    true
}

fn recinto_kind(tags: &HashMap<&str, &str>, recinto_tags: &[(String, String)]) -> Option<String> {
    recinto_tags
        .iter()
        .find(|(k, v)| tags.get(k.as_str()) == Some(&v.as_str()))
        .map(|(k, v)| format!("{k}={v}"))
}

struct Rel {
    kind: String,
    name: String,
    ways: Vec<i64>,
}

enum Pend {
    /// vía peatonal: índice en `ways_raw`
    Walk(Vec<i64>, String),
    /// vía cerrada con etiqueta de recinto
    AreaWay(Vec<i64>, String, String),
    /// relaciones que se completan al leer esta vía (su último miembro en el archivo)
    RelDone(Vec<usize>),
}

pub struct OsmData {
    pub ways: Vec<Way>,
    pub recintos: Vec<RecintoLL>,
    pub recintos_descartados: usize,
}

fn decimicro(v: i32) -> f64 {
    // osmium guarda la coordenada como entero en 1e-7 grados y la convierte dividiendo
    v as f64 / 10_000_000.0
}

pub fn read_osm(pbf: &Path, bbox: [f64; 4], recinto_tags: &[(String, String)]) -> Result<OsmData> {
    let [s, w, n, e] = bbox;
    let open =
        || ElementReader::from_path(pbf).with_context(|| format!("abriendo {}", pbf.display()));

    // Pasada 1: nodos dentro del bbox + relaciones multipolígono de recintos.
    let (mut in_bbox, rels): (Vec<i64>, Vec<(i64, Rel)>) = open()?.par_map_reduce(
        |el| {
            let mut nodes = Vec::new();
            let mut rels = Vec::new();
            let inb = |la: i32, lo: i32| {
                let (la, lo) = (decimicro(la), decimicro(lo));
                s <= la && la <= n && w <= lo && lo <= e
            };
            match el {
                Element::DenseNode(nd) => {
                    if inb(nd.decimicro_lat(), nd.decimicro_lon()) {
                        nodes.push(nd.id());
                    }
                }
                Element::Node(nd) => {
                    if inb(nd.decimicro_lat(), nd.decimicro_lon()) {
                        nodes.push(nd.id());
                    }
                }
                Element::Relation(r) => {
                    let tags: HashMap<&str, &str> = r.tags().collect();
                    let t = tags.get("type").copied().unwrap_or("");
                    if (t == "multipolygon" || t == "boundary")
                        && let Some(kind) = recinto_kind(&tags, recinto_tags)
                    {
                        let ways = r
                            .members()
                            .filter(|m| m.member_type == RelMemberType::Way)
                            .map(|m| m.member_id)
                            .collect();
                        let name = tags.get("name").copied().unwrap_or("").to_string();
                        rels.push((r.id(), Rel { kind, name, ways }));
                    }
                }
                Element::Way(_) => {}
            }
            (nodes, rels)
        },
        || (Vec::new(), Vec::new()),
        |(mut a, mut b), (c, d)| {
            a.extend(c);
            b.extend(d);
            (a, b)
        },
    )?;
    in_bbox.sort_unstable();
    let in_bbox: HashSet<i64> = in_bbox.into_iter().collect();
    let mut rels = rels;
    rels.sort_by_key(|(id, _)| *id);
    let rels: Vec<Rel> = rels.into_iter().map(|(_, r)| r).collect();
    // vía miembro → relaciones que la usan
    let mut member_of: HashMap<i64, Vec<usize>> = HashMap::new();
    for (ri, r) in rels.iter().enumerate() {
        for &wid in &r.ways {
            member_of.entry(wid).or_default().push(ri);
        }
    }

    // Pasada 2 (en orden de archivo): vías peatonales, vías-recinto y miembros de relaciones.
    let mut pend: Vec<Pend> = Vec::new();
    let mut member_refs: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut missing: Vec<usize> = rels
        .iter()
        .map(|r| r.ways.iter().collect::<HashSet<_>>().len())
        .collect();
    open()?.for_each(|el| {
        let Element::Way(way) = el else { return };
        let tags: HashMap<&str, &str> = way.tags().collect();
        let refs = || way.refs().collect::<Vec<i64>>();
        if let Some(hw) = tags.get("highway") {
            let r = refs();
            if !r.is_empty()
                && (in_bbox.contains(&r[0]) || in_bbox.contains(&r[r.len() - 1]))
                && walk_ok(&tags)
            {
                pend.push(Pend::Walk(r, hw.to_string()));
            }
        }
        // área desde vía cerrada (osmium: cerrada y con ≥ 4 nodos)
        if let Some(kind) = recinto_kind(&tags, recinto_tags) {
            let r = refs();
            if r.len() >= 4 && r[0] == r[r.len() - 1] && tags.get("area") != Some(&"no") {
                let name = tags.get("name").copied().unwrap_or("").to_string();
                pend.push(Pend::AreaWay(r, kind, name));
            }
        }
        if let Some(rs) = member_of.get(&way.id()) {
            member_refs.entry(way.id()).or_insert_with(refs);
            let mut done = Vec::new();
            for &ri in rs {
                missing[ri] -= 1;
                if missing[ri] == 0 {
                    done.push(ri);
                }
            }
            if !done.is_empty() {
                pend.push(Pend::RelDone(done));
            }
        }
    })?;

    // Pasada 3: coordenadas de los nodos que se usan.
    let mut needed: Vec<i64> = Vec::new();
    for p in &pend {
        match p {
            Pend::Walk(r, _) | Pend::AreaWay(r, _, _) => needed.extend_from_slice(r),
            Pend::RelDone(_) => {}
        }
    }
    for r in member_refs.values() {
        needed.extend_from_slice(r);
    }
    needed.sort_unstable();
    needed.dedup();
    let needed_set: HashSet<i64> = needed.into_iter().collect();
    let locs: HashMap<i64, (i32, i32)> = open()?
        .par_map_reduce(
            |el| {
                let mut out = Vec::new();
                match el {
                    Element::DenseNode(nd) if needed_set.contains(&nd.id()) => {
                        out.push((nd.id(), (nd.decimicro_lon(), nd.decimicro_lat())))
                    }
                    Element::Node(nd) if needed_set.contains(&nd.id()) => {
                        out.push((nd.id(), (nd.decimicro_lon(), nd.decimicro_lat())))
                    }
                    _ => {}
                }
                out
            },
            Vec::new,
            |mut a, b| {
                a.extend(b);
                a
            },
        )?
        .into_iter()
        .collect();
    let ll = |id: &i64| locs.get(id).map(|&(lo, la)| (decimicro(lo), decimicro(la)));

    let mut ways = Vec::new();
    let mut recintos = Vec::new();
    let mut skipped = 0usize;
    let mut push_recinto =
        |kind: &str, name: &str, g: Option<MultiPolygon<f64>>, skipped: &mut usize| {
            let Some(g) = g else {
                *skipped += 1;
                return;
            };
            use geo::BoundingRect;
            let Some(b) = g.bounding_rect() else {
                *skipped += 1;
                return;
            };
            if b.max().x < w || b.min().x > e || b.max().y < s || b.min().y > n {
                return;
            }
            recintos.push(RecintoLL {
                kind: kind.to_string(),
                name: name.to_string(),
                geom: g,
            });
        };
    for p in pend {
        match p {
            Pend::Walk(r, hw) => {
                let mut ids = Vec::with_capacity(r.len());
                let mut lonlat = Vec::with_capacity(r.len());
                for id in &r {
                    if let Some(c) = ll(id) {
                        ids.push(*id);
                        lonlat.push(c);
                    }
                }
                if ids.len() >= 2 {
                    ways.push(Way {
                        ids,
                        lonlat,
                        highway: hw,
                    });
                }
            }
            Pend::AreaWay(r, kind, name) => {
                let coords: Option<Vec<(f64, f64)>> = r.iter().map(ll).collect();
                let g = coords.and_then(|c| assemble(vec![c]));
                push_recinto(&kind, &name, g, &mut skipped);
            }
            Pend::RelDone(rs) => {
                for ri in rs {
                    let rel = &rels[ri];
                    let mut parts = Vec::new();
                    let mut ok = true;
                    let mut seen = HashSet::new();
                    for wid in &rel.ways {
                        if !seen.insert(*wid) {
                            continue;
                        }
                        match member_refs
                            .get(wid)
                            .and_then(|r| r.iter().map(ll).collect::<Option<Vec<_>>>())
                        {
                            Some(c) if c.len() >= 2 => parts.push(c),
                            _ => ok = false,
                        }
                    }
                    let g = if ok { assemble(parts) } else { None };
                    push_recinto(&rel.kind, &rel.name, g, &mut skipped);
                }
            }
        }
    }
    if skipped > 0 {
        tracing::warn!("read_osm: {skipped} recintos descartados por geometría inválida");
    }
    Ok(OsmData {
        ways,
        recintos,
        recintos_descartados: skipped,
    })
}

/// Une tramos en anillos cerrados y arma un multipolígono (los anillos dentro de otro, a
/// profundidad impar, son huecos). `None` si algún tramo no cierra.
pub fn assemble(parts: Vec<Vec<(f64, f64)>>) -> Option<MultiPolygon<f64>> {
    let mut open: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut rings: Vec<Vec<(f64, f64)>> = Vec::new();
    for p in parts {
        if p.len() >= 4 && p[0] == p[p.len() - 1] {
            rings.push(p);
        } else {
            open.push(p);
        }
    }
    while let Some(mut cur) = open.pop() {
        loop {
            if cur.len() >= 4 && cur[0] == cur[cur.len() - 1] {
                rings.push(cur);
                break;
            }
            let end = *cur.last().unwrap();
            let start = cur[0];
            let pos = open.iter().position(|o| {
                o[0] == end
                    || *o.last().unwrap() == end
                    || o[0] == start
                    || *o.last().unwrap() == start
            })?;
            let mut o = open.swap_remove(pos);
            if o[0] == end {
                cur.extend_from_slice(&o[1..]);
            } else if *o.last().unwrap() == end {
                o.reverse();
                cur.extend_from_slice(&o[1..]);
            } else if *o.last().unwrap() == start {
                o.extend_from_slice(&cur[1..]);
                cur = o;
            } else {
                o.reverse();
                o.extend_from_slice(&cur[1..]);
                cur = o;
            }
        }
    }
    if rings.is_empty() {
        return None;
    }
    use geo::{Area, Contains};
    let polys: Vec<Polygon<f64>> = rings
        .into_iter()
        .map(|r| {
            Polygon::new(
                LineString::from(
                    r.into_iter()
                        .map(|(x, y)| Coord { x, y })
                        .collect::<Vec<_>>(),
                ),
                vec![],
            )
        })
        .collect();
    let mut order: Vec<usize> = (0..polys.len()).collect();
    order.sort_by(|&a, &b| {
        polys[b]
            .unsigned_area()
            .total_cmp(&polys[a].unsigned_area())
    });
    // profundidad = cuántos anillos más grandes lo contienen; contenedor = el más chico de ellos
    let mut outers: Vec<(usize, Vec<LineString<f64>>)> = Vec::new();
    for (k, &i) in order.iter().enumerate() {
        let probe = polys[i].exterior().0[0];
        let containers: Vec<usize> = order[..k]
            .iter()
            .copied()
            .filter(|&j| polys[j].contains(&geo::Point(probe)))
            .collect();
        if containers.len().is_multiple_of(2) {
            outers.push((i, vec![]));
        } else if let Some(&c) = containers.last()
            && let Some(o) = outers.iter_mut().find(|(oi, _)| *oi == c)
        {
            o.1.push(polys[i].exterior().clone());
        }
    }
    Some(MultiPolygon(
        outers
            .into_iter()
            .map(|(i, holes)| Polygon::new(polys[i].exterior().clone(), holes))
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags<'a>(kv: &[(&'a str, &'a str)]) -> HashMap<&'a str, &'a str> {
        kv.iter().copied().collect()
    }

    #[test]
    fn filtro_peatonal() {
        assert!(walk_ok(&tags(&[("highway", "residential")])));
        assert!(walk_ok(&tags(&[("highway", "footway")])));
        assert!(!walk_ok(&tags(&[("highway", "motorway")])));
        assert!(!walk_ok(&tags(&[("highway", "busway")])));
        assert!(!walk_ok(&tags(&[
            ("highway", "residential"),
            ("foot", "no")
        ])));
        assert!(!walk_ok(&tags(&[("highway", "trunk")])));
        assert!(walk_ok(&tags(&[
            ("highway", "trunk"),
            ("sidewalk", "both")
        ])));
        assert!(!walk_ok(&tags(&[
            ("highway", "service"),
            ("access", "private")
        ])));
        assert!(walk_ok(&tags(&[
            ("highway", "service"),
            ("access", "private"),
            ("foot", "yes")
        ])));
        assert!(!walk_ok(&tags(&[
            ("highway", "residential"),
            ("sidewalk:left", "separate")
        ])));
        assert!(!walk_ok(&tags(&[
            ("highway", "pedestrian"),
            ("area", "yes")
        ])));
    }

    #[test]
    fn arma_anillo_desde_tramos() {
        let a = vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)];
        let b = vec![(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)]; // al revés
        let g = assemble(vec![a, b]).unwrap();
        assert_eq!(g.0.len(), 1);
        use geo::Area;
        assert!((g.unsigned_area() - 1.0).abs() < 1e-12);
        assert!(assemble(vec![vec![(0.0, 0.0), (1.0, 0.0)]]).is_none());
    }

    #[test]
    fn hueco_dentro_de_exterior() {
        let ext = vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (0.0, 0.0)];
        let hole = vec![(1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (1.0, 2.0), (1.0, 1.0)];
        let g = assemble(vec![hole, ext]).unwrap();
        assert_eq!(g.0.len(), 1);
        assert_eq!(g.0[0].interiors().len(), 1);
    }
}
