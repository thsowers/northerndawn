import { defineStore } from 'pinia'
import { ref } from 'vue'
import {
  api,
  type ViewlinePoint,
  type OvationData,
  type KpIndex,
  type KpForecast,
  type SolarWind,
  type StatusResponse,
  type TonightViewlineResponse,
} from '@/api/client'

export const useAuroraStore = defineStore('aurora', () => {
  const viewline = ref<ViewlinePoint[]>([])
  const tonightViewline = ref<TonightViewlineResponse | null>(null)
  const ovation = ref<OvationData | null>(null)
  const kpCurrent = ref<KpIndex[]>([])
  const kpForecast = ref<KpForecast[]>([])
  const solarWind = ref<SolarWind[]>([])
  const status = ref<StatusResponse | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  let viewlineInterval: ReturnType<typeof setInterval> | null = null
  let kpInterval: ReturnType<typeof setInterval> | null = null
  let solarWindInterval: ReturnType<typeof setInterval> | null = null

  const VIEWLINE_POLL_MS = 5 * 60 * 1000
  const KP_POLL_MS = 60 * 1000
  const SOLAR_WIND_POLL_MS = 5 * 60 * 1000

  async function fetchViewline() {
    try {
      viewline.value = await api.getViewline()
    } catch (e) {
      console.error('Failed to fetch viewline:', e)
    }
  }

  async function fetchTonightViewline() {
    tonightViewline.value = await api.getTonightViewline()
  }

  async function fetchOvation() {
    try {
      ovation.value = await api.getOvation()
    } catch (e) {
      console.error('Failed to fetch ovation:', e)
    }
  }

  async function fetchKp() {
    try {
      kpCurrent.value = await api.getKp()
    } catch (e) {
      console.error('Failed to fetch Kp:', e)
    }
  }

  async function fetchKpForecast() {
    try {
      kpForecast.value = await api.getKpForecast()
    } catch (e) {
      console.error('Failed to fetch Kp forecast:', e)
    }
  }

  async function fetchSolarWind() {
    try {
      solarWind.value = await api.getSolarWind()
    } catch (e) {
      console.error('Failed to fetch solar wind:', e)
    }
  }

  async function fetchStatus() {
    try {
      status.value = await api.getStatus()
    } catch (e) {
      console.error('Failed to fetch status:', e)
    }
  }

  async function fetchAll() {
    loading.value = true
    error.value = null
    try {
      await Promise.all([
        fetchViewline(),
        fetchTonightViewline(),
        fetchOvation(),
        fetchKp(),
        fetchKpForecast(),
        fetchSolarWind(),
        fetchStatus(),
      ])
    } catch (e) {
      error.value = 'Failed to fetch data'
    } finally {
      loading.value = false
    }
  }

  function startPolling() {
    fetchAll()

    viewlineInterval = setInterval(() => {
      fetchViewline()
      fetchTonightViewline()
      fetchOvation()
      fetchStatus()
    }, VIEWLINE_POLL_MS)

    kpInterval = setInterval(() => {
      fetchKp()
      fetchKpForecast()
    }, KP_POLL_MS)

    solarWindInterval = setInterval(() => {
      fetchSolarWind()
    }, SOLAR_WIND_POLL_MS)
  }

  function stopPolling() {
    if (viewlineInterval) clearInterval(viewlineInterval)
    if (kpInterval) clearInterval(kpInterval)
    if (solarWindInterval) clearInterval(solarWindInterval)
  }

  return {
    viewline,
    tonightViewline,
    ovation,
    kpCurrent,
    kpForecast,
    solarWind,
    status,
    loading,
    error,
    fetchAll,
    startPolling,
    stopPolling,
  }
})
