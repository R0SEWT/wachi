//! Regla de selección (ADR 015), presentación (ADR 011, 010) y el motor que los une.

use crate::config::Settings;
use crate::geo_utm::{to_ll, to_utm};
use crate::network::load_or_build;
use crate::num::{round_half_even, round_nd};
use crate::router::{Measure, Route, Router};
use crate::surface::{Surface, load_events};
use anyhow::Result;
use serde::Serialize;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteError {
    FueraDeCobertura,
    SinViaPeatonalCerca(&'static str),
    SinRuta,
}

impl RouteError {
    pub fn code(&self) -> &'static str {
        match self {
            RouteError::FueraDeCobertura => "fuera_de_cobertura",
            RouteError::SinViaPeatonalCerca(_) => "sin_via_peatonal_cerca",
            RouteError::SinRuta => "sin_ruta",
        }
    }
    pub fn which(&self) -> Option<&'static str> {
        match self {
            RouteError::SinViaPeatonalCerca(w) => Some(w),
            _ => None,
        }
    }
}

pub struct Selection<'a> {
    pub fast: &'a Route,
    pub fast_m: &'a Measure,
    pub calm: Option<(&'a Route, &'a Measure)>,
    pub reduction: f64,
    pub detour_m: f64,
}

/// Rápida = más corta; Tranquila = menor exposición dentro del tope de desvío, si reduce ≥ gain_min.
pub fn select<'a>(
    fast: &'a Route,
    fast_m: &'a Measure,
    cands: &'a [(Route, Measure)],
    cfg: &Settings,
) -> Selection<'a> {
    let none = Selection { fast, fast_m, calm: None, reduction: 0.0, detour_m: 0.0 };
    let l0 = fast.length_m;
    let cap = (l0 * (1.0 + cfg.cap_pct)).min(l0 + cfg.cap_min * cfg.walk_m_per_min());
    let ok: Vec<&(Route, Measure)> =
        cands.iter().filter(|(r, _)| r.length_m <= cap + 1e-6 && r.edges != fast.edges).collect();
    if ok.is_empty() || fast_m.exposure <= 0.0 {
        return none;
    }
    // min por (round(exposición, 6), round(largo, 3)); en empate, el primero (como min() de Python)
    let key = |rm: &&(Route, Measure)| (round_nd(rm.1.exposure, 6), round_nd(rm.0.length_m, 3));
    let mut best = ok[0];
    for c in &ok[1..] {
        let (a, b) = (key(c), key(&best));
        if a.0 < b.0 || (a.0 == b.0 && a.1 < b.1) {
            best = c;
        }
    }
    let (calm, calm_m) = (&best.0, &best.1);
    if calm_m.exposure > (1.0 - cfg.gain_min) * fast_m.exposure {
        return none;
    }
    Selection {
        fast,
        fast_m,
        calm: Some((calm, calm_m)),
        reduction: 1.0 - calm_m.exposure / fast_m.exposure,
        detour_m: calm.length_m - l0,
    }
}

#[derive(Serialize, Debug)]
pub struct Card {
    pub geometry: Vec<[f64; 2]>,
    pub distance_m: i64,
    pub duration_s: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detour_m: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detour_s: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exposure_reduction_pct: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks_fewer_near_robberies: Option<i64>,
    pub low_data_segments: Vec<[usize; 2]>,
}

#[derive(Serialize, Debug)]
pub struct Params {
    pub cap_pct: f64,
    pub cap_min: f64,
    pub gain_min: f64,
    pub radius_m: f64,
    pub beta: f64,
}

#[derive(Serialize, Debug)]
pub struct Response {
    pub fast: Card,
    pub calm: Option<Card>,
    pub data_as_of: String,
    pub params: Params,
}

/// Cuadras (aristas) con ≥ block_min_events robos registrados a 150 m, en promedio.
fn blocks_near_robberies(r: &Route, router: &Router) -> i64 {
    r.edges
        .iter()
        .filter(|&&e| router.rbar[e as usize] * router.p75 >= router.cfg.block_min_events)
        .count() as i64
}

fn low_data_segments(m: &Measure, surf: &Surface) -> Vec<[usize; 2]> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in m.cells.iter().enumerate() {
        let low = surf.is_low(*c);
        if low && start.is_none() {
            start = Some(i);
        }
        if !low {
            if let Some(s) = start.take() {
                out.push([s, i - 1]);
            }
        }
    }
    if let Some(s) = start {
        out.push([s, m.cells.len() - 1]);
    }
    out
}

fn geometry(r: &Route) -> Vec<[f64; 2]> {
    r.xy.iter()
        .map(|p| {
            let (lon, lat) = to_ll(p[0], p[1]);
            [round_nd(lat, 6), round_nd(lon, 6)]
        })
        .collect()
}

fn duration_s(length_m: f64, cfg: &Settings) -> i64 {
    round_half_even(length_m / (cfg.walk_kmh * 1000.0 / 3600.0)) as i64
}

pub fn build_response(sel: &Selection, router: &Router) -> Response {
    let cfg = &router.cfg;
    let surf = &router.surf;
    let fast = Card {
        geometry: geometry(sel.fast),
        distance_m: round_half_even(sel.fast.length_m) as i64,
        duration_s: duration_s(sel.fast.length_m, cfg),
        detour_m: None,
        detour_s: None,
        exposure_reduction_pct: None,
        blocks_fewer_near_robberies: None,
        low_data_segments: low_data_segments(sel.fast_m, surf),
    };
    let calm = sel.calm.map(|(r, m)| {
        let fewer = blocks_near_robberies(sel.fast, router) - blocks_near_robberies(r, router);
        Card {
            geometry: geometry(r),
            distance_m: round_half_even(r.length_m) as i64,
            duration_s: duration_s(r.length_m, cfg),
            detour_m: Some(round_half_even(sel.detour_m) as i64),
            detour_s: Some(duration_s(sel.detour_m, cfg)),
            exposure_reduction_pct: Some(round_half_even(100.0 * sel.reduction) as i64),
            blocks_fewer_near_robberies: Some(fewer.max(0)),
            low_data_segments: low_data_segments(m, surf),
        }
    });
    Response {
        fast,
        calm,
        data_as_of: surf.data_as_of.clone(),
        params: Params {
            cap_pct: cfg.cap_pct,
            cap_min: cfg.cap_min,
            gain_min: cfg.gain_min,
            radius_m: cfg.radius_m,
            beta: cfg.betas[0],
        },
    }
}

pub struct Engine {
    pub router: Router,
}

impl Engine {
    fn snap(&self, lat: f64, lon: f64, which: &'static str) -> Result<u32, RouteError> {
        let [s, w, n, e] = self.router.cfg.bbox;
        if !(s <= lat && lat <= n && w <= lon && lon <= e) {
            return Err(RouteError::FueraDeCobertura);
        }
        let (x, y) = to_utm(lon, lat);
        match self.router.snap(x, y) {
            Some((node, d)) if d <= self.router.cfg.snap_max_m => Ok(node),
            _ => Err(RouteError::SinViaPeatonalCerca(which)),
        }
    }

    pub fn route(&self, origin: (f64, f64), destination: (f64, f64)) -> Result<Response, RouteError> {
        let a = self.snap(origin.0, origin.1, "origin")?;
        let b = self.snap(destination.0, destination.1, "destination")?;
        let r = &self.router;
        let allowed = r.allowed_polys(&[a, b]);
        let fast = r.route(a, b, 0.0, Some(&allowed)).ok_or(RouteError::SinRuta)?;
        let cands: Vec<(Route, Measure)> = r
            .cfg
            .betas
            .iter()
            .filter_map(|&beta| r.route(a, b, beta, Some(&allowed)))
            .map(|rt| {
                let m = r.measure(&rt);
                (rt, m)
            })
            .collect();
        let fast_m = r.measure(&fast);
        let sel = select(&fast, &fast_m, &cands, &r.cfg);
        Ok(build_response(&sel, r))
    }
}

pub fn load_engine(cfg: &Settings) -> Result<Engine> {
    let t0 = Instant::now();
    let net = load_or_build(cfg)?;
    tracing::info!("red: {}", serde_json::to_string(&net.stats)?);
    let ev = load_events(&cfg.events_path, &cfg.events_sha256)?;
    let surf = Surface::build(&ev, cfg)?;
    tracing::info!(
        "eventos: {}, data_as_of={}, umbral pocos datos={:.1}",
        surf.n_events, surf.data_as_of, surf.low_threshold
    );
    let router = Router::new(net, surf, cfg.clone());
    tracing::info!("motor listo en {:.1}s (p75 S150 = {:.2})", t0.elapsed().as_secs_f64(), router.p75);
    Ok(Engine { router })
}
