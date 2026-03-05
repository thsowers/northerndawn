use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// --- OVATION Aurora ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvationResponse {
    #[serde(rename = "Observation Time")]
    pub observation_time: String,
    #[serde(rename = "Forecast Time")]
    pub forecast_time: String,
    pub coordinates: Vec<[f64; 3]>, // [lon, lat, probability]
}

// --- Kp Index (1-minute) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KpIndex {
    pub time_tag: String,
    pub kp_index: f64,
    pub estimated_kp: Option<f64>,
    pub kp: Option<String>,
}

// --- Kp Forecast ---
// The NOAA forecast endpoint returns an array of arrays (first row is headers).

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KpForecast {
    pub time_tag: String,
    pub kp: f64,
    pub observed: String,
    pub noaa_scale: String,
}

// --- Solar Wind ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolarWind {
    pub time_tag: String,
    pub speed: f64,
    pub density: f64,
    pub bz: f64,
    pub bt: f64,
}

// --- Viewline ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewlinePoint {
    pub lon: f64,
    pub lat: f64,
}

// --- Alert ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub timestamp: DateTime<Utc>,
    pub alert_type: AlertType,
    pub viewline_lat: f64,
    pub user_lat: f64,
    pub kp: f64,
    pub notified_via: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    AuroraVisible,
    KpThresholdExceeded,
}

impl std::fmt::Display for AlertType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertType::AuroraVisible => write!(f, "aurora_visible"),
            AlertType::KpThresholdExceeded => write!(f, "kp_threshold_exceeded"),
        }
    }
}

// --- Tonight's Viewline ---

#[derive(Debug, Clone, Serialize)]
pub struct TonightViewlineResponse {
    pub viewline: Vec<ViewlinePoint>,
    pub max_kp: f64,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
}

// --- API response wrappers ---

#[derive(Debug, Clone, Serialize)]
pub struct StatusResponse {
    pub healthy: bool,
    pub last_ovation_poll: Option<DateTime<Utc>>,
    pub last_kp_poll: Option<DateTime<Utc>>,
    pub last_solar_wind_poll: Option<DateTime<Utc>>,
    pub alert_active: bool,
    pub location: LocationInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocationInfo {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
}
