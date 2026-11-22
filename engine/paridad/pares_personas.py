"""Pares origen-destino como los pediría una persona: un punto al azar sobre la red (ponderado por
largo) desplazado 0-25 m hacia un lado (vereda, puerta), con destino a 0,5-5 km.

    uv run python ../engine/paridad/pares_personas.py data/network.npz SALIDA.jsonl [N] [SEMILLA]
"""
import json
import sys

import numpy as np
import shapely

from wachi.geo import TO_LL


def main(net_path: str, out: str, n: int = 2000, seed: int = 31) -> None:
    z = np.load(net_path, allow_pickle=True)
    xy, off, length = z["xy"], z["off"], z["length"]
    E = len(length)
    lines = shapely.linestrings(xy, indices=np.repeat(np.arange(E), np.diff(off)))
    rng = np.random.default_rng(seed)

    def persona(k):
        e = rng.choice(E, k, p=length / length.sum())
        p = shapely.line_interpolate_point(lines[e], rng.uniform(0, 1, k) * length[e])
        a, r = rng.uniform(0, 2 * np.pi, k), rng.uniform(0, 25, k)
        return np.c_[shapely.get_x(p) + r * np.cos(a), shapely.get_y(p) + r * np.sin(a)]

    o = persona(n)
    ang, d = rng.uniform(0, 2 * np.pi, n), rng.uniform(500, 5000, n)
    dest_aprox = o + np.c_[d * np.cos(ang), d * np.sin(ang)]
    # el destino también es una persona: el punto de la red más cercano al destino aproximado
    cand = persona(20 * n)
    from scipy.spatial import cKDTree
    _, i = cKDTree(cand).query(dest_aprox)
    dst = cand[i]
    lo1, la1 = TO_LL.transform(o[:, 0], o[:, 1])
    lo2, la2 = TO_LL.transform(dst[:, 0], dst[:, 1])
    with open(out, "w") as f:
        for k in range(n):
            f.write(json.dumps({"id": f"p{k:05d}",
                                "origin": {"lat": round(float(la1[k]), 6), "lon": round(float(lo1[k]), 6)},
                                "destination": {"lat": round(float(la2[k]), 6), "lon": round(float(lo2[k]), 6)}}) + "\n")
    print(f"{n} pares → {out}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], *(int(a) for a in sys.argv[3:5]))
