"""Motor propio: Dijkstra sobre la red peatonal con costo largo·(1+β·r̄) (ADR 014)."""
from __future__ import annotations

import itertools
import math
import threading
from dataclasses import dataclass

import numpy as np
import pandas as pd
import shapely
from scipy.sparse import csr_matrix
from scipy.sparse.csgraph import dijkstra
from scipy.spatial import cKDTree

from wachi.config import Settings
from wachi.geo import TO_LL, TO_UTM
from wachi.graph import Network
from wachi.surface import Surface

CSR_CACHE_MAX = 8   # cada grafo ponderado ocupa decenas de MB con la red de Lima


@dataclass
class Route:
    nodes: list[int]
    edges: list[int]
    xy: np.ndarray
    length_m: float


@dataclass
class Measure:
    length_m: float
    exposure: float
    cells: list[str]


class Router:
    def __init__(self, net: Network, surf: Surface, cfg: Settings):
        self.net, self.surf, self.cfg = net, surf, cfg
        E = len(net.u)
        lines = shapely.linestrings(net.xy, indices=np.repeat(np.arange(E), np.diff(net.off)))
        n = np.maximum(1, np.round(net.length / cfg.step_m)).astype(int)
        rep = np.repeat(np.arange(E), n)
        k = np.arange(len(rep)) - np.repeat(np.cumsum(n) - n, n)
        pos = (k + 0.5) * net.length[rep] / n[rep]
        pts = shapely.line_interpolate_point(lines[rep], pos)
        vals = surf.at(shapely.get_x(pts), shapely.get_y(pts))["S150"]
        p75 = float(np.percentile(vals, 75))
        self.p75 = p75 if p75 > 0 else 1.0   # sin exposición en el p75: r̄ = S150 medio, costos finitos
        self.rbar = np.bincount(rep, vals, E) / n / self.p75
        # recintos: polígono dominante de cada arista (≥ 50 % de sus muestras dentro)
        self.shrunk = [g.buffer(-cfg.recinto_shrink_m) for _, _, g in net.polys]
        self.ptree = shapely.STRtree(self.shrunk)
        self.otree = shapely.STRtree([g for _, _, g in net.polys])
        self.edge_poly = -np.ones(E, np.int64)
        if net.polys:
            pi, gi = self.ptree.query(pts, predicate="within")
            first = pd.Series(gi, index=pi).groupby(level=0).first()
            samp_poly = -np.ones(len(rep), np.int64)
            samp_poly[first.index.values] = first.values
            df = pd.DataFrame({"e": rep, "p": samp_poly})
            inside = df[df.p >= 0].groupby(["e", "p"]).size().rename("k").reset_index()
            inside = inside.sort_values("k").drop_duplicates("e", keep="last")
            ok = inside.k.values >= 0.5 * n[inside.e.values]
            self.edge_poly[inside.e.values[ok]] = inside.p.values[ok]
        self.node_tree = cKDTree(net.nxy)
        self._csr: dict = {}
        self._lock = threading.Lock()

    def graph(self, beta: float, blocked: frozenset | None):
        key = (beta, blocked)
        with self._lock:   # la API llama desde varios hilos: búsqueda, construcción y expulsión van juntas
            hit = self._csr.get(key)
            if hit is not None:
                return hit
            net = self.net
            w = net.length * (1 + beta * self.rbar) if beta else net.length.copy()
            w = np.maximum(w, 1e-3)
            mask = np.ones(len(w), bool)
            if blocked:
                mask &= ~np.isin(self.edge_poly, list(blocked))
            e = np.flatnonzero(mask)
            uu, vv, ww, ee = np.r_[net.u[e], net.v[e]], np.r_[net.v[e], net.u[e]], np.r_[w[e], w[e]], np.r_[e, e]
            o = np.lexsort((ww, vv, uu))
            uu, vv, ww, ee = uu[o], vv[o], ww[o], ee[o]
            first = np.r_[True, (uu[1:] != uu[:-1]) | (vv[1:] != vv[:-1])]
            uu, vv, ww, ee = uu[first], vv[first], ww[first], ee[first]
            N = len(net.nxy)
            G = csr_matrix((ww, (uu, vv)), shape=(N, N))
            G.sort_indices()
            if len(self._csr) >= CSR_CACHE_MAX:
                self._csr.pop(next(iter(self._csr)))
            self._csr[key] = (G, ee)
            return G, ee

    def blocked_for(self, allowed: set[int]) -> frozenset:
        return frozenset(set(range(len(self.net.polys))) - allowed)

    def route(self, src: int, dst: int, beta: float, blocked: frozenset | None) -> Route | None:
        if src == dst:
            return Route([src], [], self.net.nxy[[src, src]].copy(), 0.0)
        G, ee = self.graph(beta, blocked)
        crow = float(np.hypot(*(self.net.nxy[src] - self.net.nxy[dst])))
        lim = 1.5 * crow + 1000
        while True:
            dist, pred = dijkstra(G, directed=True, indices=src, return_predecessors=True, limit=lim)
            if np.isfinite(dist[dst]) or lim == np.inf:
                break
            lim = lim * 2 if lim <= 2e5 else np.inf   # último intento sin tope: el costo ponderado puede superar cualquier límite
        if not np.isfinite(dist[dst]):
            return None
        nodes = [dst]
        while nodes[-1] != src:
            nodes.append(int(pred[nodes[-1]]))
        nodes = nodes[::-1]
        edges, pieces = [], []
        for a, b in itertools.pairwise(nodes):
            lo, hi = G.indptr[a], G.indptr[a + 1]
            pos = lo + np.searchsorted(G.indices[lo:hi], b)
            e = int(ee[pos])
            edges.append(e)
            c = self.net.edge_xy(e, reverse=self.net.u[e] != a)
            pieces.append(c if not pieces else c[1:])
        return Route(nodes, edges, np.concatenate(pieces), float(self.net.length[edges].sum()))

    def measure(self, route: Route) -> Measure:
        vx, vy = route.xy[:, 0], route.xy[:, 1]
        cells = self.surf.at(vx, vy)["cells"]
        if route.length_m == 0:
            return Measure(0.0, 0.0, cells)
        line = shapely.linestrings(route.xy)
        L = float(shapely.length(line))
        n = max(1, math.ceil(L / self.cfg.step_m))
        starts = np.arange(n) * self.cfg.step_m
        seg = np.minimum(self.cfg.step_m, L - starts)
        pts = shapely.line_interpolate_point(line, starts + seg / 2)
        s150 = self.surf.at(shapely.get_x(pts), shapely.get_y(pts))["S150"]
        return Measure(L, float((s150 * seg / 1000).sum()), cells)

    def snap(self, lat: float, lon: float) -> tuple[int, float]:
        x, y = TO_UTM.transform(lon, lat)
        d, i = self.node_tree.query([x, y])
        return int(i), float(d)

    def node_ll(self, i: int) -> tuple[float, float]:
        lon, lat = TO_LL.transform(*self.net.nxy[i])
        return round(float(lat), 6), round(float(lon), 6)

    def allowed_polys(self, *nodes: int) -> set[int]:
        if not self.net.polys:
            return set()
        pts = shapely.points(self.net.nxy[list(nodes)])
        _, gi = self.otree.query(pts, predicate="within")
        return set(gi.tolist())
