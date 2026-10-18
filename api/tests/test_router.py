from tests.conftest import ll
from wachi.router import CSR_CACHE_MAX, Router
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


def test_route_none_when_unreachable(net_recinto, events, cfg):
    r = _router(net_recinto, events, cfg)
    a, _ = r.snap(*ll(0, 0)[::-1])
    c, _ = r.snap(*ll(500, 200)[::-1])
    assert r.route(a, a, beta=0.0, blocked=None).length_m == 0
    assert r.route(a, c, beta=0.0, blocked=frozenset()) is not None
    # `blocked` son índices de recinto: con el recinto 0 bloqueado, C queda aislado (A–C y C–B caen dentro)
    assert r.route(a, c, beta=0.0, blocked=frozenset({0})) is None


def test_graph_cache_thread_safe(net, events, cfg):
    from concurrent.futures import ThreadPoolExecutor
    r = _router(net, events, cfg)
    keys = [(0.5, frozenset({i})) for i in range(120)]   # más claves distintas que el tope del caché
    with ThreadPoolExecutor(16) as ex:
        got = list(ex.map(lambda k: r.graph(*k), keys * 3))
    assert len(r._csr) <= CSR_CACHE_MAX
    ref = _router(net, events, cfg)
    for k, (G, ee) in zip(keys * 3, got):
        G2, ee2 = ref.graph(*k)
        assert (G != G2).nnz == 0 and (ee == ee2).all()
