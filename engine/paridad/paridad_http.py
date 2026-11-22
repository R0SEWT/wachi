"""Paridad HTTP byte a byte y carga: dos servidores con los mismos datos.

    uv run python ../engine/paridad/paridad_http.py http://127.0.0.1:8100 http://127.0.0.1:8101 data/pares.jsonl

Compara estado + cuerpo (bytes) de /health, de cada par de /route y de cuerpos inválidos, y los
encabezados CORS. Después mide latencia con 16 clientes concurrentes contra cada servidor.
"""
import asyncio
import json
import statistics
import sys
import time

import httpx

RAROS = [
    '{"origin": {"lat": NaN, "lon": -77.03}, "destination": {"lat": -12.08, "lon": -77.03}}',
    '{"origin": {"lat": -12.08, "lon": Infinity}, "destination": {"lat": -12.08, "lon": -77.03}}',
    '{"origin": {"lat": 1}}',
    '{no es json',
    '',
    '[]',
    '{"origin": {"lat": 91, "lon": -77.03}, "destination": {"lat": -12.08, "lon": -77.03}}',
    '{"origin": {"lat": "-12.05", "lon": "-77.04"}, "destination": {"lat": -12.08, "lon": -77.03}}',
    '{"origin": {"lat": -12.05, "lon": -77.04, "extra": 1}, "destination": {"lat": -12.08, "lon": -77.03}, "x": 2}',
]


async def comparar(a: str, b: str, pares: list[dict]) -> int:
    malos = 0
    async with httpx.AsyncClient(timeout=60) as c:
        async def par(method, path, **kw):
            ra, rb = await c.request(method, a + path, **kw), await c.request(method, b + path, **kw)
            return (ra.status_code, ra.content), (rb.status_code, rb.content)

        x, y = await par("GET", "/health")
        if x != y:
            malos += 1
            print("health", x, y)
        for p in pares:
            body = {"origin": p["origin"], "destination": p["destination"]}
            x, y = await par("POST", "/route", json=body)
            if x != y:
                malos += 1
                if malos < 5:
                    print("route", p["id"], x[0], y[0], x[1][:200], y[1][:200])
        for raw in RAROS:
            x, y = await par("POST", "/route", content=raw, headers={"content-type": "application/json"})
            if x != y:
                malos += 1
                print("raro", repr(raw[:50]), x, y)
        for path in ("/docs", "/openapi.json", "/nada"):
            x, y = await par("GET", path)
            if x[0] != y[0]:
                malos += 1
                print("404", path, x[0], y[0])
        h = {"origin": "http://localhost:8080", "access-control-request-method": "POST",
             "access-control-request-headers": "content-type"}
        ra, rb = await c.options(a + "/route", headers=h), await c.options(b + "/route", headers=h)
        ka = {k: v for k, v in ra.headers.items() if k.startswith("access-control-allow-")}
        kb = {k: v for k, v in rb.headers.items() if k.startswith("access-control-allow-")}
        print("CORS preflight:", ra.status_code, rb.status_code, "| py", ka, "| rs", kb)
    return malos


async def carga(url: str, pares: list[dict], clientes: int = 16, vueltas: int = 2) -> dict:
    cola = asyncio.Queue()
    for _ in range(vueltas):
        for p in pares:
            cola.put_nowait({"origin": p["origin"], "destination": p["destination"]})
    lat, estados = [], {}
    async with httpx.AsyncClient(timeout=60, limits=httpx.Limits(max_connections=clientes)) as c:
        async def w():
            while not cola.empty():
                body = cola.get_nowait()
                t = time.perf_counter()
                r = await c.post(url + "/route", json=body)
                lat.append((time.perf_counter() - t) * 1000)
                estados[r.status_code] = estados.get(r.status_code, 0) + 1
        t0 = time.perf_counter()
        await asyncio.gather(*[w() for _ in range(clientes)])
        dur = time.perf_counter() - t0
    lat.sort()
    return {"rps": round(len(lat) / dur, 1), "ms_p50": round(statistics.median(lat), 1),
            "ms_p95": round(lat[int(0.95 * len(lat))], 1), "estados": estados}


async def main(a, b, pares_path):
    pares = [json.loads(l) for l in open(pares_path)]
    malos = await comparar(a, b, pares)
    print(f"paridad HTTP: {len(pares) + len(RAROS) + 4} comparaciones, {malos} distintas")
    for nombre, url in (("python", a), ("rust", b)):
        print(nombre, await carga(url, pares))


if __name__ == "__main__":
    asyncio.run(main(*sys.argv[1:4]))
