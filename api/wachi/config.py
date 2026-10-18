from pathlib import Path

from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="WACHI_", env_file=".env", extra="ignore")

    pbf_path: Path = Path("data/peru-latest.osm.pbf")
    pbf_sha256: str = ""
    events_path: Path = Path("data/eventos.parquet")
    events_sha256: str = ""
    graph_cache: Path = Path("data/network.npz")

    bbox: tuple[float, float, float, float] = (-12.28, -77.13, -11.87, -76.85)  # S, W, N, E
    radius_m: float = 150.0
    step_m: float = 20.0
    h3_res: int = 8
    walk_kmh: float = 5.0
    betas: list[float] = [0.5]
    cap_pct: float = 0.15
    cap_min: float = 5.0
    gain_min: float = 0.20
    block_min_events: float = 1.0
    low_data_pct: float = 10.0
    snap_max_m: float = 300.0
    recinto_shrink_m: float = 5.0
    recinto_tags: list[tuple[str, str]] = [("amenity", "hospital"), ("amenity", "university"),
                                           ("landuse", "cemetery"), ("landuse", "military")]
    cors_origins: list[str] = ["http://localhost:8080", "http://127.0.0.1:8080"]

    @property
    def walk_m_per_min(self) -> float:
        return self.walk_kmh * 1000 / 60
