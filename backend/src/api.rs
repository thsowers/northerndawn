use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use std::sync::Arc;

use crate::models::{LocationInfo, StatusResponse};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct HistoryQuery {
    hours: Option<i64>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/aurora/viewline", get(get_viewline))
        .route("/api/aurora/viewline/tonight", get(get_tonight_viewline))
        .route("/api/aurora/ovation", get(get_ovation))
        .route("/api/aurora/kp", get(get_kp))
        .route("/api/aurora/kp/forecast", get(get_kp_forecast))
        .route("/api/aurora/kp/history", get(get_kp_history))
        .route("/api/aurora/solar-wind", get(get_solar_wind))
        .route("/api/aurora/solar-wind/history", get(get_solar_wind_history))
        .route("/api/status", get(get_status))
        .route("/api/config", get(get_config))
        .route("/api/alerts", get(get_alerts))
}

async fn get_viewline(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let viewline = state.cache.viewline.read().unwrap().clone();
    Json(serde_json::to_value(viewline).unwrap())
}

async fn get_tonight_viewline(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let tonight = state.cache.tonight_viewline.read().unwrap().clone();
    match tonight {
        Some(data) => Ok(Json(serde_json::to_value(data).unwrap())),
        None => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn get_ovation(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let ovation = state.cache.ovation.read().unwrap().clone();
    match ovation {
        Some(data) => Ok(Json(serde_json::to_value(data).unwrap())),
        None => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn get_kp(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let kp = state.cache.kp_current.read().unwrap().clone();
    Json(serde_json::to_value(kp).unwrap())
}

async fn get_kp_forecast(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let forecast = state.cache.kp_forecast.read().unwrap().clone();
    Json(serde_json::to_value(forecast).unwrap())
}

async fn get_kp_history(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let hours = query.hours.unwrap_or(24);
    match state.db.get_kp_history(hours) {
        Ok(data) => Ok(Json(serde_json::to_value(data).unwrap())),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_solar_wind(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let sw = state.cache.solar_wind.read().unwrap().clone();
    Json(serde_json::to_value(sw).unwrap())
}

async fn get_solar_wind_history(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let hours = query.hours.unwrap_or(24);
    match state.db.get_solar_wind_history(hours) {
        Ok(data) => Ok(Json(serde_json::to_value(data).unwrap())),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_status(
    State(state): State<Arc<AppState>>,
) -> Json<StatusResponse> {
    let status = StatusResponse {
        healthy: true,
        last_ovation_poll: *state.cache.last_ovation_poll.read().unwrap(),
        last_kp_poll: *state.cache.last_kp_poll.read().unwrap(),
        last_solar_wind_poll: *state.cache.last_solar_wind_poll.read().unwrap(),
        alert_active: *state.cache.alert_active.read().unwrap(),
        location: LocationInfo {
            name: state.config.location.name.clone(),
            latitude: state.config.location.latitude,
            longitude: state.config.location.longitude,
        },
    };
    Json(status)
}

async fn get_config(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let config = serde_json::json!({
        "location": {
            "latitude": state.config.location.latitude,
            "longitude": state.config.location.longitude,
            "name": state.config.location.name,
        },
        "thresholds": {
            "aurora_probability_min": state.config.thresholds.aurora_probability_min,
            "kp_min": state.config.thresholds.kp_min,
        },
    });
    Json(config)
}

async fn get_alerts(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.db.get_recent_alerts(50) {
        Ok(alerts) => Ok(Json(serde_json::to_value(alerts).unwrap())),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
