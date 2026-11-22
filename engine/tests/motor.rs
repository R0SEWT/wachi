//! Port de `api/tests/` (conftest + test_graph/router/select/explain/surface/api) al motor Rust.
//! Mismos fixtures: nodos A=(0,0) B=(1000,0) C=(500,200) D=(500,600) m alrededor de (-77.03, -12.08).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};
use tower::ServiceExt;
use wachi_engine::api::{AppState, app};
use wachi_engine::config::Settings;
use wachi_engine::engine::{Engine, RouteError, build_response, select};
use wachi_engine::geo_utm::{to_ll, to_utm};
use wachi_engine::network::{
    CACHE_VERSION, Network, build_network, load_cache, load_or_build_with, save_cache,
};
use wachi_engine::osm::{OsmData, RecintoLL, Way};
use wachi_engine::router::{Measure, Route, Router};
use wachi_engine::surface::{Events, Surface, cell_of, civil_from_days};

const LON0: f64 = -77.03;
const LAT0: f64 = -12.08;

fn ll(dx: f64, dy: f64) -> (f64, f64) {
    let mlon = 111_320.0 * LAT0.to_radians().cos();
    (LON0 + dx / mlon, LAT0 + dy / 110_574.0)
}

fn node(i: i64) -> (f64, f64) {
    match i {
        1 => ll(0.0, 0.0),
        2 => ll(1000.0, 0.0),
        3 => ll(500.0, 200.0),
        4 => ll(500.0, 600.0),
        _ => unreachable!(),
    }
}

fn way(ids: &[i64], hw: &str) -> Way {
    Way {
        ids: ids.to_vec(),
        lonlat: ids.iter().map(|&i| node(i)).collect(),
        highway: hw.into(),
    }
}

fn ways() -> Vec<Way> {
    vec![
        way(&[1, 2], "residential"),
        way(&[1, 3], "residential"),
        way(&[3, 2], "residential"),
        way(&[3, 4], "footway"),
    ]
}

fn cfg(dir: &std::path::Path) -> Settings {
    let (w, s) = ll(-1500.0, -1500.0);
    let (e, n) = ll(2500.0, 2500.0);
    Settings {
        bbox: [s, w, n, e],
        graph_cache: dir.join("net.wachi"),
        events_path: dir.join("ev.parquet"),
        ..Settings::default()
    }
}

/// Rectángulo UTM x∈[200,800], y∈[50,350] alrededor de C: cubre ≥ 50 % de A–C y C–B.
fn recinto() -> RecintoLL {
    let (x0, y0) = to_utm(LON0, LAT0);
    let c = |dx: f64, dy: f64| {
        let (lo, la) = to_ll(x0 + dx, y0 + dy);
        geo::Coord { x: lo, y: la }
    };
    let ring = vec![
        c(200.0, 50.0),
        c(800.0, 50.0),
        c(800.0, 350.0),
        c(200.0, 350.0),
        c(200.0, 50.0),
    ];
    RecintoLL {
        kind: "amenity=hospital".into(),
        name: "Hospital Test".into(),
        geom: geo::MultiPolygon(vec![geo::Polygon::new(geo::LineString(ring), vec![])]),
    }
}

fn net() -> Network {
    build_network(OsmData {
        ways: ways(),
        recintos: vec![],
        recintos_descartados: 0,
    })
}

fn net_recinto() -> Network {
    build_network(OsmData {
        ways: ways(),
        recintos: vec![recinto()],
        recintos_descartados: 0,
    })
}

fn day(y: i64, m: i64, d: i64) -> i64 {
    (0..30000)
        .map(|z| z - 5000)
        .find(|&z| civil_from_days(z) == (y, m, d))
        .unwrap()
}

fn events() -> Events {
    let (lon, lat) = ll(500.0, 0.0);
    let mut day_v = vec![day(2024, 3, 1); 9];
    day_v.push(day(2024, 4, 30));
    Events {
        lat: vec![lat; 10],
        lon: vec![lon; 10],
        day: day_v,
    }
}

fn router(n: Network, c: &Settings) -> Router {
    let s = Surface::build(&events(), c).unwrap();
    Router::new(n, s, c.clone())
}

fn snap(r: &Router, p: (f64, f64)) -> u32 {
    let (x, y) = to_utm(p.0, p.1);
    r.snap(x, y).unwrap().0
}

fn tmp() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "wachi-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ---------------- test_graph ----------------

#[test]
fn red_forma() {
    let n = net();
    assert_eq!((n.n_nodes(), n.n_edges()), (4, 4));
    let mut l: Vec<f64> = n.length.iter().map(|x| x.round()).collect();
    l.sort_by(|a, b| a.total_cmp(b));
    assert!((l[3] - 1000.0).abs() < 3.0);
    assert_eq!(l.iter().filter(|x| (*x - 538.5).abs() < 3.0).count(), 2);
    assert!((l[0] - 400.0).abs() < 3.0);
    assert_eq!(*n.off.last().unwrap() as usize, n.xy.len());
}

#[test]
fn red_componente_mayor() {
    let mut w = ways();
    w.push(Way {
        ids: vec![90, 91],
        lonlat: vec![(-77.1, -12.0), (-77.1001, -12.0001)],
        highway: "path".into(),
    });
    let n = build_network(OsmData {
        ways: w,
        recintos: vec![],
        recintos_descartados: 0,
    });
    assert_eq!(n.n_nodes(), 4);
}

#[test]
fn cache_ida_y_vuelta() {
    let d = tmp();
    let n = net_recinto();
    let p = d.join("rt.wachi");
    save_cache(&n, &p).unwrap();
    let b = load_cache(&p).unwrap();
    assert_eq!(
        (b.u.clone(), b.v.clone(), b.off.clone(), b.hw.clone()),
        (n.u.clone(), n.v.clone(), n.off.clone(), n.hw.clone())
    );
    assert_eq!(b.xy, n.xy);
    assert_eq!(b.nxy, n.nxy);
    assert_eq!(b.length, n.length);
    assert_eq!(b.recintos[0].kind, "amenity=hospital");
    assert_eq!(b.recintos[0].geom, n.recintos[0].geom);
}

#[test]
fn arista_ab_y_su_reverso() {
    let n = net();
    let e = (0..n.n_edges())
        .max_by(|&a, &b| n.length[a].total_cmp(&n.length[b]))
        .unwrap();
    let pts = n.edge_xy(e);
    assert_eq!(pts.len(), 2);
    let a = to_utm(ll(0.0, 0.0).0, ll(0.0, 0.0).1);
    let b = to_utm(ll(1000.0, 0.0).0, ll(1000.0, 0.0).1);
    let close = |p: [f64; 2], q: (f64, f64)| (p[0] - q.0).abs() < 0.01 && (p[1] - q.1).abs() < 0.01;
    assert!((close(pts[0], a) && close(pts[1], b)) || (close(pts[0], b) && close(pts[1], a)));
}

fn fake_reader(
    calls: &std::cell::Cell<usize>,
) -> impl Fn(&Settings) -> anyhow::Result<OsmData> + '_ {
    move |_| {
        calls.set(calls.get() + 1);
        Ok(OsmData {
            ways: ways(),
            recintos: vec![],
            recintos_descartados: 0,
        })
    }
}

fn sha_hex(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(b))
}

#[test]
fn load_or_build_guarda_sha_y_version() {
    let d = tmp().join("sha");
    std::fs::create_dir_all(&d).unwrap();
    let mut c = cfg(&d);
    c.pbf_path = d.join("lima.osm.pbf");
    std::fs::write(&c.pbf_path, b"pbf v1").unwrap();
    let calls = std::cell::Cell::new(0);
    let n = load_or_build_with(&c, fake_reader(&calls)).unwrap();
    assert_eq!(n.stats.pbf_sha256, sha_hex(b"pbf v1"));
    assert_eq!(n.stats.cache_version, CACHE_VERSION);
    load_or_build_with(&c, fake_reader(&calls)).unwrap();
    assert_eq!(calls.get(), 1, "la segunda vez sale de la caché");
    // pbf nuevo con su sha fijado: reconstruye
    std::fs::write(&c.pbf_path, b"pbf v2").unwrap();
    c.pbf_sha256 = sha_hex(b"pbf v2");
    let n = load_or_build_with(&c, fake_reader(&calls)).unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(n.stats.pbf_sha256, c.pbf_sha256);
    assert_eq!(
        load_cache(&c.graph_cache).unwrap().stats.pbf_sha256,
        c.pbf_sha256
    );
    // sha fijado que no coincide: se niega
    c.pbf_sha256 = "0".repeat(64);
    std::fs::remove_file(&c.graph_cache).unwrap();
    let e = load_or_build_with(&c, fake_reader(&calls)).err().unwrap();
    assert!(e.to_string().contains("sha256"));
}

#[test]
fn caché_corrupta_se_reconstruye() {
    let d = tmp().join("corrupta");
    std::fs::create_dir_all(&d).unwrap();
    let mut c = cfg(&d);
    c.pbf_path = d.join("lima.osm.pbf");
    std::fs::write(&c.pbf_path, b"pbf v1").unwrap();
    std::fs::write(&c.graph_cache, b"no es una cache").unwrap();
    let calls = std::cell::Cell::new(0);
    load_or_build_with(&c, fake_reader(&calls)).unwrap();
    assert_eq!(calls.get(), 1);
}

// ---------------- test_router ----------------

#[test]
fn snap_al_nodo() {
    let c = cfg(&tmp());
    let r = router(net(), &c);
    let p = ll(10.0, 5.0);
    let (x, y) = to_utm(p.0, p.1);
    let (node, d) = r.snap(x, y).unwrap();
    assert!(d < 15.0);
    let (lon, lat) = to_ll(r.net.nxy[node as usize][0], r.net.nxy[node as usize][1]);
    assert!((lat - LAT0).abs() < 1e-5 && (lon - LON0).abs() < 1e-5);
}

#[test]
fn beta0_toma_la_mas_corta_y_beta_evita_eventos() {
    let c = cfg(&tmp());
    let r = router(net(), &c);
    let (a, b) = (snap(&r, ll(0.0, 0.0)), snap(&r, ll(1000.0, 0.0)));
    let fast = r.route(a, b, 0.0, None).unwrap();
    assert!(fast.edges.len() == 1 && (fast.length_m - 1000.0).abs() < 5.0);
    let calm = r.route(a, b, 0.5, None).unwrap();
    assert!(calm.edges.len() == 2 && (calm.length_m - 1077.0).abs() < 5.0);
    assert_eq!(r.measure(&calm).exposure, 0.0);
    let m = r.measure(&fast);
    // 10 eventos en x=500 cuentan en las muestras con x∈[350,650]: ~300 m → 10 · 0.3 km = 3
    assert!(m.exposure > 2.6 && m.exposure < 3.4, "{}", m.exposure);
    assert_eq!(m.cells.len(), fast.xy.len());
}

#[test]
fn recinto_bloquea_el_atajo() {
    let c = cfg(&tmp());
    let r = router(net_recinto(), &c);
    let (a, b, cc) = (
        snap(&r, ll(0.0, 0.0)),
        snap(&r, ll(1000.0, 0.0)),
        snap(&r, ll(500.0, 200.0)),
    );
    let polys: HashSet<i32> = r.edge_poly.iter().copied().filter(|&p| p >= 0).collect();
    assert_eq!(polys, HashSet::from([0]));
    let allowed = r.allowed_polys(&[a, b]);
    assert_eq!(
        r.route(a, b, 0.5, Some(&allowed)).unwrap().edges.len(),
        1,
        "cae a la recta"
    );
    assert_eq!(
        r.allowed_polys(&[cc]),
        HashSet::from([0]),
        "origen dentro: permitido"
    );
    let allowed = r.allowed_polys(&[cc, b]);
    assert_eq!(r.route(cc, b, 0.0, Some(&allowed)).unwrap().edges.len(), 1);
}

#[test]
fn sin_camino_cuando_no_hay_paso() {
    let c = cfg(&tmp());
    let r = router(net_recinto(), &c);
    let (a, cc) = (snap(&r, ll(0.0, 0.0)), snap(&r, ll(500.0, 200.0)));
    assert_eq!(r.route(a, a, 0.0, None).unwrap().length_m, 0.0);
    assert!(r.route(a, cc, 0.0, Some(&HashSet::from([0]))).is_some());
    // con el recinto 0 bloqueado, C queda aislado (A–C y C–B caen dentro)
    assert!(r.route(a, cc, 0.0, Some(&HashSet::new())).is_none());
}

// ---------------- test_select ----------------

fn rr(length: f64, edges: &[u32]) -> Route {
    Route {
        nodes: vec![0, 1],
        edges: edges.to_vec(),
        xy: vec![[0.0; 2]; 2],
        length_m: length,
    }
}

fn mm(length: f64, exposure: f64) -> Measure {
    Measure {
        length_m: length,
        exposure,
        cells: vec![None, None],
    }
}

#[test]
fn seleccion() {
    let c = Settings::default();
    let (f, fm) = (rr(1000.0, &[0]), mm(1000.0, 3.0));
    let cands = [(rr(1077.0, &[1, 2]), mm(1077.0, 0.0))];
    let s = select(&f, &fm, &cands, &c);
    assert!(s.calm.is_some() && s.reduction == 1.0 && (s.detour_m - 77.0).abs() < 1e-9);
    // tope = min(1000·1.15, 1000 + 5·83.3) = 1150
    let cands = [(rr(1200.0, &[1, 2]), mm(1200.0, 0.0))];
    let s = select(&f, &fm, &cands, &c);
    assert!(s.calm.is_none() && s.reduction == 0.0);
    let cands = [(rr(1050.0, &[1]), mm(1050.0, 2.5))];
    assert!(
        select(&f, &fm, &cands, &c).calm.is_none(),
        "no alcanza gain_min"
    );
    let cands = [(rr(1000.0, &[0]), mm(1000.0, 3.0))];
    assert!(select(&f, &fm, &cands, &c).calm.is_none(), "mismas aristas");
    let fm0 = mm(1000.0, 0.0);
    let cands = [(rr(1050.0, &[1]), mm(1050.0, 0.0))];
    assert!(
        select(&f, &fm0, &cands, &c).calm.is_none(),
        "rápida sin exposición"
    );
    let cands = [
        (rr(1100.0, &[1]), mm(1100.0, 1.0)),
        (rr(1120.0, &[2]), mm(1120.0, 0.5)),
    ];
    assert_eq!(select(&f, &fm, &cands, &c).calm.unwrap().0.edges, vec![2]);
}

// ---------------- test_explain ----------------

#[test]
fn respuesta_y_cuadras() {
    let c = cfg(&tmp());
    let r = router(net(), &c);
    let (a, b) = (snap(&r, ll(0.0, 0.0)), snap(&r, ll(1000.0, 0.0)));
    let fast = r.route(a, b, 0.0, None).unwrap();
    let calm = r.route(a, b, 0.5, None).unwrap();
    let (fm, cm) = (r.measure(&fast), r.measure(&calm));
    let cands = [(calm.clone(), cm)];
    let sel = select(&fast, &fm, &cands, &c);
    let out = serde_json::to_value(build_response(&sel, &r)).unwrap();
    assert_eq!(out["fast"]["distance_m"], 1000);
    assert_eq!(out["fast"]["duration_s"], 720);
    assert_eq!(out["calm"]["detour_m"], 77);
    assert_eq!(out["calm"]["exposure_reduction_pct"], 100);
    assert_eq!(out["calm"]["blocks_fewer_near_robberies"], 1);
    assert_eq!(out["data_as_of"], "2024-04-30");
    assert_eq!(out["params"]["beta"], 0.5);
    let n = out["fast"]["geometry"].as_array().unwrap().len();
    for s in out["fast"]["low_data_segments"].as_array().unwrap() {
        let (i, j) = (
            s[0].as_u64().unwrap() as usize,
            s[1].as_u64().unwrap() as usize,
        );
        assert!(i <= j && j < n);
    }
    // orden de claves igual que Python (se mira el texto: Value ordena las claves)
    let txt = serde_json::to_string(&build_response(&sel, &r)).unwrap();
    let calm_txt = &txt[txt.find("\"calm\"").unwrap()..];
    let pos: Vec<usize> = [
        "geometry",
        "distance_m",
        "duration_s",
        "detour_m",
        "detour_s",
        "exposure_reduction_pct",
        "blocks_fewer_near_robberies",
        "low_data_segments",
        "data_as_of",
        "params",
    ]
    .iter()
    .map(|k| calm_txt.find(&format!("\"{k}\"")).unwrap())
    .collect();
    assert!(pos.windows(2).all(|w| w[0] < w[1]), "{calm_txt}");
    assert!(txt.starts_with("{\"fast\":{\"geometry\""));
    let sel2 = select(&fast, &fm, &[], &c);
    assert!(serde_json::to_value(build_response(&sel2, &r)).unwrap()["calm"].is_null());
}

// ---------------- test_surface ----------------

#[test]
fn superficie_cuenta_a_150m_y_cobertura() {
    let c = cfg(&tmp());
    let s = Surface::build(&events(), &c).unwrap();
    let p0 = ll(500.0, 0.0);
    let (x0, y0) = to_utm(p0.0, p0.1);
    let p1 = ll(500.0, 200.0);
    let (x1, y1) = to_utm(p1.0, p1.1);
    assert_eq!(
        [s.s150(x0, y0), s.s150(x0 + 140.0, y0), s.s150(x1, y1)],
        [10.0, 10.0, 0.0]
    );
    assert_eq!((s.n_events, s.data_as_of.as_str()), (10, "2024-04-30"));
    let res = h3o::Resolution::Eight;
    let hot = cell_of(p0.1, p0.0, res);
    let cold = cell_of(ll(500.0, 2000.0).1, ll(500.0, 2000.0).0, res);
    assert!(!s.is_low(hot) && s.is_low(cold));
}

// ---------------- test_api ----------------

fn engine() -> Arc<Engine> {
    let c = cfg(&tmp());
    Arc::new(Engine {
        router: router(net(), &c),
    })
}

async fn call(
    state: AppState,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> (StatusCode, Value) {
    let c = cfg(&tmp());
    let a = app(state, &c, 16);
    let mut req = Request::builder().method(method).uri(path);
    if body.is_some() {
        req = req.header("content-type", "application/json");
    }
    let resp = a
        .oneshot(
            req.body(Body::from(body.unwrap_or("").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let st = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (st, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

fn ready() -> AppState {
    let slot = Arc::new(OnceLock::new());
    let _ = slot.set(engine());
    AppState { engine: slot }
}

fn body(o: (f64, f64), d: (f64, f64)) -> String {
    json!({"origin": {"lat": o.1, "lon": o.0}, "destination": {"lat": d.1, "lon": d.0}}).to_string()
}

#[tokio::test]
async fn api_health_y_dos_tarjetas() {
    let (s, j) = call(ready(), "GET", "/health", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (j["nodes"].as_u64(), j["events"].as_u64()),
        (Some(4), Some(10))
    );
    let (s, j) = call(
        ready(),
        "POST",
        "/route",
        Some(&body(ll(0.0, 0.0), ll(1000.0, 0.0))),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (
            j["calm"]["exposure_reduction_pct"].as_i64(),
            j["calm"]["detour_m"].as_i64()
        ),
        (Some(100), Some(77))
    );
    assert_ne!(j["fast"]["geometry"][0], j["fast"]["geometry"][1]);
    let keys: HashSet<&str> = j.as_object().unwrap().keys().map(|k| k.as_str()).collect();
    assert_eq!(
        keys,
        HashSet::from(["fast", "calm", "data_as_of", "params"])
    );
}

#[tokio::test]
async fn api_mismo_nodo_y_errores() {
    let (s, j) = call(
        ready(),
        "POST",
        "/route",
        Some(&body(ll(0.0, 0.0), ll(5.0, 5.0))),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(j["fast"]["distance_m"], 0);
    assert!(j["calm"].is_null());
    assert_eq!(j["fast"]["geometry"][0], j["fast"]["geometry"][1]);
    let (s, j) = call(
        ready(),
        "POST",
        "/route",
        Some(&body((-76.0, -12.0), ll(1000.0, 0.0))),
    )
    .await;
    assert_eq!(
        (s, j),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({"error": "fuera_de_cobertura"})
        )
    );
    let (s, j) = call(
        ready(),
        "POST",
        "/route",
        Some(&body(ll(0.0, 0.0), ll(2000.0, 2000.0))),
    )
    .await;
    assert_eq!(
        (s, j),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({"error": "sin_via_peatonal_cerca", "which": "destination"})
        )
    );
    for raw in [
        r#"{"origin": {"lat": NaN, "lon": -77.03}, "destination": {"lat": -12.08, "lon": -77.03}}"#,
        r#"{"origin": {"lat": -12.08, "lon": Infinity}, "destination": {"lat": -12.08, "lon": -77.03}}"#,
        r#"{"origin": {"lat": 1}}"#,
        "{no es json",
    ] {
        let (s, j) = call(ready(), "POST", "/route", Some(raw)).await;
        assert_eq!(
            (s, j),
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                json!({"error": "peticion_invalida"})
            )
        );
    }
}

#[tokio::test]
async fn api_503_mientras_carga_y_sin_docs() {
    let loading = AppState {
        engine: Arc::new(OnceLock::new()),
    };
    let (s, j) = call(loading.clone(), "GET", "/health", None).await;
    assert_eq!(
        (s, j),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"status": "iniciando"})
        )
    );
    let (s, j) = call(
        loading,
        "POST",
        "/route",
        Some(&body(ll(0.0, 0.0), ll(1000.0, 0.0))),
    )
    .await;
    assert_eq!(
        (s, j),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error": "iniciando"})
        )
    );
    for p in ["/docs", "/redoc", "/openapi.json"] {
        assert_eq!(call(ready(), "GET", p, None).await.0, StatusCode::NOT_FOUND);
    }
}

#[test]
fn engine_errores_de_ruta() {
    let e = engine();
    assert_eq!(
        e.route((-12.0, -76.0), (LAT0, LON0)).err(),
        Some(RouteError::FueraDeCobertura)
    );
    let far = ll(2000.0, 2000.0);
    assert_eq!(
        e.route((LAT0, LON0), (far.1, far.0)).err(),
        Some(RouteError::SinViaPeatonalCerca("destination"))
    );
}

// ---------------- propiedad: A* = Dijkstra ----------------

fn lcg(s: &mut u64) -> f64 {
    *s = s
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*s >> 11) as f64 / (1u64 << 53) as f64
}

/// Grilla 40×40 (~60 m) con nodos movidos, diagonales al azar, aristas curvas (un vértice
/// intermedio desplazado), eventos al azar y un recinto. Para 400 pares y tres β, A* y Dijkstra
/// deben dar el mismo costo óptimo (el camino puede variar solo si hay empates exactos).
#[test]
fn astar_da_el_mismo_optimo_que_dijkstra() {
    let mut s = 12345u64;
    let n = 40i64;
    let id = |i: i64, j: i64| 1000 + i * n + j;
    let mut pos = std::collections::HashMap::new();
    for i in 0..n {
        for j in 0..n {
            pos.insert(
                id(i, j),
                ll(
                    i as f64 * 60.0 + (lcg(&mut s) - 0.5) * 30.0,
                    j as f64 * 60.0 + (lcg(&mut s) - 0.5) * 30.0,
                ),
            );
        }
    }
    let mut ws = Vec::new();
    let mut extra = 10_000_000i64;
    let mut edge = |a: i64, b: i64, s: &mut u64| {
        let (pa, pb) = (pos[&a], pos[&b]);
        let mut ids = vec![a];
        let mut lonlat = vec![pa];
        if lcg(s) < 0.4 {
            // vértice intermedio desplazado: la arista mide más que la recta
            extra += 1;
            ids.push(extra);
            lonlat.push((
                (pa.0 + pb.0) / 2.0 + (lcg(s) - 0.5) * 2e-4,
                (pa.1 + pb.1) / 2.0 + (lcg(s) - 0.5) * 2e-4,
            ));
        }
        ids.push(b);
        lonlat.push(pb);
        Way {
            ids,
            lonlat,
            highway: "residential".into(),
        }
    };
    for i in 0..n {
        for j in 0..n {
            if i + 1 < n && lcg(&mut s) < 0.9 {
                ws.push(edge(id(i, j), id(i + 1, j), &mut s));
            }
            if j + 1 < n && lcg(&mut s) < 0.9 {
                ws.push(edge(id(i, j), id(i, j + 1), &mut s));
            }
            if i + 1 < n && j + 1 < n && lcg(&mut s) < 0.15 {
                ws.push(edge(id(i, j), id(i + 1, j + 1), &mut s));
            }
        }
    }
    let (x0, y0) = to_utm(LON0, LAT0);
    let c = |dx: f64, dy: f64| {
        let (lo, la) = to_ll(x0 + dx, y0 + dy);
        geo::Coord { x: lo, y: la }
    };
    let rec = RecintoLL {
        kind: "amenity=hospital".into(),
        name: "Recinto".into(),
        geom: geo::MultiPolygon(vec![geo::Polygon::new(
            geo::LineString(vec![
                c(700.0, 700.0),
                c(1300.0, 700.0),
                c(1300.0, 1100.0),
                c(700.0, 1100.0),
                c(700.0, 700.0),
            ]),
            vec![],
        )]),
    };
    let net = build_network(OsmData {
        ways: ws,
        recintos: vec![rec],
        recintos_descartados: 0,
    });
    let k = 3000;
    let mut ev = Events {
        lat: vec![],
        lon: vec![],
        day: vec![],
    };
    for _ in 0..k {
        // la mitad en tres focos, el resto disperso
        let (dx, dy) = if lcg(&mut s) < 0.5 {
            let f = [(500.0, 500.0), (1500.0, 900.0), (900.0, 1800.0)]
                [(lcg(&mut s) * 3.0) as usize % 3];
            (
                f.0 + (lcg(&mut s) - 0.5) * 300.0,
                f.1 + (lcg(&mut s) - 0.5) * 300.0,
            )
        } else {
            (lcg(&mut s) * 2400.0, lcg(&mut s) * 2400.0)
        };
        let (lo, la) = ll(dx, dy);
        ev.lat.push(la);
        ev.lon.push(lo);
        ev.day.push(19_000);
    }
    let cf = cfg(&tmp());
    let r = Router::new(net, Surface::build(&ev, &cf).unwrap(), cf.clone());
    assert!(r.edge_poly.contains(&0), "el recinto debe cubrir aristas");
    let nn = r.net.n_nodes() as f64;
    let mut iguales = 0;
    for q in 0..400 {
        let a = (lcg(&mut s) * nn) as u32;
        let b = (lcg(&mut s) * nn) as u32;
        let allowed = if q % 2 == 0 {
            Some(r.allowed_polys(&[a, b]))
        } else {
            None
        };
        for beta in [0.0, 0.5, 2.0] {
            let x = r.route(a, b, beta, allowed.as_ref());
            let y = r.route_dijkstra(a, b, beta, allowed.as_ref());
            match (x, y) {
                (None, None) => {}
                (Some(x), Some(y)) => {
                    let (cx, cy) = (r.cost(&x, beta), r.cost(&y, beta));
                    assert!(
                        (cx - cy).abs() <= 1e-9 * cy.max(1.0),
                        "par {a}→{b} β={beta}: A* {cx} vs Dijkstra {cy}"
                    );
                    iguales += (x.edges == y.edges) as usize;
                }
                (x, y) => panic!(
                    "par {a}→{b} β={beta}: A* {:?} vs Dijkstra {:?}",
                    x.is_some(),
                    y.is_some()
                ),
            }
        }
    }
    assert!(
        iguales >= 1150,
        "mismo camino en {iguales} de 1200 búsquedas"
    );
}
