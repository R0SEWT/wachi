//! Superficie de robos registrados a ≤ 150 m (ADR 016) y cobertura por celda H3 (ADR 010).

use crate::config::Settings;
use crate::geo_utm::{to_ll, to_utm};
use crate::num::percentile;
use crate::spatial::Grid;
use anyhow::{Context, Result, bail};
use arrow::array::{Array, AsArray};
use arrow::datatypes::{DataType, Float32Type, Float64Type, TimeUnit};
use h3o::{CellIndex, LatLng, Resolution};
use parquet::arrow::ProjectionMask;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

pub struct Events {
    pub lat: Vec<f64>,
    pub lon: Vec<f64>,
    /// días desde 1970-01-01 (fecha local, sin zona)
    pub day: Vec<i64>,
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    // Howard Hinnant, days_from_civil
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn parse_day(s: &str) -> Option<i64> {
    let s = s.trim();
    let (y, m, d) = (
        s.get(0..4)?.parse().ok()?,
        s.get(5..7)?.parse().ok()?,
        s.get(8..10)?.parse().ok()?,
    );
    Some(days_from_civil(y, m, d))
}

/// Día civil de cada fila de la columna `fecha` (timestamp, date o texto ISO).
fn column_days(fe: &dyn Array) -> Result<Vec<Option<i64>>> {
    let n = fe.len();
    let per_day = |unit: &TimeUnit| -> i64 {
        match unit {
            TimeUnit::Second => 86_400,
            TimeUnit::Millisecond => 86_400_000,
            TimeUnit::Microsecond => 86_400_000_000,
            TimeUnit::Nanosecond => 86_400_000_000_000,
        }
    };
    Ok(match fe.data_type() {
        DataType::Timestamp(unit, _) => {
            let k = per_day(unit);
            let raw = arrow::compute::cast(fe, &DataType::Int64)?;
            let raw = raw.as_primitive::<arrow::datatypes::Int64Type>();
            (0..n)
                .map(|i| (!raw.is_null(i)).then(|| raw.value(i).div_euclid(k)))
                .collect()
        }
        DataType::Date32 => {
            let a = fe.as_primitive::<arrow::datatypes::Date32Type>();
            (0..n)
                .map(|i| (!a.is_null(i)).then(|| a.value(i) as i64))
                .collect()
        }
        DataType::Date64 => {
            let a = fe.as_primitive::<arrow::datatypes::Date64Type>();
            (0..n)
                .map(|i| (!a.is_null(i)).then(|| a.value(i).div_euclid(86_400_000)))
                .collect()
        }
        DataType::Utf8 => {
            let a = fe.as_string::<i32>();
            (0..n)
                .map(|i| {
                    if a.is_null(i) {
                        None
                    } else {
                        parse_day(a.value(i))
                    }
                })
                .collect()
        }
        DataType::LargeUtf8 => {
            let a = fe.as_string::<i64>();
            (0..n)
                .map(|i| {
                    if a.is_null(i) {
                        None
                    } else {
                        parse_day(a.value(i))
                    }
                })
                .collect()
        }
        t => bail!("fecha de tipo {t} no soportado"),
    })
}

pub fn load_events(path: &Path, sha256: &str) -> Result<Events> {
    if !sha256.is_empty() {
        let digest = crate::network::file_sha256(path)?;
        if digest != sha256 {
            bail!(
                "sha256 de {} no coincide: {:.12}… ≠ {:.12}…",
                path.display(),
                digest,
                sha256
            );
        }
    }
    let f = std::fs::File::open(path).with_context(|| format!("abriendo {}", path.display()))?;
    let b = ParquetRecordBatchReaderBuilder::try_new(f)?;
    let schema = b.parquet_schema();
    let idx: Vec<usize> = ["lat", "lon", "fecha"]
        .iter()
        .map(|c| {
            schema
                .columns()
                .iter()
                .position(|col| col.path().string() == *c)
                .with_context(|| format!("{} no tiene la columna {c}", path.display()))
        })
        .collect::<Result<_>>()?;
    let mask = ProjectionMask::roots(schema, idx);
    let reader = b.with_projection(mask).build()?;
    let mut ev = Events {
        lat: vec![],
        lon: vec![],
        day: vec![],
    };
    for batch in reader {
        let batch = batch?;
        let col = |name: &str| {
            batch
                .column_by_name(name)
                .cloned()
                .with_context(|| format!("falta {name}"))
        };
        let float = |a: &dyn Array, i: usize| -> Option<f64> {
            if a.is_null(i) {
                return None;
            }
            match a.data_type() {
                DataType::Float64 => Some(a.as_primitive::<Float64Type>().value(i)),
                DataType::Float32 => Some(a.as_primitive::<Float32Type>().value(i) as f64),
                _ => None,
            }
        };
        // lat/lon numéricas de cualquier tipo (float, entero, decimal) pasan a float64, como pandas
        let as_f64 = |a: arrow::array::ArrayRef, name: &str| -> Result<arrow::array::ArrayRef> {
            if !a.data_type().is_numeric() {
                bail!("{name} debe ser numérica, es {}", a.data_type());
            }
            Ok(arrow::compute::cast(&a, &DataType::Float64)?)
        };
        let (la, lo, fe) = (
            as_f64(col("lat")?, "lat")?,
            as_f64(col("lon")?, "lon")?,
            col("fecha")?,
        );
        let days = column_days(fe.as_ref())?;
        for (i, d) in days.iter().enumerate() {
            let (Some(lat), Some(lon)) = (float(la.as_ref(), i), float(lo.as_ref(), i)) else {
                continue;
            };
            if lat.is_nan() || lon.is_nan() {
                continue; // dropna
            }
            ev.lat.push(lat);
            ev.lon.push(lon);
            ev.day.push(d.unwrap_or(i64::MIN));
        }
    }
    Ok(ev)
}

pub struct Surface {
    grid: Grid,
    pub s8: HashMap<CellIndex, u32>,
    pub data_as_of: String,
    pub n_events: usize,
    pub low_threshold: f64,
    pub radius_m: f64,
    pub h3_res: Resolution,
}

pub fn cell_of(lat: f64, lon: f64, res: Resolution) -> Option<CellIndex> {
    LatLng::new(lat, lon).ok().map(|ll| ll.to_cell(res))
}

impl Surface {
    pub fn build(ev: &Events, cfg: &Settings) -> Result<Surface> {
        let res = Resolution::try_from(cfg.h3_res).context("WACHI_H3_RES fuera de rango (0-15)")?;
        let xy: Vec<[f64; 2]> = (0..ev.lat.len())
            .into_par_iter()
            .map(|i| {
                let (x, y) = to_utm(ev.lon[i], ev.lat[i]);
                [x, y]
            })
            .collect();
        let cells: Vec<Option<CellIndex>> = (0..ev.lat.len())
            .into_par_iter()
            .map(|i| cell_of(ev.lat[i], ev.lon[i], res))
            .collect();
        let mut s8: HashMap<CellIndex, u32> = HashMap::new();
        for c in cells.into_iter().flatten() {
            *s8.entry(c).or_default() += 1;
        }
        let counts: Vec<f64> = s8.values().map(|&v| v as f64).collect();
        let low = if counts.is_empty() {
            1.0
        } else {
            percentile(&counts, cfg.low_data_pct)
        };
        let low = low.max(1.0); // una celda sin ningún evento siempre es "pocos datos"
        let data_as_of = ev
            .day
            .iter()
            .copied()
            .filter(|&d| d != i64::MIN)
            .max()
            .map(|d| {
                let (y, m, dd) = civil_from_days(d);
                format!("{y:04}-{m:02}-{dd:02}")
            })
            .unwrap_or_default();
        Ok(Surface {
            grid: Grid::new(&xy, cfg.radius_m),
            s8,
            data_as_of,
            n_events: ev.lat.len(),
            low_threshold: low,
            radius_m: cfg.radius_m,
            h3_res: res,
        })
    }

    /// S150: eventos a ≤ radius_m del punto UTM.
    pub fn s150(&self, x: f64, y: f64) -> f64 {
        self.grid.count_within(x, y, self.radius_m) as f64
    }

    pub fn cell_utm(&self, x: f64, y: f64) -> Option<CellIndex> {
        let (lon, lat) = to_ll(x, y);
        cell_of(lat, lon, self.h3_res)
    }

    pub fn is_low(&self, cell: Option<CellIndex>) -> bool {
        let n = cell.and_then(|c| self.s8.get(&c)).copied().unwrap_or(0);
        (n as f64) < self.low_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fechas_civiles() {
        for d in [-1000, 0, 19_000, 20_268] {
            let (y, m, dd) = civil_from_days(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
        assert_eq!(civil_from_days(days_from_civil(2025, 6, 30)), (2025, 6, 30));
        assert_eq!(
            parse_day("2024-04-30T10:00:00"),
            Some(days_from_civil(2024, 4, 30))
        );
    }
}
