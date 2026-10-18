"""Casos UX del experimento (od_pairs.yaml: ux_elena, ux_mateo). Corre solo con api/data/."""
import pytest
import shapely

from wachi.config import Settings
from wachi.engine import load_engine
from wachi.geo import TO_UTM

pytestmark = pytest.mark.slow

ELENA = ((-12.088071, -77.033871), (-12.073791, -77.046886))
MATEO = ((-12.069692, -77.079569), (-12.076985, -77.077202))


@pytest.fixture(scope="module")
def engine():
    cfg = Settings()
    if not (cfg.events_path.exists() and (cfg.graph_cache.exists() or cfg.pbf_path.exists())):
        pytest.skip("sin datos reales en api/data/")
    return load_engine(cfg)


def test_mateo_shows_calm(engine):
    out = engine.route(*MATEO)
    assert out["calm"] is not None
    assert 120 <= out["calm"]["detour_m"] <= 160          # spec §11
    assert out["calm"]["exposure_reduction_pct"] >= 45


def test_elena_calm_does_not_cross_rebagliati(engine):
    out = engine.route(*ELENA)
    reb = [g for k, n, g in engine.router.net.polys if "Rebagliati" in n]
    assert reb, "el pbf debe traer el hospital Rebagliati"
    assert out["calm"] is not None, "Elena debería tener Tranquila (+264 m en 2026-10-18)"
    reb_geom = shapely.union_all(reb)

    def line(route):
        return shapely.linestrings([TO_UTM.transform(lon, lat) for lat, lon in route["geometry"]])

    dist = shapely.distance(reb_geom, line(out["fast"]))
    assert dist < 300, f"el caso Elena debe seguir anclado al hospital; la rápida pasa a {dist:.0f} m"
    inside = shapely.length(shapely.intersection(line(out["calm"]), reb_geom))
    assert inside < 20


def test_health_numbers(engine):
    assert engine.surf.n_events > 10_000 and len(engine.router.net.nxy) > 200_000
