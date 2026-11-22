//! WGS84 ↔ UTM 18S (EPSG:32718): port operación por operación de PROJ 9.8.1.
//!
//! pyproj (que usa la API en Python) corre el pipeline
//! `unitconvert deg→rad | utm zone=18 south` con el algoritmo exacto de Poder/Engsager
//! (`tmerc_default_algo = poder_engsager`). Este módulo repite sus mismas operaciones en el mismo
//! orden (`src/projections/tmerc.cpp`, `src/latitudes.cpp`, `src/fwd.cpp`, `src/inv.cpp`,
//! `src/conversions/unitconvert.cpp`) y llama a las mismas funciones de la libm del sistema, para
//! que las coordenadas salgan idénticas bit a bit. Importa: OSM guarda 7 decimales y la API
//! redondea a 6, así que una de cada diez coordenadas cae justo en un empate de redondeo y
//! cualquier diferencia en el último bit la cambia.

use std::sync::OnceLock;

// Funciones de la libm de glibc, las mismas que llama libproj. No se usan los métodos de f64
// porque algunos (asinh) están implementados en Rust y difieren en el último bit.
unsafe extern "C" {
    fn sin(x: f64) -> f64;
    fn cos(x: f64) -> f64;
    fn tan(x: f64) -> f64;
    fn asin(x: f64) -> f64;
    fn atan2(y: f64, x: f64) -> f64;
    fn hypot(x: f64, y: f64) -> f64;
    fn asinh(x: f64) -> f64;
    fn sinh(x: f64) -> f64;
    fn exp(x: f64) -> f64;
    fn pow(x: f64, y: f64) -> f64;
}

fn c_sin(x: f64) -> f64 {
    unsafe { sin(x) }
}
fn c_cos(x: f64) -> f64 {
    unsafe { cos(x) }
}

const M_PI: f64 = 3.141_592_653_589_793_238_46;
const M_TWOPI: f64 = 6.283_185_307_179_586_476_93;
const DEG_TO_RAD: f64 = 0.017_453_292_519_943_296;
const ORDER: usize = 6;

struct Proj {
    a: f64,
    ra: f64,
    lam0: f64,
    x0: f64,
    y0: f64,
    qn: f64,
    zb: f64,
    cgb: [f64; ORDER],
    cbg: [f64; ORDER],
    utg: [f64; ORDER],
    gtu: [f64; ORDER],
    rad_to_deg_factor: f64,
}

/// latitudes.cpp: C[eta,zeta] para las cuatro conversiones que usa tmerc (k = 6·salida + entrada).
#[rustfmt::skip]
const COEFFS_PHI_CHI: [f64; 21] = [ // k=4: conforme → geográfica (cgb)
    2.0, -2.0 / 3.0, -2.0, 116.0 / 45.0, 26.0 / 45.0, -2854.0 / 675.0,
    7.0 / 3.0, -8.0 / 5.0, -227.0 / 45.0, 2704.0 / 315.0, 2323.0 / 945.0,
    56.0 / 15.0, -136.0 / 35.0, -1262.0 / 105.0, 73814.0 / 2835.0,
    4279.0 / 630.0, -332.0 / 35.0, -399572.0 / 14175.0, 4174.0 / 315.0,
    -144838.0 / 6237.0, 601676.0 / 22275.0,
];
#[rustfmt::skip]
const COEFFS_MU_CHI: [f64; 21] = [ // k=22: conforme → rectificante (gtu)
    1.0 / 2.0, -2.0 / 3.0, 5.0 / 16.0, 41.0 / 180.0, -127.0 / 288.0,
    7891.0 / 37800.0, 13.0 / 48.0, -3.0 / 5.0, 557.0 / 1440.0,
    281.0 / 630.0, -1983433.0 / 1935360.0, 61.0 / 240.0, -103.0 / 140.0,
    15061.0 / 26880.0, 167603.0 / 181440.0, 49561.0 / 161280.0,
    -179.0 / 168.0, 6601661.0 / 7257600.0, 34729.0 / 80640.0,
    -3418889.0 / 1995840.0, 212378941.0 / 319334400.0,
];
#[rustfmt::skip]
const COEFFS_CHI_PHI: [f64; 21] = [ // k=24: geográfica → conforme (cbg)
    -2.0, 2.0 / 3.0, 4.0 / 3.0, -82.0 / 45.0, 32.0 / 45.0, 4642.0 / 4725.0,
    5.0 / 3.0, -16.0 / 15.0, -13.0 / 9.0, 904.0 / 315.0, -1522.0 / 945.0,
    -26.0 / 15.0, 34.0 / 21.0, 8.0 / 5.0, -12686.0 / 2835.0, 1237.0 / 630.0,
    -12.0 / 5.0, -24832.0 / 14175.0, -734.0 / 315.0, 109598.0 / 31185.0,
    444337.0 / 155925.0,
];
#[rustfmt::skip]
const COEFFS_CHI_MU: [f64; 21] = [ // k=27: rectificante → conforme (utg)
    -1.0 / 2.0, 2.0 / 3.0, -37.0 / 96.0, 1.0 / 360.0, 81.0 / 512.0,
    -96199.0 / 604800.0, -1.0 / 48.0, -1.0 / 15.0, 437.0 / 1440.0,
    -46.0 / 105.0, 1118711.0 / 3870720.0, -17.0 / 480.0, 37.0 / 840.0,
    209.0 / 4480.0, -5569.0 / 90720.0, -4397.0 / 161280.0, 11.0 / 504.0,
    830251.0 / 7257600.0, -4583.0 / 161280.0, 108847.0 / 3991680.0,
    -20648693.0 / 638668800.0,
];

/// pj_polyval: Σ p[i]·x^i por Horner (p de largo N+1).
fn polyval(x: f64, p: &[f64], n: usize) -> f64 {
    let mut n = n;
    let mut y = p[n];
    while n > 0 {
        n -= 1;
        y = y * x + p[n];
    }
    y
}

/// pj_auxlat_coeffs, rama de conversiones que involucran la latitud conforme (polinomio en n).
fn auxlat_coeffs(n: f64, c: &[f64; 21]) -> [f64; ORDER] {
    let mut f = [0.0; ORDER];
    let (mut d, mut o) = (n, 0usize);
    for (l, fl) in f.iter_mut().enumerate() {
        let m = ORDER - l - 1;
        *fl = d * polyval(n, &c[o..], m);
        o += m + 1;
        d *= n;
    }
    f
}

/// pj_clenshaw: Σ F[k]·sin((2k+2)·ζ).
fn clenshaw(szeta: f64, czeta: f64, f: &[f64; ORDER]) -> f64 {
    let (mut u0, mut u1) = (0.0f64, 0.0f64);
    let x = 2.0 * (czeta - szeta) * (czeta + szeta);
    let mut k = ORDER;
    while k > 0 {
        k -= 1;
        let t = x * u0 - u1 + f[k];
        u1 = u0;
        u0 = t;
    }
    2.0 * szeta * czeta * u0
}

fn auxlat_convert(zeta: f64, f: &[f64; ORDER]) -> f64 {
    zeta + clenshaw(c_sin(zeta), c_cos(zeta), f)
}

/// clenS de tmerc.cpp (suma de Clenshaw compleja). Devuelve (R, I).
fn clen_s(a: &[f64; ORDER], sin_arg_r: f64, cos_arg_r: f64, sinh_arg_i: f64, cosh_arg_i: f64) -> (f64, f64) {
    let mut r = 2.0 * cos_arg_r * cosh_arg_i;
    let mut i = -2.0 * sin_arg_r * sinh_arg_i;
    let mut p = ORDER;
    let (mut hr, mut hr1, mut hi, mut hi1);
    hi1 = 0.0;
    hr1 = 0.0;
    hi = 0.0;
    p -= 1;
    hr = a[p];
    while p > 0 {
        let hr2 = hr1;
        let hi2 = hi1;
        hr1 = hr;
        hi1 = hi;
        p -= 1;
        hr = -hr2 + r * hr1 - i * hi1 + a[p];
        hi = -hi2 + i * hr1 + r * hi1;
    }
    r = sin_arg_r * cosh_arg_i;
    i = cos_arg_r * sinh_arg_i;
    (r * hr - i * hi, r * hi + i * hr)
}

fn adjlon(lon: f64) -> f64 {
    if lon.abs() < M_PI + 1e-12 {
        return lon;
    }
    let mut l = lon + M_PI;
    l -= M_TWOPI * (l / M_TWOPI).floor();
    l - M_PI
}

fn proj() -> &'static Proj {
    static P: OnceLock<Proj> = OnceLock::new();
    P.get_or_init(|| {
        // ell_set.cpp con ellps=WGS84 (a, rf)
        let a = 6_378_137.0f64;
        let rf = 298.257_223_563f64;
        let f = 1.0 / rf;
        let es = 2.0 * f - f * f;
        let e = es.sqrt();
        let alpha = unsafe { asin(e) };
        let n = unsafe { pow(tan(alpha / 2.0), 2.0) };
        let ra = 1.0 / a;
        // utm: zone=18 → zone-1 = 17
        let zone = 17.0f64;
        let lam0 = (zone + 0.5) * M_PI / 30.0 - M_PI;
        let k0 = 0.9996f64;
        // setup_exact
        let cgb = auxlat_coeffs(n, &COEFFS_PHI_CHI);
        let cbg = auxlat_coeffs(n, &COEFFS_CHI_PHI);
        let coeff_rad = [1.0, 1.0 / 4.0, 1.0 / 64.0, 1.0 / 256.0];
        let qn = k0 * (polyval(n * n, &coeff_rad, 3) / (1.0 + n));
        let utg = auxlat_coeffs(n, &COEFFS_CHI_MU);
        let gtu = auxlat_coeffs(n, &COEFFS_MU_CHI);
        let z = auxlat_convert(0.0, &cbg); // phi0 = 0
        let zb = -qn * auxlat_convert(z, &gtu);
        Proj {
            a,
            ra,
            lam0,
            x0: 500_000.0,
            y0: 10_000_000.0,
            qn,
            zb,
            cgb,
            cbg,
            utg,
            gtu,
            rad_to_deg_factor: 1.0 / DEG_TO_RAD,
        }
    })
}

/// (lon, lat) en grados → (x, y) UTM 18S en metros. Igual a
/// `Transformer.from_crs("EPSG:4326", "EPSG:32718", always_xy=True)`.
pub fn to_utm(lon: f64, lat: f64) -> (f64, f64) {
    let q = proj();
    // unitconvert xy_in=deg xy_out=rad
    let mut lam = lon * DEG_TO_RAD;
    let phi = lat * DEG_TO_RAD;
    // fwd_prepare
    lam = adjlon(lam);
    lam = (lam - 0.0) - q.lam0;
    lam = adjlon(lam);
    // exact_e_fwd
    let mut cn = auxlat_convert(phi, &q.cbg);
    let sin_cn = c_sin(cn);
    let cos_cn = c_cos(cn);
    let sin_ce = c_sin(lam);
    let cos_ce = c_cos(lam);
    let cos_cn_cos_ce = cos_cn * cos_ce;
    cn = unsafe { atan2(sin_cn, cos_cn_cos_ce) };
    let inv_denom_tan_ce = 1.0 / unsafe { hypot(sin_cn, cos_cn_cos_ce) };
    let tan_ce = sin_ce * cos_cn * inv_denom_tan_ce;
    let mut ce = unsafe { asinh(tan_ce) };
    let two_inv_denom_tan_ce = 2.0 * inv_denom_tan_ce;
    let two_inv_denom_tan_ce_square = two_inv_denom_tan_ce * inv_denom_tan_ce;
    let tmp_r = cos_cn_cos_ce * two_inv_denom_tan_ce_square;
    let sin_arg_r = sin_cn * tmp_r;
    let cos_arg_r = cos_cn_cos_ce * tmp_r - 1.0;
    let sinh_arg_i = tan_ce * two_inv_denom_tan_ce;
    let cosh_arg_i = two_inv_denom_tan_ce_square - 1.0;
    let (d_cn, d_ce) = clen_s(&q.gtu, sin_arg_r, cos_arg_r, sinh_arg_i, cosh_arg_i);
    cn += d_cn;
    ce += d_ce;
    let mut y = q.qn * cn + q.zb;
    let mut x = q.qn * ce;
    // fwd_finalize (CLASSIC → PROJECTED), fr_meter = 1
    x *= q.a;
    y *= q.a;
    (1.0 * (x + q.x0), 1.0 * (y + q.y0))
}

/// (x, y) UTM 18S → (lon, lat) en grados. Igual a
/// `Transformer.from_crs("EPSG:32718", "EPSG:4326", always_xy=True)`.
pub fn to_ll(x: f64, y: f64) -> (f64, f64) {
    let q = proj();
    // inv_prepare (CLASSIC), to_meter = 1
    let x = (1.0 * x - q.x0) * q.ra;
    let y = (1.0 * y - q.y0) * q.ra;
    // exact_e_inv
    let mut cn = (y - q.zb) / q.qn;
    let mut ce = x / q.qn;
    let sin_arg_r = c_sin(2.0 * cn);
    let cos_arg_r = c_cos(2.0 * cn);
    let exp_2_ce = unsafe { exp(2.0 * ce) };
    let half_inv_exp_2_ce = 0.5 / exp_2_ce;
    let sinh_arg_i = 0.5 * exp_2_ce - half_inv_exp_2_ce;
    let cosh_arg_i = 0.5 * exp_2_ce + half_inv_exp_2_ce;
    let (d_cn, d_ce) = clen_s(&q.utg, sin_arg_r, cos_arg_r, sinh_arg_i, cosh_arg_i);
    cn += d_cn;
    ce += d_ce;
    let sin_cn = c_sin(cn);
    let cos_cn = c_cos(cn);
    let sinh_ce = unsafe { sinh(ce) };
    ce = unsafe { atan2(sinh_ce, cos_cn) };
    let modulus_ce = unsafe { hypot(sinh_ce, cos_cn) };
    let rr = unsafe { hypot(sin_cn, modulus_ce) };
    cn = unsafe { atan2(sin_cn, modulus_ce) };
    let phi = cn + clenshaw(sin_cn / rr, modulus_ce / rr, &q.cgb);
    let mut lam = ce;
    // inv_finalize
    lam = lam + 0.0 + q.lam0;
    lam = adjlon(lam);
    // unitconvert xy_in=rad xy_out=deg
    (lam * q.rad_to_deg_factor, phi * q.rad_to_deg_factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta() {
        for &(lon, lat) in &[(-77.03, -12.08), (-76.85, -11.87), (-77.13, -12.28), (-75.0, -10.0)] {
            let (x, y) = to_utm(lon, lat);
            let (lo, la) = to_ll(x, y);
            assert!((lo - lon).abs() < 1e-11 && (la - lat).abs() < 1e-11, "{lon},{lat} → {lo},{la}");
        }
    }

    #[test]
    fn bit_a_bit_con_pyproj() {
        // pyproj 3.8.0 / PROJ 9.8.1 (repr de Python, así que son los floats exactos)
        assert_eq!(to_utm(-77.03, -12.08), (279038.95867153915, 8663780.658875449));
        assert_eq!(to_ll(279038.958671539, 8663780.658875449), (-77.03, -12.079999999999993));
        assert_eq!(to_ll(280000.5, 8665000.25), (-77.02108736473073, -12.0690420500559));
        assert_eq!(to_ll(270000.0, 8650000.0), (-77.11398588661865, -12.203918923471013));
    }

    /// Muestra grande contra pyproj: `WACHI_PROJ_REF=archivo.bin cargo test` (6 f64 por punto:
    /// lon, lat con 7 decimales, x, y de pyproj y la vuelta de pyproj). Sin la variable no hace nada.
    #[test]
    fn bit_a_bit_muestra_pyproj() {
        let Ok(p) = std::env::var("WACHI_PROJ_REF") else { return };
        let b = std::fs::read(p).unwrap();
        let v: Vec<f64> = b.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect();
        let mut malos = 0;
        for r in v.chunks_exact(6) {
            if to_utm(r[0], r[1]) != (r[2], r[3]) || to_ll(r[2], r[3]) != (r[4], r[5]) {
                malos += 1;
            }
        }
        assert_eq!(malos, 0, "{malos} de {} puntos difieren de pyproj", v.len() / 6);
    }
}
