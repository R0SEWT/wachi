/**
 * Wachi — Frontend Móvil & Prototipo de Interacción UX
 * Stack: Vanilla JS + Leaflet.js (Sin build step)
 */

document.addEventListener('DOMContentLoaded', () => {

  // ?api= solo sirve en desarrollo local; en producción la PWA habla siempre con /api (proxy de Netlify)
  const IS_LOCAL = ['localhost', '127.0.0.1'].includes(window.location.hostname);
  const API_BASE = (IS_LOCAL && new URLSearchParams(window.location.search).get('api')) || '/api';
  const ROUTE_MSG = {
    fuera_de_cobertura: 'Por ahora solo cubrimos Lima Metropolitana.',
    sin_via_peatonal_cerca: 'No hay una vía peatonal cerca de ese punto.',
    sin_ruta: 'No encontramos una ruta a pie entre esos puntos.',
    iniciando: 'El servicio está iniciando, intenta en un momento.'
  };

  let routeReqId = 0;      // descarta respuestas viejas
  let lastRoute = null;    // última respuesta válida de /route
  let routeLoading = false;
  let routeErrorMsg = null;
  let routeRetryable = false;
  let lastDestLabel = '';
  const cardTranquila = document.getElementById('cardTranquila');
  const cardRapida = document.getElementById('cardRapida');

  function setSparse(card, text) {
    card.querySelector('.sparse-desc').innerText = text;
    card.querySelector('.card-metric-secondary').classList.toggle('hidden', !text);
  }

  // retryable: 503 o red; un 422 (fuera de Lima, sin vía cerca) no cambia por reintentar
  function showRouteError(msg, retryable = false) {
    routeLoading = false;
    routeRetryable = retryable;
    routeErrorMsg = msg;
    const el = document.getElementById('routeError');
    if (el) {
      el.innerText = msg;
      el.classList.remove('hidden');
    }
    // En comparar y en el detalle el mensaje queda en la tarjeta hasta el próximo intento
    const cardT = cardTranquila;
    const cardR = cardRapida;
    cardT.classList.add('hidden');
    cardR.querySelector('.route-duration').innerText = '—';
    cardR.querySelector('.delta-time').innerText = '';
    cardR.querySelector('.delta-desc').innerText = retryable ? `${msg} Toca aquí para reintentar.` : msg;
    setSparse(cardR, '');
    renderDetail();
    renderShare();
  }

  function hideRouteError() {
    routeErrorMsg = null;
    const el = document.getElementById('routeError');
    if (el) el.classList.add('hidden');
  }

  function fmtMin(seconds) {
    return `${Math.max(1, Math.round(seconds / 60))} min`;
  }

  function fmtDist(m) {
    return m >= 1000 ? `${(m / 1000).toFixed(1)} km` : `${Math.round(m)} m`;
  }

  function sliceSegments(coords, segments) {
    const out = [];
    (segments || []).forEach(([i, j]) => out.push(coords.slice(i, j + 2)));
    return out;
  }

  // =========================================================================
  // 1. Datos Mockeados para Casos de Prueba (Lima Peatonal)
  // =========================================================================
  const PRESETS = {
    elena: {
      name: 'Elena',
      originName: 'Mi ubicación (Av. Javier Prado con Arequipa)',
      destName: 'Jr. Lloque Yupanqui 1120',
      originCoords: [-12.08807, -77.03387],
      destCoords: [-12.07379, -77.04650]
    },
    mateo: {
      name: 'Mateo',
      originName: 'Campus PUCP (Puerta Principal)',
      destName: 'Estación La Marina / Universitaria',
      originCoords: [-12.0695, -77.0795],
      destCoords: [-12.0768, -77.077]
    }
  };

  // =========================================================================
  // 2. Estado de la Aplicación
  // =========================================================================
  let state = {
    screen: 'search',          // 'search' | 'compare' | 'detail' | 'walking' | 'share'
    preset: 'elena',           // 'elena' | 'mateo'
    theme: 'dark',             // 'dark' | 'light'
    selectedRoute: 'safe',     // 'safe' | 'fast'
    isDeviated: false,
    customOrigin: null,        // { name, coords: [lat, lon] }
    customDest: null,          // { name, coords: [lat, lon] }
    isPickingOnMap: false
  };

  // Origen y destino en uso: lo que eligió la persona o, si no, el preset activo
  function currentEndpoints() {
    const data = PRESETS[state.preset];
    return {
      origin: state.customOrigin || { name: data.originName, coords: data.originCoords },
      dest: state.customDest || { name: data.destName, coords: data.destCoords }
    };
  }

  function parseCoords(v) {
    const m = /^(-?\d+(?:\.\d+)?),(-?\d+(?:\.\d+)?)$/.exec(v || '');
    if (!m) return null;
    const lat = Number(m[1]), lon = Number(m[2]);
    return Math.abs(lat) <= 90 && Math.abs(lon) <= 180 ? [lat, lon] : null;
  }

  const fmtCoords = (c) => `${c[0].toFixed(5)},${c[1].toFixed(5)}`;

  // =========================================================================
  // 3. Inicialización del Mapa (Leaflet)
  // =========================================================================
  const mapElement = document.getElementById('map');
  const map = L.map(mapElement, {
    zoomControl: false,
    attributionControl: true
  }).setView(PRESETS.elena.originCoords, 15);

  const ESRI_DARK_BASE = 'https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Dark_Gray_Base/MapServer/tile/{z}/{y}/{x}';
  const ESRI_DARK_REF = 'https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Dark_Gray_Reference/MapServer/tile/{z}/{y}/{x}';
  const ESRI_LIGHT_BASE = 'https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Light_Gray_Base/MapServer/tile/{z}/{y}/{x}';
  const ESRI_ATTR = '&copy; Esri, HERE, Garmin, OpenStreetMap';

  let currentTileBase = L.tileLayer(ESRI_DARK_BASE, { attribution: ESRI_ATTR, maxZoom: 18 }).addTo(map);
  let currentTileRef = L.tileLayer(ESRI_DARK_REF, { attribution: '', maxZoom: 18, opacity: 0.7 }).addTo(map);

  // Capas de mapa
  let layerTranquila = null;
  let layerSparse = null;
  let layerRapida = null;
  let originMarker = null;
  let destMarker = null;
  let liveGpsMarker = null;

  function initMapLayers(preferred = 'safe') {
    const { origin, dest } = currentEndpoints();

    // Limpiar capas previas
    if (layerTranquila) map.removeLayer(layerTranquila);
    if (layerSparse) map.removeLayer(layerSparse);
    if (layerRapida) map.removeLayer(layerRapida);
    if (originMarker) map.removeLayer(originMarker);
    if (destMarker) map.removeLayer(destMarker);
    if (liveGpsMarker) map.removeLayer(liveGpsMarker);

    // Ruta Rápida (delgada, color ámbar/atenuado); la geometría llega de /api/route
    layerRapida = L.polyline([], { color: '#f59e0b', weight: 3.5, opacity: 0.6, dashArray: '5, 8' }).addTo(map);
    // Ruta Tranquila (trazo sólido de alto contraste)
    layerTranquila = L.polyline([], { color: '#22c55e', weight: 5.5, opacity: 0.95 }).addTo(map);
    layerSparse = null;

    // Marcadores de origen y destino personalizados
    const iconOrigin = L.divIcon({
      className: 'custom-map-pin',
      html: '<div style="background:#3b82f6; width:14px; height:14px; border-radius:50%; border:2px solid #fff; box-shadow:0 0 6px rgba(0,0,0,0.5);"></div>',
      iconSize: [14, 14],
      iconAnchor: [7, 7]
    });

    const iconDest = L.divIcon({
      className: 'custom-map-pin',
      html: '<div style="background:#22c55e; width:14px; height:14px; border-radius:3px; border:2px solid #fff; box-shadow:0 0 6px rgba(0,0,0,0.5);"></div>',
      iconSize: [14, 14],
      iconAnchor: [7, 7]
    });

    originMarker = L.marker(origin.coords, { icon: iconOrigin }).addTo(map);
    destMarker = L.marker(dest.coords, { icon: iconDest }).addTo(map);

    // Marcador GPS en marcha
    const iconLiveGps = L.divIcon({
      className: 'custom-live-gps',
      html: '<div style="background:#22c55e; width:16px; height:16px; border-radius:50%; border:3px solid #ffffff; box-shadow:0 0 10px #22c55e;"></div>',
      iconSize: [16, 16],
      iconAnchor: [8, 8]
    });
    liveGpsMarker = L.marker(origin.coords, { icon: iconLiveGps });

    map.setView(origin.coords, 15);
    calculateDynamicRoute(origin.coords, dest.coords, dest.name, preferred);
  }

  // =========================================================================
  // 4. Gestión de Vistas / Pantallas (Flujo de 5 Pantallas)
  // =========================================================================
  const screens = {
    search: document.getElementById('screenSearch'),
    compare: document.getElementById('screenCompare'),
    detail: document.getElementById('screenDetail'),
    walking: document.getElementById('screenWalking'),
    share: document.getElementById('screenShare')
  };

  function setScreen(screenName, updateHistory = true) {
    if (!screens[screenName]) return;
    state.screen = screenName;

    // Ocultar todas y activar la actual
    Object.values(screens).forEach(el => el.classList.remove('active'));
    screens[screenName].classList.add('active');

    // Ajustar visibilidad de capas de mapa según la pantalla
    if (screenName === 'walking') {
      if (!map.hasLayer(liveGpsMarker)) liveGpsMarker.addTo(map);
      // En modo en marcha se atenúa o apaga la ruta alternativa para no distraer
      if (layerRapida) map.removeLayer(layerRapida);
    } else {
      if (map.hasLayer(liveGpsMarker)) map.removeLayer(liveGpsMarker);
      if (layerRapida && !map.hasLayer(layerRapida)) layerRapida.addTo(map);
    }

    if (screenName === 'share') {
      renderShare();
    }

    if (updateHistory) {
      updateUrlParams();
    }
  }

  // =========================================================================
  // 5. Serialización de Estado en la URL (Deep Links)
  // =========================================================================
  function updateUrlParams() {
    const params = new URLSearchParams();
    params.set('screen', state.screen);
    params.set('preset', state.preset);
    params.set('route', state.selectedRoute);
    params.set('theme', state.theme);
    addEndpointParams(params);
    const api = new URLSearchParams(window.location.search).get('api');
    if (IS_LOCAL && api) params.set('api', api);
    window.history.replaceState({}, '', `${window.location.pathname}?${params.toString()}`);
  }

  function addEndpointParams(params) {
    if (state.customOrigin) {
      params.set('o', fmtCoords(state.customOrigin.coords));
      params.set('on', state.customOrigin.name);
    }
    if (state.customDest) {
      params.set('d', fmtCoords(state.customDest.coords));
      params.set('dn', state.customDest.name);
    }
  }

  function readUrlParams() {
    const params = new URLSearchParams(window.location.search);
    if (params.get('preset') && PRESETS[params.get('preset')]) {
      state.preset = params.get('preset');
    }
    if (params.get('theme')) {
      state.theme = params.get('theme');
    }
    if (params.get('route') === 'fast' || params.get('route') === 'safe') {
      state.selectedRoute = params.get('route');
    }
    if (params.get('salida') && /^\d{2}:\d{2}$/.test(params.get('salida'))) {
      sharedDeparture = params.get('salida');
    }
    const initialScreen = params.get('screen') || 'search';

    applyTheme(state.theme);
    updatePresetUI();
    const o = parseCoords(params.get('o'));
    const d = parseCoords(params.get('d'));
    if (o) {
      state.customOrigin = { name: params.get('on') || `Punto (${o[0].toFixed(4)}, ${o[1].toFixed(4)})`, coords: o };
      inputOrigin.value = state.customOrigin.name;
    }
    if (d) {
      state.customDest = { name: params.get('dn') || `Punto (${d[0].toFixed(4)}, ${d[1].toFixed(4)})`, coords: d };
      inputDest.value = state.customDest.name;
    }
    if (o || d) presetChips.forEach(chip => chip.classList.remove('active'));
    initMapLayers(state.selectedRoute);
    setScreen(initialScreen, false);

    if (params.get('pocket') === '1') {
      const pocketOverlay = document.getElementById('pocketOverlay');
      if (pocketOverlay) pocketOverlay.classList.remove('hidden');
    }

    if (params.get('deviated') === '1') {
      triggerDeviationAlert();
    }

    if (params.get('search')) {
      const q = params.get('search');
      if (inputDest) {
        inputDest.value = q;
        if (btnClearDest) btnClearDest.classList.remove('hidden');
      }
      fetchPhotonSuggestions(q, 'dest');
    }

    if (params.get('pickmap') === '1') {
      startMapPicking();
    }
  }

  // Hora de salida que trae un enlace compartido (HH:MM); si no hay, la salida es ahora
  let sharedDeparture = null;

  const fmtClock = (d) => `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;

  function departureDate() {
    const now = new Date();
    now.setSeconds(0, 0);       // la salida viaja como HH:MM: quien abre el enlace calcula la misma llegada
    if (!sharedDeparture) return now;
    const [h, m] = sharedDeparture.split(':').map(Number);
    const d = new Date(now);
    d.setHours(h, m, 0, 0);
    return d;
  }

  function chosenRoute() {
    if (!lastRoute) return null;
    return state.selectedRoute === 'fast' || !lastRoute.calm ? lastRoute.fast : lastRoute.calm;
  }

  function routeName() {
    return state.selectedRoute === 'fast' || !(lastRoute && lastRoute.calm) ? 'Rápida' : 'Tranquila';
  }

  // Vista acompañante: todo sale de la ruta calculada; sin ruta no hay enlace ni hora
  function renderShare() {
    const { origin, dest } = currentEndpoints();
    const route = chosenRoute();
    const pomTime = document.querySelector('.pom-time');
    const pomTitle = document.querySelector('.pom-title');
    const shareDest = document.getElementById('shareDest');
    const shareEta = document.getElementById('shareEta');
    const shareInput = document.getElementById('shareUrlInput');
    const shareBox = document.querySelector('.share-link-box');
    const btnWpp = document.getElementById('btnShareWhatsApp');
    const destShort = (lastDestLabel || dest.name).split('(')[0].trim();
    if (shareDest) shareDest.innerText = lastDestLabel || dest.name;
    if (!route) {
      if (pomTitle) pomTitle.innerText = routeErrorMsg || 'Calculando la ruta…';
      if (pomTime) pomTime.innerText = '';
      if (shareEta) shareEta.innerText = '—';
      if (shareBox) shareBox.classList.add('hidden');
      if (btnWpp) btnWpp.classList.add('hidden');
      return;
    }
    const salida = departureDate();
    const llegada = new Date(salida.getTime() + route.duration_s * 1000);
    if (pomTitle) pomTitle.innerText = `Caminando hacia ${destShort} por la ruta ${routeName()}`;
    if (pomTime) pomTime.innerText = `Salida: ${fmtClock(salida)}`;
    if (shareEta) shareEta.innerText = `${fmtClock(llegada)} (${fmtMin(route.duration_s)} a pie, ${fmtDist(route.distance_m)})`;
    const params = new URLSearchParams();
    params.set('screen', 'share');
    params.set('route', state.selectedRoute);
    // Siempre las coordenadas reales, aunque vengan de un preset
    params.set('o', fmtCoords(origin.coords));
    params.set('on', origin.name);
    params.set('d', fmtCoords(dest.coords));
    params.set('dn', lastDestLabel || dest.name);
    params.set('salida', fmtClock(salida));
    if (shareInput) shareInput.value = `${window.location.origin}${window.location.pathname}?${params.toString()}`;
    if (shareBox) shareBox.classList.remove('hidden');
    if (btnWpp) btnWpp.classList.remove('hidden');
  }

  // =========================================================================
  // 6. Tema Pantalla Discreta / Noche
  // =========================================================================
  const btnThemeToggle = document.getElementById('btnThemeToggle');
  const iconMoon = document.getElementById('iconMoon');
  const iconSun = document.getElementById('iconSun');

  function applyTheme(theme) {
    state.theme = theme;
    if (theme === 'dark') {
      document.body.classList.add('mode-dark');
      document.body.classList.remove('mode-light');
      iconMoon.classList.remove('hidden');
      iconSun.classList.add('hidden');
      currentTileBase.setUrl(ESRI_DARK_BASE);
      if (!map.hasLayer(currentTileRef)) currentTileRef.addTo(map);
    } else {
      document.body.classList.remove('mode-dark');
      document.body.classList.add('mode-light');
      iconMoon.classList.add('hidden');
      iconSun.classList.remove('hidden');
      currentTileBase.setUrl(ESRI_LIGHT_BASE);
      if (map.hasLayer(currentTileRef)) map.removeLayer(currentTileRef);
    }
    updateUrlParams();
  }

  btnThemeToggle.addEventListener('click', () => {
    applyTheme(state.theme === 'dark' ? 'light' : 'dark');
  });

  // =========================================================================
  // 7. Presets Rápidos (Elena / Mateo)
  // =========================================================================
  const presetChips = document.querySelectorAll('.chip-preset');
  function updatePresetUI() {
    state.customDest = null;
    state.customOrigin = null;
    const data = PRESETS[state.preset];
    document.getElementById('inputOrigin').value = data.originName;
    document.getElementById('inputDest').value = data.destName;
    const clearDest = document.getElementById('btnClearDest');
    if (clearDest) clearDest.classList.remove('hidden');
    const clearOrig = document.getElementById('btnClearOrigin');
    if (clearOrig) clearOrig.classList.add('hidden');
    const dropdown = document.getElementById('autocompleteDropdown');
    if (dropdown) dropdown.classList.add('hidden');
    const walkingDest = document.querySelector('.walking-dest-name');
    if (walkingDest) walkingDest.innerText = data.destName;

    presetChips.forEach(chip => {
      chip.classList.toggle('active', chip.dataset.preset === state.preset);
    });
  }

  presetChips.forEach(chip => {
    chip.addEventListener('click', () => {
      state.preset = chip.dataset.preset;
      updatePresetUI();
      initMapLayers();
      updateUrlParams();
    });
  });

  // =========================================================================
  // 7.1. Geocodificación en Vivo y Autocompletado (Photon Komoot)
  // =========================================================================
  const inputOrigin = document.getElementById('inputOrigin');
  const inputDest = document.getElementById('inputDest');
  const btnClearOrigin = document.getElementById('btnClearOrigin');
  const btnClearDest = document.getElementById('btnClearDest');
  const autocompleteDropdown = document.getElementById('autocompleteDropdown');
  const suggestionsList = document.getElementById('suggestionsList');
  const autocompleteSpinner = document.getElementById('autocompleteSpinner');
  const dropdownEmpty = document.getElementById('dropdownEmpty');
  const searchInputsCard = document.getElementById('searchInputsCard');
  const btnPickOnMap = document.getElementById('btnPickOnMap');
  const btnPickOnMapFallback = document.getElementById('btnPickOnMapFallback');
  const btnCurrentGps = document.getElementById('btnCurrentGps');
  const mapPickingToast = document.getElementById('mapPickingToast');
  const btnCancelMapPick = document.getElementById('btnCancelMapPick');

  let currentSearchAbortController = null;
  let activeSearchField = 'dest';

  function debounce(fn, delay = 280) {
    let timer;
    return (...args) => {
      clearTimeout(timer);
      timer = setTimeout(() => fn(...args), delay);
    };
  }

  function escapeHtml(text) {
    if (!text) return '';
    return text.replace(/[&<>"']/g, m => ({
      '&': '&amp;',
      '<': '&lt;',
      '>': '&gt;',
      '"': '&quot;',
      "'": '&#39;'
    })[m]);
  }

  async function fetchPhotonSuggestions(query, targetField = 'dest') {
    query = (query || '').trim();
    activeSearchField = targetField;

    if (query.length < 2) {
      if (autocompleteDropdown) autocompleteDropdown.classList.add('hidden');
      if (suggestionsList) suggestionsList.innerHTML = '';
      if (dropdownEmpty) dropdownEmpty.classList.add('hidden');
      return;
    }

    if (currentSearchAbortController) {
      currentSearchAbortController.abort();
    }
    currentSearchAbortController = new AbortController();

    if (autocompleteSpinner) autocompleteSpinner.classList.remove('hidden');
    if (autocompleteDropdown) autocompleteDropdown.classList.remove('hidden');
    if (dropdownEmpty) dropdownEmpty.classList.add('hidden');

    // Bounding Box para Lima Metropolitana: [lonMin, latMin, lonMax, latMax]
    const url = `https://photon.komoot.io/api/?q=${encodeURIComponent(query)}&bbox=-77.25,-12.35,-76.80,-11.75&limit=5`;

    try {
      const resp = await fetch(url, { signal: currentSearchAbortController.signal });
      if (!resp.ok) throw new Error('Error al consultar geocodificador');
      const data = await resp.json();
      renderSuggestions(data.features || [], targetField, query);
    } catch (err) {
      if (err.name === 'AbortError') return;
      console.warn('[Wachi Geocoding] Error de búsqueda:', err);
      if (dropdownEmpty) dropdownEmpty.classList.remove('hidden');
      if (suggestionsList) suggestionsList.innerHTML = '';
    } finally {
      if (autocompleteSpinner) autocompleteSpinner.classList.add('hidden');
    }
  }

  function renderSuggestions(features, targetField, query) {
    if (!suggestionsList) return;
    suggestionsList.innerHTML = '';
    if (!features || features.length === 0) {
      if (dropdownEmpty) dropdownEmpty.classList.remove('hidden');
      return;
    }
    if (dropdownEmpty) dropdownEmpty.classList.add('hidden');

    features.forEach((feat) => {
      const p = feat.properties;
      const name = p.name || p.street || query;
      const district = p.district || p.city || p.county || 'Lima';
      const streetInfo = p.street && p.street !== name ? `${p.street}, ` : '';
      const subtitle = `${streetInfo}${district}, Lima`;
      const coords = [feat.geometry.coordinates[1], feat.geometry.coordinates[0]]; // [lat, lon]

      const li = document.createElement('li');
      li.className = 'suggestion-item';
      li.setAttribute('role', 'option');
      li.innerHTML = `
        <div class="suggestion-icon">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z"/><circle cx="12" cy="10" r="3"/></svg>
        </div>
        <div class="suggestion-info">
          <span class="suggestion-primary">${escapeHtml(name)}</span>
          <span class="suggestion-secondary">${escapeHtml(subtitle)}</span>
        </div>
      `;

      li.addEventListener('click', () => {
        selectSuggestion(targetField, name, subtitle, coords);
      });

      suggestionsList.appendChild(li);
    });
  }

  function selectSuggestion(targetField, name, subtitle, coords) {
    if (autocompleteDropdown) autocompleteDropdown.classList.add('hidden');
    if (suggestionsList) suggestionsList.innerHTML = '';

    const districtPart = subtitle.split(',')[0].trim();
    const fullLabel = `${name} (${districtPart})`;

    if (targetField === 'dest') {
      if (inputDest) inputDest.value = fullLabel;
      if (btnClearDest) btnClearDest.classList.remove('hidden');
      state.customDest = { name: fullLabel, coords };
      if (destMarker) {
        destMarker.setLatLng(coords);
      } else {
        const iconDest = L.divIcon({
          className: 'custom-map-pin',
          html: '<div style="background:#22c55e; width:14px; height:14px; border-radius:3px; border:2px solid #fff; box-shadow:0 0 6px rgba(0,0,0,0.5);"></div>',
          iconSize: [14, 14],
          iconAnchor: [7, 7]
        });
        destMarker = L.marker(coords, { icon: iconDest }).addTo(map);
      }
    } else {
      if (inputOrigin) inputOrigin.value = fullLabel;
      if (btnClearOrigin) btnClearOrigin.classList.remove('hidden');
      state.customOrigin = { name: fullLabel, coords };
      if (originMarker) {
        originMarker.setLatLng(coords);
      } else {
        const iconOrigin = L.divIcon({
          className: 'custom-map-pin',
          html: '<div style="background:#3b82f6; width:14px; height:14px; border-radius:50%; border:2px solid #fff; box-shadow:0 0 6px rgba(0,0,0,0.5);"></div>',
          iconSize: [14, 14],
          iconAnchor: [7, 7]
        });
        originMarker = L.marker(coords, { icon: iconOrigin }).addTo(map);
      }
    }

    // Desactivar chips de presets al usar una ubicación personalizada
    presetChips.forEach(chip => chip.classList.remove('active'));

    const { origin, dest } = currentEndpoints();
    calculateDynamicRoute(origin.coords, dest.coords, dest.name);
    updateUrlParams();
  }

  function resetRouteUI() {
    lastRoute = null;
    if (layerRapida) layerRapida.setLatLngs([]);
    if (layerTranquila) layerTranquila.setLatLngs([]);
    if (layerSparse) { map.removeLayer(layerSparse); layerSparse = null; }
    const cardT = document.getElementById('cardTranquila');
    const cardR = document.getElementById('cardRapida');
    cardT.classList.remove('hidden');
    [[cardT, ''], [cardR, '']].forEach(([card]) => {
      card.querySelector('.route-duration').innerText = '—';
      card.querySelector('.delta-time').innerText = '';
      card.querySelector('.delta-desc').innerText = '· calculando…';
    });
    setSparse(cardT, '');
    setSparse(cardR, '');
    renderDetail();
    const etaMins = document.querySelector('.eta-mins');
    if (etaMins) etaMins.innerText = '—';
    const etaDist = document.querySelector('.eta-dist');
    if (etaDist) etaDist.innerText = '—';
  }

  function updateChosenSummary() {
    const chosen = chosenRoute();
    if (!chosen) return;
    const etaMins = document.querySelector('.eta-mins');
    if (etaMins) etaMins.innerText = fmtMin(chosen.duration_s);
    const etaDist = document.querySelector('.eta-dist');
    if (etaDist) etaDist.innerText = fmtDist(chosen.distance_m);
    drawSparse();
    renderDetail();
    renderShare();
  }

  const plural = (n, one, many) => `${n} ${n === 1 ? one : many}`;

  function benefitText(calm) {
    const blocks = calm.blocks_fewer_near_robberies;
    return blocks > 0
      ? `pasa ${plural(blocks, 'cuadra', 'cuadras')} menos junto a robos registrados`
      : `${calm.exposure_reduction_pct} % menos robos registrados cerca`;
  }

  const detourMin = (calm) => `+${Math.max(1, Math.round(calm.detour_s / 60))} min`;

  // Tramos con pocos datos de la ruta elegida (ADR 011 #4: se marcan siempre, también en la Rápida)
  function drawSparse() {
    if (layerSparse) { map.removeLayer(layerSparse); layerSparse = null; }
    const chosen = chosenRoute();
    if (!chosen) return;
    const sparse = sliceSegments(chosen.geometry, chosen.low_data_segments);
    if (sparse.length) {
      layerSparse = L.polyline(sparse, { color: '#94a3b8', weight: 6, dashArray: '4, 10', opacity: 0.9 }).addTo(map);
    }
  }

  // Pantalla 3 ("Por qué este camino"): solo cifras de la respuesta de /route
  function renderDetail() {
    const summary = document.getElementById('detailSummary');
    const benefit = document.getElementById('detailBenefit');
    const sparseLi = document.getElementById('detailSparse');
    const source = document.getElementById('detailSource');
    if (!summary) return;
    const chosen = chosenRoute();
    [benefit, sparseLi, source].forEach(el => el.classList.add('hidden'));
    if (!chosen) {
      summary.innerText = routeErrorMsg || 'Calculando…';
      return;
    }
    const { fast, calm } = lastRoute;
    const isCalm = chosen === calm;
    summary.innerText = `Ruta ${routeName()} · ${fmtMin(chosen.duration_s)} · ${fmtDist(chosen.distance_m)}`;
    let title, desc;
    if (isCalm) {
      title = `${detourMin(calm)} · ${benefitText(calm)}`;
      desc = `Camina ${calm.detour_m} m más que la Rápida (${fmtMin(fast.duration_s)} · ${fmtDist(fast.distance_m)}).`;
    } else if (calm) {
      title = 'La más corta';
      desc = `La Tranquila toma ${detourMin(calm)} · ${benefitText(calm)}.`;
    } else {
      title = 'La más corta';
      desc = 'No hay un desvío que valga la pena.';
    }
    benefit.querySelector('strong').innerText = title;
    benefit.querySelector('p').innerText = desc;
    benefit.classList.remove('hidden');
    const n = chosen.low_data_segments.length;
    if (n) {
      sparseLi.querySelector('strong').innerText = `${plural(n, 'tramo', 'tramos')} con pocos datos`;
      sparseLi.classList.remove('hidden');
    }
    source.querySelector('strong').innerText = `Robos y hurtos registrados hasta ${lastRoute.data_as_of}`;
    const radius = lastRoute.params && lastRoute.params.radius_m;
    source.querySelector('p').innerText = radius
      ? `Se cuentan los registrados por la PNP a ${Math.round(radius)} m o menos del camino.`
      : 'Registros de la PNP.';
    source.classList.remove('hidden');
  }

  // Marca la tarjeta elegida y resalta su trazo
  function selectRoute(which) {
    state.selectedRoute = which;
    const safe = which === 'safe';
    cardTranquila.classList.toggle('selected', safe);
    cardRapida.classList.toggle('selected', !safe);
    cardTranquila.querySelector('.card-radio').innerText = safe ? '●' : '○';
    cardRapida.querySelector('.card-radio').innerText = safe ? '○' : '●';
    if (layerTranquila && layerRapida) {
      (safe ? layerTranquila : layerRapida).setStyle({ opacity: 0.95, weight: 5.5 });
      (safe ? layerRapida : layerTranquila).setStyle({ opacity: 0.4, weight: 3 });
    }
  }

  async function calculateDynamicRoute(originCoords, destCoords, destLabel, preferred = 'safe') {
    const reqId = ++routeReqId;
    lastDestLabel = destLabel;
    routeLoading = true;
    hideRouteError();
    // Cada ruta nueva empieza en la Tranquila: un `calm: null` anterior no fija la Rápida para siempre
    selectRoute(preferred === 'fast' ? 'fast' : 'safe');
    resetRouteUI();
    let resp, data;
    try {
      resp = await fetch(`${API_BASE}/route`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          origin: { lat: originCoords[0], lon: originCoords[1] },
          destination: { lat: destCoords[0], lon: destCoords[1] }
        })
      });
      data = await resp.json().catch(() => ({}));
    } catch (err) {
      if (reqId !== routeReqId) return;
      console.warn('[Wachi] Error pidiendo la ruta:', err);
      showRouteError(ROUTE_MSG.iniciando, true);
      return;
    }
    if (reqId !== routeReqId) return;
    if (!resp.ok) {
      const known = ROUTE_MSG[data && data.error];
      const retryable = resp.status >= 500 || resp.status === 429;
      showRouteError(known || (retryable ? ROUTE_MSG.iniciando : 'No pudimos calcular la ruta.'), retryable);
      return;
    }
    try {
      renderRoutes(data, destLabel);
    } catch (err) {
      console.error('[Wachi] Error pintando la ruta:', err);
      showRouteError('No pudimos calcular la ruta.');
    }
  }

  function renderRoutes(data, destLabel) {
    const fast = data.fast;
    const calm = data.calm;

    if (layerRapida) layerRapida.setLatLngs(fast.geometry);
    // Sin Tranquila no se repite la Rápida en verde
    if (layerTranquila) layerTranquila.setLatLngs(calm ? calm.geometry : []);

    const sparseText = (route) => {
      const n = route.low_data_segments.length;
      return n ? `${plural(n, 'tramo', 'tramos')} con pocos datos` : 'Cobertura normal en el tramo';
    };
    const cardT = document.getElementById('cardTranquila');
    const cardR = document.getElementById('cardRapida');
    if (calm) {
      cardT.classList.remove('hidden');
      cardT.querySelector('.route-duration').innerText = fmtMin(calm.duration_s);
      cardT.querySelector('.delta-time').innerText = detourMin(calm);
      cardT.querySelector('.delta-desc').innerText = `· ${benefitText(calm)}`;
      setSparse(cardT, sparseText(calm));
    } else {
      cardT.classList.add('hidden');
    }
    cardR.querySelector('.route-duration').innerText = fmtMin(fast.duration_s);
    cardR.querySelector('.delta-time').innerText = 'Directa';
    cardR.querySelector('.delta-desc').innerText = calm
      ? '· La más corta; pasa junto a más robos registrados'
      : '· La más corta; no hay un desvío que valga la pena';
    setSparse(cardR, sparseText(fast));

    lastRoute = data;
    routeLoading = false;
    if (!calm) selectRoute('fast');
    const walkingDest = document.querySelector('.walking-dest-name');
    if (walkingDest) walkingDest.innerText = destLabel;
    updateChosenSummary();
    const turnInst = document.querySelector('.turn-instruction');
    if (turnInst) turnInst.innerText = `Rumbo a ${destLabel.split('(')[0].trim()}`;
    const asOf = document.getElementById('dataAsOf');
    if (asOf) asOf.innerText = `Robos y hurtos registrados hasta ${data.data_as_of}`;

    const bounds = L.latLngBounds(fast.geometry);
    if (calm) bounds.extend(L.latLngBounds(calm.geometry));
    map.fitBounds(bounds, { padding: [50, 50], maxZoom: 16 });
  }

  async function reverseGeocode(lat, lon) {
    const url = `https://photon.komoot.io/reverse?lat=${lat}&lon=${lon}`;
    try {
      const resp = await fetch(url);
      if (!resp.ok) throw new Error('Reverse geocode error');
      const data = await resp.json();
      if (data.features && data.features.length > 0) {
        const p = data.features[0].properties;
        const name = p.name || p.street || 'Punto fijado';
        const district = p.district || p.city || '';
        return `${name}${district ? ' (' + district + ')' : ''}`;
      }
    } catch (err) {
      console.warn('[Wachi Reverse] Error al geocodificar:', err);
    }
    return `Punto (${lat.toFixed(4)}, ${lon.toFixed(4)})`;
  }

  function startMapPicking() {
    state.isPickingOnMap = true;
    if (mapPickingToast) mapPickingToast.classList.remove('hidden');
    if (sheet) sheet.classList.add('collapsed');
    map.getContainer().style.cursor = 'crosshair';
    map.once('click', onMapPicked);
  }

  function stopMapPicking() {
    state.isPickingOnMap = false;
    if (mapPickingToast) mapPickingToast.classList.add('hidden');
    if (sheet) sheet.classList.remove('collapsed');
    map.getContainer().style.cursor = '';
    map.off('click', onMapPicked);
  }

  async function onMapPicked(e) {
    const coords = [e.latlng.lat, e.latlng.lng];
    stopMapPicking();

    if (destMarker) {
      destMarker.setLatLng(coords);
    } else {
      const iconDest = L.divIcon({
        className: 'custom-map-pin',
        html: '<div style="background:#22c55e; width:14px; height:14px; border-radius:3px; border:2px solid #fff; box-shadow:0 0 6px rgba(0,0,0,0.5);"></div>',
        iconSize: [14, 14],
        iconAnchor: [7, 7]
      });
      destMarker = L.marker(coords, { icon: iconDest }).addTo(map);
    }

    const label = await reverseGeocode(coords[0], coords[1]);
    if (inputDest) inputDest.value = label;
    if (btnClearDest) btnClearDest.classList.remove('hidden');
    state.customDest = { name: label, coords };

    presetChips.forEach(chip => chip.classList.remove('active'));

    calculateDynamicRoute(currentEndpoints().origin.coords, coords, label);
    updateUrlParams();
  }

  // Listeners de búsqueda con Debounce (280ms)
  const debouncedSearchDest = debounce((q) => fetchPhotonSuggestions(q, 'dest'), 280);
  const debouncedSearchOrigin = debounce((q) => fetchPhotonSuggestions(q, 'origin'), 280);

  if (inputDest) {
    inputDest.addEventListener('input', (e) => {
      const val = e.target.value;
      if (btnClearDest) btnClearDest.classList.toggle('hidden', !val);
      debouncedSearchDest(val);
    });

    inputDest.addEventListener('focus', () => {
      if (inputDest.value.trim().length >= 2) {
        fetchPhotonSuggestions(inputDest.value, 'dest');
      }
    });

    inputDest.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && autocompleteDropdown) {
        autocompleteDropdown.classList.add('hidden');
      }
    });
  }

  if (inputOrigin) {
    inputOrigin.addEventListener('input', (e) => {
      const val = e.target.value;
      if (btnClearOrigin) btnClearOrigin.classList.toggle('hidden', !val);
      debouncedSearchOrigin(val);
    });

    inputOrigin.addEventListener('focus', () => {
      if (inputOrigin.value.trim().length >= 2) {
        fetchPhotonSuggestions(inputOrigin.value, 'origin');
      }
    });

    inputOrigin.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && autocompleteDropdown) {
        autocompleteDropdown.classList.add('hidden');
      }
    });
  }

  // Limpiar campos
  if (btnClearDest) {
    btnClearDest.addEventListener('click', () => {
      if (inputDest) {
        inputDest.value = '';
        inputDest.focus();
      }
      btnClearDest.classList.add('hidden');
      if (autocompleteDropdown) autocompleteDropdown.classList.add('hidden');
      if (suggestionsList) suggestionsList.innerHTML = '';
      state.customDest = null;
    });
  }

  if (btnClearOrigin) {
    btnClearOrigin.addEventListener('click', () => {
      if (inputOrigin) {
        inputOrigin.value = '';
        inputOrigin.focus();
      }
      btnClearOrigin.classList.add('hidden');
      if (autocompleteDropdown) autocompleteDropdown.classList.add('hidden');
      if (suggestionsList) suggestionsList.innerHTML = '';
      state.customOrigin = null;
    });
  }

  // Cerrar dropdown si se toca fuera
  document.addEventListener('click', (e) => {
    if (searchInputsCard && autocompleteDropdown) {
      if (!searchInputsCard.contains(e.target) && !autocompleteDropdown.contains(e.target)) {
        autocompleteDropdown.classList.add('hidden');
      }
    }
  });

  // Acciones auxiliares: Tocar en el mapa
  if (btnPickOnMap) {
    btnPickOnMap.addEventListener('click', () => startMapPicking());
  }
  if (btnPickOnMapFallback) {
    btnPickOnMapFallback.addEventListener('click', () => startMapPicking());
  }
  if (btnCancelMapPick) {
    btnCancelMapPick.addEventListener('click', () => stopMapPicking());
  }

  // Botón Mi GPS
  if (btnCurrentGps) {
    btnCurrentGps.addEventListener('click', () => {
      if ('geolocation' in navigator) {
        navigator.geolocation.getCurrentPosition(
          async (pos) => {
            const coords = [pos.coords.latitude, pos.coords.longitude];
            if (originMarker) originMarker.setLatLng(coords);
            const label = await reverseGeocode(coords[0], coords[1]);
            if (inputOrigin) inputOrigin.value = `Mi GPS: ${label}`;
            if (btnClearOrigin) btnClearOrigin.classList.remove('hidden');
            state.customOrigin = { name: label, coords };
            map.setView(coords, 16);
            const { dest } = currentEndpoints();
            calculateDynamicRoute(coords, dest.coords, dest.name);
            updateUrlParams();
          },
          (err) => {
            console.warn('[Wachi GPS] Ubicación no autorizada:', err);
            map.setView(currentEndpoints().origin.coords, 16);
          },
          { enableHighAccuracy: true, timeout: 6000 }
        );
      }
    });
  }

  // =========================================================================
  // 8. Flujo entre Pantallas e Interacciones
  // =========================================================================

  // Pantalla 1 -> 2: Comparar Rutas
  // Sin ruta (falló o nunca llegó), "Comparar rutas" o tocar la tarjeta con el error la vuelven a pedir
  function retryRoute() {
    if (lastRoute || routeLoading) return false;
    const { origin, dest } = currentEndpoints();
    calculateDynamicRoute(origin.coords, dest.coords, dest.name);
    return true;
  }

  document.getElementById('btnCalculateRoute').addEventListener('click', () => {
    retryRoute();
    setScreen('compare');
  });

  // Selección de Tarjeta (Tranquila vs Rápida)
  cardTranquila.addEventListener('click', () => {
    if (!lastRoute || !lastRoute.calm) return;
    selectRoute('safe');
    updateChosenSummary();
    updateUrlParams();
  });

  cardRapida.addEventListener('click', () => {
    if (!lastRoute) {
      if (routeRetryable) retryRoute();
      return;
    }
    selectRoute('fast');
    updateChosenSummary();
    updateUrlParams();
  });

  // Pantalla 2 -> 3: Ver Detalle
  document.getElementById('btnViewDetail').addEventListener('click', () => {
    setScreen('detail');
  });

  // Pantalla 3 -> 2: Volver a Comparar
  document.getElementById('btnBackToCompare').addEventListener('click', () => {
    setScreen('compare');
  });

  // Pantalla 2 y 3 -> 4: Iniciar Marcha
  document.getElementById('btnStartWalk').addEventListener('click', () => {
    setScreen('walking');
    triggerOfflinePrecache();
  });
  document.getElementById('btnStartFromDetail').addEventListener('click', () => {
    setScreen('walking');
    triggerOfflinePrecache();
  });

  // Pantalla 4 -> 1: Finalizar Trayecto
  document.getElementById('btnEndWalk').addEventListener('click', () => {
    setScreen('search');
    document.getElementById('deviationAlert').classList.add('hidden');
  });

  // Pantalla 2 / 4 -> 5: Compartir
  document.getElementById('btnShareFromCompare').addEventListener('click', () => {
    setScreen('share');
  });
  document.getElementById('btnShareLive').addEventListener('click', () => {
    setScreen('share');
  });
  document.getElementById('btnBackFromShare').addEventListener('click', () => {
    setScreen('compare');
  });

  // =========================================================================
  // 9. Simulación de Desvío (>50 m) y Alerta Háptica
  // =========================================================================
  const deviationAlert = document.getElementById('deviationAlert');
  const btnSimulateDeviation = document.getElementById('btnSimulateDeviation');

  function triggerDeviationAlert() {
    state.isDeviated = true;
    deviationAlert.classList.remove('hidden');

    // Vibración háptica doble silenciosa: [150ms vibra, 100ms pausa, 150ms vibra]
    if ('vibrate' in navigator) {
      try {
        navigator.vibrate([150, 100, 150]);
      } catch (_) {}
    }
  }

  btnSimulateDeviation.addEventListener('click', () => {
    if (deviationAlert.classList.contains('hidden')) {
      triggerDeviationAlert();
    } else {
      deviationAlert.classList.add('hidden');
    }
  });

  document.getElementById('btnRecalculate').addEventListener('click', () => {
    deviationAlert.classList.add('hidden');
    alert('Ruta recalculada incorporando tu posición actual.');
  });

  document.getElementById('btnMuteAlert').addEventListener('click', () => {
    deviationAlert.classList.add('hidden');
  });

  // =========================================================================
  // 10. Fallback a Google Maps con Advertencia
  // =========================================================================
  document.getElementById('btnOpenGoogleMaps').addEventListener('click', () => {
    const { origin, dest } = currentEndpoints();
    const url = `https://www.google.com/maps/dir/?api=1&origin=${origin.coords[0]},${origin.coords[1]}&destination=${dest.coords[0]},${dest.coords[1]}&travelmode=walking`;
    const confirmRedirect = confirm(
      'Aviso de Wachi:\n\nGoogle Maps calculará el camino más directo y podría desviarte hacia calles más transitadas o con mayor exposición que las recomendadas aquí.\n\n¿Deseas abrir Google Maps de todos modos?'
    );
    if (confirmRedirect) {
      window.open(url, '_blank');
    }
  });

  // =========================================================================
  // 11. Compartir Deep Link & WhatsApp
  // =========================================================================
  const btnCopyLink = document.getElementById('btnCopyLink');
  btnCopyLink.addEventListener('click', () => {
    const shareInput = document.getElementById('shareUrlInput');
    shareInput.select();
    navigator.clipboard.writeText(shareInput.value).then(() => {
      btnCopyLink.innerText = '¡Copiado!';
      setTimeout(() => { btnCopyLink.innerText = 'Copiar'; }, 2000);
    });
  });

  document.getElementById('btnShareWhatsApp').addEventListener('click', () => {
    const shareInput = document.getElementById('shareUrlInput').value;
    const text = `Hola, voy caminando por la ruta ${routeName()} que calculé con Wachi. Puedes ver mi trayecto y hora estimada aquí: ${shareInput}`;
    window.open(`https://api.whatsapp.com/send?text=${encodeURIComponent(text)}`, '_blank');
  });

  // =========================================================================
  // 12. Onboarding Modal & Selector de Horario
  // =========================================================================
  const modalOnboarding = document.getElementById('modalOnboarding');
  const btnAcceptOnboarding = document.getElementById('btnAcceptOnboarding');
  const btnInfo = document.getElementById('btnInfo');

  // Mostrar onboarding si es la primera vez
  if (!localStorage.getItem('wachi_onboarding_accepted')) {
    modalOnboarding.classList.remove('hidden');
  }

  btnAcceptOnboarding.addEventListener('click', () => {
    localStorage.setItem('wachi_onboarding_accepted', 'true');
    modalOnboarding.classList.add('hidden');
  });

  btnInfo.addEventListener('click', () => {
    modalOnboarding.classList.remove('hidden');
  });

  // Recentrar mapa
  document.getElementById('btnRecenterGPS').addEventListener('click', () => {
    map.setView(currentEndpoints().origin.coords, 16);
  });

  // Simulación de precache offline
  function triggerOfflinePrecache() {
    const offlineIndicator = document.getElementById('offlineIndicator');
    offlineIndicator.classList.remove('hidden');
    setTimeout(() => {
      offlineIndicator.classList.add('hidden');
    }, 3000);
  }

  // =========================================================================
  // Modo Bolsillo (PocketNavigator - Pielot et al.)
  // =========================================================================
  const btnPocketMode = document.getElementById('btnPocketMode');
  const pocketOverlay = document.getElementById('pocketOverlay');

  if (btnPocketMode && pocketOverlay) {
    btnPocketMode.addEventListener('click', () => {
      pocketOverlay.classList.remove('hidden');
      if ('vibrate' in navigator) {
        try { navigator.vibrate([100, 50, 100]); } catch (_) {}
      }
    });

    // Tocar la pantalla oscura para despertar
    pocketOverlay.addEventListener('click', () => {
      pocketOverlay.classList.add('hidden');
      if ('vibrate' in navigator) {
        try { navigator.vibrate([60]); } catch (_) {}
      }
    });
  }

  // =========================================================================
  // 14. Gestos táctiles de arrastre con inercia (Bottom Sheet Móvil)
  // =========================================================================
  const sheet = document.getElementById('bottomSheet');
  const sheetHandle = document.getElementById('sheetHandle');

  if (sheet && sheetHandle) {
    let startY = 0;
    let currentDeltaY = 0;
    let isDragging = false;
    let startTime = 0;

    sheetHandle.addEventListener('touchstart', (e) => {
      startY = e.touches[0].clientY;
      startTime = Date.now();
      isDragging = true;
      sheet.classList.add('dragging');
    }, { passive: true });

    window.addEventListener('touchmove', (e) => {
      if (!isDragging) return;
      const currentY = e.touches[0].clientY;
      const deltaY = currentY - startY;

      // Resistencia elástica si se intenta subir más allá del tope
      let translateY = deltaY;
      if (deltaY < 0) {
        translateY = deltaY * 0.35;
      }
      currentDeltaY = deltaY;
      sheet.style.transform = `translateY(${Math.max(-40, translateY)}px)`;
    }, { passive: true });

    window.addEventListener('touchend', () => {
      if (!isDragging) return;
      isDragging = false;
      sheet.classList.remove('dragging');
      sheet.style.transform = '';

      const elapsed = Date.now() - startTime;
      const velocity = currentDeltaY / Math.max(1, elapsed);

      // Swipe rápido hacia abajo o arrastre > 60px: colapsar
      if (velocity > 0.4 || currentDeltaY > 60) {
        sheet.classList.add('collapsed');
        sheet.classList.remove('expanded');
      }
      // Swipe rápido hacia arriba o arrastre < -40px: expandir
      else if (velocity < -0.4 || currentDeltaY < -40) {
        sheet.classList.add('expanded');
        sheet.classList.remove('collapsed');
      }
      currentDeltaY = 0;
    }, { passive: true });

    // Alternar con toque en el asa
    sheetHandle.addEventListener('click', () => {
      if (sheet.classList.contains('collapsed')) {
        sheet.classList.remove('collapsed');
      } else if (sheet.classList.contains('expanded')) {
        sheet.classList.remove('expanded');
      } else {
        sheet.classList.add('expanded');
      }
    });
  }

  // =========================================================================
  // 15. Registro de Service Worker (PWA Offline)
  // =========================================================================
  if ('serviceWorker' in navigator && (window.location.protocol.startsWith('http') || window.location.hostname === 'localhost')) {
    window.addEventListener('load', () => {
      navigator.serviceWorker.register('./sw.js').then((reg) => {
        console.log('[Wachi PWA] Service Worker activo:', reg.scope);
      }).catch((err) => {
        console.warn('[Wachi PWA] Registro de SW omitido:', err);
      });
    });
  }

  // =========================================================================
  // 13. Carga inicial
  // =========================================================================
  readUrlParams();
});
