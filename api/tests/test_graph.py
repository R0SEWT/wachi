import numpy as np

from tests.conftest import ll
from wachi.geo import to_utm
from wachi.graph import build_network, load_cache, save_cache, walk_ok


def test_walk_ok_rules():
    assert walk_ok({"highway": "residential"})
    assert walk_ok({"highway": "footway"})
    assert not walk_ok({"highway": "motorway"})
    assert not walk_ok({"highway": "busway"})
    assert not walk_ok({"highway": "residential", "foot": "no"})
    assert not walk_ok({"highway": "residential", "area": "yes"})
    assert not walk_ok({"highway": "service", "service": "private"})
    assert not walk_ok({"highway": "residential", "sidewalk": "separate"})
    assert not walk_ok({"highway": "residential", "access": "private"})
    assert walk_ok({"highway": "residential", "access": "private", "foot": "yes"})
    assert not walk_ok({"highway": "trunk"})
    assert walk_ok({"highway": "trunk", "sidewalk": "both"})
    assert walk_ok({"highway": "trunk", "foot": "designated"})
    assert not walk_ok({})


def test_build_network_shape(net):
    assert len(net.nxy) == 4 and len(net.u) == 4
    lengths = sorted(net.length.round(0))
    assert abs(lengths[-1] - 1000) < 3          # A–B
    assert sum(abs(x - 538.5) < 3 for x in lengths) == 2  # A–C y C–B
    assert abs(lengths[0] - 400) < 3            # C–D
    assert net.off[-1] == len(net.xy)


def test_build_network_keeps_largest_component():
    from tests.conftest import way
    ways = [way([1, 2]), way([1, 3]), way([3, 2]), way([3, 4])]
    isolated = (np.array([90, 91], np.int64), np.array([-77.1, -77.1001]), np.array([-12.0, -12.0001]), "path")
    net = build_network(ways + [isolated], [])
    assert len(net.nxy) == 4


def test_cache_roundtrip(net_recinto, tmp_path):
    p = tmp_path / "net.npz"
    save_cache(net_recinto, p)
    back = load_cache(p)
    assert np.array_equal(back.u, net_recinto.u) and np.allclose(back.xy, net_recinto.xy)
    assert back.polys[0][0] == "amenity=hospital" and back.polys[0][2].equals(net_recinto.polys[0][2])
    assert back.stats == net_recinto.stats
    assert np.array_equal(back.v, net_recinto.v) and np.array_equal(back.off, net_recinto.off)
    assert np.allclose(back.length, net_recinto.length) and np.allclose(back.nxy, net_recinto.nxy)
    assert np.array_equal(back.hw, net_recinto.hw)
    for name in ("u", "v", "off", "hw"):
        assert getattr(back, name).dtype == getattr(net_recinto, name).dtype


def test_edge_xy_ab_and_reverse(net):
    e = int(np.argmax(net.length))                  # A–B, el tramo de 1000 m
    pts = net.edge_xy(e)
    assert pts.shape == (2, 2)
    a_xy, b_xy = np.array(to_utm(*ll(0, 0))), np.array(to_utm(*ll(1000, 0)))
    assert (np.allclose(pts[0], a_xy, atol=0.01) and np.allclose(pts[1], b_xy, atol=0.01)) or \
        (np.allclose(pts[0], b_xy, atol=0.01) and np.allclose(pts[1], a_xy, atol=0.01))
    assert np.array_equal(net.edge_xy(e, True), pts[::-1])
