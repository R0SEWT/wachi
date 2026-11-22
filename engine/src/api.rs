//! API HTTP: `POST /route` y `GET /health`, mismo contrato que `api/wachi/api.py`.

use crate::config::Settings;
use crate::engine::{Engine, RouteError};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router as AxRouter};
use serde_json::{Value, json};
use std::sync::{Arc, OnceLock};
use tower::ServiceBuilder;
use tower_http::cors::{AllowHeaders, CorsLayer};

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<OnceLock<Arc<Engine>>>,
}

fn err(code: StatusCode, body: Value) -> Response {
    (code, Json(body)).into_response()
}

async fn health(State(st): State<AppState>) -> Response {
    match st.engine.get() {
        None => err(StatusCode::SERVICE_UNAVAILABLE, json!({"status": "iniciando"})),
        Some(e) => Json(json!({
            "status": "ok",
            "nodes": e.router.net.n_nodes(),
            "events": e.router.surf.n_events,
            "data_as_of": e.router.surf.data_as_of,
        }))
        .into_response(),
    }
}

/// Valida el cuerpo como pydantic: objeto con origin/destination, cada uno con lat∈[-90,90] y
/// lon∈[-180,180] finitos. Acepta números y textos numéricos (modo laxo de pydantic); cualquier
/// otra cosa es `None` → 422 sin eco del cuerpo.
pub fn parse_body(body: &[u8]) -> Option<((f64, f64), (f64, f64))> {
    let v: Value = serde_json::from_slice(body).ok()?;
    let num = |x: &Value| -> Option<f64> {
        let f = match x {
            Value::Number(n) => n.as_f64()?,
            Value::String(s) => s.trim().parse::<f64>().ok()?,
            _ => return None,
        };
        f.is_finite().then_some(f)
    };
    let pt = |k: &str| -> Option<(f64, f64)> {
        let o = v.get(k)?.as_object()?;
        let (lat, lon) = (num(o.get("lat")?)?, num(o.get("lon")?)?);
        ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some((lat, lon))
    };
    Some((pt("origin")?, pt("destination")?))
}

async fn route(State(st): State<AppState>, body: Bytes) -> Response {
    let Some(eng) = st.engine.get().cloned() else {
        return err(StatusCode::SERVICE_UNAVAILABLE, json!({"error": "iniciando"}));
    };
    let Some((o, d)) = parse_body(&body) else {
        return err(StatusCode::UNPROCESSABLE_ENTITY, json!({"error": "peticion_invalida"}));
    };
    match tokio::task::spawn_blocking(move || eng.route(o, d)).await {
        Ok(Ok(resp)) => Json(resp).into_response(),
        Ok(Err(e)) => {
            let mut b = json!({"error": e.code()});
            if let Some(w) = e.which() {
                b["which"] = json!(w);
            }
            err(StatusCode::UNPROCESSABLE_ENTITY, b)
        }
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, json!({"error": "interno"})),
    }
}

async fn fallback() -> Response {
    err(StatusCode::NOT_FOUND, json!({"detail": "Not Found"}))
}

pub fn app(st: AppState, cfg: &Settings, max_concurrency: usize) -> AxRouter {
    let origins: Vec<HeaderValue> = cfg.cors_origins.iter().filter_map(|o| o.parse().ok()).collect();
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(AllowHeaders::mirror_request());
    AxRouter::new()
        .route("/health", get(health))
        .route("/route", post(route))
        .fallback(fallback)
        .with_state(st)
        .layer(
            ServiceBuilder::new()
                // más de `max_concurrency` peticiones a la vez reciben 503 en vez de encolarse
                .layer(axum::error_handling::HandleErrorLayer::new(|_: tower::BoxError| async {
                    err(StatusCode::SERVICE_UNAVAILABLE, json!({"error": "ocupado"}))
                }))
                .load_shed()
                .concurrency_limit(max_concurrency),
        )
        .layer(cors)
}

#[allow(dead_code)]
fn _assert_error_codes(e: RouteError) -> &'static str {
    e.code()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuerpos_invalidos() {
        let ok = br#"{"origin":{"lat":-12.08,"lon":-77.03},"destination":{"lat":-12.0,"lon":-77.0}}"#;
        assert!(parse_body(ok).is_some());
        assert!(parse_body(br#"{"origin":{"lat":1}}"#).is_none());
        assert!(parse_body(br#"{no es json"#).is_none());
        assert!(parse_body(br#"{"origin":{"lat":NaN,"lon":-77.03},"destination":{"lat":-12.0,"lon":-77.0}}"#).is_none());
        assert!(parse_body(br#"{"origin":{"lat":91,"lon":-77.03},"destination":{"lat":-12.0,"lon":-77.0}}"#).is_none());
        let s = br#"{"origin":{"lat":"-12.08","lon":-77.03},"destination":{"lat":-12.0,"lon":-77.0}}"#;
        assert_eq!(parse_body(s).unwrap().0, (-12.08, -77.03));
    }
}
