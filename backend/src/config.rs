use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub location: LocationConfig,
    pub thresholds: ThresholdsConfig,
    pub polling: PollingConfig,
    pub notifications: NotificationsConfig,
    pub email: EmailConfig,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LocationConfig {
    pub latitude: f64,
    pub longitude: f64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThresholdsConfig {
    pub aurora_probability_min: f64,
    pub kp_min: f64,
}

fn default_swpc_alerts_interval() -> u64 {
    60
}

#[derive(Debug, Clone, Deserialize)]
pub struct PollingConfig {
    pub ovation_interval_secs: u64,
    pub kp_interval_secs: u64,
    pub kp_forecast_interval_secs: u64,
    pub solar_wind_interval_secs: u64,
    #[serde(default = "default_swpc_alerts_interval")]
    pub swpc_alerts_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationsConfig {
    pub webhook_url: String,
    pub email_enabled: bool,
    pub desktop_enabled: bool,
    pub cooldown_minutes: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_pass: String,
    pub to_address: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
}

pub fn load_config() -> anyhow::Result<AppConfig> {
    let settings = config::Config::builder()
        .add_source(config::File::with_name("config").required(false))
        .add_source(config::Environment::with_prefix("NORTHERNDAWN").separator("__"))
        .build()?;

    let config: AppConfig = settings.try_deserialize()?;
    Ok(config)
}
