use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{error, info, warn};

use crate::models::{Alert, AlertType};
use crate::noaa::NoaaClient;
use crate::state::AppState;
use crate::viewline;

pub fn spawn_polling_tasks(state: Arc<AppState>) {
    let noaa = Arc::new(NoaaClient::new());

    spawn_ovation_poll(Arc::clone(&state), Arc::clone(&noaa));
    spawn_kp_poll(Arc::clone(&state), Arc::clone(&noaa));
    spawn_kp_forecast_poll(Arc::clone(&state), Arc::clone(&noaa));
    spawn_solar_wind_poll(Arc::clone(&state), Arc::clone(&noaa));
}

fn spawn_ovation_poll(state: Arc<AppState>, noaa: Arc<NoaaClient>) {
    let interval_secs = state.config.polling.ovation_interval_secs;

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(interval_secs));

        loop {
            interval.tick().await;

            match noaa.fetch_ovation().await {
                Ok(ovation) => {
                    // Compute viewline from OVATION data
                    let mut vl = viewline::compute_viewline_from_ovation(&ovation);

                    // Fall back to Kp-based viewline if OVATION yields no boundary
                    if vl.is_empty() {
                        let kp_data = state.cache.kp_current.read().unwrap();
                        if let Some(latest) = kp_data.last() {
                            let kp = latest.kp_index;
                            vl = viewline::compute_viewline(kp);
                            info!(kp = kp, "OVATION boundary empty, using Kp fallback");
                        }
                    } else {
                        info!(viewline_points = vl.len(), "Viewline computed from OVATION");
                    }

                    if !vl.is_empty() {
                        // Check if aurora is visible at user's location
                        let user_lat = state.config.location.latitude;
                        let user_lon = state.config.location.longitude;

                        if let Some(vl_lat) =
                            viewline::is_aurora_visible(&vl, user_lat, user_lon)
                        {
                            info!(
                                viewline_lat = vl_lat,
                                user_lat = user_lat,
                                "Aurora potentially visible!"
                            );

                            let kp = state
                                .cache
                                .kp_current
                                .read()
                                .unwrap()
                                .last()
                                .map(|k| k.kp_index)
                                .unwrap_or(0.0);

                            let mut alert = Alert {
                                timestamp: Utc::now(),
                                alert_type: AlertType::AuroraVisible,
                                viewline_lat: vl_lat,
                                user_lat,
                                kp,
                                notified_via: Vec::new(),
                            };

                            if let Some(_used) = state.notifications.notify(&mut alert) {
                                if let Err(e) = state.db.insert_alert(&alert) {
                                    error!("Failed to persist alert: {}", e);
                                }
                            }

                            *state.cache.alert_active.write().unwrap() = true;
                        } else {
                            *state.cache.alert_active.write().unwrap() = false;
                        }

                        // Persist viewline snapshot
                        let now = Utc::now();
                        if let Err(e) = state.db.insert_viewline_snapshot(&now, &vl) {
                            warn!("Failed to persist viewline snapshot: {}", e);
                        }

                        *state.cache.viewline.write().unwrap() = vl;
                    }

                    *state.cache.ovation.write().unwrap() = Some(ovation);
                    *state.cache.last_ovation_poll.write().unwrap() = Some(Utc::now());
                }
                Err(e) => error!("Failed to fetch OVATION data: {}", e),
            }
        }
    });
}

fn spawn_kp_poll(state: Arc<AppState>, noaa: Arc<NoaaClient>) {
    let interval_secs = state.config.polling.kp_interval_secs;

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(interval_secs));

        loop {
            interval.tick().await;

            match noaa.fetch_kp_index().await {
                Ok(kp_data) => {
                    // Persist the latest reading
                    if let Some(latest) = kp_data.last() {
                        if let Err(e) = state.db.insert_kp_reading(latest) {
                            warn!("Failed to persist Kp reading: {}", e);
                        }

                        // Check Kp threshold
                        let kp = latest.kp_index;
                        if kp >= state.config.thresholds.kp_min {
                            info!(kp = kp, "Kp threshold exceeded");
                        }
                    }

                    *state.cache.kp_current.write().unwrap() = kp_data;
                    *state.cache.last_kp_poll.write().unwrap() = Some(Utc::now());
                }
                Err(e) => error!("Failed to fetch Kp index: {}", e),
            }
        }
    });
}

fn spawn_kp_forecast_poll(state: Arc<AppState>, noaa: Arc<NoaaClient>) {
    let interval_secs = state.config.polling.kp_forecast_interval_secs;

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(interval_secs));

        loop {
            interval.tick().await;

            match noaa.fetch_kp_forecast().await {
                Ok(forecast) => {
                    let tonight = viewline::compute_tonight_viewline(&forecast, Utc::now());
                    *state.cache.tonight_viewline.write().unwrap() = tonight;
                    *state.cache.kp_forecast.write().unwrap() = forecast;
                }
                Err(e) => error!("Failed to fetch Kp forecast: {}", e),
            }
        }
    });
}

fn spawn_solar_wind_poll(state: Arc<AppState>, noaa: Arc<NoaaClient>) {
    let interval_secs = state.config.polling.solar_wind_interval_secs;

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(interval_secs));

        loop {
            interval.tick().await;

            match noaa.fetch_solar_wind().await {
                Ok(sw_data) => {
                    // Persist latest reading
                    if let Some(latest) = sw_data.last() {
                        if let Err(e) = state.db.insert_solar_wind(latest) {
                            warn!("Failed to persist solar wind reading: {}", e);
                        }
                    }

                    *state.cache.solar_wind.write().unwrap() = sw_data;
                    *state.cache.last_solar_wind_poll.write().unwrap() = Some(Utc::now());
                }
                Err(e) => error!("Failed to fetch solar wind: {}", e),
            }
        }
    });
}
