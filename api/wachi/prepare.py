"""Construye la caché del grafo y muestra los sha256 para fijar en .env."""
import hashlib
import sys
from pathlib import Path

from wachi.config import Settings
from wachi.graph import load_or_build


def sha(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    cfg = Settings()
    for p in (cfg.pbf_path, cfg.events_path):
        if not p.exists():
            sys.exit(f"falta {p}")
    net = load_or_build(cfg)
    print("red:", net.stats)
    print(f"WACHI_PBF_SHA256={sha(cfg.pbf_path)}")
    print(f"WACHI_EVENTS_SHA256={sha(cfg.events_path)}")


if __name__ == "__main__":
    main()
