<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useAuroraStore } from '@/stores/aurora'
import L from 'leaflet'
import 'leaflet/dist/leaflet.css'
import 'leaflet.heat'

const store = useAuroraStore()
const mapContainer = ref<HTMLElement | null>(null)
let map: L.Map | null = null
let ovationViewlineLayer: L.Polyline | null = null
let tonightViewlineLayer: L.Polyline | null = null
let heatLayer: L.Layer | null = null
let userMarker: L.Marker | null = null

const userLocation = computed(() => {
  if (!store.status) return null
  return {
    lat: store.status.location.latitude,
    lon: store.status.location.longitude,
    name: store.status.location.name,
  }
})

onMounted(() => {
  if (!mapContainer.value) return

  map = L.map(mapContainer.value, {
    center: [50, -30],
    zoom: 3,
    minZoom: 2,
    maxZoom: 8,
  })

  L.tileLayer('https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png', {
    attribution: '&copy; OpenStreetMap contributors &copy; CARTO',
    maxZoom: 20,
  }).addTo(map)

  updateMap()
})

watch(
  () => [store.viewline, store.tonightViewline, store.ovation, store.status],
  () => updateMap(),
  { deep: true },
)

function updateMap() {
  if (!map) return

  // OVATION viewline — dashed, dimmed (background reference)
  if (ovationViewlineLayer) {
    map.removeLayer(ovationViewlineLayer)
  }

  if (store.viewline.length > 0) {
    const latLngs: L.LatLngExpression[] = store.viewline.map((p) => [p.lat, p.lon])
    ovationViewlineLayer = L.polyline(latLngs, {
      color: '#aaaaaa',
      weight: 2,
      opacity: 0.4,
      dashArray: '6, 8',
      smoothFactor: 1,
    }).addTo(map)
  }

  // Tonight's Kp forecast viewline — solid red, matching NOAA style
  if (tonightViewlineLayer) {
    map.removeLayer(tonightViewlineLayer)
  }

  if (store.tonightViewline && store.tonightViewline.viewline.length > 0) {
    const latLngs: L.LatLngExpression[] = store.tonightViewline.viewline.map((p) => [p.lat, p.lon])
    tonightViewlineLayer = L.polyline(latLngs, {
      color: '#ff3333',
      weight: 3,
      opacity: 0.8,
      smoothFactor: 1,
    })
      .bindTooltip(`Tonight's viewline (Kp ${store.tonightViewline.max_kp})`, { sticky: true })
      .addTo(map)
  }

  // OVATION heatmap
  if (heatLayer) {
    map.removeLayer(heatLayer)
  }

  if (store.ovation && store.ovation.coordinates.length > 0) {
    // OVATION longitudes are 0..359, convert to -180..180 for Leaflet
    const normalizeLon = (lon: number) => (lon > 180 ? lon - 360 : lon)
    const heatData: [number, number, number][] = store.ovation.coordinates
      .filter((c) => c[2] > 0 && c[1] > 0) // Northern hemisphere, non-zero probability
      .map((c) => [c[1], normalizeLon(c[0]), c[2] / 100]) // [lat, lon, intensity]

    heatLayer = (L as any)
      .heatLayer(heatData, {
        radius: 15,
        blur: 20,
        maxZoom: 6,
        max: 1.0,
        gradient: {
          0.0: 'transparent',
          0.2: '#003300',
          0.4: '#006600',
          0.6: '#00cc00',
          0.8: '#66ff66',
          1.0: '#ffffff',
        },
      })
      .addTo(map)
  }

  // User location marker
  if (userMarker) {
    map.removeLayer(userMarker)
  }

  if (userLocation.value) {
    const icon = L.divIcon({
      html: `<div style="
        width: 12px; height: 12px;
        background: #4fc3f7;
        border: 2px solid white;
        border-radius: 50%;
        box-shadow: 0 0 8px rgba(79, 195, 247, 0.6);
      "></div>`,
      iconSize: [16, 16],
      iconAnchor: [8, 8],
      className: '',
    })

    userMarker = L.marker([userLocation.value.lat, userLocation.value.lon], { icon })
      .bindPopup(`<b>${userLocation.value.name}</b><br>${userLocation.value.lat}°N, ${userLocation.value.lon}°W`)
      .addTo(map)
  }
}
</script>

<template>
  <div ref="mapContainer" class="aurora-map"></div>
</template>

<style scoped>
.aurora-map {
  width: 100%;
  height: 100%;
  min-height: 400px;
  border-radius: 8px;
  overflow: hidden;
}
</style>
