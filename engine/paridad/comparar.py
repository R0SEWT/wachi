"""Compara las respuestas de dos motores (salidas de referencia.py / `wachi batch`).

    python comparar.py ref_python.jsonl ref_rust.jsonl [--detalle]

Una respuesta es *idéntica* si el JSON coincide campo a campo. Si no, se clasifica la
diferencia: geometría (máxima separación en grados), tarjetas (presencia de la Tranquila) o
cifras (distancia, desvío, % de reducción, cuadras, tramos con pocos datos).
"""
import json
import sys
from collections import Counter


def cargar(p):
    return {r["id"]: r for r in map(json.loads, open(p))}


def max_sep(a, b):
    if len(a) != len(b):
        return None
    return max((abs(x[0] - y[0]) + abs(x[1] - y[1]) for x, y in zip(a, b)), default=0.0)


def main(pa, pb, detalle=False):
    A, B = cargar(pa), cargar(pb)
    tipos = Counter()
    ejemplos = {}
    for k in A:
        a, b = A[k], B.get(k)
        if b is None:
            tipos["falta"] += 1
            continue
        if "ok" not in a or "ok" not in b:
            t = "idéntica" if (a.get("error"), a.get("which")) == (b.get("error"), b.get("which")) else "error_distinto"
            tipos[t] += 1
            ejemplos.setdefault(t, (k, a.get("error"), b.get("error")))
            continue
        ra, rb = a["ok"], b["ok"]
        if ra == rb:
            tipos["idéntica"] += 1
            continue
        diffs = []
        for card in ("fast", "calm"):
            ca, cb = ra[card], rb[card]
            if (ca is None) != (cb is None):
                diffs.append(f"{card}:presencia")
                continue
            if ca is None:
                continue
            for f in ca:
                if f == "geometry":
                    s = max_sep(ca[f], cb[f])
                    if s is None:
                        diffs.append(f"{card}:geometría_distinta")
                    elif s > 0:
                        diffs.append(f"{card}:geometría_±{s:.0e}°")
                elif ca[f] != cb.get(f):
                    diffs.append(f"{card}:{f}")
        for f in ("data_as_of", "params"):
            if ra[f] != rb[f]:
                diffs.append(f)
        t = ",".join(sorted(set(diffs))) or "orden_de_claves"
        tipos[t] += 1
        ejemplos.setdefault(t, (k, [(c, ra[c] and {x: ra[c][x] for x in ra[c] if x != "geometry"},
                                     rb[c] and {x: rb[c][x] for x in rb[c] if x != "geometry"}) for c in ("fast", "calm")]))
    n = len(A)
    print(f"{n} pares · idénticas {tipos['idéntica']} ({100 * tipos['idéntica'] / n:.1f} %)")
    for t, c in tipos.most_common():
        if t != "idéntica":
            print(f"  {c:4d}  {t}")
            if detalle:
                print("        ej.", ejemplos[t])


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], "--detalle" in sys.argv)
