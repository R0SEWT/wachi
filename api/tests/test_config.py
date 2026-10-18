from wachi.config import Settings
from wachi.geo import to_ll, to_utm


def test_defaults_match_spec():
    s = Settings(_env_file=None)
    assert s.radius_m == 150 and s.step_m == 20 and s.h3_res == 8
    assert s.cap_pct == 0.15 and s.cap_min == 5.0 and s.gain_min == 0.20
    assert s.betas == [0.5] and s.snap_max_m == 300.0
    assert s.bbox == (-12.28, -77.13, -11.87, -76.85)
    assert abs(s.walk_m_per_min - 83.333) < 0.01


def test_env_override(monkeypatch):
    monkeypatch.setenv("WACHI_CAP_MIN", "7")
    assert Settings(_env_file=None).cap_min == 7.0


def test_utm_roundtrip():
    x, y = to_utm(-77.03, -12.08)
    lon, lat = to_ll(x, y)
    assert abs(lon + 77.03) < 1e-6 and abs(lat + 12.08) < 1e-6
