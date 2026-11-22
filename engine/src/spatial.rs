//! Rejilla uniforme de puntos: conteo dentro de un radio y vecino más cercano, exactos.

pub struct Grid {
    x0: f64,
    y0: f64,
    cell: f64,
    nx: usize,
    ny: usize,
    /// inicio de cada celda en `pts` (CSR), len nx·ny+1
    start: Vec<u32>,
    pts: Vec<[f64; 2]>,
    ids: Vec<u32>,
}

impl Grid {
    pub fn new(points: &[[f64; 2]], cell: f64) -> Grid {
        assert!(cell > 0.0);
        if points.is_empty() {
            return Grid {
                x0: 0.0,
                y0: 0.0,
                cell,
                nx: 1,
                ny: 1,
                start: vec![0, 0],
                pts: vec![],
                ids: vec![],
            };
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in points {
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
        let nx = ((x1 - x0) / cell) as usize + 1;
        let ny = ((y1 - y0) / cell) as usize + 1;
        let key = |p: &[f64; 2]| {
            let cx = ((p[0] - x0) / cell) as usize;
            let cy = ((p[1] - y0) / cell) as usize;
            cy.min(ny - 1) * nx + cx.min(nx - 1)
        };
        let mut count = vec![0u32; nx * ny + 1];
        for p in points {
            count[key(p) + 1] += 1;
        }
        for i in 1..count.len() {
            count[i] += count[i - 1];
        }
        let start = count.clone();
        let mut fill = count;
        let mut pts = vec![[0.0; 2]; points.len()];
        let mut ids = vec![0u32; points.len()];
        for (i, p) in points.iter().enumerate() {
            let k = key(p);
            let at = fill[k] as usize;
            pts[at] = *p;
            ids[at] = i as u32;
            fill[k] += 1;
        }
        Grid {
            x0,
            y0,
            cell,
            nx,
            ny,
            start,
            pts,
            ids,
        }
    }

    fn range(&self, lo: f64, hi: f64, origin: f64, n: usize) -> Option<(usize, usize)> {
        let a = ((lo - origin) / self.cell).floor();
        let b = ((hi - origin) / self.cell).floor();
        if b < 0.0 || a > (n - 1) as f64 {
            return None;
        }
        Some((a.max(0.0) as usize, (b as usize).min(n - 1)))
    }

    /// Cantidad de puntos a distancia ≤ r (como `cKDTree.query_ball_point(..., return_length=True)`).
    pub fn count_within(&self, x: f64, y: f64, r: f64) -> u32 {
        let r2 = r * r;
        let (Some((cx0, cx1)), Some((cy0, cy1))) = (
            self.range(x - r, x + r, self.x0, self.nx),
            self.range(y - r, y + r, self.y0, self.ny),
        ) else {
            return 0;
        };
        let mut n = 0;
        for cy in cy0..=cy1 {
            let row = cy * self.nx;
            let (s, e) = (
                self.start[row + cx0] as usize,
                self.start[row + cx1 + 1] as usize,
            );
            for p in &self.pts[s..e] {
                let (dx, dy) = (p[0] - x, p[1] - y);
                if dx * dx + dy * dy <= r2 {
                    n += 1;
                }
            }
        }
        n
    }

    /// Índices originales de los puntos a distancia ≤ r.
    pub fn within_ids(&self, x: f64, y: f64, r: f64, out: &mut Vec<u32>) {
        out.clear();
        let r2 = r * r;
        let (Some((cx0, cx1)), Some((cy0, cy1))) = (
            self.range(x - r, x + r, self.x0, self.nx),
            self.range(y - r, y + r, self.y0, self.ny),
        ) else {
            return;
        };
        for cy in cy0..=cy1 {
            let row = cy * self.nx;
            let (s, e) = (
                self.start[row + cx0] as usize,
                self.start[row + cx1 + 1] as usize,
            );
            for i in s..e {
                let p = self.pts[i];
                if (p[0] - x).powi(2) + (p[1] - y).powi(2) <= r2 {
                    out.push(self.ids[i]);
                }
            }
        }
    }

    /// Índice original y distancia del punto más cercano (empate: el de menor índice).
    pub fn nearest(&self, x: f64, y: f64) -> Option<(usize, f64)> {
        if self.pts.is_empty() {
            return None;
        }
        // celda de la consulta (puede estar fuera de la rejilla) y anillos crecientes
        let qx = ((x - self.x0) / self.cell).floor() as i64;
        let qy = ((y - self.y0) / self.cell).floor() as i64;
        let (nx, ny) = (self.nx as i64, self.ny as i64);
        let mut best: Option<(f64, u32)> = None;
        let max_ring = (nx.max(ny) + qx.abs().max(qy.abs())) + 1;
        for ring in 0..=max_ring {
            // la distancia mínima a cualquier celda del anillo k es ≥ (k−1)·cell
            if let Some((d2, _)) = best {
                let lim = (ring as f64 - 1.0).max(0.0) * self.cell;
                if lim * lim > d2 {
                    break;
                }
            }
            for cy in (qy - ring)..=(qy + ring) {
                if cy < 0 || cy >= ny {
                    continue;
                }
                let edge_row = cy == qy - ring || cy == qy + ring;
                let mut cx = qx - ring;
                while cx <= qx + ring {
                    if cx >= 0 && cx < nx {
                        let k = (cy * nx + cx) as usize;
                        for i in self.start[k] as usize..self.start[k + 1] as usize {
                            let p = self.pts[i];
                            let d2 = (p[0] - x).powi(2) + (p[1] - y).powi(2);
                            let id = self.ids[i];
                            if best.is_none_or(|(bd, bi)| d2 < bd || (d2 == bd && id < bi)) {
                                best = Some((d2, id));
                            }
                        }
                    }
                    cx += if edge_row || ring == 0 { 1 } else { 2 * ring };
                }
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
        }
    }
}
