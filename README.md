# Northern Dawn

A real-time aurora borealis monitoring dashboard. Tracks solar wind conditions, geomagnetic activity, and aurora viewline position using live data from NOAA's Space Weather Prediction Center (SWPC). Get notified when the northern lights might be visible from your location.

![Dashboard](docs/screenshots/dashboard.png)

## Features

- **Live Aurora Viewline Map** — Interactive Leaflet map showing the real-time aurora viewline (how far south the aurora extends) overlaid on your configured location
- **Kp Index Tracking** — Current planetary K-index with a color-coded 3-day forecast bar chart
- **Solar Wind Monitor** — Real-time speed, density, and Bz/Bt magnetic field readings with historical sparkline charts
- **SWPC Alerts Feed** — Live stream of space weather alerts, watches, and warnings from NOAA
- **NOAA Scale Indicators** — Current geomagnetic storm, solar radiation, and radio blackout scale levels
- **Notifications** — Configurable alerts via desktop notifications, webhooks, or email when aurora conditions reach your thresholds
- **Notification Cooldown** — Avoid alert fatigue with configurable cooldown periods between notifications
- **WebSocket Updates** — Dashboard updates in real-time without polling
- **Historical Data** — Kp index and solar wind data stored in SQLite for historical queries
- **TUI Client** — Terminal-based dashboard using Ratatui for headless/SSH environments
- **Responsive Layout** — Works on desktop and mobile

<details>
<summary>Mobile View</summary>

![Mobile](docs/screenshots/mobile.png)

</details>

## Architecture

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐
│  NOAA SWPC  │────>│   Backend    │────>│   Frontend   │
│   APIs      │     │  (Rust/Axum) │     │  (Vue/Vite)  │
└─────────────┘     └──────┬───────┘     └──────────────┘
                           │
                    ┌──────┴───────┐
                    │   SQLite DB  │
                    └──────────────┘
```

- **Backend** — Rust with Axum. Polls NOAA SWPC APIs on configurable intervals, computes the aurora viewline at your longitude, caches current state, persists history to SQLite, and pushes updates over WebSocket.
- **Frontend** — Vue 3 + Vite + TypeScript. Interactive dashboard with Leaflet map, real-time charts, and a Pinia store synced via WebSocket.
- **TUI** — Ratatui-based terminal client that queries the backend API and renders Kp forecasts, solar wind data, and alerts in the terminal.

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (1.82+)
- [Node.js](https://nodejs.org/) (20+)

### Development

```bash
# Clone and configure
git clone https://github.com/thsowers/northerndawn.git
cd northerndawn/backend
cp config.example.toml config.toml
# Edit config.toml with your latitude/longitude
```

```bash
# Start both servers (backend + frontend dev)
./dev.sh
```

The dashboard will be available at `http://localhost:5173` with the API at `http://localhost:3000`.

### Production

```bash
# Build and run optimized
./prod.sh
```

This builds the frontend, compiles the backend in release mode, and serves everything from the backend on port 3000.

### Docker

```bash
docker build -t northerndawn .
docker run -p 3000:3000 -v ./backend/config.toml:/app/config.toml northerndawn
```

### Systemd

A service file is included for running as a system service:

```bash
sudo cp northerndawn.service /etc/systemd/system/
sudo systemctl enable --now northerndawn
```

### TUI

```bash
cargo run -p tui
```

Requires the backend to be running. Connects to `http://localhost:3000` by default.

## Configuration

All configuration lives in `backend/config.toml`. Copy from `backend/config.example.toml` to get started.

| Section | Key | Description |
|---|---|---|
| `[location]` | `latitude`, `longitude`, `name` | Your observation location |
| `[thresholds]` | `aurora_probability_min` | Minimum aurora probability (%) to trigger alerts |
| | `kp_min` | Minimum Kp index to trigger alerts |
| `[polling]` | `ovation_interval_secs` | How often to poll OVATION aurora model (default: 300) |
| | `kp_interval_secs` | Kp index poll interval (default: 60) |
| | `solar_wind_interval_secs` | Solar wind data poll interval (default: 300) |
| `[notifications]` | `webhook_url` | URL for webhook POST notifications |
| | `email_enabled` | Enable email notifications |
| | `desktop_enabled` | Enable desktop notifications |
| | `cooldown_minutes` | Minimum time between notifications (default: 30) |
| `[email]` | `smtp_host`, `smtp_port`, etc. | SMTP configuration for email alerts |
| `[server]` | `host`, `port` | Backend listen address (default: 127.0.0.1:3000) |
| `[database]` | `path` | SQLite database file path |

## API

The backend exposes a REST API:

| Endpoint | Description |
|---|---|
| `GET /api/aurora/viewline` | Current aurora viewline |
| `GET /api/aurora/viewline/tonight` | Tonight's viewline forecast |
| `GET /api/aurora/ovation` | OVATION aurora model data |
| `GET /api/aurora/kp` | Current Kp index |
| `GET /api/aurora/kp/forecast` | 3-day Kp forecast |
| `GET /api/aurora/kp/history?hours=24` | Historical Kp data |
| `GET /api/aurora/solar-wind` | Current solar wind conditions |
| `GET /api/aurora/solar-wind/history?hours=24` | Historical solar wind data |
| `GET /api/aurora/swpc-alerts` | NOAA SWPC alerts |
| `GET /api/aurora/noaa-scales` | Current NOAA space weather scales |
| `GET /api/status` | Backend health and polling status |
| `GET /api/config` | Current location and threshold config |
| `GET /api/alerts` | Recent notification history |
| `WS /api/ws` | WebSocket for real-time updates |

## Data Sources

All data is sourced from [NOAA Space Weather Prediction Center](https://www.swpc.noaa.gov/):

- **OVATION Aurora Model** — Short-term aurora forecast
- **Planetary K-index** — Global geomagnetic activity indicator
- **ACE/DSCOVR Solar Wind** — Real-time solar wind speed, density, and magnetic field
- **SWPC Alerts & Warnings** — Official space weather alerts

## License

MIT
