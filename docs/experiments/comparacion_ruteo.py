# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "osmium>=4.0",
#   "duckdb>=1.1",
#   "pandas>=2.2",
#   "pyarrow>=16",
#   "numpy>=1.26",
#   "scipy>=1.12",
#   "shapely>=2.0",
#   "pyproj>=3.6",
#   "h3>=4.0",
#   "httpx>=0.27",
#   "pyyaml>=6",
#   "python-dotenv>=1.0",
# ]
# ///
"""Experimento wachi-06f.1 — comparación de ruteo, superficie, regla de selección y recintos.

Compara, sobre ~50 pares origen-destino de Lima (``config/experiments/od_pairs.yaml``):

1. Ruteo: (A) ORS ``foot-walking`` K=3 + selección posterior (mismos parámetros de
   alternativas que ``src/routing/ors_client.py``) vs (B) grafo peatonal OSM propio con
   Dijkstra y costo ``largo·(1 + β·r)``, ``r = riesgo / p75`` de la superficie.
2. Superficie: (S8) conteo por celda H3 res 8 vs (S150) conteo de eventos a ≤150 m.
3. Regla de selección (Rápida = más corta; Tranquila = menor exposición con desvío
   ≤ min(+15 %, +5 min a 5 km/h); segunda tarjeta solo si reduce ≥20 %) y, como
   referencia, la J actual con SAFE (α, β) = (1, 2) sobre las rutas de A.
4. Filtro de recintos (ADR 013): metros dentro de hospital/universidad/cementerio/militar.

Uso (desde la raíz del repo)::

    uv run scripts/experiments/comparacion_ruteo.py --gen-pairs   # regenera los pares
    uv run scripts/experiments/comparacion_ruteo.py               # corre el experimento
    uv run scripts/experiments/comparacion_ruteo.py --holdout     # validación fuera de muestra

Rutas por defecto (todas relativas a la raíz del repo, sobreescribibles por CLI):
``data/raw/LIMA.parquet``, ``data/raw/osm/peru-latest.osm.pbf``, ``.env`` (clave
``CS_ORS_API_KEY``, nunca se imprime), cachés en ``data/experiments/cache`` (red OSM
procesada y respuestas ORS) y salida en ``data/experiments/comparacion_ruteo[_holdout]``
(todo gitignored). Con ``--holdout`` las rutas se optimizan y seleccionan con los eventos
2023-07..2023-11 y se evalúan también con los de 2023-12..2024-04.

Datos de terceros (SIDPOL/MININTER): los eventos solo viven en memoria. A disco van
únicamente agregados (por ruta y por par); nunca coordenadas de eventos.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import sys
import time
from dataclasses import dataclass
from pathlib import Path

import duckdb
import h3
import httpx
import numpy as np
import pandas as pd
import shapely
import yaml
from pyproj import Transformer
from scipy.sparse import csr_matrix
from scipy.sparse.csgraph import connected_components, dijkstra
from scipy.spatial import cKDTree

REPO = Path(__file__).resolve().parents[2]

# ── parámetros fijos del experimento ────────────────────────────────────────
SEED = 20261007
STEP_M = 20.0                 # muestreo de rutas y aristas
RADIUS_M = 150.0              # superficie S150
H3_RES = 8                    # superficie S8 (ADR 006)
WALK_KMH = 5.0
WALK_M_PER_MIN = WALK_KMH * 1000 / 60
CAP_PCT = 0.15                # desvío máximo relativo de la Tranquila
CAP_MIN = 5.0                 # desvío máximo absoluto (min a 5 km/h)
GAIN_MIN = 0.20               # reducción mínima de exposición para mostrar 2.ª tarjeta
BETAS = (0.5, 1.0, 2.0, 4.0)
J_SAFE = (1.0, 2.0)           # (α, β) SAFE de docs/mvp-spec.md
N_BUCKETS = 6                 # la J actual usa riesgo por franja; S8/6 ≈ franja media
OVERLAP_BUF_M = 20.0
RECINTO_TAGS = (("amenity", "hospital"), ("amenity", "university"),
                ("landuse", "cemetery"), ("landuse", "military"))
RECINTO_SHRINK_M = 5.0        # encoge el polígono para no contar veredas del borde
RECINTO_MIN_M = 20.0          # una ruta "cruza" un recinto si recorre > 20 m dentro
# Área de trabajo: cubre todos los pares + ≥2 km de margen (se verifica al correr).
BBOX = (-12.28, -77.13, -11.87, -76.85)  # S, W, N, E
MARGIN_M = 2000.0
ORS_URL = "https://api.openrouteservice.org/v2/directions/foot-walking/geojson"
ORS_ALT = {"target_count": 3, "share_factor": 0.6, "weight_factor": 1.4}  # = ors_client.py
ORS_SLEEP_S = 1.6             # < 40 req/min del plan gratuito
WINDOW = ("2023-07-01", "2024-04-30")   # fechas locales America/Lima, inclusive
SPLIT = "2023-12-01"          # validación fuera de muestra: entrena 2023-07..11, evalúa 2023-12..2024-04
PILE_MAX = 30
EXCL_MODALIDADES = ("HURTO DE VEHICULO", "HURTO DE ACCESORIOS Y AUTOPARTES DE VEHICULOS",
                    "ASALTO Y ROBO DE VEHICULOS", "HURTO AGRAVADO EN CASA HABITADA")
SURFACES = ("S8", "S150")
BANDS = ((800, 1500, "0.8-1.5"), (1500, 2500, "1.5-2.5"), (2500, 4000, "2.5-4"))

TO_UTM = Transformer.from_crs("EPSG:4326", "EPSG:32718", always_xy=True)
TO_LL = Transformer.from_crs("EPSG:32718", "EPSG:4326", always_xy=True)


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 22), b""):
            h.update(chunk)
    return h.hexdigest()


# ════════════════════════════════════════════════════════════════════════════
# 1. Eventos y superficies
# ════════════════════════════════════════════════════════════════════════════
@dataclass
class Surfaces:
    s8: dict            # h3 cell -> conteo
    tree: cKDTree       # eventos en UTM (solo memoria)
    stats: dict

    def at(self, x: np.ndarray, y: np.ndarray) -> dict[str, np.ndarray]:
        lon, lat = TO_LL.transform(x, y)
        cells = [h3.latlng_to_cell(a, b, H3_RES) for a, b in zip(lat, lon)]
        s8 = np.fromiter((self.s8.get(c, 0) for c in cells), float, len(cells))
        s150 = self.tree.query_ball_point(np.c_[x, y], r=RADIUS_M, return_length=True, workers=-1)
        return {"S8": s8, "S150": np.asarray(s150, float), "cells": cells}


def _surface(df: pd.DataFrame, stats: dict) -> Surfaces:
    x, y = TO_UTM.transform(df.lon.values, df.lat.values)
    cells = pd.Series([h3.latlng_to_cell(a, b, H3_RES) for a, b in zip(df.lat, df.lon)])
    s8 = cells.value_counts().to_dict()
    stats = {**stats, "eventos": int(len(df)), "celdas_h3_activas": len(s8),
             "conteo_celda_p50": float(np.median(list(s8.values()))),
             "conteo_celda_p75": float(np.percentile(list(s8.values()), 75))}
    return Surfaces(s8=s8, tree=cKDTree(np.c_[x, y]), stats=stats)


def load_surfaces(raw: Path) -> dict[str, Surfaces]:
    """Universo ADR 007/008, ventana fija, superficie de todo el día.

    Devuelve la superficie de la ventana completa ("full") y, para la validación
    fuera de muestra, la de los 5 primeros meses ("train") y los 5 últimos ("test").
    """
    con = duckdb.connect()
    con.execute("SET TimeZone='UTC'")
    excl = ", ".join(f"'{m}'" for m in EXCL_MODALIDADES)
    src = str(raw).replace("'", "''")
    con.execute(f"""
        CREATE TEMP TABLE w AS
        SELECT lat_hecho AS lat, long_hecho AS lon, tipo_hecho, subtipo_hecho,
               modalidad_hecho, estado_coord,
               CAST(timezone('America/Lima', to_timestamp(fecha_hora_hecho / 1000.0)) AS DATE) AS d
        FROM read_parquet('{src}')
        WHERE CAST(timezone('America/Lima', to_timestamp(fecha_hora_hecho / 1000.0)) AS DATE)
              BETWEEN DATE '{WINDOW[0]}' AND DATE '{WINDOW[1]}'
    """)
    n_window = con.execute("SELECT count(*) FROM w").fetchone()[0]
    con.execute("""
        CREATE TEMP TABLE pile AS
        SELECT round(lat, 5) AS la, round(lon, 5) AS lo, count(*) AS n
        FROM w WHERE lat IS NOT NULL AND lon IS NOT NULL GROUP BY 1, 2
    """)
    base = f"""
        FROM w JOIN pile p ON round(w.lat, 5) = p.la AND round(w.lon, 5) = p.lo
        WHERE w.estado_coord = 'CON COORDENADA'
          AND w.tipo_hecho = 'PATRIMONIO (DELITO)'
          AND w.subtipo_hecho IN ('ROBO', 'HURTO')
          AND w.modalidad_hecho NOT IN ({excl})
    """
    n_universe = con.execute(f"SELECT count(*) {base}").fetchone()[0]
    df = con.execute(f"SELECT w.lat, w.lon, w.d {base} AND p.n <= {PILE_MAX}").df()
    n_pile = len(df)
    S, W, N, E = BBOX
    pad = 0.03
    df = df[df.lat.between(S - pad, N + pad) & df.lon.between(W - pad, E + pad)]
    base_stats = {"filas_ventana": int(n_window), "universo_con_coordenada": int(n_universe),
                  "eventos_tras_pileup": int(n_pile)}
    d = pd.to_datetime(df.d)
    out = {"full": _surface(df, {**base_stats, "periodo": WINDOW}),
           "train": _surface(df[d < SPLIT], {"periodo": (WINDOW[0], "2023-11-30")}),
           "test": _surface(df[d >= SPLIT], {"periodo": (SPLIT, WINDOW[1])})}
    log("eventos", {k: v.stats for k, v in out.items()})
    return out


# ════════════════════════════════════════════════════════════════════════════
# 2. OSM: red peatonal y recintos
# ════════════════════════════════════════════════════════════════════════════
# osmnx "walk" + busway/bus_stop (carril exclusivo del Metropolitano, paraderos mapeados como vía)
_EXCL_HW = re.compile(r"abandoned|bus_guideway|busway|bus_stop|construction|cycleway|motor|no|planned|"
                      r"platform|proposed|raceway|razed")
_SIDEWALK_KEYS = ("sidewalk", "sidewalk:both", "sidewalk:left", "sidewalk:right")
_FOOT_OK = ("yes", "designated", "permissive")


def walk_ok(t) -> bool:
    """Filtro tipo osmnx ``network_type='walk'`` + trunk sin acera y access=no."""
    hw = t.get("highway")
    if hw is None or t.get("area") == "yes" or _EXCL_HW.search(hw):
        return False
    foot = t.get("foot", "")
    if foot == "no" or t.get("service") == "private":
        return False
    if any(t.get(k) == "separate" for k in _SIDEWALK_KEYS):
        return False
    if t.get("access") in ("private", "no") and foot not in _FOOT_OK:
        return False
    if hw in ("trunk", "trunk_link"):
        has_sw = any(t.get(k) in ("yes", "both", "left", "right") for k in _SIDEWALK_KEYS)
        if not has_sw and foot not in _FOOT_OK:
            return False
    return True


@dataclass
class Network:
    u: np.ndarray           # nodo inicial (0..N-1)
    v: np.ndarray
    length: np.ndarray      # m
    off: np.ndarray         # offsets en xy (len E+1)
    xy: np.ndarray          # coordenadas UTM de todas las geometrías (concatenadas)
    nxy: np.ndarray         # coordenadas UTM de nodos
    polys: list             # [(kind, name, shapely geom UTM)]
    stats: dict
    hw: np.ndarray          # highway=* de cada arista

    def edge_xy(self, e: int, reverse: bool = False) -> np.ndarray:
        c = self.xy[self.off[e]:self.off[e + 1]]
        return c[::-1] if reverse else c


def parse_osm(pbf: Path, cache: Path) -> Network:
    if cache.exists():
        z = np.load(cache, allow_pickle=True)
        polys = [(k, n, shapely.from_wkb(g)) for k, n, g in z["polys"]]
        return Network(z["u"], z["v"], z["length"], z["off"], z["xy"], z["nxy"], polys,
                       json.loads(str(z["stats"])), z["hw"])
    import osmium

    S, W, N, E = BBOX
    t0 = time.time()
    wkbf = osmium.geom.WKBFactory()
    fp = (osmium.FileProcessor(str(pbf)).with_locations()
          .with_areas(osmium.filter.TagFilter(*RECINTO_TAGS))
          .with_filter(osmium.filter.KeyFilter("highway", "amenity", "landuse")))
    ways, polys = [], []
    n_ways_bbox = 0

    def inb(n):
        return n.location.valid() and S <= n.lat <= N and W <= n.lon <= E

    for o in fp:
        if o.is_way():
            if "highway" not in o.tags:
                continue
            ns = o.nodes
            if not (inb(ns[0]) or inb(ns[len(ns) - 1])):
                continue
            n_ways_bbox += 1
            if not walk_ok(o.tags):
                continue
            ids, lo, la = [], [], []
            for n in ns:
                if n.location.valid():
                    ids.append(n.ref)
                    lo.append(n.lon)
                    la.append(n.lat)
            if len(ids) >= 2:
                ways.append((np.array(ids, np.int64), np.array(lo), np.array(la), o.tags.get("highway")))
        elif o.is_area():
            kind = next((f"{k}={v}" for k, v in RECINTO_TAGS if o.tags.get(k) == v), None)
            if kind is None:
                continue
            try:
                g = shapely.from_wkb(wkbf.create_multipolygon(o))
            except Exception:
                continue
            b = g.bounds
            if b[2] < W or b[0] > E or b[3] < S or b[1] > N:
                continue
            polys.append((kind, o.tags.get("name", ""), shapely.transform(
                g, lambda c: np.c_[TO_UTM.transform(c[:, 0], c[:, 1])])))
    log(f"OSM leído en {time.time() - t0:.0f}s: {len(ways)} vías peatonales, {len(polys)} recintos")

    # nodos de corte: usados por ≥2 vías o extremos
    all_ids = np.concatenate([w[0] for w in ways])
    uniq, cnt = np.unique(all_ids, return_counts=True)
    multi = set(uniq[cnt >= 2].tolist())
    eu, ev, ehw, coords, off = [], [], [], [], [0]
    for ids, lo, la, hw in ways:
        cut = [0] + [i for i in range(1, len(ids) - 1) if int(ids[i]) in multi] + [len(ids) - 1]
        for a, b in zip(cut[:-1], cut[1:]):
            if ids[a] == ids[b] and b - a < 2:
                continue
            eu.append(ids[a])
            ev.append(ids[b])
            ehw.append(hw)
            coords.append(np.c_[lo[a:b + 1], la[a:b + 1]])
            off.append(off[-1] + b - a + 1)
    ll = np.concatenate(coords)
    x, y = TO_UTM.transform(ll[:, 0], ll[:, 1])
    xy = np.c_[x, y]
    off = np.array(off, np.int64)
    lines = shapely.linestrings(xy, indices=np.repeat(np.arange(len(eu)), np.diff(off)))
    length = shapely.length(lines)
    eu = np.array(eu)
    ev = np.array(ev)
    keep = (length > 0.5) & (eu != ev)
    node_ids, inv = np.unique(np.r_[eu, ev], return_inverse=True)
    ui, vi = inv[:len(eu)], inv[len(eu):]
    # nodo -> coordenada (primer punto de una arista que lo usa)
    nxy = np.zeros((len(node_ids), 2))
    nxy[ui] = xy[off[:-1]]
    nxy[vi] = xy[off[1:] - 1]
    # componente conexa mayor
    A = csr_matrix((np.ones(keep.sum()), (ui[keep], vi[keep])), shape=(len(node_ids),) * 2)
    _, lab = connected_components(A, directed=False)
    big = np.bincount(lab).argmax()
    keep &= lab[ui] == big
    # reindexar
    old = np.flatnonzero(lab == big)
    remap = -np.ones(len(node_ids), np.int64)
    remap[old] = np.arange(len(old))
    sel = np.flatnonzero(keep)
    new_off = [0]
    pieces = []
    for e in sel:
        pieces.append(xy[off[e]:off[e + 1]])
        new_off.append(new_off[-1] + off[e + 1] - off[e])
    net = Network(u=remap[ui[sel]], v=remap[vi[sel]], length=length[sel],
                  off=np.array(new_off, np.int64), xy=np.concatenate(pieces), nxy=nxy[old],
                  polys=polys, stats={}, hw=np.array(ehw)[sel])
    net.stats = {"vias_highway_en_bbox": n_ways_bbox, "vias_peatonales": len(ways),
                 "nodos_lcc": int(len(old)), "aristas_lcc": int(len(sel)),
                 "km_red_lcc": float(net.length.sum() / 1000), "recintos": len(polys),
                 "recintos_por_tipo": pd.Series([p[0] for p in polys]).value_counts().to_dict()}
    log("red", net.stats)
    cache.parent.mkdir(parents=True, exist_ok=True)
    np.savez(cache, u=net.u, v=net.v, length=net.length, off=net.off, xy=net.xy, nxy=net.nxy, hw=net.hw,
             polys=np.array([(k, n, shapely.to_wkb(g)) for k, n, g in polys], dtype=object),
             stats=json.dumps(net.stats))
    return net


# ════════════════════════════════════════════════════════════════════════════
# 3. Motor B: Dijkstra con costo largo·(1+β·r)
# ════════════════════════════════════════════════════════════════════════════
class Router:
    def __init__(self, net: Network, surf: Surfaces):
        self.net = net
        E = len(net.u)
        lines = shapely.linestrings(net.xy, indices=np.repeat(np.arange(E), np.diff(net.off)))
        n = np.maximum(1, np.round(net.length / STEP_M)).astype(int)
        rep = np.repeat(np.arange(E), n)
        k = np.arange(len(rep)) - np.repeat(np.cumsum(n) - n, n)
        pos = (k + 0.5) * net.length[rep] / n[rep]
        pts = shapely.line_interpolate_point(lines[rep], pos)
        px, py = shapely.get_x(pts), shapely.get_y(pts)
        t0 = time.time()
        vals = surf.at(px, py)
        log(f"superficies muestreadas en {len(rep)} puntos de la red ({time.time() - t0:.0f}s)")
        self.p75 = {s: float(np.percentile(vals[s], 75)) for s in SURFACES}
        self.rbar = {s: np.bincount(rep, vals[s], E) / n / self.p75[s] for s in SURFACES}
        self.net_stats = {
            "puntos_red": int(len(rep)),
            **{f"p75_{s}": self.p75[s] for s in SURFACES},
            **{f"p50_{s}": float(np.median(vals[s])) for s in SURFACES},
            **{f"share_red_{s}_0": float((vals[s] == 0).mean()) for s in SURFACES},
            "spearman_S8_S150_red": float(pd.Series(vals["S8"]).corr(pd.Series(vals["S150"]),
                                                                       method="spearman")),
        }
        # recintos: polígono dominante de cada arista (≥50 % de sus muestras dentro)
        self.shrunk = [g.buffer(-RECINTO_SHRINK_M) for _, _, g in net.polys]
        self.ptree = shapely.STRtree(self.shrunk)
        self.otree = shapely.STRtree([g for _, _, g in net.polys])
        pi, gi = self.ptree.query(pts, predicate="within")
        first = pd.Series(gi, index=pi).groupby(level=0).first()
        samp_poly = -np.ones(len(rep), np.int64)
        samp_poly[first.index.values] = first.values
        df = pd.DataFrame({"e": rep, "p": samp_poly})
        inside = df[df.p >= 0].groupby(["e", "p"]).size().rename("k").reset_index()
        inside = inside.sort_values("k").drop_duplicates("e", keep="last")
        self.edge_poly = -np.ones(E, np.int64)
        ok = inside.k.values >= 0.5 * n[inside.e.values]
        self.edge_poly[inside.e.values[ok]] = inside.p.values[ok]
        self.node_tree = cKDTree(net.nxy)
        self._csr: dict = {}

    # ── grafo por configuración ───────────────────────────────────────────
    def graph(self, surface: str | None, beta: float, blocked: frozenset | None):
        key = (surface if beta else None, beta, blocked)
        if key in self._csr:
            return self._csr[key]
        net = self.net
        w = net.length * (1 + beta * self.rbar[surface]) if beta else net.length.copy()
        w = np.maximum(w, 1e-3)
        mask = np.ones(len(w), bool)
        if blocked is not None:
            mask &= ~np.isin(self.edge_poly, list(blocked))
        e = np.flatnonzero(mask)
        uu = np.r_[net.u[e], net.v[e]]
        vv = np.r_[net.v[e], net.u[e]]
        ww = np.r_[w[e], w[e]]
        ee = np.r_[e, e]
        o = np.lexsort((ww, vv, uu))
        uu, vv, ww, ee = uu[o], vv[o], ww[o], ee[o]
        first = np.r_[True, (uu[1:] != uu[:-1]) | (vv[1:] != vv[:-1])]
        uu, vv, ww, ee = uu[first], vv[first], ww[first], ee[first]
        N = len(net.nxy)
        G = csr_matrix((ww, (uu, vv)), shape=(N, N))
        G.sort_indices()
        res = (G, ee, w)
        if len(self._csr) > 40:
            self._csr.pop(next(iter(self._csr)))
        self._csr[key] = res
        return res

    def blocked_for(self, allowed: set[int]) -> frozenset:
        return frozenset(set(range(len(self.net.polys))) - allowed)

    def route(self, src: int, dst: int, surface: str | None, beta: float,
              blocked: frozenset | None, limit: float | None):
        """Devuelve (coords UTM, edges, cost, ms) o None si no hay ruta."""
        G, ee, w = self.graph(surface, beta, blocked)
        t0 = time.perf_counter()
        crow = float(np.hypot(*(self.net.nxy[src] - self.net.nxy[dst])))
        lim = limit if limit is not None else 1.5 * crow + 1000
        while True:
            dist, pred = dijkstra(G, directed=True, indices=src, return_predecessors=True,
                                  limit=lim)
            if np.isfinite(dist[dst]) or limit is not None or lim > 2e5:
                break
            lim *= 2
        if not np.isfinite(dist[dst]):
            return None
        nodes = [dst]
        while nodes[-1] != src:
            nodes.append(pred[nodes[-1]])
        nodes = nodes[::-1]
        edges, pieces = [], []
        for a, b in zip(nodes[:-1], nodes[1:]):
            lo, hi = G.indptr[a], G.indptr[a + 1]
            pos = lo + np.searchsorted(G.indices[lo:hi], b)
            e = int(ee[pos])
            edges.append(e)
            c = self.net.edge_xy(e, reverse=self.net.u[e] != a)
            pieces.append(c if not pieces else c[1:])
        ms = (time.perf_counter() - t0) * 1000
        return np.concatenate(pieces), edges, float(dist[dst]), ms

    def path_cost(self, edges, surface, beta) -> float:
        e = np.asarray(edges)
        return float((self.net.length[e] * (1 + beta * self.rbar[surface][e])).sum())

    def snap(self, lat: float, lon: float) -> tuple[int, float]:
        x, y = TO_UTM.transform(lon, lat)
        d, i = self.node_tree.query([x, y])
        return int(i), float(d)

    def node_ll(self, i: int) -> tuple[float, float]:
        lon, lat = TO_LL.transform(*self.net.nxy[i])
        return round(float(lat), 6), round(float(lon), 6)

    def allowed_polys(self, *nodes: int) -> set[int]:
        pts = shapely.points(self.net.nxy[list(nodes)])
        _, gi = self.otree.query(pts, predicate="within")
        return set(gi.tolist())


# ════════════════════════════════════════════════════════════════════════════
# 4. Evaluación de rutas
# ════════════════════════════════════════════════════════════════════════════
def evaluate(xy: np.ndarray, surf: Surfaces, router: Router, allowed: set[int],
             test: Surfaces | None = None) -> dict:
    line = shapely.linestrings(xy)
    L = float(shapely.length(line))
    n = max(1, math.ceil(L / STEP_M))
    starts = np.arange(n) * STEP_M
    seg = np.minimum(STEP_M, L - starts)
    pts = shapely.line_interpolate_point(line, starts + seg / 2)
    px, py = shapely.get_x(pts), shapely.get_y(pts)
    vals = surf.at(px, py)
    km = seg / 1000
    extra = {}
    if test is not None:
        tv = test.at(px, py)
        extra = {f"E_{s}_test": float((tv[s] * km).sum()) for s in SURFACES}
    cand = [i for i in router.ptree.query(line) if i not in allowed]
    inside = 0.0
    if cand:
        inside = float(shapely.length(shapely.intersection(line, shapely.union_all(
            [router.shrunk[i] for i in cand]))))
    return {"len_m": L, "E_S8": float((vals["S8"] * km).sum()),
            "E_S150": float((vals["S150"] * km).sum()),
            "risk_total_J": float((vals["S8"] / N_BUCKETS * seg).sum()),
            "cero_S8_m": float(seg[vals["S8"] == 0].sum()), "cero_S150_m": float(seg[vals["S150"] == 0].sum()),
            "inside_m": inside, "cells": set(vals["cells"]), "line": line, **extra}


# ════════════════════════════════════════════════════════════════════════════
# 5. ORS con caché en disco y presupuesto
# ════════════════════════════════════════════════════════════════════════════
class ORS:
    def __init__(self, cache: Path, env: Path, budget: int, offline: bool):
        self.cache = cache
        cache.mkdir(parents=True, exist_ok=True)
        self.counter = cache / "llamadas_en_vivo.json"
        self.budget = budget
        self.offline = offline
        self.env = env
        self.live = 0
        self.hits = 0
        self._key = None

    def _used(self) -> int:
        return json.loads(self.counter.read_text())["n"] if self.counter.exists() else 0

    def get(self, o: tuple[float, float], d: tuple[float, float], shortest: bool = False) -> dict | None:
        """Por defecto, la misma consulta que ors_client.py (preference por defecto = recommended).

        ``shortest=True``: una sola ruta con ``preference=shortest`` (solo para medir el grafo).
        """
        body = {"coordinates": [[o[1], o[0]], [d[1], d[0]]], "geometry": True, "format": "geojson"}
        if shortest:
            body["preference"] = "shortest"
        else:
            body["alternative_routes"] = ORS_ALT
        key = hashlib.sha1(json.dumps(body, sort_keys=True).encode()).hexdigest()[:16]
        f = self.cache / f"{key}.json"
        if f.exists():
            self.hits += 1
            return json.loads(f.read_text())
        if self.offline:
            return None
        used = self._used()
        if used >= self.budget:
            log(f"presupuesto ORS agotado ({used}/{self.budget}); par sin ruta A")
            return None
        if self._key is None:
            from dotenv import dotenv_values
            self._key = dotenv_values(self.env).get("CS_ORS_API_KEY") or ""
            if not self._key:
                sys.exit(f"falta CS_ORS_API_KEY en {self.env}")
        time.sleep(ORS_SLEEP_S)
        t0 = time.perf_counter()
        r = httpx.post(ORS_URL, json=body, timeout=30,
                       headers={"Authorization": self._key, "Content-Type": "application/json",
                                "Accept": "application/json, application/geo+json"})
        ms = (time.perf_counter() - t0) * 1000
        self.counter.write_text(json.dumps({"n": used + 1}))
        self.live += 1
        try:
            payload = r.json()
        except ValueError:
            payload = {"error": r.text[:300]}
        rec = {"status": r.status_code, "latency_ms": ms, "body": body, "json": payload}
        if r.status_code in (200, 400, 404):   # 429/5xx no se cachean (reintentables)
            f.write_text(json.dumps(rec))
        else:
            log("ORS", r.status_code, str(payload)[:200])
        return rec


# ════════════════════════════════════════════════════════════════════════════
# 6. Regla de selección y métricas
# ════════════════════════════════════════════════════════════════════════════
def select(cands: list[dict], s: str) -> tuple[int, int, bool]:
    """Rápida = más corta; Tranquila = menor exposición dentro del tope de desvío."""
    r = min(range(len(cands)), key=lambda i: (round(cands[i]["len_m"], 3), i))
    L0 = cands[r]["len_m"]
    cap = min(L0 * (1 + CAP_PCT), L0 + CAP_MIN * WALK_M_PER_MIN)
    ok = [i for i in range(len(cands)) if cands[i]["len_m"] <= cap + 1e-6]
    t = min(ok, key=lambda i: (round(cands[i][f"E_{s}"], 6), round(cands[i]["len_m"], 3), i))
    if t != r and cands[t][f"E_{s}"] >= cands[r][f"E_{s}"] - 1e-9:
        t = r
    shown = t != r and cands[t][f"E_{s}"] <= (1 - GAIN_MIN) * cands[r][f"E_{s}"]
    return r, t, shown


def pair_metrics(cands: list[dict], s: str) -> dict:
    other = "S150" if s == "S8" else "S8"
    r, t, shown = select(cands, s)
    R, T = cands[r], cands[t]
    out = {"n_cand": len(cands), "differs": t != r, "shown": shown,
           "len_R": R["len_m"], "len_T": T["len_m"],
           "E_R_sel": R[f"E_{s}"], "E_T_sel": T[f"E_{s}"],
           "E_R_oth": R[f"E_{other}"], "E_T_oth": T[f"E_{other}"],
           "inside_R": R["inside_m"], "inside_T": T["inside_m"]}
    red = lambda a, b: (1 - b / a) if a > 0 else 0.0  # noqa: E731
    out["red_sel"] = red(R[f"E_{s}"], T[f"E_{s}"])
    out["red_oth"] = red(R[f"E_{other}"], T[f"E_{other}"])
    for s2 in SURFACES:
        if f"E_{s2}_test" in R:
            out[f"red_test_{s2}"] = red(R[f"E_{s2}_test"], T[f"E_{s2}_test"])
    # "sin datos ≠ seguro" (ADR 010): % del largo sin señal en la superficie de selección
    out["cero_R_%"] = 100 * R[f"cero_{s}_m"] / max(R["len_m"], 1)
    out["cero_T_%"] = 100 * T[f"cero_{s}_m"] / max(T["len_m"], 1)
    out["detour_m"] = T["len_m"] - R["len_m"]
    out["detour_min"] = out["detour_m"] / WALK_M_PER_MIN
    out["m_per_pp"] = out["detour_m"] / (100 * out["red_sel"]) if out["red_sel"] > 0 else np.nan
    if t != r:
        rl, tl = R["line"], T["line"]
        out["overlap"] = float(shapely.length(shapely.intersection(
            tl, shapely.buffer(rl, OVERLAP_BUF_M))) / max(T["len_m"], 1))
        cu = R["cells"] | T["cells"]
        out["jaccard_h3"] = len(R["cells"] & T["cells"]) / len(cu)
        out["celdas_nuevas_T"] = len(T["cells"] - R["cells"])
    else:
        out["overlap"] = 1.0
        out["jaccard_h3"] = 1.0
        out["celdas_nuevas_T"] = 0
    return out


def summarize(df: pd.DataFrame) -> dict:
    df = df.assign(differs=df.differs.astype(bool), shown=df.shown.astype(bool))
    d = df[df.differs]
    q = lambda s, p=50: float(np.nanpercentile(s, p)) if len(s) and s.notna().any() else np.nan  # noqa: E731
    return {
        "n": int(len(df)),
        "pct_T_ne_R": 100 * float(df.differs.mean()) if len(df) else np.nan,
        "pct_2a_tarjeta": 100 * float(df.shown.mean()) if len(df) else np.nan,
        "red_sel_media_todos_%": 100 * float(df.red_sel.mean()) if len(df) else np.nan,
        "red_oth_media_todos_%": 100 * float(df.red_oth.mean()) if len(df) else np.nan,
        "red_sel_med_dif_%": 100 * q(d.red_sel),
        "red_oth_med_dif_%": 100 * q(d.red_oth),
        "pct_dif_con_red_oth<=0": 100 * float((d.red_oth <= 0).mean()) if len(d) else np.nan,
        "desvio_m_med_dif": q(d.detour_m),
        "desvio_min_med_dif": q(d.detour_min),
        "m_por_pp_med_dif": q(d.m_per_pp),
        "solape_med_dif": q(d.overlap),
        "jaccard_h3_med_dif": q(d.jaccard_h3),
        "celdas_nuevas_med_dif": q(d.celdas_nuevas_T),
        # lo que ve el usuario: pares con 2.ª tarjeta
        "red_sel_med_2a_%": 100 * q(df[df.shown].red_sel),
        "red_oth_med_2a_%": 100 * q(df[df.shown].red_oth),
        "desvio_m_med_2a": q(df[df.shown].detour_m),
        "cero_R_%_medio_2a": float(df[df.shown]["cero_R_%"].mean()) if df.shown.any() else np.nan,
        "cero_T_%_medio_2a": float(df[df.shown]["cero_T_%"].mean()) if df.shown.any() else np.nan,
        **{k: v for s in SURFACES if f"red_test_{s}" in df for k, v in {
            f"red_test_{s}_media_todos_%": 100 * float(df[f"red_test_{s}"].mean()),
            f"red_test_{s}_med_dif_%": 100 * q(d[f"red_test_{s}"]),
            f"red_test_{s}_med_2a_%": 100 * q(df[df.shown][f"red_test_{s}"]),
            f"pct_dif_con_red_test_{s}>0": 100 * float((d[f"red_test_{s}"] > 0).mean()) if len(d) else np.nan,
        }.items()},
        "inside_m_R_medio": float(df.inside_R.mean()) if len(df) else np.nan,
        "inside_m_T_medio": float(df.inside_T.mean()) if len(df) else np.nan,
        "pct_pares_con_cruce_T": 100 * float((df.inside_T > RECINTO_MIN_M).mean()) if len(df) else np.nan,
    }


# ════════════════════════════════════════════════════════════════════════════
# 7. Generación de pares
# ════════════════════════════════════════════════════════════════════════════
CASOS = [  # (id, nota, origen (lat, lon), destino)
    ("ux_elena", "UX Elena; también caso previo 'elena'", (-12.08807, -77.03387), (-12.07379, -77.04650)),
    ("ux_mateo", "UX Mateo: PUCP (puerta principal) -> La Marina/Universitaria", (-12.0695, -77.0795), (-12.0768, -77.0770)),
    ("prev_sanmarcos", "prototipo previo", (-12.0566, -77.0830), (-12.0771, -77.0827)),
    ("prev_cercado", "prototipo previo", (-12.0465, -77.0426), (-12.0583, -77.0368)),
    ("prev_gamarra", "prototipo previo", (-12.0665, -77.0145), (-12.0672, -77.0337)),
    ("prev_miraflores", "prototipo previo", (-12.1219, -77.0298), (-12.1352, -77.0222)),
    ("prev_surquillo", "prototipo previo", (-12.1128, -77.0002), (-12.1336, -76.9994)),
]
RECINTOS = [  # (id, regex del nombre OSM, extremo dentro, otro extremo (lat, lon))
    ("rec_rebagliati", r"Rebagliati", "O", (-12.0855, -77.0335)),
    # Dos de Mayo descartado: ninguna vía peatonal pública de OSM entra a su polígono.
    ("rec_almenara", r"Guillermo Almenara", "O", (-12.0665, -77.0145)),
    ("rec_pucp", r"^Pontificia Universidad Católica del Perú$", "D", (-12.0770, -77.0830)),
    ("rec_sanmarcos", r"^Ciudad Universitaria de la Universidad Nacional Mayor de San Marcos$", "D",
     (-12.0600, -77.0740)),
]
DISTRITOS = [  # (slug, nombre, cobertura a priori, ancla (lat, lon))
    ("miraflores", "Miraflores", "alta", (-12.1200, -77.0310)),
    ("sanisidro", "San Isidro", "alta", (-12.0975, -77.0365)),
    ("lince", "Lince", "alta", (-12.0830, -77.0335)),
    ("jesusmaria", "Jesús María", "alta", (-12.0740, -77.0460)),
    ("cercado", "Cercado de Lima", "alta", (-12.0500, -77.0330)),
    ("sjl", "San Juan de Lurigancho", "baja", (-11.9850, -77.0050)),
    ("losolivos", "Los Olivos", "baja", (-11.9700, -77.0700)),
    ("comas", "Comas", "baja", (-11.9330, -77.0500)),
    ("ves", "Villa El Salvador", "baja", (-12.2130, -76.9370)),
    ("ate", "Ate", "baja", (-12.0280, -76.9200)),
]
ANCHOR_R_M = 1200.0
SNAP_MAX_M = 150.0


def band_of(L: float) -> str:
    for lo, hi, lab in BANDS:
        if lo <= L < hi:
            return lab
    return "<0.8" if L < BANDS[0][0] else ">4"


def gen_pairs(router: Router, path: Path, pbf_sha: str):
    rng = np.random.default_rng(SEED)
    net = router.net
    in_poly = np.zeros(len(net.nxy), bool)
    pi, _ = router.otree.query(shapely.points(net.nxy), predicate="within")
    in_poly[pi] = True
    pairs = []

    def netlen(a, b):
        r = router.route(a, b, None, 0.0, None, None)
        return None if r is None else float(shapely.length(shapely.linestrings(r[0])))

    def add(pid, estrato, distrito, cobertura, a, b, nota):
        L = netlen(a, b)
        oa, da = router.node_ll(a), router.node_ll(b)
        pairs.append({"id": pid, "estrato": estrato, "distrito": distrito, "cobertura": cobertura,
                      "banda": band_of(L), "dist_red_m": round(L),
                      "recinto_od": bool(in_poly[a] or in_poly[b]),
                      "origen": {"lat": oa[0], "lon": oa[1]}, "destino": {"lat": da[0], "lon": da[1]},
                      "nota": nota})

    for pid, nota, o, d in CASOS:
        a, da_ = router.snap(*o)
        b, db_ = router.snap(*d)
        add(pid, "caso", "", "", a, b, f"{nota}; ajuste a nodo {da_:.0f} m / {db_:.0f} m")
    for pid, rx, role, other in RECINTOS:
        idx = [i for i, (_, n, _) in enumerate(net.polys) if re.search(rx, n)]
        if not idx:
            log("recinto no encontrado", rx)
            continue
        g = max((net.polys[i][2] for i in idx), key=lambda g: g.area)
        cand = np.flatnonzero(shapely.contains_xy(g, net.nxy[:, 0], net.nxy[:, 1]))
        rp = shapely.get_coordinates(g.representative_point())[0]
        if len(cand):
            inside = int(cand[np.argmin(np.hypot(*(net.nxy[cand] - rp).T))])
        else:
            inside = int(router.node_tree.query(rp)[1])
        b, _ = router.snap(*other)
        a_, b_ = (inside, b) if role == "O" else (b, inside)
        where = "origen" if role == "O" else "destino"
        add(pid, "recinto", "", "", a_, b_,
            f"{where} en {net.polys[idx[0]][1]} ({'nodo dentro' if len(cand) else 'SIN nodo dentro'})")
    for k, (slug, dist, cob, anchor) in enumerate(DISTRITOS):
        ax, ay = TO_UTM.transform(anchor[1], anchor[0])
        near = router.node_tree.query_ball_point([ax, ay], ANCHOR_R_M)
        near = [i for i in near if not in_poly[i]]
        bands = list(BANDS) + [BANDS[k % 3]]
        for j, (lo, hi, lab) in enumerate(bands):
            for _ in range(200):
                a = int(rng.choice(near))
                ang = rng.uniform(0, 2 * np.pi)
                dc = rng.uniform(lo / 1.3, hi / 1.3)
                tx, ty = net.nxy[a] + dc * np.array([np.cos(ang), np.sin(ang)])
                d, b = router.node_tree.query([tx, ty])
                if d > SNAP_MAX_M or in_poly[b]:
                    continue
                L = netlen(a, int(b))
                if L is not None and lo <= L < hi:
                    break
            else:
                log("sin par para", dist, lab)
                continue
            add(f"{slug}_{j + 1}", f"cobertura_{cob}", dist, cob, a, int(b), f"aleatorio banda {lab}")
    header = (f"# Pares origen-destino del experimento wachi-06f.1 (comparación de ruteo).\n"
              f"# Generado con: uv run scripts/experiments/comparacion_ruteo.py --gen-pairs\n"
              f"# Semilla {SEED}; nodos de la red peatonal OSM (Geofabrik peru-latest, sha256 {pbf_sha[:16]}…).\n"
              f"# Coordenadas WGS84 ya ajustadas a un nodo de la componente conexa mayor de la red.\n"
              f"# estrato: caso (UX y prototipo previo) | recinto | cobertura_alta | cobertura_baja.\n"
              f"# dist_red_m: camino más corto en la red propia (β=0); banda según esa distancia.\n")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(header + yaml.safe_dump({"semilla": SEED, "pares": pairs}, sort_keys=False,
                                            allow_unicode=True, width=120))
    log(f"{len(pairs)} pares escritos en {path}")


# ════════════════════════════════════════════════════════════════════════════
# 8. Experimento
# ════════════════════════════════════════════════════════════════════════════
def strip(r: dict) -> dict:
    return {k: v for k, v in r.items() if k not in ("line", "cells")}


def run(args, router: Router, surf: Surfaces, meta: dict, test: Surfaces | None = None):
    pairs = yaml.safe_load(args.pairs.read_text())["pares"]
    if args.limit:
        pairs = pairs[: args.limit]
    ors = ORS(args.ors_cache, args.env, args.ors_budget, args.offline)
    out = args.out
    route_rows, sel_rows, lat_rows, geo = [], [], [], []
    # subconjunto para ORS preference=shortest: casos, recintos y un par por distrito (bandas rotadas)
    shortest_ids = {p["id"] for p in pairs if p["estrato"] in ("caso", "recinto")}
    shortest_ids |= {f"{slug}_{k % 3 + 1}" for k, (slug, *_) in enumerate(DISTRITOS)}
    S, W, N, E = BBOX
    for p in pairs:
        for end in ("origen", "destino"):
            x, y = TO_UTM.transform(p[end]["lon"], p[end]["lat"])
            bx = TO_UTM.transform([W, E], [S, N])
            assert bx[0][0] + MARGIN_M <= x <= bx[0][1] - MARGIN_M and \
                bx[1][0] + MARGIN_M <= y <= bx[1][1] - MARGIN_M, f"{p['id']} fuera del área+margen"

    for n_p, p in enumerate(pairs, 1):
        pid = p["id"]
        o = (p["origen"]["lat"], p["origen"]["lon"])
        d = (p["destino"]["lat"], p["destino"]["lon"])
        a, _ = router.snap(*o)
        b, _ = router.snap(*d)
        allowed = router.allowed_polys(a, b)
        blocked = router.blocked_for(allowed)
        routes: dict[str, dict | None] = {}

        def ev(xy, tag, ms=None):
            r = evaluate(xy, surf, router, allowed, test)
            r.update(pair=pid, route=tag, ms=ms)
            routes[tag] = r
            return r

        # --- A: ORS -------------------------------------------------------
        rec = ors.get(o, d)
        a_tags = []
        if rec is not None and rec["status"] == 200:
            lat_rows.append({"pair": pid, "engine": "A", "ms": rec["latency_ms"]})
            for i, f in enumerate(rec["json"].get("features", [])):
                c = np.asarray(f["geometry"]["coordinates"])[:, :2]
                xy = np.c_[TO_UTM.transform(c[:, 0], c[:, 1])]
                r = ev(xy, f"A{i}", ms=rec["latency_ms"])
                r["len_ors_m"] = f["properties"]["summary"]["distance"]
                a_tags.append(f"A{i}")
        a_status = None if rec is None else rec["status"]
        if pid in shortest_ids:   # referencia de calidad del grafo (no es candidata de A)
            rs = ors.get(o, d, shortest=True)
            if rs is not None and rs["status"] == 200 and rs["json"].get("features"):
                c = np.asarray(rs["json"]["features"][0]["geometry"]["coordinates"])[:, :2]
                ev(np.c_[TO_UTM.transform(c[:, 0], c[:, 1])], "As", ms=rs["latency_ms"])
        # --- B: Dijkstra ----------------------------------------------------
        for excl in (False, True):
            sfx = "x" if excl else ""
            blk = blocked if excl else None
            r0 = router.route(a, b, None, 0.0, blk, None)
            if r0 is None:
                routes[f"B{sfx}_0"] = None
                continue
            rr = ev(r0[0], f"B{sfx}_0", ms=r0[3])
            e0 = np.asarray(r0[1])
            mix = pd.Series(router.net.length[e0]).groupby(router.net.hw[e0]).sum()
            rr.update({f"hw_{k}": float(v) for k, v in mix.items()})
            lat_rows.append({"pair": pid, "engine": f"B{sfx}", "beta": 0.0, "ms": r0[3]})
            for s in SURFACES:
                for beta in BETAS:
                    lim = router.path_cost(r0[1], s, beta) * (1 + 1e-9) + 1e-6
                    rb = router.route(a, b, s, beta, blk, lim)
                    tag = f"B{sfx}_{s}_{beta:g}"
                    if rb is None:
                        routes[tag] = None
                        continue
                    ev(rb[0], tag, ms=rb[3])
                    lat_rows.append({"pair": pid, "engine": f"B{sfx}", "surface": s, "beta": beta,
                                     "ms": rb[3]})
        for tag, r in routes.items():
            if r is not None:
                route_rows.append(strip(r))
                if args.geojson:
                    lon, lat = TO_LL.transform(*shapely.get_coordinates(r["line"]).T)
                    geo.append({"type": "Feature", "properties": {"pair": pid, "route": tag},
                                "geometry": {"type": "LineString",
                                             "coordinates": np.round(np.c_[lon, lat], 6).tolist()}})

        # --- estrategias -----------------------------------------------------
        def cands(tags):
            return [routes[t] for t in tags if routes.get(t) is not None]

        a_routes = cands(a_tags)
        a_filt = [r for r in a_routes if r["inside_m"] <= RECINTO_MIN_M]
        if a_routes and not a_filt:
            a_filt = [min(a_routes, key=lambda r: r["inside_m"])]
        strategies = {}
        for s in SURFACES:
            strategies[("A", s, "")] = a_routes
            strategies[("A+recintos", s, "")] = a_filt
            for sfx, eng in (("", "B"), ("x", "B+recintos")):
                base = routes.get(f"B{sfx}_0")
                for beta in BETAS:
                    strategies[(eng, s, f"{beta:g}")] = cands([f"B{sfx}_0", f"B{sfx}_{s}_{beta:g}"]) \
                        if base else []
                strategies[(eng, s, "multi")] = cands([f"B{sfx}_0"] + [f"B{sfx}_{s}_{bb:g}" for bb in BETAS]) \
                    if base else []
        for (eng, s, beta), cs in strategies.items():
            row = {"pair": pid, "estrato": p["estrato"], "banda": p["banda"],
                   "cobertura": p.get("cobertura", ""), "recinto_od": p.get("recinto_od", False),
                   "engine": eng, "surface": s, "beta": beta}
            if not cs:
                row["no_route"] = True
                if eng.startswith("A"):
                    row["ors_status"] = a_status
                sel_rows.append(row)
                continue
            row["no_route"] = False
            row.update(pair_metrics(cs, s))
            sel_rows.append(row)
        # --- J SAFE actual sobre A ------------------------------------------
        if a_routes:
            al, be = J_SAFE
            j = min(range(len(a_routes)), key=lambda i: (al * a_routes[i]["len_m"] + be * a_routes[i]["risk_total_J"], i))
            r = min(range(len(a_routes)), key=lambda i: (a_routes[i]["len_m"], i))
            R, J = a_routes[r], a_routes[j]
            L0 = R["len_m"]
            cap = min(L0 * (1 + CAP_PCT), L0 + CAP_MIN * WALK_M_PER_MIN)
            _, t8, _ = select(a_routes, "S8")
            sel_rows.append({"pair": pid, "estrato": p["estrato"], "banda": p["banda"],
                             "cobertura": p.get("cobertura", ""), "recinto_od": p.get("recinto_od", False),
                             "engine": "A·J_SAFE", "surface": "S8/6", "beta": "2", "no_route": False,
                             "differs": j != r, "len_R": L0, "len_T": J["len_m"],
                             "detour_m": J["len_m"] - L0, "detour_min": (J["len_m"] - L0) / WALK_M_PER_MIN,
                             "excede_tope": J["len_m"] > cap + 1e-6, "coincide_regla_S8": j == t8,
                             "red_sel": 1 - J["E_S8"] / R["E_S8"] if R["E_S8"] else 0.0,
                             "red_oth": 1 - J["E_S150"] / R["E_S150"] if R["E_S150"] else 0.0,
                             "beta_risk_sobre_alpha_dist": be * R["risk_total_J"] / (al * L0),
                             "shown": False, "inside_R": R["inside_m"], "inside_T": J["inside_m"]})
        log(f"[{n_p}/{len(pairs)}] {pid}: A={len(a_routes)} rutas (status {a_status}), "
            f"B0={routes.get('B_0', {}) and round(routes['B_0']['len_m'])} m")

    out.mkdir(parents=True, exist_ok=True)
    rt = pd.DataFrame(route_rows)
    sl = pd.DataFrame(sel_rows)
    lt = pd.DataFrame(lat_rows)
    rt.to_csv(out / "rutas.csv", index=False)
    sl.to_csv(out / "seleccion.csv", index=False)
    lt.to_csv(out / "latencias.csv", index=False)
    if args.geojson:
        (out / "rutas.geojson").write_text(json.dumps({"type": "FeatureCollection", "features": geo}))
    meta["ors"] = {"llamadas_en_vivo_esta_corrida": ors.live, "aciertos_cache": ors.hits,
                   "llamadas_en_vivo_acumuladas": ors._used(), "presupuesto": ors.budget}
    report(rt, sl, lt, pairs, meta, out)


# ════════════════════════════════════════════════════════════════════════════
# 9. Agregados y tablas
# ════════════════════════════════════════════════════════════════════════════
def fmt(v, nd=1):
    if v is None or (isinstance(v, float) and not np.isfinite(v)):
        return "–"
    if isinstance(v, (bool, np.bool_)):
        return "sí" if v else "no"
    if isinstance(v, (int, np.integer)):
        return str(int(v))
    return f"{v:.{nd}f}"


def md_table(df: pd.DataFrame, cols: list[str], nd: dict | None = None) -> str:
    nd = nd or {}
    lines = ["| " + " | ".join(cols) + " |", "|" + "---|" * len(cols)]
    for _, r in df.iterrows():
        lines.append("| " + " | ".join(fmt(r[c], nd.get(c, 1)) if not isinstance(r[c], str) else r[c]
                                      for c in cols) + " |")
    return "\n".join(lines)


def report(rt, sl, lt, pairs, meta, out: Path):
    summ = {"meta": meta}
    main = sl[sl.engine != "A·J_SAFE"]
    rows = []
    for (eng, s, beta), g in main.groupby(["engine", "surface", "beta"], sort=False):
        ok = g[~g.no_route]
        rows.append({"estrategia": eng, "superficie": s, "beta": beta,
                     "pct_sin_ruta": 100 * float(g.no_route.mean()), **summarize(ok)})
    agg = pd.DataFrame(rows)
    agg.to_csv(out / "agregados.csv", index=False)
    summ["agregados"] = rows

    # estratos
    srows = []
    for key in ("estrato", "banda"):
        for (eng, s, beta, k), g in main.groupby(["engine", "surface", "beta", key], sort=False):
            ok = g[~g.no_route]
            srows.append({"por": key, "grupo": k, "estrategia": eng, "superficie": s, "beta": beta,
                          "pct_sin_ruta": 100 * float(g.no_route.mean()), **summarize(ok)})
    strat = pd.DataFrame(srows)
    strat.to_csv(out / "agregados_estrato.csv", index=False)

    # J SAFE
    js = sl[sl.engine == "A·J_SAFE"]
    jsafe = {"n": int(len(js)), "pct_elige_no_mas_corta": 100 * float(js.differs.mean()),
             "pct_excede_tope_desvio": 100 * float(js.excede_tope.mean()),
             "pct_coincide_con_regla_S8": 100 * float(js.coincide_regla_S8.mean()),
             "desvio_m_med_si_difiere": float(js[js.differs].detour_m.median()) if js.differs.any() else np.nan,
             "desvio_m_max": float(js.detour_m.max()),
             "red_S8_med_si_difiere_%": 100 * float(js[js.differs].red_sel.median()) if js.differs.any() else np.nan,
             "ratio_beta_riesgo_sobre_alpha_dist_mediana": float(js.beta_risk_sobre_alpha_dist.median())}
    summ["j_safe"] = jsafe

    # calidad del grafo: B0 vs ORS más corta
    a = rt[rt.route.str.match(r"^A\d$")].groupby("pair").len_m.min()
    aors = rt[rt.route.str.match(r"^A\d$")].groupby("pair").len_ors_m.min()
    b0 = rt[rt.route == "B_0"].set_index("pair").len_m
    gq = pd.DataFrame({"A_min": a, "A_min_ors": aors, "B0": b0}).dropna()
    gq["diff_pct"] = 100 * (gq.B0 / gq.A_min - 1)
    summ["calidad_grafo"] = {
        "n": int(len(gq)), "diff_pct_p5": float(gq.diff_pct.quantile(.05)),
        "diff_pct_p50": float(gq.diff_pct.median()), "diff_pct_p95": float(gq.diff_pct.quantile(.95)),
        "pct_abs_diff_<=5": 100 * float((gq.diff_pct.abs() <= 5).mean()),
        "pct_B0_mas_largo_>10": 100 * float((gq.diff_pct > 10).mean()),
        "pct_B0_mas_corto_>10": 100 * float((gq.diff_pct < -10).mean()),
        "len_geom_vs_len_ors_pct_p50": float((100 * (gq.A_min / gq.A_min_ors - 1)).median())}
    gq["As"] = rt[rt.route == "As"].set_index("pair").len_m
    gs = gq.dropna(subset=["As"])
    dB = 100 * (gs.B0 / gs.As - 1)
    dA = 100 * (gs.A_min / gs.As - 1)
    summ["calidad_grafo"]["vs_ors_shortest"] = {
        "n": int(len(gs)), "B0_vs_shortest_pct_p5": float(dB.quantile(.05)),
        "B0_vs_shortest_pct_p50": float(dB.median()), "B0_vs_shortest_pct_p95": float(dB.quantile(.95)),
        "pct_abs_diff_<=5": 100 * float((dB.abs() <= 5).mean()),
        "pct_B0_mas_corto_>5": 100 * float((dB < -5).mean()),
        "pct_B0_mas_largo_>5": 100 * float((dB > 5).mean()),
        "A_recommended_min_vs_shortest_pct_p50": float(dA.median()),
        "A_recommended_min_vs_shortest_pct_p95": float(dA.quantile(.95))}
    hwc = [c for c in rt.columns if c.startswith("hw_")]
    mix = rt[rt.route == "B_0"].set_index("pair")[hwc].fillna(0)
    mix = mix.div(mix.sum(axis=1), axis=0) * 100
    gq = gq.join(mix.round(1))
    grp = np.where(gq.diff_pct < -10, "B0_>10%_mas_corto", "resto")
    summ["calidad_grafo"]["tipo_via_B0_%_largo"] = gq[hwc].groupby(grp).mean().round(1) \
        .loc[:, lambda x: x.max() >= 2].to_dict("index")
    gq.round(1).to_csv(out / "calidad_grafo.csv")

    # alternativas de ORS
    na = rt[rt.route.str.match(r"^A\d$")].groupby("pair").size()
    summ["ors_alternativas"] = {"pares_con_respuesta": int(len(na)),
                                "dist_n_rutas": na.value_counts().sort_index().to_dict()}

    # latencias
    lat = {}
    la = lt[lt.engine == "A"].ms
    lat["A_ors_por_llamada_ms"] = {"n": int(len(la)), "p50": float(la.median()) if len(la) else np.nan,
                                   "p95": float(la.quantile(.95)) if len(la) else np.nan}
    lb = lt[lt.engine == "B"]
    lat["B_dijkstra_por_corrida_ms"] = {"n": int(len(lb)), "p50": float(lb.ms.median()),
                                        "p95": float(lb.ms.quantile(.95))}
    per_q = lb.groupby("pair").ms.sum()
    lat["B_consulta_multi_beta_un_superficie_ms(aprox)"] = {
        "p50": float((lb[lb.surface.isin(["S8"]) | lb.beta.eq(0)].groupby("pair").ms.sum()).median()),
        "p95": float((lb[lb.surface.isin(["S8"]) | lb.beta.eq(0)].groupby("pair").ms.sum()).quantile(.95))}
    lat["B_todas_las_corridas_del_par_ms"] = {"p50": float(per_q.median()), "p95": float(per_q.quantile(.95))}
    summ["latencias"] = lat

    # recintos (decisión 4)
    rec = {}
    ar = rt[rt.route.str.match(r"^A\d$")]
    rec["A_rutas_n"] = int(len(ar))
    rec["A_rutas_que_cruzan_%"] = 100 * float((ar.inside_m > RECINTO_MIN_M).mean())
    rec["A_pares_con_alguna_alt_que_cruza"] = int(ar[ar.inside_m > RECINTO_MIN_M].pair.nunique())
    rec["A_pares_todas_cruzan"] = int((ar.groupby("pair").inside_m.min() > RECINTO_MIN_M).sum())
    rec["A_metros_dentro_total"] = float(ar.inside_m.sum())
    rec["A_rutas_que_cruzan_detalle"] = ar[ar.inside_m > RECINTO_MIN_M][["pair", "route", "inside_m", "len_m"]] \
        .round(0).to_dict("records")
    for tag, lab in (("B_0", "B0"), ("Bx_0", "B0+recintos")):
        br = rt[rt.route == tag]
        rec[f"{lab}_pares_que_cruzan"] = int((br.inside_m > RECINTO_MIN_M).sum())
        rec[f"{lab}_metros_dentro_total"] = float(br.inside_m.sum())
    bb = rt[rt.route.str.match(r"^B_S")]
    bx = rt[rt.route.str.match(r"^Bx_S")]
    rec["B_beta_rutas_que_cruzan_%"] = 100 * float((bb.inside_m > RECINTO_MIN_M).mean())
    rec["Bx_beta_rutas_que_cruzan_%"] = 100 * float((bx.inside_m > RECINTO_MIN_M).mean())
    m = rt[rt.route.isin(["B_0", "Bx_0"])].pivot(index="pair", columns="route", values="len_m").dropna()
    rec["Bx0_vs_B0_alarga_pares"] = int(((m.Bx_0 - m.B_0) > 1).sum())
    rec["Bx0_vs_B0_alarga_m_max"] = float((m.Bx_0 - m.B_0).max())
    summ["recintos"] = rec

    # sensibilidad a β: la ruta β sin tope de desvío frente a B0
    b0r = rt[rt.route == "B_0"].set_index("pair")
    brows = []
    for s in SURFACES:
        oth = "S150" if s == "S8" else "S8"
        for beta in BETAS:
            rb = rt[rt.route == f"B_{s}_{beta:g}"].set_index("pair")
            j = rb.join(b0r, rsuffix="_0", how="inner")
            cap = np.minimum(j.len_m_0 * (1 + CAP_PCT), j.len_m_0 + CAP_MIN * WALK_M_PER_MIN)
            dif = (j.len_m - j.len_m_0) > 1
            red_s = 100 * (1 - j[f"E_{s}"] / j[f"E_{s}_0"])
            red_o = 100 * (1 - j[f"E_{oth}"] / j[f"E_{oth}_0"])
            row = {"superficie": s, "beta": beta, "pct_ruta_beta_distinta": 100 * float(dif.mean()),
                   "pct_excede_tope": 100 * float((j.len_m > cap + 1e-6).mean()),
                   "desvio_m_med": float((j.len_m - j.len_m_0)[dif].median()),
                   "desvio_pct_med": float((100 * (j.len_m / j.len_m_0 - 1))[dif].median()),
                   "red_sel_med_%": float(red_s[dif].median()), "red_oth_med_%": float(red_o[dif].median())}
            for s2 in SURFACES:
                if f"E_{s2}_test" in j:
                    row[f"red_test_{s2}_med_%"] = float((100 * (1 - j[f"E_{s2}_test"] / j[f"E_{s2}_test_0"]))[dif].median())
            brows.append(row)
    bsens = pd.DataFrame(brows)
    summ["sensibilidad_beta"] = brows

    # cobertura de datos por par (agregado)
    cov = rt[rt.route == "B_0"].set_index("pair")[["E_S8", "E_S150", "len_m"]]
    cov["S150_por_km"] = cov.E_S150 / (cov.len_m / 1000)
    cov["S8_por_km"] = cov.E_S8 / (cov.len_m / 1000)
    pdf = pd.DataFrame(pairs).set_index("id")
    cov = cov.join(pdf[["estrato", "banda"]])
    summ["cobertura_por_estrato"] = cov.groupby("estrato")[["S8_por_km", "S150_por_km"]].median().round(1).to_dict("index")
    summ["pares_por_estrato"] = pdf.estrato.value_counts().to_dict()
    summ["pares_por_banda"] = pdf.banda.value_counts().to_dict()

    (out / "resumen.json").write_text(json.dumps(summ, indent=1, ensure_ascii=False, default=float))

    # tablas markdown
    cols = ["estrategia", "superficie", "beta", "n", "pct_sin_ruta", "pct_T_ne_R", "pct_2a_tarjeta",
            "red_sel_media_todos_%", "red_oth_media_todos_%", "red_sel_med_dif_%", "red_oth_med_dif_%",
            "desvio_m_med_dif", "desvio_min_med_dif", "m_por_pp_med_dif", "solape_med_dif",
            "jaccard_h3_med_dif", "inside_m_T_medio"]
    nd = {"jaccard_h3_med_dif": 2, "solape_med_dif": 2}
    md = ["# Tablas — comparación de ruteo (wachi-06f.1)\n", "## Agregado por estrategia\n",
          md_table(agg, cols, nd), "\n## Segunda tarjeta y validación fuera de muestra\n"]
    cols2 = ["estrategia", "superficie", "beta", "pct_2a_tarjeta", "red_sel_med_2a_%", "red_oth_med_2a_%",
             "desvio_m_med_2a", "cero_R_%_medio_2a", "cero_T_%_medio_2a"]
    cols2 += [c for c in agg.columns if c.startswith(("red_test_", "pct_dif_con_red_test"))]
    md += [md_table(agg, cols2, nd), "\n## Sensibilidad a β (ruta β sin tope frente a B0)\n",
           md_table(bsens, list(bsens.columns)), "\n## Por estrato (estrategias principales)\n"]
    keep = strat[strat.beta.isin(["", "0.5", "1", "multi"]) & strat.estrategia.isin(["A", "B"])]
    scols = ["por", "grupo", "estrategia", "superficie", "beta", "n", "pct_sin_ruta", "pct_T_ne_R",
             "pct_2a_tarjeta", "red_sel_med_dif_%", "red_oth_med_dif_%", "desvio_m_med_dif"]
    scols += [c for c in strat.columns if c.startswith("red_test_") and c.endswith("med_dif_%")]
    md.append(md_table(keep, scols, nd))
    md.append("\n## Resto\n```json\n" + json.dumps({k: summ[k] for k in summ if k not in ("agregados",)},
                                                    indent=1, ensure_ascii=False, default=float) + "\n```\n")
    (out / "tablas.md").write_text("\n".join(md))
    log(f"salidas en {out}")


# ════════════════════════════════════════════════════════════════════════════
def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--raw", type=Path, default=REPO / "data/raw/LIMA.parquet",
                    help="denuncias SIDPOL (parquet; default data/raw/LIMA.parquet)")
    ap.add_argument("--pbf", type=Path, default=REPO / "data/raw/osm/peru-latest.osm.pbf",
                    help="extracto OSM Geofabrik (default data/raw/osm/peru-latest.osm.pbf)")
    ap.add_argument("--env", type=Path, default=REPO / ".env", help="archivo con CS_ORS_API_KEY")
    ap.add_argument("--pairs", type=Path, default=REPO / "config/experiments/od_pairs.yaml")
    ap.add_argument("--out", type=Path, default=None,
                    help="salida (default data/experiments/comparacion_ruteo[_holdout])")
    ap.add_argument("--cache", type=Path, default=REPO / "data/experiments/cache",
                    help="caché de la red OSM procesada y de respuestas ORS (subcarpeta ors/)")
    ap.add_argument("--holdout", action="store_true",
                    help=f"optimiza con eventos < {SPLIT} y evalúa además con eventos >= {SPLIT}")
    ap.add_argument("--ors-budget", type=int, default=120, help="máximo de llamadas ORS en vivo")
    ap.add_argument("--offline", action="store_true", help="solo caché ORS, sin llamadas en vivo")
    ap.add_argument("--gen-pairs", action="store_true", help="regenera --pairs y termina")
    ap.add_argument("--limit", type=int, default=0, help="solo los primeros N pares (prueba)")
    ap.add_argument("--geojson", action="store_true", help="escribe rutas.geojson (inspección)")
    args = ap.parse_args(argv)
    args.out = args.out or REPO / ("data/experiments/comparacion_ruteo" + ("_holdout" if args.holdout else ""))
    args.ors_cache = args.cache / "ors"
    np.random.seed(SEED)

    t0 = time.time()
    meta = {"pbf": str(args.pbf.name), "pbf_sha256": sha256(args.pbf),
            "raw": str(args.raw.name), "raw_sha256": sha256(args.raw),
            "bbox_SWNE": BBOX, "ventana": WINDOW, "pileup_max": PILE_MAX, "step_m": STEP_M,
            "radio_m": RADIUS_M, "h3_res": H3_RES, "betas": BETAS, "ors_alternativas": ORS_ALT,
            "semilla": SEED, "holdout": args.holdout, "split": SPLIT if args.holdout else None}
    log("sha256 pbf", meta["pbf_sha256"][:16], "raw", meta["raw_sha256"][:16])
    surfs = load_surfaces(args.raw)
    meta["eventos"] = {k: v.stats for k, v in surfs.items()}
    surf, test = (surfs["train"], surfs["test"]) if args.holdout else (surfs["full"], None)
    net = parse_osm(args.pbf, args.cache / f"red_v2_{meta['pbf_sha256'][:12]}.npz")
    meta["red"] = net.stats
    router = Router(net, surf)
    meta["superficie_en_red"] = router.net_stats
    log("superficie en red", router.net_stats)
    if args.gen_pairs:
        gen_pairs(router, args.pairs, meta["pbf_sha256"])
        return
    run(args, router, surf, meta, test)
    log(f"total {time.time() - t0:.0f}s")


if __name__ == "__main__":
    main()
