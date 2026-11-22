//! Parámetros del motor: los mismos nombres y valores por defecto que `api/wachi/config.py`.
//!
//! Se leen de variables `WACHI_*` (sin distinguir mayúsculas) y, si existe, de `.env` en el
//! directorio de trabajo; el entorno manda sobre el archivo, como en pydantic-settings. Las
//! listas y tuplas van en JSON (`WACHI_BBOX='[-12.28,-77.13,-11.87,-76.85]'`).

use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Settings {
    pub pbf_path: PathBuf,
    pub pbf_sha256: String,
    pub events_path: PathBuf,
    pub events_sha256: String,
    pub graph_cache: PathBuf,
    /// S, W, N, E
    pub bbox: [f64; 4],
    pub radius_m: f64,
    pub step_m: f64,
    pub h3_res: u8,
    pub walk_kmh: f64,
    pub betas: Vec<f64>,
    pub cap_pct: f64,
    pub cap_min: f64,
    pub gain_min: f64,
    pub block_min_events: f64,
    pub low_data_pct: f64,
    pub snap_max_m: f64,
    pub recinto_shrink_m: f64,
    pub recinto_tags: Vec<(String, String)>,
    pub cors_origins: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            pbf_path: "data/peru-latest.osm.pbf".into(),
            pbf_sha256: String::new(),
            events_path: "data/eventos.parquet".into(),
            events_sha256: String::new(),
            // Caché propia del motor Rust: no comparte formato con la network.npz de Python.
            graph_cache: "data/network.wachi".into(),
            bbox: [-12.28, -77.13, -11.87, -76.85],
            radius_m: 150.0,
            step_m: 20.0,
            h3_res: 8,
            walk_kmh: 5.0,
            betas: vec![0.5],
            cap_pct: 0.15,
            cap_min: 5.0,
            gain_min: 0.20,
            block_min_events: 1.0,
            low_data_pct: 10.0,
            snap_max_m: 300.0,
            recinto_shrink_m: 5.0,
            recinto_tags: [("amenity", "hospital"), ("amenity", "university"), ("landuse", "cemetery"),
                ("landuse", "military")]
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            cors_origins: vec!["http://localhost:8080".into(), "http://127.0.0.1:8080".into()],
        }
    }
}

impl Settings {
    pub fn walk_m_per_min(&self) -> f64 {
        self.walk_kmh * 1000.0 / 60.0
    }

    /// Entorno + `.env` del directorio actual.
    pub fn from_env() -> Result<Self> {
        let mut vars: HashMap<String, String> = HashMap::new();
        if let Ok(it) = dotenvy::from_path_iter(".env") {
            for kv in it {
                let (k, v) = kv.context("leyendo .env")?;
                vars.insert(k.to_ascii_lowercase(), v);
            }
        }
        for (k, v) in std::env::vars() {
            vars.insert(k.to_ascii_lowercase(), v);
        }
        Self::from_map(&vars)
    }

    pub fn from_map(vars: &HashMap<String, String>) -> Result<Self> {
        let mut s = Settings::default();
        let get = |name: &str| vars.get(&format!("wachi_{name}"));
        fn num<T: std::str::FromStr>(name: &str, v: &str) -> Result<T> {
            v.trim().parse().map_err(|_| anyhow::anyhow!("WACHI_{} inválido: {v:?}", name.to_uppercase()))
        }
        fn json<T: DeserializeOwned>(name: &str, v: &str) -> Result<T> {
            serde_json::from_str(v).with_context(|| format!("WACHI_{} no es JSON válido", name.to_uppercase()))
        }
        if let Some(v) = get("pbf_path") { s.pbf_path = v.into(); }
        if let Some(v) = get("pbf_sha256") { s.pbf_sha256 = v.clone(); }
        if let Some(v) = get("events_path") { s.events_path = v.into(); }
        if let Some(v) = get("events_sha256") { s.events_sha256 = v.clone(); }
        if let Some(v) = get("graph_cache_rs") { s.graph_cache = v.into(); }
        if let Some(v) = get("bbox") { s.bbox = json("bbox", v)?; }
        if let Some(v) = get("radius_m") { s.radius_m = num("radius_m", v)?; }
        if let Some(v) = get("step_m") { s.step_m = num("step_m", v)?; }
        if let Some(v) = get("h3_res") { s.h3_res = num("h3_res", v)?; }
        if let Some(v) = get("walk_kmh") { s.walk_kmh = num("walk_kmh", v)?; }
        if let Some(v) = get("betas") { s.betas = json("betas", v)?; }
        if let Some(v) = get("cap_pct") { s.cap_pct = num("cap_pct", v)?; }
        if let Some(v) = get("cap_min") { s.cap_min = num("cap_min", v)?; }
        if let Some(v) = get("gain_min") { s.gain_min = num("gain_min", v)?; }
        if let Some(v) = get("block_min_events") { s.block_min_events = num("block_min_events", v)?; }
        if let Some(v) = get("low_data_pct") { s.low_data_pct = num("low_data_pct", v)?; }
        if let Some(v) = get("snap_max_m") { s.snap_max_m = num("snap_max_m", v)?; }
        if let Some(v) = get("recinto_shrink_m") { s.recinto_shrink_m = num("recinto_shrink_m", v)?; }
        if let Some(v) = get("recinto_tags") { s.recinto_tags = json("recinto_tags", v)?; }
        if let Some(v) = get("cors_origins") { s.cors_origins = json("cors_origins", v)?; }
        if s.betas.is_empty() {
            bail!("WACHI_BETAS no puede estar vacío: la Tranquila sale del primer β");
        }
        if !(s.step_m > 0.0 && s.radius_m > 0.0) {
            bail!("WACHI_STEP_M y WACHI_RADIUS_M deben ser positivos");
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn por_defecto_igual_que_python() {
        let s = Settings::default();
        assert_eq!(s.bbox, [-12.28, -77.13, -11.87, -76.85]);
        assert_eq!(s.betas, vec![0.5]);
        assert!((s.walk_m_per_min() - 83.333_333_333).abs() < 1e-6);
    }

    #[test]
    fn variables_json_y_mayusculas() {
        let m: HashMap<String, String> = [
            ("wachi_bbox", "[-1, -2, 3, 4]"),
            ("wachi_betas", "[0.25, 1.0]"),
            ("wachi_radius_m", "100"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let s = Settings::from_map(&m).unwrap();
        assert_eq!(s.bbox, [-1.0, -2.0, 3.0, 4.0]);
        assert_eq!(s.betas, vec![0.25, 1.0]);
        assert_eq!(s.radius_m, 100.0);
    }
}
