"""Construye la caché del grafo y muestra los sha256 para fijar en .env."""
import sys

from wachi.config import Settings
from wachi.graph import file_sha256, load_or_build


def main():
    cfg = Settings()
    for p in (cfg.pbf_path, cfg.events_path):
        if not p.exists():
            sys.exit(f"falta {p}")
    # Los sha van primero: si el pbf cambió y el .env tiene el sha viejo, load_or_build se niega a construir.
    print(f"WACHI_PBF_SHA256={file_sha256(cfg.pbf_path)}")
    print(f"WACHI_EVENTS_SHA256={file_sha256(cfg.events_path)}")
    net = load_or_build(cfg)
    print("red:", net.stats)


if __name__ == "__main__":
    main()
