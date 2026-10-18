"""Casos UX del experimento (od_pairs.yaml: ux_elena, ux_mateo). Corre solo con api/data/."""
import pytest
import shapely

from wachi.config import Settings
from wachi.engine import load_engine

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
    assert 100 <= out["calm"]["detour_m"] <= 180
    assert out["calm"]["exposure_reduction_pct"] >= 40


def test_elena_calm_does_not_cross_rebagliati(engine):
    out = engine.route(*ELENA)
    reb = [g for k, n, g in engine.router.net.polys if "Rebagliati" in n]
    assert reb, "el pbf debe traer el hospital Rebagliati"
    if out["calm"] is None:
        return
    from wachi.geo import TO_UTM
    xy = [TO_UTM.transform(lon, lat) for lat, lon in out["calm"]["geometry"]]
    inside = shapely.length(shapely.intersection(shapely.linestrings(xy), shapely.union_all(reb)))
    assert inside < 20


def test_health_numbers(engine):
    assert engine.surf.n_events > 10_000 and len(engine.router.net.nxy) > 200_000
