//! Motor de ruteo: camino mínimo sobre la red peatonal con costo largo·(1+β·r̄) (ADR 014).
//!
//! Diferencias con `api/wachi/router.py` que no cambian el resultado:
//! - A* con heurística euclidiana en vez de Dijkstra completo de scipy: toda arista mide al menos
//!   la recta entre sus extremos y el costo es ≥ largo, así que la heurística es admisible y
//!   consistente y el camino es el mismo (salvo empates exactos de costo).
//! - No hay caché de grafos ponderados por (β, recintos bloqueados): el peso se calcula al relajar
//!   cada arista y el bloqueo es una comprobación por arista. Menos memoria y sin candados.

use crate::config::Settings;
use crate::network::Network;
use crate::num::{percentile, round_half_even};
use crate::spatial::Grid;
use crate::surface::Surface;
use geo::{BoundingRect, Buffer, Contains, MultiPolygon, Point, Rect};
use h3o::CellIndex;
use rayon::prelude::*;
use std::cell::RefCell;
use std::collections::{BinaryHeap, HashSet};

#[derive(Clone, Debug)]
pub struct Route {
    pub nodes: Vec<u32>,
    pub edges: Vec<u32>,
    pub xy: Vec<[f64; 2]>,
    pub length_m: f64,
}

#[derive(Clone, Debug)]
pub struct Measure {
    pub length_m: f64,
    pub exposure: f64,
    pub cells: Vec<Option<CellIndex>>,
}

/// `np.sum` de numpy sobre float64 contiguo (suma por pares con bloques de 8 acumuladores).
pub fn np_sum(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut r = 0.0;
        for &x in a {
            r += x;
        }
        r
    } else if n <= 128 {
        let mut r = [a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]];
        let mut i = 8;
        while i < n - (n % 8) {
            for j in 0..8 {
                r[j] += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        np_sum(&a[..n2]) + np_sum(&a[n2..])
    }
}

fn seg_len(p: [f64; 2], q: [f64; 2]) -> f64 {
    ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt()
}

/// `shapely.length` de una polilínea: suma secuencial de segmentos.
pub fn polyline_len(c: &[[f64; 2]]) -> f64 {
    let mut s = 0.0;
    for w in c.windows(2) {
        s += seg_len(w[0], w[1]);
    }
    s
}

/// `shapely.line_interpolate_point(line, d)` (GEOS LengthIndexedLine::extractPoint).
pub fn interpolate(c: &[[f64; 2]], d: f64) -> [f64; 2] {
    let mut total = 0.0;
    for w in c.windows(2) {
        let l = seg_len(w[0], w[1]);
        if total + l > d {
            let frac = (d - total) / l;
            return [
                w[0][0] + frac * (w[1][0] - w[0][0]),
                w[0][1] + frac * (w[1][1] - w[0][1]),
            ];
        }
        total += l;
    }
    *c.last().unwrap()
}

struct Recinto {
    shrunk: MultiPolygon<f64>,
    shrunk_bb: Option<Rect<f64>>,
    orig_bb: Option<Rect<f64>>,
}

#[derive(Default)]
struct Scratch {
    dist: Vec<f64>,
    pred: Vec<u32>,
    stamp: Vec<u32>,
    done: Vec<u32>,
    epoch: u32,
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

#[derive(PartialEq)]
struct Item(f64, u32);
impl Eq for Item {}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        // montículo de mínimos; a igual prioridad, el nodo de menor índice primero
        o.0.total_cmp(&self.0).then_with(|| o.1.cmp(&self.1))
    }
}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

pub struct Router {
    pub net: Network,
    pub surf: Surface,
    pub cfg: Settings,
    /// S150 medio de cada arista, normalizado por el p75 de las muestras
    pub rbar: Vec<f64>,
    pub p75: f64,
    /// recinto dominante de cada arista (≥ 50 % de sus muestras dentro del recinto encogido), o −1
    pub edge_poly: Vec<i32>,
    recintos: Vec<Recinto>,
    node_grid: Grid,
    /// Solo con `WACHI_SNAP=arista`: muestras de las aristas (cada ≤ 30 m) y su arista.
    edge_samples: Option<(Grid, Vec<u32>)>,
    /// adyacencia no dirigida: para cada nodo, (arista, vecino); primero las que salen por `u`
    adj_start: Vec<u32>,
    adj: Vec<(u32, u32)>,
}

impl Router {
    pub fn new(net: Network, surf: Surface, cfg: Settings) -> Router {
        let ne = net.n_edges();
        let step = cfg.step_m;
        // muestras cada ~step_m sobre cada arista: n = max(1, round(largo/step)) (redondeo al par)
        let ns: Vec<u32> = net
            .length
            .iter()
            .map(|&l| round_half_even(l / step).max(1.0) as u32)
            .collect();
        let mut first = vec![0usize; ne + 1];
        for e in 0..ne {
            first[e + 1] = first[e] + ns[e] as usize;
        }
        let samples: Vec<[f64; 2]> = (0..ne)
            .into_par_iter()
            .flat_map_iter(|e| {
                let c = net.edge_xy(e);
                let (l, n) = (net.length[e], ns[e]);
                (0..n).map(move |k| interpolate(c, (k as f64 + 0.5) * l / n as f64))
            })
            .collect();
        let vals: Vec<f64> = samples.par_iter().map(|p| surf.s150(p[0], p[1])).collect();
        let p75 = percentile(&vals, 75.0);
        let p75 = if p75 > 0.0 { p75 } else { 1.0 }; // sin exposición en el p75: r̄ = S150 medio
        let rbar: Vec<f64> = (0..ne)
            .map(|e| {
                let mut s = 0.0;
                for v in &vals[first[e]..first[e + 1]] {
                    s += v; // np.bincount acumula en orden
                }
                s / ns[e] as f64 / p75
            })
            .collect();

        let recintos: Vec<Recinto> = net
            .recintos
            .par_iter()
            .map(|r| {
                let shrunk = r.geom.buffer(-cfg.recinto_shrink_m);
                Recinto {
                    shrunk_bb: shrunk.bounding_rect(),
                    shrunk,
                    orig_bb: r.geom.bounding_rect(),
                }
            })
            .collect();
        let mut edge_poly = vec![-1i32; ne];
        if !recintos.is_empty() {
            edge_poly = (0..ne)
                .into_par_iter()
                .map(|e| {
                    let mut counts: Vec<(i32, u32)> = Vec::new();
                    for p in &samples[first[e]..first[e + 1]] {
                        // primer recinto (menor índice) que contiene la muestra
                        let hit = recintos.iter().position(|rc| {
                            rc.shrunk_bb.is_some_and(|b| {
                                p[0] > b.min().x
                                    && p[0] < b.max().x
                                    && p[1] > b.min().y
                                    && p[1] < b.max().y
                            }) && rc.shrunk.contains(&Point::new(p[0], p[1]))
                        });
                        if let Some(i) = hit {
                            match counts.iter_mut().find(|(pi, _)| *pi == i as i32) {
                                Some(c) => c.1 += 1,
                                None => counts.push((i as i32, 1)),
                            }
                        }
                    }
                    match counts
                        .iter()
                        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
                    {
                        Some(&(pi, k)) if k as f64 >= 0.5 * ns[e] as f64 => pi,
                        _ => -1,
                    }
                })
                .collect();
        }

        let nn = net.n_nodes();
        let mut deg = vec![0u32; nn + 1];
        for e in 0..ne {
            deg[net.u[e] as usize + 1] += 1;
            deg[net.v[e] as usize + 1] += 1;
        }
        for i in 1..=nn {
            deg[i] += deg[i - 1];
        }
        let adj_start = deg.clone();
        let mut fill = deg;
        let mut adj = vec![(0u32, 0u32); 2 * ne];
        // como el lexsort de Python: por (u, v), y entre aristas paralelas primero las "hacia adelante"
        // en orden de arista y después las invertidas
        for e in 0..ne {
            let (u, v) = (net.u[e], net.v[e]);
            adj[fill[u as usize] as usize] = (e as u32, v);
            fill[u as usize] += 1;
        }
        for e in 0..ne {
            let (u, v) = (net.u[e], net.v[e]);
            adj[fill[v as usize] as usize] = (e as u32, u);
            fill[v as usize] += 1;
        }
        let node_grid = Grid::new(&net.nxy, 100.0);
        let edge_samples = (cfg.snap == crate::config::Snap::Arista).then(|| {
            let mut owner = vec![0u32; samples.len()];
            for e in 0..ne {
                owner[first[e]..first[e + 1]].fill(e as u32);
            }
            (Grid::new(&samples, 50.0), owner)
        });
        Router {
            net,
            surf,
            cfg,
            rbar,
            p75,
            edge_poly,
            recintos,
            node_grid,
            edge_samples,
            adj_start,
            adj,
        }
    }

    pub fn n_samples_hint(&self) -> usize {
        self.net
            .length
            .iter()
            .map(|&l| round_half_even(l / self.cfg.step_m).max(1.0) as usize)
            .sum()
    }

    #[inline]
    pub fn weight(&self, e: usize, beta: f64) -> f64 {
        let l = self.net.length[e];
        let w = if beta != 0.0 {
            l * (1.0 + beta * self.rbar[e])
        } else {
            l
        };
        w.max(1e-3)
    }

    /// Recintos que contienen a alguno de los nodos (en ellos sí se puede rutear).
    pub fn allowed_polys(&self, nodes: &[u32]) -> HashSet<i32> {
        let mut out = HashSet::new();
        for &n in nodes {
            let [x, y] = self.net.nxy[n as usize];
            for (i, (rc, r)) in self.recintos.iter().zip(&self.net.recintos).enumerate() {
                if rc.orig_bb.is_some_and(|b| {
                    x >= b.min().x && x <= b.max().x && y >= b.min().y && y <= b.max().y
                }) && r.geom.contains(&Point::new(x, y))
                {
                    out.insert(i as i32);
                }
            }
        }
        out
    }

    /// Camino de menor costo; `allowed` son los recintos por los que sí se puede pasar
    /// (`None`: ninguno bloqueado). `None` si no hay camino.
    pub fn route(
        &self,
        src: u32,
        dst: u32,
        beta: f64,
        allowed: Option<&HashSet<i32>>,
    ) -> Option<Route> {
        self.search(src, dst, beta, allowed, true)
    }

    /// Dijkstra puro (sin heurística). Solo para comprobar que A* da el mismo óptimo.
    #[doc(hidden)]
    pub fn route_dijkstra(
        &self,
        src: u32,
        dst: u32,
        beta: f64,
        allowed: Option<&HashSet<i32>>,
    ) -> Option<Route> {
        self.search(src, dst, beta, allowed, false)
    }

    /// Costo de una ruta con el β dado (suma secuencial de los pesos de sus aristas).
    pub fn cost(&self, r: &Route, beta: f64) -> f64 {
        r.edges.iter().map(|&e| self.weight(e as usize, beta)).sum()
    }

    fn search(
        &self,
        src: u32,
        dst: u32,
        beta: f64,
        allowed: Option<&HashSet<i32>>,
        astar: bool,
    ) -> Option<Route> {
        let nxy = &self.net.nxy;
        if src == dst {
            let p = nxy[src as usize];
            return Some(Route {
                nodes: vec![src],
                edges: vec![],
                xy: vec![p, p],
                length_m: 0.0,
            });
        }
        let blocked = |e: usize| {
            let p = self.edge_poly[e];
            p >= 0 && allowed.is_some_and(|a| !a.contains(&p))
        };
        let t = nxy[dst as usize];
        // margen para que el redondeo de la recta nunca supere al largo de la arista
        let h = |n: u32| {
            if !astar {
                return 0.0;
            }
            let p = nxy[n as usize];
            ((p[0] - t[0]).powi(2) + (p[1] - t[1]).powi(2)).sqrt() * (1.0 - 1e-9)
        };
        SCRATCH.with(|s| {
            let mut s = s.borrow_mut();
            let nn = nxy.len();
            if s.stamp.len() != nn {
                *s = Scratch {
                    dist: vec![0.0; nn],
                    pred: vec![0; nn],
                    stamp: vec![0; nn],
                    done: vec![0; nn],
                    epoch: 0,
                };
            }
            s.epoch = s.epoch.wrapping_add(1);
            if s.epoch == 0 {
                s.stamp.fill(0);
                s.done.fill(0);
                s.epoch = 1;
            }
            let g = s.epoch;
            let Scratch {
                dist,
                pred,
                stamp,
                done,
                ..
            } = &mut *s;
            let mut heap = BinaryHeap::new();
            dist[src as usize] = 0.0;
            stamp[src as usize] = g;
            heap.push(Item(h(src), src));
            let mut found = false;
            while let Some(Item(_, n)) = heap.pop() {
                let ni = n as usize;
                if done[ni] == g {
                    continue;
                }
                done[ni] = g;
                if n == dst {
                    found = true;
                    break;
                }
                let dn = dist[ni];
                for &(e, m) in
                    &self.adj[self.adj_start[ni] as usize..self.adj_start[ni + 1] as usize]
                {
                    let mi = m as usize;
                    if done[mi] == g || blocked(e as usize) {
                        continue;
                    }
                    let alt = dn + self.weight(e as usize, beta);
                    if stamp[mi] != g || alt < dist[mi] {
                        stamp[mi] = g;
                        dist[mi] = alt;
                        pred[mi] = e;
                        heap.push(Item(alt + h(m), m));
                    }
                }
            }
            if !found {
                return None;
            }
            let mut nodes = vec![dst];
            let mut edges = Vec::new();
            let mut cur = dst;
            while cur != src {
                let e = pred[cur as usize];
                edges.push(e);
                let (u, v) = (self.net.u[e as usize], self.net.v[e as usize]);
                cur = if v == cur { u } else { v };
                nodes.push(cur);
            }
            nodes.reverse();
            edges.reverse();
            let mut xy: Vec<[f64; 2]> = Vec::new();
            for (k, &e) in edges.iter().enumerate() {
                let c = self.net.edge_xy(e as usize);
                let a = nodes[k];
                let fwd = self.net.u[e as usize] == a;
                let it: Box<dyn Iterator<Item = &[f64; 2]>> = if fwd {
                    Box::new(c.iter())
                } else {
                    Box::new(c.iter().rev())
                };
                xy.extend(it.skip(if k == 0 { 0 } else { 1 }));
            }
            let lens: Vec<f64> = edges.iter().map(|&e| self.net.length[e as usize]).collect();
            Some(Route {
                nodes,
                edges,
                xy,
                length_m: np_sum(&lens),
            })
        })
    }

    /// Exposición: Σ S150·tramo/1000 muestreando cada step_m sobre la ruta (ADR 003).
    pub fn measure(&self, r: &Route) -> Measure {
        let cells: Vec<Option<CellIndex>> =
            r.xy.iter()
                .map(|p| self.surf.cell_utm(p[0], p[1]))
                .collect();
        if r.length_m == 0.0 {
            return Measure {
                length_m: 0.0,
                exposure: 0.0,
                cells,
            };
        }
        let l = polyline_len(&r.xy);
        let step = self.cfg.step_m;
        let n = ((l / step).ceil() as usize).max(1);
        let terms: Vec<f64> = (0..n)
            .map(|i| {
                let start = i as f64 * step;
                let seg = step.min(l - start);
                let p = interpolate(&r.xy, start + seg / 2.0);
                self.surf.s150(p[0], p[1]) * seg / 1000.0
            })
            .collect();
        Measure {
            length_m: l,
            exposure: np_sum(&terms),
            cells,
        }
    }

    pub fn snap(&self, x: f64, y: f64) -> Option<(u32, f64)> {
        self.node_grid.nearest(x, y).map(|(i, d)| (i as u32, d))
    }
}

/// Un punto sobre una arista: `t` metros desde su extremo `u`, sobre la polilínea.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgePoint {
    pub edge: u32,
    pub t: f64,
    pub xy: [f64; 2],
    /// distancia del punto pedido a la arista
    pub dist: f64,
}

/// Punto más cercano de la polilínea `c` a (x, y): (distancia, t desde el inicio, punto).
pub fn project(c: &[[f64; 2]], x: f64, y: f64) -> (f64, f64, [f64; 2]) {
    let mut best = (f64::INFINITY, 0.0, c[0]);
    let mut acc = 0.0;
    for w in c.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        let l = l2.sqrt();
        let f = if l2 > 0.0 {
            (((x - a[0]) * dx + (y - a[1]) * dy) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let p = [a[0] + f * dx, a[1] + f * dy];
        let d = ((p[0] - x).powi(2) + (p[1] - y).powi(2)).sqrt();
        if d < best.0 {
            best = (d, acc + f * l, p);
        }
        acc += l;
    }
    best
}

/// Tramo de la polilínea `c` desde la posición `t` hasta su inicio (`hacia_u`) o su final.
fn partial(c: &[[f64; 2]], t: f64, p: [f64; 2], hacia_u: bool) -> Vec<[f64; 2]> {
    // segmento que contiene t
    let mut acc = 0.0;
    let mut k = c.len() - 2;
    for (i, w) in c.windows(2).enumerate() {
        let l = seg_len(w[0], w[1]);
        if acc + l >= t {
            k = i;
            break;
        }
        acc += l;
    }
    let mut out = vec![p];
    if hacia_u {
        out.extend(c[..=k].iter().rev());
    } else {
        out.extend(c[k + 1..].iter());
    }
    out.dedup();
    out
}

impl Router {
    /// Punto más cercano de la red (exacto). Requiere `WACHI_SNAP=arista`.
    pub fn snap_edge(&self, x: f64, y: f64) -> Option<EdgePoint> {
        let (grid, owner) = self.edge_samples.as_ref()?;
        let (_, d0) = grid.nearest(x, y)?;
        // toda posición de una arista está a ≤ 15 m de alguna de sus muestras (separación ≤ 30 m),
        // así que la arista más cercana tiene una muestra a ≤ d0 + 15
        let mut ids = Vec::new();
        grid.within_ids(x, y, d0 + 15.0 + 1e-6, &mut ids);
        let mut cand: Vec<u32> = ids.iter().map(|&i| owner[i as usize]).collect();
        cand.sort_unstable();
        cand.dedup();
        let mut best: Option<EdgePoint> = None;
        for e in cand {
            let (d, t, p) = project(self.net.edge_xy(e as usize), x, y);
            if best.is_none_or(|b| d < b.dist) {
                best = Some(EdgePoint {
                    edge: e,
                    t,
                    xy: p,
                    dist: d,
                });
            }
        }
        best
    }

    /// Recintos que contienen a alguno de los puntos (UTM).
    pub fn allowed_polys_xy(&self, pts: &[[f64; 2]]) -> HashSet<i32> {
        let mut out = HashSet::new();
        for &[x, y] in pts {
            for (i, (rc, r)) in self.recintos.iter().zip(&self.net.recintos).enumerate() {
                if rc.orig_bb.is_some_and(|b| {
                    x >= b.min().x && x <= b.max().x && y >= b.min().y && y <= b.max().y
                }) && r.geom.contains(&Point::new(x, y))
                {
                    out.insert(i as i32);
                }
            }
        }
        out
    }

    /// Camino de menor costo entre dos puntos sobre aristas. Las aristas de origen y destino se
    /// recorren en parte (costo proporcional a su largo) y nunca se bloquean: la persona ya está
    /// ahí. A* con extremos virtuales; se detiene cuando la mejor llegada no puede mejorar.
    pub fn route_points(
        &self,
        s: &EdgePoint,
        d: &EdgePoint,
        beta: f64,
        allowed: Option<&HashSet<i32>>,
    ) -> Option<Route> {
        let net = &self.net;
        let factor = |e: u32| self.weight(e as usize, beta) / net.length[e as usize];
        let (se, de) = (s.edge as usize, d.edge as usize);
        let (ls, ld) = (net.length[se], net.length[de]);
        if s.edge == d.edge && s.t == d.t {
            return Some(Route {
                nodes: vec![],
                edges: vec![],
                xy: vec![s.xy, d.xy],
                length_m: 0.0,
            });
        }
        let blocked = |e: usize| {
            let p = self.edge_poly[e];
            e != se && e != de && p >= 0 && allowed.is_some_and(|a| !a.contains(&p))
        };
        let tgt = d.xy;
        let h = |n: u32| {
            let p = net.nxy[n as usize];
            ((p[0] - tgt[0]).powi(2) + (p[1] - tgt[1]).powi(2)).sqrt() * (1.0 - 1e-9)
        };
        // llegada directa por la misma arista
        let mut best: (f64, Option<u32>) = if s.edge == d.edge {
            ((s.t - d.t).abs() * factor(s.edge), None)
        } else {
            (f64::INFINITY, None)
        };
        // costo de cada extremo de la arista destino hasta el punto
        let (du, dv) = (net.u[de], net.v[de]);
        let to_t = |n: u32| {
            if n == du {
                d.t * factor(d.edge)
            } else {
                (ld - d.t) * factor(d.edge)
            }
        };
        const SRC: u32 = u32::MAX;
        let found = SCRATCH.with(|sc| {
            let mut sc = sc.borrow_mut();
            let nn = net.nxy.len();
            if sc.stamp.len() != nn {
                *sc = Scratch {
                    dist: vec![0.0; nn],
                    pred: vec![0; nn],
                    stamp: vec![0; nn],
                    done: vec![0; nn],
                    epoch: 0,
                };
            }
            sc.epoch = sc.epoch.wrapping_add(1);
            if sc.epoch == 0 {
                sc.stamp.fill(0);
                sc.done.fill(0);
                sc.epoch = 1;
            }
            let g = sc.epoch;
            let Scratch {
                dist,
                pred,
                stamp,
                done,
                ..
            } = &mut *sc;
            let mut heap = BinaryHeap::new();
            for (n, c) in [
                (net.u[se], s.t * factor(s.edge)),
                (net.v[se], (ls - s.t) * factor(s.edge)),
            ] {
                let ni = n as usize;
                if stamp[ni] != g || c < dist[ni] {
                    stamp[ni] = g;
                    dist[ni] = c;
                    pred[ni] = SRC;
                    heap.push(Item(c + h(n), n));
                }
            }
            while let Some(Item(k, n)) = heap.pop() {
                if k >= best.0 {
                    break;
                }
                let ni = n as usize;
                if done[ni] == g {
                    continue;
                }
                done[ni] = g;
                let dn = dist[ni];
                if n == du || n == dv {
                    let c = dn + to_t(n);
                    if c < best.0 {
                        best = (c, Some(n));
                    }
                }
                for &(e, m) in
                    &self.adj[self.adj_start[ni] as usize..self.adj_start[ni + 1] as usize]
                {
                    let mi = m as usize;
                    if done[mi] == g || blocked(e as usize) {
                        continue;
                    }
                    let alt = dn + self.weight(e as usize, beta);
                    if stamp[mi] != g || alt < dist[mi] {
                        stamp[mi] = g;
                        dist[mi] = alt;
                        pred[mi] = e;
                        heap.push(Item(alt + h(m), m));
                    }
                }
            }
            if !best.0.is_finite() {
                return None;
            }
            // cadena de aristas completas desde el nodo de llegada hasta un extremo del origen
            let mut nodes = Vec::new();
            let mut edges = Vec::new();
            if let Some(end) = best.1 {
                let mut cur = end;
                nodes.push(cur);
                while pred[cur as usize] != SRC {
                    let e = pred[cur as usize];
                    edges.push(e);
                    let (u, v) = (net.u[e as usize], net.v[e as usize]);
                    cur = if v == cur { u } else { v };
                    nodes.push(cur);
                }
                nodes.reverse();
                edges.reverse();
            }
            Some((nodes, edges))
        })?;
        let (nodes, mid) = found;
        let mut xy: Vec<[f64; 2]> = Vec::new();
        let mut edges = vec![s.edge];
        let mut length;
        if nodes.is_empty() {
            // por la misma arista, sin pasar por sus extremos
            let c = net.edge_xy(se);
            let (a, b) = if s.t <= d.t { (s, d) } else { (d, s) };
            let mut seg = vec![a.xy];
            let mut acc = 0.0;
            for w in c.windows(2) {
                acc += seg_len(w[0], w[1]);
                if acc > a.t && acc < b.t {
                    seg.push(w[1]);
                }
            }
            seg.push(b.xy);
            if s.t > d.t {
                seg.reverse();
            }
            seg.dedup();
            xy = seg;
            length = (s.t - d.t).abs();
        } else {
            let first = nodes[0];
            let c = net.edge_xy(se);
            xy.extend(partial(c, s.t, s.xy, first == net.u[se]));
            length = if first == net.u[se] { s.t } else { ls - s.t };
            for (k, &e) in mid.iter().enumerate() {
                let c = net.edge_xy(e as usize);
                let fwd = net.u[e as usize] == nodes[k];
                let it: Box<dyn Iterator<Item = &[f64; 2]>> = if fwd {
                    Box::new(c.iter())
                } else {
                    Box::new(c.iter().rev())
                };
                xy.extend(it.skip(1));
                length += net.length[e as usize];
                edges.push(e);
            }
            let last = *nodes.last().unwrap();
            let mut tail = partial(net.edge_xy(de), d.t, d.xy, last == du);
            tail.reverse(); // del nodo hacia el punto
            xy.extend(tail.into_iter().skip(1));
            length += if last == du { d.t } else { ld - d.t };
            if edges.last() != Some(&d.edge) || edges.len() == 1 {
                edges.push(d.edge);
            }
        }
        xy.dedup();
        if xy.len() == 1 {
            xy.push(xy[0]);
        }
        Some(Route {
            nodes,
            edges,
            xy,
            length_m: length,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suma_por_pares_de_numpy() {
        // valores sacados de numpy 2.x donde np.sum ≠ suma secuencial
        let f =
            |n: usize, g: &dyn Fn(f64) -> f64| (0..n).map(|i| g(i as f64)).collect::<Vec<f64>>();
        assert_eq!(np_sum(&f(37, &|i| 0.1 * i + 1e-3)), 66.637);
        assert_eq!(np_sum(&f(130, &|i| 1.0 / (i + 1.0))), 5.4485913382659765);
        assert_eq!(np_sum(&f(300, &|i| 0.1 * i + 1e-3)), 4485.3);
        assert_eq!(np_sum(&f(1000, &|i| 1.0 / (i + 1.0))), 7.485470860550345);
        assert_eq!(
            np_sum(&f(1000, &|i| (i % 7.0) * 0.3 + 1.0 / (i + 3.0))),
            905.0874678655413
        );
        assert_eq!(np_sum(&[1.0, 2.0, 3.0]), 6.0);
    }

    #[test]
    fn interpolacion_como_geos() {
        let c = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]];
        assert_eq!(interpolate(&c, 5.0), [5.0, 0.0]);
        assert_eq!(interpolate(&c, 15.0), [10.0, 5.0]);
        assert_eq!(interpolate(&c, 25.0), [10.0, 10.0]);
        assert_eq!(polyline_len(&c), 20.0);
    }
}
