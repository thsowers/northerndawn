use chrono::{DateTime, Utc};
use std::sync::{Arc, RwLock};

use crate::config::AppConfig;
use crate::db::Database;
use crate::models::{KpForecast, KpIndex, OvationResponse, SolarWind, TonightViewlineResponse, ViewlinePoint};
use crate::notifications::NotificationManager;

/// Shared application state accessible from API handlers and polling tasks.
pub struct AppState {
    pub config: AppConfig,
    pub db: Database,
    pub notifications: NotificationManager,
    pub cache: Cache,
}

/// In-memory cache for latest data (served via API).
pub struct Cache {
    pub ovation: RwLock<Option<OvationResponse>>,
    pub viewline: RwLock<Vec<ViewlinePoint>>,
    pub tonight_viewline: RwLock<Option<TonightViewlineResponse>>,
    pub kp_current: RwLock<Vec<KpIndex>>,
    pub kp_forecast: RwLock<Vec<KpForecast>>,
    pub solar_wind: RwLock<Vec<SolarWind>>,
    pub last_ovation_poll: RwLock<Option<DateTime<Utc>>>,
    pub last_kp_poll: RwLock<Option<DateTime<Utc>>>,
    pub last_solar_wind_poll: RwLock<Option<DateTime<Utc>>>,
    pub alert_active: RwLock<bool>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            ovation: RwLock::new(None),
            viewline: RwLock::new(Vec::new()),
            tonight_viewline: RwLock::new(None),
            kp_current: RwLock::new(Vec::new()),
            kp_forecast: RwLock::new(Vec::new()),
            solar_wind: RwLock::new(Vec::new()),
            last_ovation_poll: RwLock::new(None),
            last_kp_poll: RwLock::new(None),
            last_solar_wind_poll: RwLock::new(None),
            alert_active: RwLock::new(false),
        }
    }
}

impl AppState {
    pub fn new(config: AppConfig) -> anyhow::Result<Arc<Self>> {
        let db = Database::new(&config.database.path)?;
        let notifications =
            NotificationManager::new(&config.notifications, &config.email);

        Ok(Arc::new(Self {
            config,
            db,
            notifications,
            cache: Cache::new(),
        }))
    }
}
