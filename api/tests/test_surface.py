import numpy as np
import pandas as pd
import pytest

from tests.conftest import ll
from wachi.geo import to_utm
from wachi.surface import build_surface, load_events


def test_at_counts_within_150m(events, cfg):
    surf = build_surface(events, cfg)
    x0, y0 = to_utm(*ll(500, 0))
    x1, y1 = to_utm(*ll(500, 200))
    v = surf.at(np.array([x0, x0 + 140, x1]), np.array([y0, y0, y1]))
    assert list(v["S150"]) == [10, 10, 0]
    assert surf.n_events == 10 and surf.data_as_of == "2024-04-30"


def test_low_data_cells(events, cfg):
    surf = build_surface(events, cfg)
    import h3
    hot = h3.latlng_to_cell(ll(500, 0)[1], ll(500, 0)[0], cfg.h3_res)
    cold = h3.latlng_to_cell(ll(500, 2000)[1], ll(500, 2000)[0], cfg.h3_res)
    assert list(surf.is_low([hot, cold])) == [False, True]


def test_load_events_drops_rows_without_coords(tmp_path):
    df = pd.DataFrame({"lat": [-12.08, None, -12.09], "lon": [-77.03, -77.04, None],
                       "fecha": pd.to_datetime(["2024-01-01", "2024-02-01", "2024-03-01"])})
    p = tmp_path / "ev.parquet"
    df.to_parquet(p)
    out = load_events(p)
    assert len(out) == 1


def test_load_events_checks_sha(tmp_path, events):
    p = tmp_path / "ev.parquet"
    events.to_parquet(p)
    with pytest.raises(ValueError, match="sha256"):
        load_events(p, sha256="0" * 64)
