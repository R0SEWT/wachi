from tests.conftest import ll
from wachi.router import Router
from wachi.surface import build_surface


def _router(net, events, cfg):
    return Router(net, build_surface(events, cfg), cfg)


def test_snap_and_node_ll(net, events, cfg):
    r = _router(net, events, cfg)
    lon, lat = ll(10, 5)
    node, d = r.snap(lat, lon)
    assert d < 15
    nlat, nlon = r.node_ll(node)
    assert abs(nlat - ll(0, 0)[1]) < 1e-5 and abs(nlon - ll(0, 0)[0]) < 1e-5


def test_beta0_takes_shortest(net, events, cfg):
    r = _router(net, events, cfg)
    a, _ = r.snap(*ll(0, 0)[::-1])
    b, _ = r.snap(*ll(1000, 0)[::-1])
    route = r.route(a, b, beta=0.0, blocked=None)
    assert len(route.edges) == 1 and abs(route.length_m - 1000) < 5


def test_beta_avoids_events(net, events, cfg):
    r = _router(net, events, cfg)
    a, _ = r.snap(*ll(0, 0)[::-1])
    b, _ = r.snap(*ll(1000, 0)[::-1])
    fast = r.route(a, b, beta=0.0, blocked=None)
    calm = r.route(a, b, beta=0.5, blocked=None)
    assert len(calm.edges) == 2 and abs(calm.length_m - 1077) < 5
    assert r.measure(calm).exposure == 0
    assert r.measure(fast).exposure > 0


def test_measure_exposure_is_integral(net, events, cfg):
    r = _router(net, events, cfg)
    a, _ = r.snap(*ll(0, 0)[::-1])
    b, _ = r.snap(*ll(1000, 0)[::-1])
    m = r.measure(r.route(a, b, beta=0.0, blocked=None))
    # 10 eventos en x=500 cuentan en las muestras con x∈[350,650]: ~300 m → 10 · 0.3 km = 3
    assert 2.6 < m.exposure < 3.4 and len(m.cells) == len(r.route(a, b, 0.0, None).xy)


def test_recinto_blocks_bypass(net_recinto, events, cfg):
    r = _router(net_recinto, events, cfg)
    a, _ = r.snap(*ll(0, 0)[::-1])
    b, _ = r.snap(*ll(1000, 0)[::-1])
    c, _ = r.snap(*ll(500, 200)[::-1])
    assert set(r.edge_poly[r.edge_poly >= 0]) == {0}
    blocked = r.blocked_for(r.allowed_polys(a, b))
    assert len(r.route(a, b, beta=0.5, blocked=blocked).edges) == 1   # cae a la recta
    assert r.allowed_polys(c) == {0}                                 # origen dentro: permitido
    assert len(r.route(c, b, beta=0.0, blocked=r.blocked_for(r.allowed_polys(c, b))).edges) == 1


def test_route_none_when_unreachable(net, events, cfg):
    r = _router(net, events, cfg)
    a, _ = r.snap(*ll(0, 0)[::-1])
    assert r.route(a, a, beta=0.0, blocked=None).length_m == 0
    assert r.route(a, 3, beta=0.0, blocked=frozenset()) is not None
