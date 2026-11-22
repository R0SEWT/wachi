//! Rejilla dispersa de puntos: conteo dentro de un radio y vecino más cercano, exactos.
//!
//! Solo guarda las celdas que tienen puntos (tabla hash), así que la extensión de los datos no
//! importa: un evento con coordenadas basura (0, 0) no infla la memoria, igual que con el
//! `cKDTree` de Python.

use std::collections::HashMap;

pub struct Grid {
    cell: f64,
    /// celda (cx, cy) → rango [inicio, fin) en `pts`
    cells: HashMap<(i64, i64), (u32, u32)>,
    pts: Vec<[f64; 2]>,
    ids: Vec<u32>,
    /// límites de las celdas ocupadas
    bounds: (i64, i64, i64, i64),
}

impl Grid {
    pub fn new(points: &[[f64; 2]], cell: f64) -> Grid {
        assert!(cell > 0.0);
        let key = |p: &[f64; 2]| ((p[0] / cell).floor() as i64, (p[1] / cell).floor() as i64);
        let mut order: Vec<u32> = (0..points.len() as u32).collect();
        order.sort_by_key(|&i| key(&points[i as usize]));
        let mut cells = HashMap::new();
        let mut pts = Vec::with_capacity(points.len());
        let mut ids = Vec::with_capacity(points.len());
        let mut bounds = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
        let mut i = 0;
        while i < order.len() {
            let k = key(&points[order[i] as usize]);
            let start = i;
            while i < order.len() && key(&points[order[i] as usize]) == k {
                pts.push(points[order[i] as usize]);
                ids.push(order[i]);
                i += 1;
            }
            cells.insert(k, (start as u32, i as u32));
            bounds = (
                bounds.0.min(k.0),
                bounds.1.min(k.1),
                bounds.2.max(k.0),
                bounds.3.max(k.1),
            );
        }
        Grid {
            cell,
            cells,
            pts,
            ids,
            bounds,
        }
    }

    fn cell_range(&self, lo: f64, hi: f64) -> (i64, i64) {
        (
            (lo / self.cell).floor() as i64,
            (hi / self.cell).floor() as i64,
        )
    }

    fn for_cells_in(&self, x: f64, y: f64, r: f64, mut f: impl FnMut(usize)) {
        let (cx0, cx1) = self.cell_range(x - r, x + r);
        let (cy0, cy1) = self.cell_range(y - r, y + r);
        let (cx0, cx1) = (cx0.max(self.bounds.0), cx1.min(self.bounds.2));
        let (cy0, cy1) = (cy0.max(self.bounds.1), cy1.min(self.bounds.3));
        for cx in cx0..=cx1 {
            for cy in cy0..=cy1 {
                if let Some(&(s, e)) = self.cells.get(&(cx, cy)) {
                    for i in s as usize..e as usize {
                        f(i);
                    }
                }
            }
        }
    }

    /// Cantidad de puntos a distancia ≤ r (como `cKDTree.query_ball_point(..., return_length=True)`).
    pub fn count_within(&self, x: f64, y: f64, r: f64) -> u32 {
        let r2 = r * r;
        let mut n = 0;
        self.for_cells_in(x, y, r, |i| {
            let p = self.pts[i];
            if (p[0] - x).powi(2) + (p[1] - y).powi(2) <= r2 {
                n += 1;
            }
        });
        n
    }

    /// Índices originales de los puntos a distancia ≤ r.
    pub fn within_ids(&self, x: f64, y: f64, r: f64, out: &mut Vec<u32>) {
        out.clear();
        let r2 = r * r;
        self.for_cells_in(x, y, r, |i| {
            let p = self.pts[i];
            if (p[0] - x).powi(2) + (p[1] - y).powi(2) <= r2 {
                out.push(self.ids[i]);
            }
        });
    }

    /// Índice original y distancia del punto más cercano (empate: el de menor índice).
    pub fn nearest(&self, x: f64, y: f64) -> Option<(usize, f64)> {
        if self.pts.is_empty() {
            return None;
        }
        let qx = (x / self.cell).floor() as i64;
        let qy = (y / self.cell).floor() as i64;
        // anillo más lejano que puede tener celdas ocupadas
        let (bx0, by0, bx1, by1) = self.bounds;
        let max_ring = (qx - bx0)
            .abs()
            .max((bx1 - qx).abs())
            .max((qy - by0).abs())
            .max((by1 - qy).abs());
        let mut best: Option<(f64, u32)> = None;
        let visit = |cx: i64, cy: i64, best: &mut Option<(f64, u32)>| {
            if let Some(&(s, e)) = self.cells.get(&(cx, cy)) {
                for i in s as usize..e as usize {
                    let p = self.pts[i];
                    let d2 = (p[0] - x).powi(2) + (p[1] - y).powi(2);
                    let id = self.ids[i];
                    if best.is_none_or(|(bd, bi)| d2 < bd || (d2 == bd && id < bi)) {
                        *best = Some((d2, id));
                    }
                }
            }
        };
        for ring in 0..=max_ring {
            // toda celda del anillo k está a ≥ (k−1)·cell de la consulta
            if let Some((d2, _)) = best {
                let lim = (ring as f64 - 1.0).max(0.0) * self.cell;
                if lim * lim > d2 {
                    break;
                }
            }
            if ring == 0 {
                visit(qx, qy, &mut best);
                continue;
            }
            for cx in (qx - ring)..=(qx + ring) {
                visit(cx, qy - ring, &mut best);
                visit(cx, qy + ring, &mut best);
            }
            for cy in (qy - ring + 1)..=(qy + ring - 1) {
                visit(qx - ring, cy, &mut best);
                visit(qx + ring, cy, &mut best);
            }
        }
        best.map(|(d2, i)| (i as usize, d2.sqrt()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: &mut u64) -> f64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (*seed >> 11) as f64 / (1u64 << 53) as f64
    }

    #[test]
    fn igual_que_fuerza_bruta() {
        let mut s = 42;
        let pts: Vec<[f64; 2]> = (0..3000)
            .map(|_| [lcg(&mut s) * 5000.0, lcg(&mut s) * 3000.0])
            .collect();
        let g = Grid::new(&pts, 150.0);
        for _ in 0..300 {
            let (x, y) = (lcg(&mut s) * 7000.0 - 1000.0, lcg(&mut s) * 5000.0 - 1000.0);
            let bf = pts
                .iter()
                .filter(|p| (p[0] - x).powi(2) + (p[1] - y).powi(2) <= 150.0f64.powi(2))
                .count();
            assert_eq!(g.count_within(x, y, 150.0) as usize, bf);
            let (bi, bd) = pts
                .iter()
                .enumerate()
                .map(|(i, p)| (i, ((p[0] - x).powi(2) + (p[1] - y).powi(2)).sqrt()))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap();
            let (gi, gd) = g.nearest(x, y).unwrap();
            assert_eq!(gi, bi);
            assert!((gd - bd).abs() < 1e-9);
            let mut ids = Vec::new();
            g.within_ids(x, y, 150.0, &mut ids);
            assert_eq!(ids.len(), bf);
        }
    }

    #[test]
    fn atipicos_lejanos_no_inflan_la_memoria() {
        // Lima (UTM) + un punto en (0, 0) y otro con lat/lon invertidas: miles de km de extensión
        let mut pts: Vec<[f64; 2]> = (0..1000)
            .map(|i| [279_000.0 + i as f64, 8_663_000.0])
            .collect();
        pts.push([0.0, 0.0]);
        pts.push([-5_000_000.0, 12_000_000.0]);
        let g = Grid::new(&pts, 150.0);
        assert_eq!(g.count_within(279_500.0, 8_663_000.0, 150.0), 301);
        assert_eq!(g.count_within(0.0, 0.0, 1.0), 1);
        assert_eq!(g.nearest(279_000.4, 8_663_010.0).unwrap().0, 0);
        assert_eq!(g.nearest(10.0, 10.0).unwrap().0, 1000);
    }
}
