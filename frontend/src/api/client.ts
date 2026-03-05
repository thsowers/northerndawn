import axios from 'axios'

const API_BASE = import.meta.env.VITE_API_BASE || 'http://localhost:3000'

const client = axios.create({
  baseURL: API_BASE,
  timeout: 10000,
})

export interface ViewlinePoint {
  lon: number
  lat: number
}

export interface OvationData {
  'Observation Time': string
  'Forecast Time': string
  coordinates: [number, number, number][] // [lon, lat, probability]
}

export interface KpIndex {
  time_tag: string
  kp_index: number
  estimated_kp: number | null
  kp: string | null
}

export interface KpForecast {
  time_tag: string
  kp: number
  observed: string
  noaa_scale: string
}

export interface SolarWind {
  time_tag: string
  speed: number
  density: number
  bz: number
  bt: number
}

export interface StatusResponse {
  healthy: boolean
  last_ovation_poll: string | null
  last_kp_poll: string | null
  last_solar_wind_poll: string | null
  alert_active: boolean
  location: {
    name: string
    latitude: number
    longitude: number
  }
}

export interface TonightViewlineResponse {
  viewline: ViewlinePoint[]
  max_kp: number
  window_start: string
  window_end: string
}

export interface Alert {
  timestamp: string
  alert_type: string
  viewline_lat: number
  user_lat: number
  kp: number
  notified_via: string[]
}

export const api = {
  async getViewline(): Promise<ViewlinePoint[]> {
    const { data } = await client.get('/api/aurora/viewline')
    return data
  },

  async getOvation(): Promise<OvationData> {
    const { data } = await client.get('/api/aurora/ovation')
    return data
  },

  async getKp(): Promise<KpIndex[]> {
    const { data } = await client.get('/api/aurora/kp')
    return data
  },

  async getKpForecast(): Promise<KpForecast[]> {
    const { data } = await client.get('/api/aurora/kp/forecast')
    return data
  },

  async getKpHistory(hours: number = 24): Promise<KpIndex[]> {
    const { data } = await client.get('/api/aurora/kp/history', { params: { hours } })
    return data
  },

  async getSolarWind(): Promise<SolarWind[]> {
    const { data } = await client.get('/api/aurora/solar-wind')
    return data
  },

  async getSolarWindHistory(hours: number = 24): Promise<SolarWind[]> {
    const { data } = await client.get('/api/aurora/solar-wind/history', { params: { hours } })
    return data
  },

  async getStatus(): Promise<StatusResponse> {
    const { data } = await client.get('/api/status')
    return data
  },

  async getTonightViewline(): Promise<TonightViewlineResponse | null> {
    try {
      const { data } = await client.get('/api/aurora/viewline/tonight')
      return data
    } catch {
      return null
    }
  },

  async getAlerts(): Promise<Alert[]> {
    const { data } = await client.get('/api/alerts')
    return data
  },
}
