"""Red peatonal de Lima a partir de OSM (ADR 013, 014)."""
from __future__ import annotations

import hashlib
import itertools
import json
import logging
import re
import time
from collections.abc import Mapping
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
import shapely
from scipy.sparse import csr_matrix
from scipy.sparse.csgraph import connected_components

from wachi.config import Settings
from wachi.geo import TO_UTM

# osmnx "walk" + busway/bus_stop (carril del Metropolitano y paraderos mapeados como vía)
_EXCL_HW = re.compile(r"abandoned|bus_guideway|busway|bus_stop|construction|cycleway|motor|no|planned|"
                      r"platform|proposed|raceway|razed")
_SIDEWALK_KEYS = ("sidewalk", "sidewalk:both", "sidewalk:left", "sidewalk:right")
_FOOT_OK = ("yes", "designated", "permissive")

log = logging.getLogger(__name__)

# Subir cuando cambie cómo se construye la red (filtros, recintos, formato): las cachés viejas se rehacen solas.
CACHE_VERSION = 2

Way = tuple[np.ndarray, np.ndarray, np.ndarray, str]
Poly = tuple[str, str, shapely.Geometry]


def walk_ok(t: Mapping[str, str]) -> bool:
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
    u: np.ndarray           # nodo inicial de cada arista (0..N-1)
    v: np.ndarray
    length: np.ndarray      # m
    off: np.ndarray         # offsets en xy (len E+1)
    xy: np.ndarray          # UTM de todas las geometrías, concatenadas
    nxy: np.ndarray         # UTM de nodos
    polys: list[Poly]       # recintos (kind, name, geom UTM)
    hw: np.ndarray          # highway=* de cada arista
    stats: dict = field(default_factory=dict)

    def edge_xy(self, e: int, reverse: bool = False) -> np.ndarray:
        c = self.xy[self.off[e]:self.off[e + 1]]
        return c[::-1] if reverse else c


def read_osm(pbf: Path, bbox, recinto_tags, counts: dict | None = None) -> tuple[list[Way], list[Poly]]:
    """Si se pasa ``counts``, se llena con ``recintos_descartados`` (geometrías que no se pudieron construir)."""
    import osmium

    S, W, N, E = bbox
    tags = [tuple(t) for t in recinto_tags]
    wkbf = osmium.geom.WKBFactory()
    fp = (osmium.FileProcessor(str(pbf)).with_locations()
          .with_areas(osmium.filter.TagFilter(*tags))
          .with_filter(osmium.filter.KeyFilter("highway", "amenity", "landuse")))
    ways: list[Way] = []
    polys: list[Poly] = []
    skipped = 0

    def inb(n):
        return n.location.valid() and S <= n.lat <= N and W <= n.lon <= E

    for o in fp:
        if o.is_way():
            if "highway" not in o.tags:
                continue
            ns = o.nodes
            if not (inb(ns[0]) or inb(ns[len(ns) - 1])):
                continue
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
            kind = next((f"{k}={v}" for k, v in tags if o.tags.get(k) == v), None)
            if kind is None:
                continue
            try:
                g = shapely.from_wkb(wkbf.create_multipolygon(o))
            except (RuntimeError, ValueError, shapely.errors.GEOSException):
                skipped += 1
                continue
            b = g.bounds
            if b[2] < W or b[0] > E or b[3] < S or b[1] > N:
                continue
            polys.append((kind, o.tags.get("name", ""),
                          shapely.transform(g, lambda c: np.c_[TO_UTM.transform(c[:, 0], c[:, 1])])))
    if skipped:
        log.warning("read_osm: %d recintos descartados por geometría inválida", skipped)
    if not polys:
        log.warning("read_osm: 0 recintos leídos de %s; revisar recinto_tags o la API de pyosmium", pbf)
    if counts is not None:
        counts["recintos_descartados"] = skipped
    return ways, polys


def build_network(ways: list[Way], polys: list[Poly]) -> Network:
    """Corta las vías en nodos compartidos, proyecta a UTM y se queda con la componente mayor."""
    all_ids = np.concatenate([w[0] for w in ways])
    uniq, cnt = np.unique(all_ids, return_counts=True)
    multi = set(uniq[cnt >= 2].tolist())
    eu, ev, ehw, coords, off = [], [], [], [], [0]
    for ids, lo, la, hw in ways:
        cut = [0] + [i for i in range(1, len(ids) - 1) if int(ids[i]) in multi] + [len(ids) - 1]
        for a, b in itertools.pairwise(cut):
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
    eu, ev = np.array(eu), np.array(ev)
    keep = (length > 0.5) & (eu != ev)
    node_ids, inv = np.unique(np.r_[eu, ev], return_inverse=True)
    ui, vi = inv[:len(eu)], inv[len(eu):]
    nxy = np.zeros((len(node_ids), 2))
    nxy[ui] = xy[off[:-1]]
    nxy[vi] = xy[off[1:] - 1]
    A = csr_matrix((np.ones(keep.sum()), (ui[keep], vi[keep])), shape=(len(node_ids),) * 2)
    _, lab = connected_components(A, directed=False)
    big = np.bincount(lab).argmax()
    keep &= lab[ui] == big
    old = np.flatnonzero(lab == big)
    remap = -np.ones(len(node_ids), np.int64)
    remap[old] = np.arange(len(old))
    sel = np.flatnonzero(keep)
    new_off, pieces = [0], []
    for e in sel:
        pieces.append(xy[off[e]:off[e + 1]])
        new_off.append(new_off[-1] + off[e + 1] - off[e])
    net = Network(u=remap[ui[sel]], v=remap[vi[sel]], length=length[sel],
                  off=np.array(new_off, np.int64), xy=np.concatenate(pieces), nxy=nxy[old],
                  polys=polys, hw=np.array(ehw)[sel])
    kinds: dict[str, int] = {}
    for k, _, _ in polys:
        kinds[k] = kinds.get(k, 0) + 1
    net.stats = {"vias_peatonales": len(ways), "nodos": len(old), "aristas": len(sel),
                 "km_red": float(net.length.sum() / 1000), "recintos": len(polys), "recintos_por_tipo": kinds}
    return net


def save_cache(net: Network, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    np.savez(path, u=net.u, v=net.v, length=net.length, off=net.off, xy=net.xy, nxy=net.nxy, hw=net.hw,
             polys=np.array([(k, n, shapely.to_wkb(g)) for k, n, g in net.polys], dtype=object),
             stats=json.dumps(net.stats))


def load_cache(path: Path) -> Network:
    z = np.load(path, allow_pickle=True)
    polys = [(str(k), str(n), shapely.from_wkb(g)) for k, n, g in z["polys"]]
    return Network(z["u"], z["v"], z["length"], z["off"], z["xy"], z["nxy"], polys, z["hw"],
                   json.loads(str(z["stats"])))


def file_sha256(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def stale_reason(stats: dict, cfg: Settings) -> str | None:
    """Por qué una caché no sirve para esta configuración, o None si sirve."""
    if stats.get("cache_version") != CACHE_VERSION:
        return f"cache_version {stats.get('cache_version')} ≠ {CACHE_VERSION}"
    if cfg.pbf_sha256 and stats.get("pbf_sha256") != cfg.pbf_sha256:
        return f"pbf_sha256 de la caché {str(stats.get('pbf_sha256'))[:12]}… ≠ fijado {cfg.pbf_sha256[:12]}…"
    return None


def load_or_build(cfg: Settings) -> Network:
    if cfg.graph_cache.exists():
        net = load_cache(cfg.graph_cache)
        why = stale_reason(net.stats, cfg)
        if why is None:
            return net
        log.warning("reconstruyendo %s: %s", cfg.graph_cache, why)
    t0 = time.time()
    digest = file_sha256(cfg.pbf_path)
    if cfg.pbf_sha256 and digest != cfg.pbf_sha256:
        raise ValueError(f"sha256 de {cfg.pbf_path} no coincide: {digest[:12]}… ≠ {cfg.pbf_sha256[:12]}…")
    counts: dict = {}
    ways, polys = read_osm(cfg.pbf_path, cfg.bbox, cfg.recinto_tags, counts)
    net = build_network(ways, polys)
    net.stats.update(counts)
    net.stats["pbf_sha256"] = digest
    net.stats["cache_version"] = CACHE_VERSION
    net.stats["segundos_construccion"] = round(time.time() - t0)
    save_cache(net, cfg.graph_cache)
    return net
