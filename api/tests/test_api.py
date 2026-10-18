import pytest
from fastapi.testclient import TestClient

from tests.conftest import ll
from wachi.api import create_app
from wachi.engine import Engine
from wachi.router import Router
from wachi.surface import build_surface


@pytest.fixture
def client(net, events, cfg):
    r = Router(net, build_surface(events, cfg), cfg)
    return TestClient(create_app(Engine(r, r.surf, cfg), cfg))


def body(o, d):
    return {"origin": {"lat": o[1], "lon": o[0]}, "destination": {"lat": d[1], "lon": d[0]}}


def test_health(client):
    r = client.get("/health")
    assert r.status_code == 200 and r.json()["nodes"] == 4 and r.json()["events"] == 10


def test_route_two_cards(client):
    r = client.post("/route", json=body(ll(0, 0), ll(1000, 0)))
    j = r.json()
    assert r.status_code == 200
    assert j["calm"]["exposure_reduction_pct"] == 100 and j["calm"]["detour_m"] == 77
    assert j["fast"]["geometry"][0] != j["fast"]["geometry"][-1]
    assert set(j) == {"fast", "calm", "data_as_of", "params"}


def test_route_same_node(client):
    r = client.post("/route", json=body(ll(0, 0), ll(5, 5)))
    j = r.json()
    assert r.status_code == 200 and j["fast"]["distance_m"] == 0 and j["calm"] is None
    assert j["fast"]["geometry"][0] == j["fast"]["geometry"][1]


def test_route_outside_bbox(client):
    r = client.post("/route", json=body((-76.0, -12.0), ll(1000, 0)))
    assert r.status_code == 422 and r.json()["error"] == "fuera_de_cobertura"


def test_route_no_way_nearby(client):
    r = client.post("/route", json=body(ll(0, 0), ll(2000, 2000)))
    assert r.status_code == 422
    assert r.json() == {"error": "sin_via_peatonal_cerca", "which": "destination"}


def test_route_validation(client):
    r = client.post("/route", json={"origin": {"lat": 1}})
    assert r.status_code == 422


def test_503_while_loading(cfg):
    c = TestClient(create_app(None, cfg))
    assert c.get("/health").status_code == 503
    r = c.post("/route", json=body(ll(0, 0), ll(1000, 0)))
    assert r.status_code == 503 and r.json()["error"] == "iniciando"
