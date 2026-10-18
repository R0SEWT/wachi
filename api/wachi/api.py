"""API HTTP de Wachi: POST /route y GET /health."""
from __future__ import annotations

import asyncio
import logging
import os
import threading
from contextlib import asynccontextmanager

from fastapi import FastAPI, Request
from fastapi.exceptions import RequestValidationError
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
        # Si falla, el proceso sale con código 1 y systemd (Restart=on-failure) lo vuelve a levantar:
        # quedarse vivo respondería `iniciando` para siempre.
        def _load():
            try:
                app.state.engine = load_engine(cfg)
            except Exception as e:
                logging.getLogger("wachi").exception("no se pudo cargar el motor")
                app.state.load_error = repr(e)
                os._exit(1)
        if load_on_start and app.state.engine is None:
            threading.Thread(target=_load, name="wachi-load", daemon=True).start()
        yield

    app = FastAPI(title="Wachi API", version="2.0.0", lifespan=lifespan,
                  docs_url=None, redoc_url=None, openapi_url=None)
    app.state.engine = engine
    app.state.load_error = None
    app.add_middleware(CORSMiddleware, allow_origins=cfg.cors_origins, allow_methods=["GET", "POST"],
                       allow_headers=["*"])

    @app.exception_handler(RequestValidationError)
    async def invalid_body(request: Request, exc: RequestValidationError):
        # Cuerpo mal formado (campos faltantes, fuera de rango, NaN/Infinity): 422 sin eco del cuerpo.
        return JSONResponse({"error": "peticion_invalida"}, status_code=422)

    @app.get("/health")
    async def health(request: Request):
        eng: Engine | None = request.app.state.engine
        if request.app.state.load_error is not None:
            return JSONResponse({"status": "error"}, status_code=500)
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
