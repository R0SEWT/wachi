//! WGS84 ↔ UTM 18S (EPSG:32718) con la serie de Krüger de orden 6 (Karney 2011).
//!
//! Es la misma familia de fórmulas que usa PROJ para `+proj=utm` (Poder/Engsager); dentro de la
//! zona la diferencia con pyproj es de nanómetros (ver el test `paridad_pyproj`).

const A: f64 = 6_378_137.0;
const F: f64 = 1.0 / 298.257_223_563;
const K0: f64 = 0.9996;
const LON0: f64 = -75.0; // zona 18
const FE: f64 = 500_000.0;
const FN: f64 = 10_000_000.0; // hemisferio sur

struct Coef {
    e: f64,
    a_rect: f64, // A de Krüger (radio rectificante) · k0
    alpha: [f64; 6],
    beta: [f64; 6],
}

fn coef() -> &'static Coef {
    use std::sync::OnceLock;
    static C: OnceLock<Coef> = OnceLock::new();
    C.get_or_init(|| {
        let n = F / (2.0 - F);
        let (n2, n3) = (n * n, n * n * n);
        let (n4, n5, n6) = (n2 * n2, n2 * n3, n3 * n3);
        let e2 = F * (2.0 - F);
        let a_rect = K0 * A / (1.0 + n) * (1.0 + n2 / 4.0 + n4 / 64.0 + n6 / 256.0);
        let alpha = [
            n / 2.0 - 2.0 * n2 / 3.0 + 5.0 * n3 / 16.0 + 41.0 * n4 / 180.0 - 127.0 * n5 / 288.0
                + 7891.0 * n6 / 37800.0,
            13.0 * n2 / 48.0 - 3.0 * n3 / 5.0 + 557.0 * n4 / 1440.0 + 281.0 * n5 / 630.0
                - 1983433.0 * n6 / 1935360.0,
            61.0 * n3 / 240.0 - 103.0 * n4 / 140.0 + 15061.0 * n5 / 26880.0 + 167603.0 * n6 / 181440.0,
            49561.0 * n4 / 161280.0 - 179.0 * n5 / 168.0 + 6601661.0 * n6 / 7257600.0,
            34729.0 * n5 / 80640.0 - 3418889.0 * n6 / 1995840.0,
            212378941.0 * n6 / 319334400.0,
        ];
        let beta = [
            n / 2.0 - 2.0 * n2 / 3.0 + 37.0 * n3 / 96.0 - n4 / 360.0 - 81.0 * n5 / 512.0
                + 96199.0 * n6 / 604800.0,
            n2 / 48.0 + n3 / 15.0 - 437.0 * n4 / 1440.0 + 46.0 * n5 / 105.0 - 1118711.0 * n6 / 3870720.0,
            17.0 * n3 / 480.0 - 37.0 * n4 / 840.0 - 209.0 * n5 / 4480.0 + 5569.0 * n6 / 90720.0,
            4397.0 * n4 / 161280.0 - 11.0 * n5 / 504.0 - 830251.0 * n6 / 7257600.0,
            4583.0 * n5 / 161280.0 - 108847.0 * n6 / 3991680.0,
            20648693.0 * n6 / 638668800.0,
        ];
        Coef { e: e2.sqrt(), a_rect, alpha, beta }
    })
}

/// (lon, lat) en grados → (x, y) UTM 18S en metros.
pub fn to_utm(lon: f64, lat: f64) -> (f64, f64) {
    let c = coef();
    let phi = lat.to_radians();
    let lam = (lon - LON0).to_radians();
    let s = phi.sin();
    let t = (s.atanh() - c.e * (c.e * s).atanh()).sinh();
    let xi_p = t.atan2(lam.cos());
    let eta_p = (lam.sin() / (1.0 + t * t).sqrt()).atanh();
    let (mut xi, mut eta) = (xi_p, eta_p);
    for (j, a) in c.alpha.iter().enumerate() {
        let k = 2.0 * (j as f64 + 1.0);
        xi += a * (k * xi_p).sin() * (k * eta_p).cosh();
        eta += a * (k * xi_p).cos() * (k * eta_p).sinh();
    }
    (FE + c.a_rect * eta, FN + c.a_rect * xi)
}

/// (x, y) UTM 18S → (lon, lat) en grados.
pub fn to_ll(x: f64, y: f64) -> (f64, f64) {
    let c = coef();
    let xi = (y - FN) / c.a_rect;
    let eta = (x - FE) / c.a_rect;
    let (mut xi_p, mut eta_p) = (xi, eta);
    for (j, b) in c.beta.iter().enumerate() {
        let k = 2.0 * (j as f64 + 1.0);
        xi_p -= b * (k * xi).sin() * (k * eta).cosh();
        eta_p -= b * (k * xi).cos() * (k * eta).sinh();
    }
    let tau_p = xi_p.sin() / (eta_p.sinh().powi(2) + xi_p.cos().powi(2)).sqrt();
    let lam = eta_p.sinh().atan2(xi_p.cos());
    // τ desde τ' por Newton (Karney 2011, ec. 19-21)
    let e2 = c.e * c.e;
    let mut tau = tau_p;
    for _ in 0..8 {
        let tau1 = (1.0 + tau * tau).sqrt();
        let sig = (c.e * (c.e * tau / tau1).atanh()).sinh();
        let taup_i = tau * (1.0 + sig * sig).sqrt() - sig * tau1;
        let d = (tau_p - taup_i) / (1.0 + taup_i * taup_i).sqrt() * (1.0 + (1.0 - e2) * tau * tau)
            / ((1.0 - e2) * tau1);
        tau += d;
        if d.abs() < 1e-15 {
            break;
        }
    }
    (LON0 + lam.to_degrees(), tau.atan().to_degrees())
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
    fn paridad_pyproj() {
        // pyproj 3.7 (PROJ 9): Transformer.from_crs("EPSG:4326","EPSG:32718",always_xy=True)
        let (x, y) = to_utm(-77.03, -12.08);
        assert!((x - 279_038.958_671_539).abs() < 1e-5, "{x}");
        assert!((y - 8_663_780.658_875_449).abs() < 1e-5, "{y}");
    }
}
