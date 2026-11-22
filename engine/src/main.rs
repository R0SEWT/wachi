//! `wachi`: motor de rutas Rápida/Tranquila para Lima.
//!
//!   wachi serve [--host 127.0.0.1] [--port 8000] [--max-concurrency 16]
//!   wachi prepare                       # construye la caché del grafo e imprime los sha256
//!   wachi batch PARES.jsonl SALIDA.jsonl  # paridad y benchmark (formato de engine/paridad)
//!
//! Lee la configuración de `WACHI_*` y de `.env` (ver `config.rs`), igual que la API en Python.

use wachi_engine::{api, config, engine, network, num};

use anyhow::{Context, Result, bail};
use config::Settings;
use std::io::Write;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

fn arg<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> Result<T> {
    match args.iter().position(|a| a == name) {
        Some(i) => args
            .get(i + 1)
            .and_then(|v| v.parse().ok())
            .with_context(|| format!("{name} necesita un valor válido")),
        None => Ok(default),
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .with_writer(std::io::stderr) // stdout queda para lo que imprimen prepare y batch
        .init();
    let args: Vec<String> = std::env::args().collect();
    let cfg = Settings::from_env()?;
    match args.get(1).map(String::as_str) {
        Some("serve") => serve(cfg, &args[2..]),
        Some("prepare") => prepare(cfg),
        Some("batch") if args.len() >= 4 => batch(cfg, &args[2], &args[3]),
        _ => {
            eprintln!(
                "uso: wachi serve [--host H] [--port P] [--max-concurrency N] | prepare | batch PARES SALIDA"
            );
            std::process::exit(2);
        }
    }
}

fn serve(cfg: Settings, args: &[String]) -> Result<()> {
    let host: String = arg(args, "--host", "127.0.0.1".to_string())?;
    let port: u16 = arg(args, "--port", 8000)?;
    let maxc: usize = arg(args, "--max-concurrency", 16)?;
    let slot: Arc<OnceLock<Arc<engine::Engine>>> = Arc::new(OnceLock::new());
    // La carga corre aparte y el servidor responde 503 `iniciando` mientras tanto. Si falla, el
    // proceso sale con código 1 y systemd lo vuelve a levantar.
    {
        let slot = slot.clone();
        let cfg = cfg.clone();
        std::thread::Builder::new()
            .name("wachi-load".into())
            .spawn(move || match engine::load_engine(&cfg) {
                Ok(e) => {
                    let _ = slot.set(Arc::new(e));
                }
                Err(e) => {
                    tracing::error!("no se pudo cargar el motor: {e:#}");
                    std::process::exit(1);
                }
            })?;
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        let app = api::app(api::AppState { engine: slot }, &cfg, maxc);
        let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await?;
        tracing::info!("escuchando en http://{host}:{port}");
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                // systemd detiene con SIGTERM; Ctrl+C manda SIGINT
                let mut term =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("no se pudo escuchar SIGTERM");
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
                tracing::info!("apagando: termino las peticiones en curso");
            })
            .await?;
        Ok(())
    })
}

fn prepare(cfg: Settings) -> Result<()> {
    for p in [&cfg.pbf_path, &cfg.events_path] {
        if !p.exists() {
            bail!("falta {}", p.display());
        }
    }
    println!("WACHI_PBF_SHA256={}", network::file_sha256(&cfg.pbf_path)?);
    println!(
        "WACHI_EVENTS_SHA256={}",
        network::file_sha256(&cfg.events_path)?
    );
    let net = network::load_or_build(&cfg)?;
    println!("red: {}", serde_json::to_string(&net.stats)?);
    Ok(())
}

fn rss_max_mb() -> u64 {
    // VmHWM de /proc/self/status (kB)
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
        })
        .map(|kb| kb / 1024)
        .unwrap_or(0)
}

fn batch(cfg: Settings, pares: &str, out: &str) -> Result<()> {
    let t0 = Instant::now();
    let eng = engine::load_engine(&cfg)?;
    let carga = t0.elapsed().as_secs_f64();
    let rss_carga = rss_max_mb();
    let mut f = std::io::BufWriter::new(std::fs::File::create(out)?);
    let mut tiempos = Vec::new();
    for line in std::fs::read_to_string(pares)?
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        let p: serde_json::Value = serde_json::from_str(line)?;
        let g = |k: &str, c: &str| p[k][c].as_f64().context("par mal formado");
        let (o, d) = (
            (g("origin", "lat")?, g("origin", "lon")?),
            (g("destination", "lat")?, g("destination", "lon")?),
        );
        let t = Instant::now();
        let r = eng.route(o, d);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        tiempos.push(ms);
        let mut row = serde_json::json!({"id": p["id"], "ms": (ms * 100.0).round() / 100.0});
        match r {
            Ok(resp) => row["ok"] = serde_json::to_value(resp)?,
            Err(e) => {
                row["error"] = e.code().into();
                row["which"] = e.which().into();
            }
        }
        writeln!(f, "{row}")?;
    }
    f.flush()?;
    let mut t = tiempos.clone();
    t.sort_by(|a, b| a.total_cmp(b));
    let pct = |q: f64| num::percentile_sorted(&t, q);
    println!(
        "{}",
        serde_json::json!({
            "motor": "rust", "carga_s": (carga * 10.0).round() / 10.0, "pares": t.len(),
            "ms_p50": (pct(50.0) * 10.0).round() / 10.0, "ms_p95": (pct(95.0) * 10.0).round() / 10.0,
            "ms_max": (t.last().copied().unwrap_or(0.0) * 10.0).round() / 10.0,
            "rss_carga_mb": rss_carga, "rss_max_mb": rss_max_mb(),
        })
    );
    Ok(())
}
