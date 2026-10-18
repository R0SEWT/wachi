import numpy as np

from wachi.router import Measure, Route
from wachi.select import select


def R(length, edges):
    return Route(nodes=[0, 1], edges=edges, xy=np.zeros((2, 2)), length_m=length)


def M(length, exposure):
    return Measure(length, exposure, ["a", "b"])


def test_shows_calm_within_cap_and_gain(cfg):
    s = select(R(1000, [0]), M(1000, 3.0), [(R(1077, [1, 2]), M(1077, 0.0))], cfg)
    assert s.calm is not None and s.reduction == 1.0 and abs(s.detour_m - 77) < 1e-9


def test_hides_calm_over_cap(cfg):
    # tope = min(1000·1.15, 1000 + 5·83.3) = 1150
    s = select(R(1000, [0]), M(1000, 3.0), [(R(1200, [1, 2]), M(1200, 0.0))], cfg)
    assert s.calm is None and s.reduction == 0.0


def test_hides_calm_under_gain(cfg):
    s = select(R(1000, [0]), M(1000, 3.0), [(R(1050, [1]), M(1050, 2.5))], cfg)
    assert s.calm is None


def test_hides_calm_when_same_edges(cfg):
    s = select(R(1000, [0]), M(1000, 3.0), [(R(1000, [0]), M(1000, 3.0))], cfg)
    assert s.calm is None


def test_zero_exposure_fast(cfg):
    s = select(R(1000, [0]), M(1000, 0.0), [(R(1050, [1]), M(1050, 0.0))], cfg)
    assert s.calm is None and s.reduction == 0.0


def test_picks_min_exposure_among_candidates(cfg):
    s = select(R(1000, [0]), M(1000, 3.0),
               [(R(1100, [1]), M(1100, 1.0)), (R(1120, [2]), M(1120, 0.5))], cfg)
    assert s.calm.edges == [2]
