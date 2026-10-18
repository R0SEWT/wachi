import math

import numpy as np
import pandas as pd
import pytest
import shapely

from wachi.config import Settings
from wachi.geo import to_utm

LON0, LAT0 = -77.03, -12.08
M_PER_DEG_LON = 111_320 * math.cos(math.radians(LAT0))
M_PER_DEG_LAT = 110_574


def ll(dx_m: float, dy_m: float) -> tuple[float, float]:
    """(lon, lat) del punto a dx, dy metros de (LON0, LAT0)."""
    return LON0 + dx_m / M_PER_DEG_LON, LAT0 + dy_m / M_PER_DEG_LAT


# nodos: A=(0,0) B=(1000,0) C=(500,200) D=(500,600)
NODES = {1: ll(0, 0), 2: ll(1000, 0), 3: ll(500, 200), 4: ll(500, 600)}


def way(ids, hw="residential"):
    pts = [NODES[i] for i in ids]
    return (np.array(ids, np.int64), np.array([p[0] for p in pts]), np.array([p[1] for p in pts]), hw)


@pytest.fixture
def cfg(tmp_path):
    s, w = ll(-1500, -1500)[1], ll(-1500, -1500)[0]
    n, e = ll(2500, 2500)[1], ll(2500, 2500)[0]
    return Settings(_env_file=None, bbox=(s, w, n, e), graph_cache=tmp_path / "net.npz",
                    events_path=tmp_path / "ev.parquet")


@pytest.fixture
def ways():
    return [way([1, 2]), way([1, 3]), way([3, 2]), way([3, 4], "footway")]


@pytest.fixture
def recinto():
    """Rectángulo UTM x∈[200,800], y∈[50,350] alrededor de C: cubre ≥ 50 % de A–C y C–B."""
    x0, y0 = to_utm(LON0, LAT0)
    return ("amenity=hospital", "Hospital Test", shapely.box(x0 + 200, y0 + 50, x0 + 800, y0 + 350))


@pytest.fixture
def net(ways):
    from wachi.graph import build_network
    return build_network(ways, [])


@pytest.fixture
def net_recinto(ways, recinto):
    from wachi.graph import build_network
    return build_network(ways, [recinto])


@pytest.fixture
def events():
    lon, lat = ll(500, 0)
    return pd.DataFrame({"lat": [lat] * 10, "lon": [lon] * 10,
                         "fecha": pd.to_datetime(["2024-03-01"] * 9 + ["2024-04-30"])})
