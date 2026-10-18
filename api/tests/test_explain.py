from tests.conftest import ll
from wachi.explain import blocks_near_robberies, build_response, low_data_segments
from wachi.router import Router
from wachi.select import select
from wachi.surface import build_surface


class FakeSurf:
    def __init__(self, low):
        self._low = low

    def is_low(self, cells):
        import numpy as np
        return np.array([c in self._low for c in cells])


def test_low_data_segments_runs():
    surf = FakeSurf({"b", "c", "f"})
    assert low_data_segments(["a", "b", "c", "d", "e", "f"], surf) == [[1, 2], [5, 5]]
    assert low_data_segments(["a", "a"], surf) == []


def test_blocks_and_response(net, events, cfg):
    r = Router(net, build_surface(events, cfg), cfg)
    a, _ = r.snap(*ll(0, 0)[::-1])
    b, _ = r.snap(*ll(1000, 0)[::-1])
    fast = r.route(a, b, 0.0, None)
    calm = r.route(a, b, 0.5, None)
    assert blocks_near_robberies(fast, r, cfg) == 1 and blocks_near_robberies(calm, r, cfg) == 0
    sel = select(fast, r.measure(fast), [(calm, r.measure(calm))], cfg)
    out = build_response(sel, r, r.surf, cfg)
    assert out["fast"]["distance_m"] == 1000 and out["fast"]["duration_s"] == 720
    assert out["calm"]["detour_m"] == 77 and out["calm"]["exposure_reduction_pct"] == 100
    assert out["calm"]["blocks_fewer_near_robberies"] == 1
    assert out["data_as_of"] == "2024-04-30" and out["params"]["beta"] == 0.5
    assert out["fast"]["geometry"][0] == list(r.node_ll(a))
    n = len(out["fast"]["geometry"])
    assert all(0 <= i <= j < n for i, j in out["fast"]["low_data_segments"])  # pares inclusivos [i, j] válidos (i == j es un vértice suelto)
    sel2 = select(fast, r.measure(fast), [], cfg)
    assert build_response(sel2, r, r.surf, cfg)["calm"] is None
