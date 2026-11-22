"""Eventos sintéticos para probar el motor sin datos de SIDPOL.

No son denuncias: imitan su forma (focos, ruido sobre la red, acumulaciones en una
misma coordenada) para que los dos motores reciban la misma entrada. Determinista por semilla.

    uv run python ../engine/paridad/eventos_sinteticos.py data/network.npz data/eventos.parquet
"""
import sys

import numpy as np
import pandas as pd

from wachi.geo import TO_LL


def main(net_path: str, out: str, n: int = 250_000, seed: int = 7) -> None:
    rng = np.random.default_rng(seed)
    nxy = np.load(net_path, allow_pickle=True)["nxy"]
    focos = nxy[rng.choice(len(nxy), 900, replace=False)]
    peso = rng.pareto(1.2, len(focos)) + 1
    peso /= peso.sum()
    k_foco, k_red = int(n * 0.6), int(n * 0.3)
    k_pila = n - k_foco - k_red
    f = rng.choice(len(focos), k_foco, p=peso)
    sigma = rng.uniform(40, 220, len(focos))[f]
    a = focos[f] + rng.normal(0, 1, (k_foco, 2)) * sigma[:, None]
    b = nxy[rng.choice(len(nxy), k_red)] + rng.normal(0, 8, (k_red, 2))
    pilas = nxy[rng.choice(len(nxy), max(1, k_pila // 20))]
    c = pilas[rng.integers(0, len(pilas), k_pila)]
    xy = np.r_[a, b, c]
    lon, lat = TO_LL.transform(xy[:, 0], xy[:, 1])
    t0 = np.datetime64("2023-06-30")
    fecha = t0 + rng.integers(0, 731, len(xy)).astype("timedelta64[D]")
    df = pd.DataFrame({"lat": lat, "lon": lon, "fecha": pd.to_datetime(fecha)})
    df.to_parquet(out, index=False)
    print(f"{len(df)} eventos sintéticos → {out}; data_as_of={df.fecha.max().date()}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], *(int(a) for a in sys.argv[3:5]))
