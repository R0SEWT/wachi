"""Pares origen-destino fijos para paridad y benchmark: los 51 de docs/experiments + aleatorios."""
import json
import sys
from pathlib import Path

import numpy as np
import yaml

from wachi.geo import TO_LL

RAIZ = Path(__file__).resolve().parents[2]


def main(net_path: str, out: str, n_rand: int = 450, seed: int = 20261122) -> None:
    """n_rand y seed opcionales por línea de comandos: pares.py RED SALIDA [N] [SEMILLA]."""
    pares = []
    for p in yaml.safe_load((RAIZ / "docs/experiments/od_pairs.yaml").read_text())["pares"]:
        pares.append({"id": p["id"], "origin": p["origen"], "destination": p["destino"]})
    rng = np.random.default_rng(seed)
    nxy = np.load(net_path, allow_pickle=True)["nxy"]
    while len(pares) < 51 + n_rand:
        i = rng.integers(len(nxy))
        ang, d = rng.uniform(0, 2 * np.pi), rng.uniform(300, 6000)
        a = nxy[i] + rng.normal(0, 30, 2)
        b = a + d * np.array([np.cos(ang), np.sin(ang)])
        (lo1, lo2), (la1, la2) = TO_LL.transform([a[0], b[0]], [a[1], b[1]])
        pares.append({"id": f"r{len(pares):04d}", "origin": {"lat": round(float(la1), 6), "lon": round(float(lo1), 6)},
                      "destination": {"lat": round(float(la2), 6), "lon": round(float(lo2), 6)}})
    # bordes: mismo punto, fuera del bbox, lejos de una vía (mar)
    pares += [
        {"id": "borde_mismo", "origin": {"lat": -12.0464, "lon": -77.0428}, "destination": {"lat": -12.0464, "lon": -77.0428}},
        {"id": "borde_fuera", "origin": {"lat": -13.5, "lon": -76.0}, "destination": {"lat": -12.0464, "lon": -77.0428}},
        {"id": "borde_mar", "origin": {"lat": -12.0464, "lon": -77.0428}, "destination": {"lat": -12.10, "lon": -77.12}},
    ]
    Path(out).write_text("\n".join(json.dumps(p) for p in pares) + "\n")
    print(f"{len(pares)} pares → {out}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], *(int(a) for a in sys.argv[3:5]))
