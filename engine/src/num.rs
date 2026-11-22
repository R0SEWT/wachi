//! Aritmética con la misma semántica que numpy/Python, para que las respuestas coincidan.

/// `np.percentile(v, q)` con el método por defecto (`linear`), incluida su fórmula de `lerp`.
pub fn percentile(values: &[f64], q: f64) -> f64 {
    assert!(!values.is_empty(), "percentil de un arreglo vacío");
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    percentile_sorted(&v, q)
}

pub fn percentile_sorted(v: &[f64], q: f64) -> f64 {
    let n = v.len();
    let pos = q / 100.0 * (n as f64 - 1.0);
    let lo = pos.floor();
    let t = pos - lo;
    let lo = lo as usize;
    let hi = (lo + 1).min(n - 1);
    let (a, b) = (v[lo], v[hi]);
    let d = b - a;
    // numpy._lerp: a + d·t, pero b − d·(1−t) cuando t ≥ 0.5 (más estable cerca de b)
    if t >= 0.5 { b - d * (1.0 - t) } else { a + d * t }
}

/// `round(x)` de Python sobre float: al entero más cercano, empates al par.
pub fn round_half_even(x: f64) -> f64 {
    x.round_ties_even()
}

/// `round(x, nd)` de Python: redondeo correcto sobre el valor decimal exacto del float.
///
/// El formateo de Rust con precisión fija es exacto (redondea el valor binario real), igual que
/// `_Py_dg_dtoa` en modo 3, así que pasar por texto da el mismo float que Python.
pub fn round_nd(x: f64, nd: usize) -> f64 {
    if !x.is_finite() {
        return x;
    }
    format!("{x:.nd$}").parse().expect("float formateado")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentil_como_numpy() {
        // np.percentile([1,2,3,4], 75) = 3.25 ; [1..10] p10 = 1.9 ; un solo valor
        assert_eq!(percentile(&[4.0, 1.0, 3.0, 2.0], 75.0), 3.25);
        let v: Vec<f64> = (1..=10).map(f64::from).collect();
        assert!((percentile(&v, 10.0) - 1.9).abs() < 1e-12);
        assert_eq!(percentile(&[7.0], 75.0), 7.0);
    }

    #[test]
    fn redondeos_de_python() {
        assert_eq!(round_half_even(2.5), 2.0);
        assert_eq!(round_half_even(3.5), 4.0);
        assert_eq!(round_half_even(-0.5), -0.0);
        // round(2.675, 2) == 2.67 en Python (2.675 es 2.67499999… en binario)
        assert_eq!(round_nd(2.675, 2), 2.67);
        assert_eq!(round_nd(-12.0880714999, 6), -12.088071);
    }
}
