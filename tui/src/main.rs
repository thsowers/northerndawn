use std::{
    io,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, List, ListItem, Paragraph, Sparkline},
    Frame, Terminal,
};
use reqwest::Client;
use serde::Deserialize;

const API_BASE: &str = "http://localhost:3000";
const REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const POLL_TIMEOUT: Duration = Duration::from_millis(200);

// --- Models ---

#[derive(Debug, Clone, Deserialize, Default)]
struct KpIndex {
    time_tag: String,
    kp_index: f64,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct KpForecast {
    time_tag: String,
    kp: f64,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct SolarWind {
    #[allow(dead_code)]
    time_tag: String,
    speed: f64,
    density: f64,
    bz: f64,
    bt: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct LocationInfo {
    name: String,
    latitude: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct StatusResponse {
    healthy: bool,
    last_kp_poll: Option<String>,
    alert_active: bool,
    location: LocationInfo,
}

#[derive(Debug, Clone, Deserialize)]
struct TonightViewline {
    max_kp: f64,
    window_start: String,
    window_end: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Alert {
    timestamp: String,
    alert_type: String,
    kp: f64,
}

// --- App state ---

#[derive(Default)]
struct AppData {
    kp_history: Vec<KpIndex>,
    kp_forecast: Vec<KpForecast>,
    solar_wind: Vec<SolarWind>,
    status: Option<StatusResponse>,
    tonight_viewline: Option<TonightViewline>,
    alerts: Vec<Alert>,
    error: Option<String>,
    last_refresh: Option<Instant>,
    loading: bool,
}

async fn fetch_all(client: &Client) -> AppData {
    let mut data = AppData {
        loading: false,
        ..Default::default()
    };

    let (kp_hist, forecast, sw_hist, status, tonight, alerts) = tokio::join!(
        client
            .get(format!("{}/api/aurora/kp/history?hours=24", API_BASE))
            .send(),
        client
            .get(format!("{}/api/aurora/kp/forecast", API_BASE))
            .send(),
        client
            .get(format!(
                "{}/api/aurora/solar-wind/history?hours=4",
                API_BASE
            ))
            .send(),
        client.get(format!("{}/api/status", API_BASE)).send(),
        client
            .get(format!("{}/api/aurora/viewline/tonight", API_BASE))
            .send(),
        client.get(format!("{}/api/alerts", API_BASE)).send(),
    );

    if let Ok(r) = kp_hist {
        if r.status().is_success() {
            data.kp_history = r.json().await.unwrap_or_default();
        } else {
            data.error = Some(format!("API error: {}", r.status()));
        }
    } else {
        data.error = Some("Cannot reach API".to_string());
    }
    if let Ok(r) = forecast {
        if r.status().is_success() {
            data.kp_forecast = r.json().await.unwrap_or_default();
        }
    }
    if let Ok(r) = sw_hist {
        if r.status().is_success() {
            data.solar_wind = r.json().await.unwrap_or_default();
        }
    }
    // Fallback to current solar wind if history is empty
    if data.solar_wind.is_empty() {
        if let Ok(r) = client
            .get(format!("{}/api/aurora/solar-wind", API_BASE))
            .send()
            .await
        {
            if r.status().is_success() {
                data.solar_wind = r.json().await.unwrap_or_default();
            }
        }
    }
    if let Ok(r) = status {
        if r.status().is_success() {
            data.status = r.json().await.ok();
        }
    }
    if let Ok(r) = tonight {
        if r.status().is_success() {
            data.tonight_viewline = r.json().await.ok();
        }
    }
    if let Ok(r) = alerts {
        if r.status().is_success() {
            data.alerts = r.json().await.unwrap_or_default();
        }
    }

    data.last_refresh = Some(Instant::now());
    data
}

// --- Helpers ---

fn kp_color(kp: f64) -> Color {
    if kp >= 7.0 {
        Color::Red
    } else if kp >= 5.0 {
        Color::LightRed
    } else if kp >= 3.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn safe_slice(s: &str, start: usize, end: usize) -> &str {
    if s.len() >= end {
        &s[start..end]
    } else if s.len() > start {
        &s[start..]
    } else {
        ""
    }
}

fn fmt_timestamp(ts: &str) -> String {
    // RFC 3339: "2026-03-05T12:34:56Z" → "03-05 12:34"
    format!("{} {}", safe_slice(ts, 5, 10), safe_slice(ts, 11, 16))
}

// --- Render ---

fn render_header(f: &mut Frame, area: Rect, data: &AppData) {
    let (dot_color, status_text) = match (&data.error, data.loading, data.status.as_ref()) {
        (Some(_), _, _) => (Color::Red, "Error"),
        (_, true, _) => (Color::Yellow, "Loading"),
        (_, _, Some(s)) if s.healthy => (Color::Green, "Connected"),
        _ => (Color::DarkGray, "Disconnected"),
    };

    let mut spans = vec![
        Span::styled("● ", Style::default().fg(dot_color)),
        Span::styled(status_text, Style::default().fg(dot_color)),
    ];

    if let Some(st) = &data.status {
        spans.push(Span::raw(format!(
            "   {}  ({:.1}°N)",
            st.location.name, st.location.latitude
        )));
        if let Some(t) = &st.last_kp_poll {
            spans.push(Span::styled(
                format!("   Last: {}", safe_slice(t, 0, 19)),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if st.alert_active {
            spans.push(Span::styled(
                "   ⚡ AURORA ALERT ACTIVE ⚡",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
        }
    }

    if let Some(err) = &data.error {
        spans.push(Span::styled(
            format!("   {}", err),
            Style::default().fg(Color::Red),
        ));
    }

    spans.push(Span::styled(
        "   [r] Refresh  [q] Quit",
        Style::default().fg(Color::DarkGray),
    ));

    let p = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .title(" Sunrise Winds - Aurora Monitor ")
            .borders(Borders::ALL),
    );
    f.render_widget(p, area);
}

fn render_kp(f: &mut Frame, area: Rect, data: &AppData) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(8),
        ])
        .split(area);

    // Current Kp value
    let current = data.kp_history.last();
    let kp_val = current.map(|k| k.kp_index).unwrap_or(0.0);
    let kp_display = current
        .map(|k| format!("{:.1}", k.kp_index))
        .unwrap_or_else(|| "--".to_string());
    let kp_time = current
        .map(|k| format!("   at {} UTC", safe_slice(&k.time_tag, 11, 16)))
        .unwrap_or_default();

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("  Kp {}", kp_display),
                Style::default()
                    .fg(kp_color(kp_val))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(kp_time, Style::default().fg(Color::DarkGray)),
        ]))
        .block(Block::default().title(" Kp Index ").borders(Borders::ALL)),
        chunks[0],
    );

    // 24h sparkline
    let spark_data: Vec<u64> = data
        .kp_history
        .iter()
        .map(|k| (k.kp_index * 10.0).round() as u64)
        .collect();
    f.render_widget(
        Sparkline::default()
            .block(
                Block::default()
                    .title(" 24h History ")
                    .borders(Borders::ALL),
            )
            .data(&spark_data)
            .max(90)
            .style(Style::default().fg(kp_color(kp_val))),
        chunks[1],
    );

    // 3-day forecast bar chart — every other entry to reduce density
    let forecast_subset: Vec<&KpForecast> = data.kp_forecast.iter().step_by(2).take(20).collect();
    let labels: Vec<String> = forecast_subset
        .iter()
        .map(|f| safe_slice(&f.time_tag, 11, 13).to_string())
        .collect();
    let bars: Vec<Bar> = forecast_subset
        .iter()
        .zip(labels.iter())
        .map(|(f, label)| {
            Bar::default()
                .label(Line::from(label.as_str()))
                .value((f.kp * 10.0).round() as u64)
                .style(Style::default().fg(kp_color(f.kp)))
        })
        .collect();

    f.render_widget(
        BarChart::default()
            .block(
                Block::default()
                    .title(" 3-Day Forecast (Kp×10) ")
                    .borders(Borders::ALL),
            )
            .data(BarGroup::default().bars(&bars))
            .bar_width(4)
            .bar_gap(1)
            .max(90),
        chunks[2],
    );
}

fn render_solar_wind(f: &mut Frame, area: Rect, data: &AppData) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(3)])
        .split(area);

    // Current readings
    let lines = if let Some(sw) = data.solar_wind.last() {
        let bz_color = if sw.bz < 0.0 {
            Color::Red
        } else {
            Color::Magenta
        };
        vec![
            Line::from(Span::styled(
                format!("  Speed:   {:.0} km/s", sw.speed),
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::styled(
                format!("  Density: {:.1} p/cm³", sw.density),
                Style::default().fg(Color::LightYellow),
            )),
            Line::from(Span::styled(
                format!("  Bz:      {:.1} nT", sw.bz),
                Style::default().fg(bz_color),
            )),
            Line::from(Span::styled(
                format!("  Bt:      {:.1} nT", sw.bt),
                Style::default().fg(Color::DarkGray),
            )),
        ]
    } else {
        vec![Line::from(Span::styled(
            "  No data",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    f.render_widget(
        Paragraph::new(lines).block(Block::default().title(" Solar Wind ").borders(Borders::ALL)),
        chunks[0],
    );

    // Stacked sparklines (4h window)
    let spark_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(chunks[1]);

    let speed_data: Vec<u64> = data.solar_wind.iter().map(|sw| sw.speed as u64).collect();
    let density_data: Vec<u64> = data
        .solar_wind
        .iter()
        .map(|sw| (sw.density * 10.0) as u64)
        .collect();
    let bz_data: Vec<u64> = data
        .solar_wind
        .iter()
        .map(|sw| (sw.bz.abs() * 10.0) as u64)
        .collect();

    let speed_max = speed_data.iter().copied().max().unwrap_or(1).max(1);
    let density_max = density_data.iter().copied().max().unwrap_or(1).max(1);
    let bz_max = bz_data.iter().copied().max().unwrap_or(1).max(1);

    f.render_widget(
        Sparkline::default()
            .block(
                Block::default()
                    .title(" Speed (km/s) ")
                    .borders(Borders::ALL),
            )
            .data(&speed_data)
            .max(speed_max)
            .style(Style::default().fg(Color::Cyan)),
        spark_chunks[0],
    );
    f.render_widget(
        Sparkline::default()
            .block(
                Block::default()
                    .title(" Density (p/cm³) ")
                    .borders(Borders::ALL),
            )
            .data(&density_data)
            .max(density_max)
            .style(Style::default().fg(Color::LightYellow)),
        spark_chunks[1],
    );
    f.render_widget(
        Sparkline::default()
            .block(Block::default().title(" |Bz| (nT) ").borders(Borders::ALL))
            .data(&bz_data)
            .max(bz_max)
            .style(Style::default().fg(Color::Magenta)),
        spark_chunks[2],
    );
}

fn render_viewline(f: &mut Frame, area: Rect, data: &AppData) {
    let lines = if let Some(v) = &data.tonight_viewline {
        vec![
            Line::from(Span::styled(
                format!("  Max Kp:  {:.1}", v.max_kp),
                Style::default().fg(kp_color(v.max_kp)),
            )),
            Line::from(format!(
                "  Window:  {} → {} UTC",
                safe_slice(&v.window_start, 11, 16),
                safe_slice(&v.window_end, 11, 16),
            )),
        ]
    } else {
        vec![Line::from(Span::styled(
            "  No viewline tonight",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" Tonight's Viewline ")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn render_alerts(f: &mut Frame, area: Rect, data: &AppData) {
    let items: Vec<ListItem> = if data.alerts.is_empty() {
        vec![ListItem::new(Span::styled(
            "  No recent alerts",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        data.alerts
            .iter()
            .rev()
            .take(10)
            .map(|a| {
                let color = if a.alert_type.contains("Aurora") {
                    Color::Green
                } else {
                    Color::Yellow
                };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        fmt_timestamp(&a.timestamp),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw("  "),
                    Span::styled(a.alert_type.clone(), Style::default().fg(color)),
                    Span::styled(
                        format!("  Kp {:.1}", a.kp),
                        Style::default().fg(Color::White),
                    ),
                ]))
            })
            .collect()
    };

    f.render_widget(
        List::new(items).block(
            Block::default()
                .title(" Recent Alerts ")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn render(f: &mut Frame, data: &AppData) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(f.area());

    render_header(f, root[0], data);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(root[1]);

    render_kp(f, main[0], data);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(main[1]);

    render_solar_wind(f, right[0], data);

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(right[1]);

    render_viewline(f, bottom[0], data);
    render_alerts(f, bottom[1], data);
}

// --- Main ---

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&client, &mut terminal).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run(client: &Client, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut data = AppData {
        loading: true,
        ..Default::default()
    };
    terminal.draw(|f| render(f, &data))?;
    data = fetch_all(client).await;

    loop {
        terminal.draw(|f| render(f, &data))?;

        if data
            .last_refresh
            .map(|t| t.elapsed() >= REFRESH_INTERVAL)
            .unwrap_or(false)
        {
            data = fetch_all(client).await;
            continue;
        }

        if event::poll(POLL_TIMEOUT)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        data = fetch_all(client).await;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
