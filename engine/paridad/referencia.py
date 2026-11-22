"""Corre el motor Python sobre los pares y guarda respuestas + tiempos (la referencia de paridad).

    uv run python ../engine/paridad/referencia.py pares.jsonl salida.jsonl
"""
import json
import resource
import sys
import time

from wachi.config import Settings
from wachi.engine import RouteError, load_engine


def main(pares: str, out: str) -> None:
    t0 = time.time()
    eng = load_engine(Settings(_env_file=None))
    carga = time.time() - t0
    filas, tiempos = [], []
    for line in open(pares):
        p = json.loads(line)
        o, d = (p["origin"]["lat"], p["origin"]["lon"]), (p["destination"]["lat"], p["destination"]["lon"])
        t = time.perf_counter()
        try:
            r = {"ok": eng.route(o, d)}
        except RouteError as e:
            r = {"error": e.code, "which": e.which}
        ms = (time.perf_counter() - t) * 1000
        tiempos.append(ms)
        filas.append(json.dumps({"id": p["id"], "ms": round(ms, 2), **r}))
    open(out, "w").write("\n".join(filas) + "\n")
    import numpy as np
    t = np.array(tiempos)
    print(json.dumps({"motor": "python", "carga_s": round(carga, 1), "pares": len(t),
                      "ms_p50": round(float(np.percentile(t, 50)), 1), "ms_p95": round(float(np.percentile(t, 95)), 1),
                      "ms_max": round(float(t.max()), 1),
                      "rss_max_mb": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss // 1024}))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
