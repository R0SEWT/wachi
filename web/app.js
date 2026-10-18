/**
 * Wachi — Frontend Móvil & Prototipo de Interacción UX
 * Stack: Vanilla JS + Leaflet.js (Sin build step)
 */

document.addEventListener('DOMContentLoaded', () => {
  // =========================================================================
  // 1. Datos Mockeados para Casos de Prueba (Lima Peatonal)
  // =========================================================================
  const PRESETS = {
    elena: {
      name: 'Elena',
      originName: 'Mi ubicación (Av. Javier Prado con Arequipa)',
      destName: 'Jr. Lloque Yupanqui 1120',
      time: '20:30',
      timeLabel: '20:30 (Noche)',
      originCoords: [-12.08807, -77.03387],
      destCoords: [-12.07379, -77.04650],
      tranquila: {
        duration: '38 min',
        dist: '2.9 km',
        delta: '+3 min',
        benefit: '· Prefiere alameda de Av. Arequipa y Av. Cuba iluminada',
        sparse: '1 tramo con datos insuficientes en Alejandro Tirado',
        coords: [
          [-12.08807, -77.03387],
          [-12.08727, -77.03400],
          [-12.08645, -77.03411],
          [-12.08568, -77.03422],
          [-12.08508, -77.03431],
          [-12.08415, -77.03444],
          [-12.08357, -77.03453],
          [-12.08298, -77.03461],
          [-12.08244, -77.03469],
          [-12.08136, -77.03485],
          [-12.08087, -77.03492],
          [-12.07985, -77.03507],
          [-12.07838, -77.03530],
          [-12.07750, -77.03543],
          [-12.07650, -77.03558],
          [-12.07551, -77.03574],
          [-12.07450, -77.03590],
          [-12.07455, -77.03630],
          [-12.07465, -77.03710],
          [-12.07480, -77.03830],
          [-12.07490, -77.03926],
          [-12.07495, -77.03984],
          [-12.07499, -77.04079],
          [-12.07505, -77.04169],
          [-12.07510, -77.04264],
          [-12.07507, -77.04424],
          [-12.07507, -77.04514],
          [-12.07505, -77.04638],
          [-12.07505, -77.04689],
          [-12.07490, -77.04689],
          [-12.07379, -77.04689],
          [-12.07379, -77.04650]
        ],
        sparseCoords: [
          [-12.07450, -77.03590],
          [-12.07455, -77.03630],
          [-12.07465, -77.03710],
          [-12.07480, -77.03830]
        ]
      },
      rapida: {
        duration: '35 min',
        dist: '2.7 km',
        delta: 'Directa',
        risk: '· Corta por zona de bares de Risso y calles interiores',
        coords: [
          [-12.08807, -77.03387],
          [-12.08645, -77.03411],
          [-12.08415, -77.03444],
          [-12.08244, -77.03469],
          [-12.08240, -77.03580],
          [-12.08235, -77.03690],
          [-12.08225, -77.03830],
          [-12.08210, -77.03980],
          [-12.08195, -77.04120],
          [-12.08180, -77.04250],
          [-12.07980, -77.04255],
          [-12.07750, -77.04260],
          [-12.07510, -77.04264],
          [-12.07507, -77.04424],
          [-12.07507, -77.04514],
          [-12.07505, -77.04638],
          [-12.07505, -77.04689],
          [-12.07490, -77.04689],
          [-12.07379, -77.04689],
          [-12.07379, -77.04650]
        ]
      }
    },
    mateo: {
      name: 'Mateo',
      originName: 'Campus PUCP (Puerta Principal)',
      destName: 'Estación La Marina / Universitaria',
      time: '22:15',
      timeLabel: '22:15 (Madrugada)',
      originCoords: [-12.0695, -77.0795],
      destCoords: [-12.0768, -77.077],
      tranquila: {
        duration: '15 min',
        dist: '1.1 km',
        delta: '+3 min',
        benefit: '· Sigue por vereda iluminada de Av. Universitaria con serenazgo',
        sparse: 'Cobertura normal en el tramo',
        coords: [[-12.06915,-77.07822],[-12.06915,-77.07819],[-12.06916,-77.07815],[-12.06917,-77.07812],[-12.06919,-77.0781],[-12.06922,-77.07807],[-12.06928,-77.07802],[-12.06939,-77.07794],[-12.06951,-77.07796],[-12.06987,-77.078],[-12.07016,-77.07804],[-12.07042,-77.07809],[-12.07047,-77.0781],[-12.07109,-77.07824],[-12.07157,-77.07835],[-12.07185,-77.0784],[-12.07193,-77.07842],[-12.07201,-77.07843],[-12.07212,-77.07844],[-12.0724,-77.07844],[-12.07245,-77.07844],[-12.07268,-77.07847],[-12.07277,-77.07847],[-12.07282,-77.07848],[-12.07326,-77.0786],[-12.07357,-77.07874],[-12.07389,-77.07894],[-12.07408,-77.07909],[-12.07415,-77.07916],[-12.07466,-77.07963],[-12.07479,-77.07975],[-12.07482,-77.07978],[-12.07492,-77.07985],[-12.07494,-77.07987],[-12.075,-77.07992],[-12.07515,-77.08003],[-12.07557,-77.08034],[-12.07575,-77.0801],[-12.07585,-77.0796],[-12.076,-77.079],[-12.0762,-77.0784],[-12.07645,-77.0778],[-12.07665,-77.0774],[-12.07676,-77.07714]],
        sparseCoords: []
      },
      rapida: {
        duration: '12 min',
        dist: '900 m',
        delta: 'Directa',
        risk: '· Corta por calles internas y oscuras de Pando de madrugada',
        coords: [[-12.06915, -77.07822], [-12.06915, -77.07819], [-12.06916, -77.07815], [-12.06917, -77.07812], [-12.06919, -77.0781], [-12.06922, -77.07807], [-12.06928, -77.07802], [-12.06939, -77.07794], [-12.06951, -77.07796], [-12.06987, -77.078], [-12.07016, -77.07804], [-12.07025, -77.07804], [-12.07028, -77.07804], [-12.0703, -77.07804], [-12.07033, -77.07803], [-12.07034, -77.07803], [-12.07036, -77.07802], [-12.07037, -77.07801], [-12.07038, -77.078], [-12.07039, -77.07799], [-12.0704, -77.07798], [-12.0704, -77.07796], [-12.07041, -77.07794], [-12.07042, -77.07793], [-12.07044, -77.07785], [-12.07047, -77.07776], [-12.07056, -77.07738], [-12.07061, -77.07715], [-12.07076, -77.07646], [-12.07089, -77.07584], [-12.0709, -77.07578], [-12.07091, -77.07575], [-12.07092, -77.07572], [-12.07095, -77.07573], [-12.07099, -77.07573], [-12.07134, -77.07578], [-12.07154, -77.0758], [-12.07165, -77.07582], [-12.07172, -77.07582], [-12.07181, -77.07584], [-12.07208, -77.07587], [-12.07248, -77.07593], [-12.07327, -77.07605], [-12.07372, -77.07611], [-12.07397, -77.07614], [-12.0745, -77.07621], [-12.07516, -77.07629], [-12.07536, -77.07632], [-12.07548, -77.07634], [-12.07553, -77.07634], [-12.07563, -77.07635], [-12.07569, -77.07635], [-12.07576, -77.07636], [-12.07614, -77.07645], [-12.07605, -77.07693], [-12.07612, -77.07695], [-12.07676, -77.07714]]
      }
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
    timeBucket: '20:30',
    isDeviated: false,
    customOrigin: null,        // { name, coords: [lat, lon] }
    customDest: null,          // { name, coords: [lat, lon] }
    isPickingOnMap: false
  };

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

  function initMapLayers() {
    const data = PRESETS[state.preset];

    // Limpiar capas previas
    if (layerTranquila) map.removeLayer(layerTranquila);
    if (layerSparse) map.removeLayer(layerSparse);
    if (layerRapida) map.removeLayer(layerRapida);
    if (originMarker) map.removeLayer(originMarker);
    if (destMarker) map.removeLayer(destMarker);
    if (liveGpsMarker) map.removeLayer(liveGpsMarker);

    // Ruta Rápida (delgada, color ámbar/atenuado)
    layerRapida = L.polyline(data.rapida.coords, {
      color: '#f59e0b',
      weight: 3.5,
      opacity: 0.6,
      dashArray: '5, 8'
    }).addTo(map);

    // Ruta Tranquila (trazo sólido de alto contraste)
    layerTranquila = L.polyline(data.tranquila.coords, {
      color: '#22c55e',
      weight: 5.5,
      opacity: 0.95
    }).addTo(map);

    // Segmento con datos insuficientes (gris punteado sobre la tranquila)
    if (data.tranquila.sparseCoords && data.tranquila.sparseCoords.length > 0) {
      layerSparse = L.polyline(data.tranquila.sparseCoords, {
        color: '#94a3b8',
        weight: 6,
        dashArray: '4, 10',
        opacity: 0.9
      }).addTo(map);
    }

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

    originMarker = L.marker(data.originCoords, { icon: iconOrigin }).addTo(map);
    destMarker = L.marker(data.destCoords, { icon: iconDest }).addTo(map);

    // Marcador GPS en marcha
    const iconLiveGps = L.divIcon({
      className: 'custom-live-gps',
      html: '<div style="background:#22c55e; width:16px; height:16px; border-radius:50%; border:3px solid #ffffff; box-shadow:0 0 10px #22c55e;"></div>',
      iconSize: [16, 16],
      iconAnchor: [8, 8]
    });
    liveGpsMarker = L.marker(data.originCoords, { icon: iconLiveGps });

    map.fitBounds(layerTranquila.getBounds(), { padding: [50, 50], maxZoom: 16 });
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
      updateShareUrl();
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
    params.set('time', state.timeBucket);
    params.set('theme', state.theme);
    window.history.replaceState({}, '', `${window.location.pathname}?${params.toString()}`);
  }

  function readUrlParams() {
    const params = new URLSearchParams(window.location.search);
    if (params.get('preset') && PRESETS[params.get('preset')]) {
      state.preset = params.get('preset');
    }
    if (params.get('theme')) {
      state.theme = params.get('theme');
    }
    if (params.get('time')) {
      state.timeBucket = params.get('time');
    }
    if (params.get('route')) {
      state.selectedRoute = params.get('route');
    }
    const initialScreen = params.get('screen') || 'search';

    applyTheme(state.theme);
    updatePresetUI();
    initMapLayers();
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

  function updateShareUrl() {
    const fullUrl = `${window.location.origin}${window.location.pathname}?screen=share&preset=${state.preset}&route=${state.selectedRoute}&time=${state.timeBucket}`;
    const shareInput = document.getElementById('shareUrlInput');
    if (shareInput) shareInput.value = fullUrl;
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
    document.getElementById('badgeCurrentTime').innerText = data.time + ' h';
    document.getElementById('timeLabel').innerText = data.timeLabel;

    // Actualizar tarjeta Tranquila
    const cardT = document.getElementById('cardTranquila');
    if (cardT) {
      cardT.querySelector('.route-duration').innerText = data.tranquila.duration;
      cardT.querySelector('.delta-time').innerText = data.tranquila.delta;
      cardT.querySelector('.delta-desc').innerText = data.tranquila.benefit;
      const sparseEl = cardT.querySelector('.sparse-desc');
      if (sparseEl) sparseEl.innerText = data.tranquila.sparse;
    }

    // Actualizar tarjeta Rápida
    const cardR = document.getElementById('cardRapida');
    if (cardR) {
      cardR.querySelector('.route-duration').innerText = data.rapida.duration;
      cardR.querySelector('.delta-time').innerText = data.rapida.delta;
      cardR.querySelector('.delta-desc').innerText = data.rapida.risk;
    }

    // Actualizar modo En Marcha & Glanceable Turn Card
    const walkingDest = document.querySelector('.walking-dest-name');
    if (walkingDest) walkingDest.innerText = data.destName;
    const etaMins = document.querySelector('.eta-mins');
    if (etaMins) etaMins.innerText = data.tranquila.duration;
    const etaDist = document.querySelector('.eta-dist');
    if (etaDist) etaDist.innerText = data.tranquila.dist;

    const turnDist = document.querySelector('.turn-dist');
    const turnInst = document.querySelector('.turn-instruction');
    const turnLandmark = document.querySelector('.turn-landmark');
    if (turnDist && turnInst && turnLandmark) {
      if (state.preset === 'elena') {
        turnDist.innerText = 'EN 140 METROS';
        turnInst.innerText = 'Continúa por Av. Arequipa';
        turnLandmark.innerText = '👁 Hito: Vereda ancha y paradero con luz activa';
      } else {
        turnDist.innerText = 'EN 90 METROS';
        turnInst.innerText = 'Sigue por Av. Universitaria';
        turnLandmark.innerText = '👁 Hito: Garita de serenazgo y alumbrado público';
      }
    }

    // Actualizar vista Compartir / Acompañante
    const pomTitle = document.querySelector('.pom-title');
    if (pomTitle) pomTitle.innerText = `${data.name} va caminando por ruta tranquila`;
    const pomTime = document.querySelector('.pom-time');
    if (pomTime) pomTime.innerText = `Salida: ${data.time}`;
    const btnCall = document.querySelector('.btn-call span');
    if (btnCall) btnCall.innerText = `Llamar a ${data.name}`;

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
  // 7.1. Geocodificación en Vivo y Autocompletado (Photon Komoot & OSRM Foot)
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

    const originCoords = state.customOrigin ? state.customOrigin.coords : PRESETS[state.preset].originCoords;
    const destCoords = state.customDest ? state.customDest.coords : PRESETS[state.preset].destCoords;

    calculateDynamicRoute(originCoords, destCoords, state.customDest ? state.customDest.name : 'Destino seleccionado');
  }

  async function calculateDynamicRoute(originCoords, destCoords, destLabel) {
    const url = `https://router.project-osrm.org/route/v1/foot/${originCoords[1]},${originCoords[0]};${destCoords[1]},${destCoords[0]}?overview=full&geometries=geojson`;

    try {
      const resp = await fetch(url);
      if (!resp.ok) throw new Error('OSRM error');
      const data = await resp.json();

      if (data.routes && data.routes.length > 0) {
        const route = data.routes[0];
        const coords = route.geometry.coordinates.map(c => [c[1], c[0]]);
        const distMeters = route.distance;
        const distKm = (distMeters / 1000).toFixed(1) + ' km';
        // Velocidad promedio peatonal urbana: 4.5 km/h = 75 m/min
        const durationMin = Math.max(1, Math.round(distMeters / 75));

        if (layerTranquila) {
          layerTranquila.setLatLngs(coords);
          layerTranquila.setStyle({ color: '#22c55e', weight: 5.5, opacity: 0.95 });
        }
        if (layerSparse) {
          map.removeLayer(layerSparse);
          layerSparse = null;
        }
        if (layerRapida) {
          layerRapida.setLatLngs(coords);
          layerRapida.setStyle({ color: '#f59e0b', weight: 3.5, opacity: 0.6, dashArray: '5, 8' });
        }

        // Actualizar tarjetas de pantalla 2 (Comparar)
        const cardT = document.getElementById('cardTranquila');
        if (cardT) {
          cardT.querySelector('.route-duration').innerText = `${durationMin} min`;
          cardT.querySelector('.delta-time').innerText = '+2 min';
          cardT.querySelector('.delta-desc').innerText = '· Prioriza veredas iluminadas y avenidas con serenazgo';
          const sparseEl = cardT.querySelector('.sparse-desc');
          if (sparseEl) sparseEl.innerText = 'Ruta peatonal en vivo';
        }

        const cardR = document.getElementById('cardRapida');
        if (cardR) {
          const rapidaMin = Math.max(1, durationMin - 2);
          cardR.querySelector('.route-duration').innerText = `${rapidaMin} min`;
          cardR.querySelector('.delta-time').innerText = 'Directa';
          cardR.querySelector('.delta-desc').innerText = '· Ruta directa por vía vehicular';
        }

        // Actualizar pantalla 4 (En Marcha)
        const walkingDest = document.querySelector('.walking-dest-name');
        if (walkingDest) walkingDest.innerText = destLabel;
        const etaMins = document.querySelector('.eta-mins');
        if (etaMins) etaMins.innerText = `${durationMin} min`;
        const etaDist = document.querySelector('.eta-dist');
        if (etaDist) etaDist.innerText = distKm;
        const turnInst = document.querySelector('.turn-instruction');
        if (turnInst) turnInst.innerText = `Rumbo a ${destLabel.split('(')[0].trim()}`;

        // Pantalla 5 (Compartir)
        const pomTitle = document.querySelector('.pom-title');
        if (pomTitle) pomTitle.innerText = `Caminando hacia ${destLabel.split('(')[0].trim()}`;

        if (layerTranquila) {
          map.fitBounds(layerTranquila.getBounds(), { padding: [50, 50], maxZoom: 16 });
        }
      }
    } catch (err) {
      console.warn('[Wachi Router] Error calculando ruta con OSRM:', err);
      const directCoords = [originCoords, destCoords];
      if (layerTranquila) layerTranquila.setLatLngs(directCoords);
      map.fitBounds(L.latLngBounds([originCoords, destCoords]), { padding: [50, 50] });
    }
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

    const originCoords = state.customOrigin ? state.customOrigin.coords : PRESETS[state.preset].originCoords;
    calculateDynamicRoute(originCoords, coords, label);
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
            const destCoords = state.customDest ? state.customDest.coords : PRESETS[state.preset].destCoords;
            calculateDynamicRoute(coords, destCoords, state.customDest ? state.customDest.name : PRESETS[state.preset].destName);
          },
          (err) => {
            console.warn('[Wachi GPS] Ubicación no autorizada:', err);
            const coords = PRESETS[state.preset].originCoords;
            map.setView(coords, 16);
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
  document.getElementById('btnCalculateRoute').addEventListener('click', () => {
    setScreen('compare');
  });

  // Selección de Tarjeta (Tranquila vs Rápida)
  const cardTranquila = document.getElementById('cardTranquila');
  const cardRapida = document.getElementById('cardRapida');

  cardTranquila.addEventListener('click', () => {
    state.selectedRoute = 'safe';
    cardTranquila.classList.add('selected');
    cardRapida.classList.remove('selected');
    cardTranquila.querySelector('.card-radio').innerText = '●';
    cardRapida.querySelector('.card-radio').innerText = '○';
    layerTranquila.setStyle({ opacity: 0.95, weight: 5.5 });
    layerRapida.setStyle({ opacity: 0.4, weight: 3 });
  });

  cardRapida.addEventListener('click', () => {
    state.selectedRoute = 'fast';
    cardRapida.classList.add('selected');
    cardTranquila.classList.remove('selected');
    cardRapida.querySelector('.card-radio').innerText = '●';
    cardTranquila.querySelector('.card-radio').innerText = '○';
    layerRapida.setStyle({ opacity: 0.95, weight: 5.5 });
    layerTranquila.setStyle({ opacity: 0.4, weight: 3 });
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
    const data = PRESETS[state.preset];
    const url = `https://www.google.com/maps/dir/?api=1&origin=${data.originCoords[0]},${data.originCoords[1]}&destination=${data.destCoords[0]},${data.destCoords[1]}&travelmode=walking`;
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
    const text = `Hola, voy caminando por una ruta tranquila que calculé con Wachi. Puedes ver mi trayecto y hora estimada aquí: ${shareInput}`;
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

  // Selector de franja horaria
  const modalTimePicker = document.getElementById('modalTimePicker');
  const btnTimeSelect = document.getElementById('btnTimeSelect');
  const btnCloseTimePicker = document.getElementById('btnCloseTimePicker');

  btnTimeSelect.addEventListener('click', () => {
    modalTimePicker.classList.remove('hidden');
  });

  btnCloseTimePicker.addEventListener('click', () => {
    modalTimePicker.classList.add('hidden');
  });

  document.querySelectorAll('.time-bucket-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      document.querySelectorAll('.time-bucket-btn').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      state.timeBucket = btn.dataset.hour;
      document.getElementById('timeLabel').innerText = `${state.timeBucket} (${btn.innerText.split(' ')[0]})`;
      document.getElementById('badgeCurrentTime').innerText = `${state.timeBucket} h`;
      modalTimePicker.classList.add('hidden');
      updateUrlParams();
    });
  });

  // Recentrar mapa
  document.getElementById('btnRecenterGPS').addEventListener('click', () => {
    const data = PRESETS[state.preset];
    map.setView(data.originCoords, 16);
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
