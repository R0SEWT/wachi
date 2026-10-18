"""Superficie de robos registrados a ≤ 150 m (ADR 016) y cobertura por celda H3 (ADR 010)."""
from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import Path

import h3
import numpy as np
import pandas as pd
from scipy.spatial import cKDTree

from wachi.config import Settings
from wachi.geo import TO_LL, TO_UTM


def load_events(path: Path, sha256: str = "") -> pd.DataFrame:
    if sha256:
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest != sha256:
            raise ValueError(f"sha256 de {path} no coincide: {digest[:12]}… ≠ {sha256[:12]}…")
    df = pd.read_parquet(path, columns=["lat", "lon", "fecha"])
    df = df.dropna(subset=["lat", "lon"]).reset_index(drop=True)
    df["fecha"] = pd.to_datetime(df["fecha"])
    return df


@dataclass
class Surface:
    tree: cKDTree
    s8: dict[str, int]
    data_as_of: str
    n_events: int
    low_threshold: float
    radius_m: float
    h3_res: int

    def at(self, x: np.ndarray, y: np.ndarray) -> dict:
        lon, lat = TO_LL.transform(x, y)
        cells = [h3.latlng_to_cell(a, b, self.h3_res) for a, b in zip(lat, lon)]
        s8 = np.fromiter((self.s8.get(c, 0) for c in cells), float, len(cells))
        s150 = self.tree.query_ball_point(np.c_[x, y], r=self.radius_m, return_length=True, workers=-1)
        return {"S8": s8, "S150": np.asarray(s150, float), "cells": cells}

    def is_low(self, cells: list[str]) -> np.ndarray:
        return np.fromiter((self.s8.get(c, 0) < self.low_threshold for c in cells), bool, len(cells))


def build_surface(df: pd.DataFrame, cfg: Settings) -> Surface:
    x, y = TO_UTM.transform(df.lon.values, df.lat.values)
    cells = pd.Series([h3.latlng_to_cell(a, b, cfg.h3_res) for a, b in zip(df.lat, df.lon)])
    s8 = {k: int(v) for k, v in cells.value_counts().items()}
    counts = np.array(list(s8.values()), float)
    low = float(np.percentile(counts, cfg.low_data_pct)) if len(counts) else 1.0
    low = max(low, 1.0)   # una celda sin ningún evento siempre es "pocos datos"
    data_as_of = pd.to_datetime(df.fecha).max().date().isoformat() if len(df) else ""
    return Surface(tree=cKDTree(np.c_[x, y]), s8=s8, data_as_of=data_as_of, n_events=len(df),
                   low_threshold=low, radius_m=cfg.radius_m, h3_res=cfg.h3_res)
