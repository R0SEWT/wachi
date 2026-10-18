"""API HTTP de Wachi: POST /route y GET /health."""
from __future__ import annotations

import asyncio
import logging
import threading
from contextlib import asynccontextmanager

from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field

from wachi.config import Settings
from wachi.engine import Engine, RouteError, load_engine

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(name)s | %(message)s")


class LatLon(BaseModel):
    lat: float = Field(..., ge=-90, le=90)
    lon: float = Field(..., ge=-180, le=180)


class RouteRequest(BaseModel):
    origin: LatLon
    destination: LatLon


def create_app(engine: Engine | None, cfg: Settings, load_on_start: bool = False) -> FastAPI:
    @asynccontextmanager
    async def lifespan(app: FastAPI):
        # La carga tarda minutos: corre en un hilo y el servidor responde 503 `iniciando` mientras tanto.
        def _load():
            try:
                app.state.engine = load_engine(cfg)
            except Exception:
                logging.getLogger("wachi").exception("no se pudo cargar el motor")
        if load_on_start and app.state.engine is None:
            threading.Thread(target=_load, name="wachi-load", daemon=True).start()
        yield

    app = FastAPI(title="Wachi API", version="2.0.0", lifespan=lifespan)
    app.state.engine = engine
    app.add_middleware(CORSMiddleware, allow_origins=cfg.cors_origins, allow_methods=["GET", "POST"],
                       allow_headers=["*"])

    @app.get("/health")
    async def health(request: Request):
        eng: Engine | None = request.app.state.engine
        if eng is None:
            return JSONResponse({"status": "iniciando"}, status_code=503)
        return {"status": "ok", "nodes": len(eng.router.net.nxy), "events": eng.surf.n_events,
                "data_as_of": eng.surf.data_as_of}

    @app.post("/route")
    async def route(req: RouteRequest, request: Request):
        eng: Engine | None = request.app.state.engine
        if eng is None:
            return JSONResponse({"error": "iniciando"}, status_code=503)
        try:
            out = await asyncio.to_thread(eng.route, (req.origin.lat, req.origin.lon),
                                          (req.destination.lat, req.destination.lon))
        except RouteError as e:
            body = {"error": e.code}
            if e.which:
                body["which"] = e.which
            return JSONResponse(body, status_code=422)
        return out

    return app


app = create_app(None, Settings(), load_on_start=True)
