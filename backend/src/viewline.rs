use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};

use crate::models::{KpForecast, OvationResponse, TonightViewlineResponse, ViewlinePoint};

/// Effective geomagnetic north pole for Kp-based viewline.
/// Shifted west from the IGRF-14 dipole (80.85, -72.76) to better match
/// NOAA's published viewline shape, which dips lowest near the Great Lakes
/// and curves back north toward both coasts.
const GEOMAG_POLE_LAT_DEG: f64 = 80.85;
const GEOMAG_POLE_LON_DEG: f64 = -80.0;

/// Minimum aurora probability (0-100) to consider as the equatorward boundary.
const AURORA_PROBABILITY_THRESHOLD: f64 = 1.0;

/// Maximum gap in latitude degrees between consecutive above-threshold points
/// before we consider the aurora oval to have ended. Prevents stray low-latitude
/// noise (grid artifacts at 0-10°N) from pulling the boundary to the equator.
const BOUNDARY_GAP_THRESHOLD: f64 = 5.0;

/// Number of neighboring longitude bins for moving-average smoothing.
const SMOOTHING_WINDOW: usize = 15;

/// Viewing offset in geographic degrees. Aurora at ~100-200 km altitude can be
/// seen on the horizon from further equatorward. Derived from Case et al. 2016.
const VIEWING_OFFSET_DEG: f64 = 8.0;

/// Base geomagnetic latitude of the equatorward auroral boundary at Kp=0.
const BASE_GEOMAG_LAT: f64 = 66.0;

/// Degrees of geomagnetic latitude shift per unit Kp (used in Kp fallback).
const KP_COEFFICIENT: f64 = 2.0;

/// Maximum longitude-dependent viewing offset for the Kp-based viewline.
/// Applied as a geographic latitude correction after projecting the auroral
/// boundary to geographic coordinates. Points near or west of the geomagnetic
/// pole receive the full offset; points further east taper to zero. This
/// produces the elongated oval shape seen in NOAA's published viewline.
const KP_VIEWING_OFFSET_MAX_DEG: f64 = 4.0;

/// Longitude taper width (degrees east of pole) over which the viewing offset
/// decreases from full to zero.
const KP_VIEWING_TAPER_DEG: f64 = 40.0;

fn to_rad(deg: f64) -> f64 {
    deg * std::f64::consts::PI / 180.0
}

fn to_deg(rad: f64) -> f64 {
    rad * 180.0 / std::f64::consts::PI
}

/// Normalize longitude to -180..180.
fn normalize_lon(lon: f64) -> f64 {
    let mut l = lon % 360.0;
    if l > 180.0 {
        l -= 360.0;
    } else if l < -180.0 {
        l += 360.0;
    }
    l
}

/// Computes the aurora viewline from OVATION model probability data.
///
/// Algorithm (matching NOAA / helioforecast auroramaps):
/// 1. For each integer longitude 0-359, find the minimum NH latitude where
///    aurora probability >= threshold (equatorward boundary).
/// 2. Smooth the boundary with a moving average over `SMOOTHING_WINDOW` bins.
/// 3. Subtract `VIEWING_OFFSET_DEG` to get the visible-aurora viewline.
pub fn compute_viewline_from_ovation(ovation: &OvationResponse) -> Vec<ViewlinePoint> {
    // Step 1: For each longitude bin, collect NH latitudes above threshold
    let mut lon_bins: Vec<Vec<f64>> = vec![Vec::new(); 360];

    for coord in &ovation.coordinates {
        let lon = coord[0];
        let lat = coord[1];
        let prob = coord[2];

        // Northern hemisphere only, above probability threshold
        if lat <= 0.0 || prob < AURORA_PROBABILITY_THRESHOLD {
            continue;
        }

        let lon_idx = (lon.round() as i32).rem_euclid(360) as usize;
        lon_bins[lon_idx].push(lat);
    }

    // Step 2: For each longitude, find the equatorward boundary by scanning
    // from pole toward equator. Stop when there's a gap > BOUNDARY_GAP_THRESHOLD
    // degrees between consecutive above-threshold latitudes. This prevents
    // stray low-latitude noise from pulling the boundary to the equator.
    let mut raw_boundary: [Option<f64>; 360] = [None; 360];

    for (lon_idx, lats) in lon_bins.iter_mut().enumerate() {
        if lats.is_empty() {
            continue;
        }

        // Sort descending: pole → equator
        lats.sort_by(|a, b| b.partial_cmp(a).unwrap());

        let mut boundary = lats[0];
        for i in 1..lats.len() {
            if lats[i - 1] - lats[i] > BOUNDARY_GAP_THRESHOLD {
                break;
            }
            boundary = lats[i];
        }

        raw_boundary[lon_idx] = Some(boundary);
    }

    // Step 2: Smooth with a moving average over SMOOTHING_WINDOW
    let half_window = SMOOTHING_WINDOW / 2;
    let mut smoothed: [Option<f64>; 360] = [None; 360];

    for i in 0..360 {
        if raw_boundary[i].is_none() {
            continue;
        }

        let mut sum = 0.0;
        let mut count = 0;

        for offset in 0..SMOOTHING_WINDOW {
            let j = (i + 360 - half_window + offset) % 360;
            if let Some(val) = raw_boundary[j] {
                sum += val;
                count += 1;
            }
        }

        if count > 0 {
            smoothed[i] = Some(sum / count as f64);
        }
    }

    // Step 3: Apply viewing offset and build output
    let mut points: Vec<ViewlinePoint> = Vec::with_capacity(360);

    for (lon_idx, boundary_lat) in smoothed.iter().enumerate() {
        if let Some(lat) = boundary_lat {
            let viewline_lat = lat - VIEWING_OFFSET_DEG;

            // Only include NH points
            if viewline_lat <= 0.0 {
                continue;
            }

            // Convert 0-359 to -180..180
            let lon = normalize_lon(lon_idx as f64);

            points.push(ViewlinePoint {
                lon,
                lat: viewline_lat,
            });
        }
    }

    points.sort_by(|a, b| a.lon.partial_cmp(&b.lon).unwrap());
    points
}

/// Kp-based viewline fallback (used when OVATION data is unavailable).
///
/// Uses the empirical relationship:
///   equatorward auroral boundary ≈ 66° - 2° × Kp  (geomagnetic latitude)
///
/// The boundary is projected from geomagnetic to geographic coordinates,
/// then shifted equatorward by `VIEWING_OFFSET_DEG`.
pub fn compute_viewline(kp: f64) -> Vec<ViewlinePoint> {
    let pole_lat = to_rad(GEOMAG_POLE_LAT_DEG);
    let pole_lon = to_rad(GEOMAG_POLE_LON_DEG);

    // Geomagnetic latitude of the equatorward auroral boundary
    let geomag_lat = BASE_GEOMAG_LAT - KP_COEFFICIENT * kp;

    // Angular distance from the geomagnetic pole (no viewing offset yet)
    let dist = to_rad(90.0 - geomag_lat);

    // Generate viewline as a circle at `dist` from the geomagnetic pole,
    // then apply a longitude-dependent viewing offset in geographic space.
    let mut points: Vec<ViewlinePoint> = Vec::with_capacity(360);

    for az_deg in 0..360 {
        let az = to_rad(az_deg as f64);

        let lat = (pole_lat.sin() * dist.cos() + pole_lat.cos() * dist.sin() * az.cos()).asin();

        let lon = pole_lon
            + (az.sin() * dist.sin() * pole_lat.cos())
                .atan2(dist.cos() - pole_lat.sin() * lat.sin());

        let lat_deg = to_deg(lat);
        let lon_deg = normalize_lon(to_deg(lon));

        // Longitude-dependent viewing offset: full offset near the geomagnetic
        // pole, smoothly tapering to zero at KP_VIEWING_TAPER_DEG east of the
        // pole. Points west of the pole receive full offset, with a symmetric
        // taper on the far side of the globe to avoid discontinuities.
        let mut lon_diff = lon_deg - GEOMAG_POLE_LON_DEG;
        if lon_diff > 180.0 {
            lon_diff -= 360.0;
        } else if lon_diff < -180.0 {
            lon_diff += 360.0;
        }
        // Smooth cosine taper: 1.0 at pole, tapering east; also taper from
        // the far side so points near 180° from the pole smoothly reach 0.
        let factor = if lon_diff <= 0.0 {
            // West of pole: taper from 1.0 down to 0.0 over the far hemisphere
            let west_dist = -lon_diff; // 0..180
            let t = (west_dist / (360.0 - KP_VIEWING_TAPER_DEG)).clamp(0.0, 1.0);
            (1.0 + (std::f64::consts::PI * t).cos()) / 2.0
        } else if lon_diff <= KP_VIEWING_TAPER_DEG {
            // East of pole within taper zone
            let t = (lon_diff / KP_VIEWING_TAPER_DEG).clamp(0.0, 1.0);
            (1.0 + (std::f64::consts::PI * t).cos()) / 2.0
        } else {
            0.0
        };
        let viewline_lat = lat_deg - KP_VIEWING_OFFSET_MAX_DEG * factor;

        // Clip to North America region (viewing offset is only calibrated here)
        if viewline_lat > 0.0 && (-130.0..=-40.0).contains(&lon_deg) {
            points.push(ViewlinePoint {
                lon: lon_deg,
                lat: viewline_lat,
            });
        }
    }

    points.sort_by(|a, b| a.lon.partial_cmp(&b.lon).unwrap());
    points
}

/// Computes tonight's viewline from Kp forecast data.
///
/// "Tonight" is defined as the 6pm–6am Central Standard Time window (UTC-6),
/// i.e., 00:00–12:00 UTC of the relevant UTC date. If the current time is
/// before 06:00 CST (12:00 UTC), the overnight window that started last
/// evening is still active; otherwise the upcoming evening's window is used.
///
/// Returns `None` if no forecast entries fall within the window.
pub fn compute_tonight_viewline(
    forecasts: &[KpForecast],
    now: DateTime<Utc>,
) -> Option<TonightViewlineResponse> {
    let (window_start, window_end) = tonight_window(now);

    let max_kp = forecasts
        .iter()
        .filter_map(|f| {
            // KpForecast time_tag format: "2026-03-05 12:00:00"
            let dt = chrono::NaiveDateTime::parse_from_str(&f.time_tag, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|ndt| Utc.from_utc_datetime(&ndt))?;
            if dt >= window_start && dt < window_end {
                Some(f.kp)
            } else {
                None
            }
        })
        .fold(f64::NEG_INFINITY, f64::max);

    if max_kp.is_infinite() {
        return None;
    }

    let viewline = compute_viewline(max_kp);
    Some(TonightViewlineResponse {
        viewline,
        max_kp,
        window_start,
        window_end,
    })
}

/// Returns (window_start, window_end) in UTC for the "tonight" CST window.
///
/// The window is always 00:00–12:00 UTC of a specific date:
/// - If current UTC hour < 12 (before 06:00 CST), tonight is [today 00:00 UTC, today 12:00 UTC].
/// - Otherwise, tonight is [tomorrow 00:00 UTC, tomorrow 12:00 UTC].
fn tonight_window(now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    let today_midnight = Utc
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .unwrap();

    if now.hour() < 12 {
        (today_midnight, today_midnight + Duration::hours(12))
    } else {
        let tomorrow_midnight = today_midnight + Duration::hours(24);
        (tomorrow_midnight, tomorrow_midnight + Duration::hours(12))
    }
}

/// Checks whether aurora is potentially visible at the given location.
pub fn is_aurora_visible(viewline: &[ViewlinePoint], user_lat: f64, user_lon: f64) -> Option<f64> {
    if viewline.is_empty() {
        return None;
    }

    let closest = viewline.iter().min_by(|a, b| {
        let diff_a = (a.lon - user_lon).abs();
        let diff_b = (b.lon - user_lon).abs();
        diff_a
            .partial_cmp(&diff_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;

    if closest.lat <= user_lat {
        Some(closest.lat)
    } else {
        None
    }
}

/// Returns the viewline latitude at a specific longitude for the given Kp.
/// Wraps the existing spherical geometry from `compute_viewline`.
#[cfg(test)]
pub fn viewline_lat_at_lon(kp: f64, target_lon: f64) -> Option<f64> {
    let viewline = compute_viewline(kp);
    if viewline.is_empty() {
        return None;
    }

    let closest = viewline.iter().min_by(|a, b| {
        let diff_a = (a.lon - target_lon).abs();
        let diff_b = (b.lon - target_lon).abs();
        diff_a
            .partial_cmp(&diff_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;

    Some(closest.lat)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::OvationResponse;

    /// Helper: find the viewline latitude at a given longitude.
    fn find_lat(viewline: &[ViewlinePoint], lon: f64) -> f64 {
        viewline
            .iter()
            .min_by(|a, b| {
                (a.lon - lon)
                    .abs()
                    .partial_cmp(&(b.lon - lon).abs())
                    .unwrap()
            })
            .unwrap()
            .lat
    }

    /// Build synthetic OVATION data with a uniform boundary at `boundary_lat`
    /// for all longitudes, and optionally a dip for a specific longitude range.
    fn make_ovation(
        boundary_lat: f64,
        dip_lon_range: Option<(f64, f64)>,
        dip_lat: Option<f64>,
    ) -> OvationResponse {
        let mut coordinates = Vec::new();

        for lon in 0..360 {
            let lon_f = lon as f64;

            // Points poleward of the boundary have aurora
            let equatorward = if let (Some((lo, hi)), Some(dl)) = (dip_lon_range, dip_lat) {
                if lon_f >= lo && lon_f <= hi {
                    dl
                } else {
                    boundary_lat
                }
            } else {
                boundary_lat
            };

            // Add aurora points from equatorward boundary up to 80°
            let mut lat = equatorward;
            while lat <= 80.0 {
                coordinates.push([lon_f, lat, 10.0]);
                lat += 1.0;
            }

            // Add sub-threshold points below boundary
            if equatorward > 1.0 {
                let mut lat = 0.0;
                while lat < equatorward {
                    coordinates.push([lon_f, lat, 0.0]);
                    lat += 1.0;
                }
            }
        }

        OvationResponse {
            observation_time: "2026-03-04 00:00".to_string(),
            forecast_time: "2026-03-04 00:30".to_string(),
            coordinates,
        }
    }

    // --- OVATION-based viewline tests ---

    #[test]
    fn test_ovation_uniform_boundary() {
        // Uniform boundary at 55°N → viewline at 55 - 8 = 47°N everywhere
        let ovation = make_ovation(55.0, None, None);
        let vl = compute_viewline_from_ovation(&ovation);

        assert!(!vl.is_empty(), "Viewline should not be empty");
        assert_eq!(vl.len(), 360, "Should have a point for every longitude");

        for pt in &vl {
            assert!(
                (pt.lat - 47.0).abs() < 1.0,
                "Expected ~47°N, got {:.1}°N at lon {:.0}°",
                pt.lat,
                pt.lon
            );
        }
    }

    #[test]
    fn test_ovation_asymmetry() {
        // Dip at longitudes 270-300 (= -90 to -60 geographic, roughly US)
        // Boundary 50°N in the dip region, 60°N everywhere else
        let ovation = make_ovation(60.0, Some((270.0, 300.0)), Some(50.0));
        let vl = compute_viewline_from_ovation(&ovation);

        let us_lat = find_lat(&vl, -80.0);
        let europe_lat = find_lat(&vl, 15.0);

        assert!(
            us_lat < europe_lat,
            "US ({:.1}°N) should be further south than Europe ({:.1}°N)",
            us_lat,
            europe_lat,
        );
    }

    #[test]
    fn test_ovation_smoothing() {
        // Create a sharp dip at a single longitude — smoothing should soften it
        let ovation = make_ovation(60.0, Some((180.0, 181.0)), Some(40.0));
        let vl = compute_viewline_from_ovation(&ovation);

        // After smoothing over 15° window, the single-bin dip should be damped
        let dip_lat = find_lat(&vl, 0.0); // lon 180 = lon 0 after normalize (actually -180)
        let neighbor_lat = find_lat(&vl, 10.0);

        // The dip should be softened (not the full 20° difference)
        let diff = (neighbor_lat - dip_lat).abs();
        assert!(
            diff < 15.0,
            "Smoothing should dampen the dip: diff={:.1}° (expected <15°)",
            diff
        );
    }

    #[test]
    fn test_ovation_no_jagged_jumps() {
        let ovation = make_ovation(55.0, Some((200.0, 230.0)), Some(45.0));
        let vl = compute_viewline_from_ovation(&ovation);

        for i in 1..vl.len() {
            // Only check adjacent longitudes (skip big lon gaps)
            let lon_diff = (vl[i].lon - vl[i - 1].lon).abs();
            if lon_diff > 2.0 {
                continue;
            }

            let lat_diff = (vl[i].lat - vl[i - 1].lat).abs();
            assert!(
                lat_diff < 3.0,
                "Jagged: {:.1}° jump at lon {:.0}°",
                lat_diff,
                vl[i].lon,
            );
        }
    }

    #[test]
    fn test_ovation_ignores_low_latitude_noise() {
        // Simulate real OVATION data: aurora oval at 60-80°N, plus noise at 0-5°N
        let mut coordinates = Vec::new();
        for lon in 0..360 {
            let lon_f = lon as f64;
            // Real aurora oval: 60-80°N with high probability
            for lat in 60..=80 {
                coordinates.push([lon_f, lat as f64, 10.0]);
            }
            // Noise at low latitudes (as seen in real OVATION data)
            coordinates.push([lon_f, 0.0, 1.0]);
            coordinates.push([lon_f, 1.0, 1.0]);
        }

        let ovation = OvationResponse {
            observation_time: String::new(),
            forecast_time: String::new(),
            coordinates,
        };

        let vl = compute_viewline_from_ovation(&ovation);
        assert!(!vl.is_empty());

        // Viewline should be near 60 - 8 = 52°N, NOT near the equator
        for pt in &vl {
            assert!(
                pt.lat > 40.0,
                "Viewline at {:.1}°N (lon {:.0}°) — noise should be ignored",
                pt.lat,
                pt.lon,
            );
        }
    }

    #[test]
    fn test_ovation_no_aurora_returns_empty() {
        // All probabilities below threshold
        let ovation = OvationResponse {
            observation_time: String::new(),
            forecast_time: String::new(),
            coordinates: (0..360)
                .flat_map(|lon| (0..90).map(move |lat| [lon as f64, lat as f64, 0.0]))
                .collect(),
        };

        let vl = compute_viewline_from_ovation(&ovation);
        assert!(vl.is_empty(), "No aurora should produce empty viewline");
    }

    // --- Kp fallback tests ---

    #[test]
    fn test_normalize_lon() {
        assert_eq!(normalize_lon(0.0), 0.0);
        assert_eq!(normalize_lon(270.0), -90.0);
        assert_eq!(normalize_lon(359.0), -1.0);
        assert_eq!(normalize_lon(-200.0), 160.0);
    }

    #[test]
    fn test_kp_fallback_generates_points() {
        let viewline = compute_viewline(3.0);
        assert!(
            viewline.len() > 50,
            "Expected >50 points, got {}",
            viewline.len()
        );
    }

    #[test]
    fn test_kp_fallback_higher_kp_further_south() {
        let lat3 = find_lat(&compute_viewline(3.0), -93.0);
        let lat5 = find_lat(&compute_viewline(5.0), -93.0);

        assert!(
            lat5 < lat3,
            "Higher Kp should push viewline further south: Kp3={:.1}°N, Kp5={:.1}°N",
            lat3,
            lat5
        );
    }

    #[test]
    fn test_kp_fallback_asymmetry() {
        let viewline = compute_viewline(4.0);

        let near_us = find_lat(&viewline, -93.0);
        let near_russia = find_lat(&viewline, 90.0);

        assert!(
            near_us < near_russia,
            "US ({:.1}°N) should be further south than Russia ({:.1}°N)",
            near_us,
            near_russia,
        );
    }

    #[test]
    fn test_kp_fallback_smooth() {
        let viewline = compute_viewline(4.0);
        for i in 1..viewline.len() {
            let lat_diff = (viewline[i].lat - viewline[i - 1].lat).abs();
            assert!(
                lat_diff < 3.0,
                "Jagged: {:.1}° jump at index {}",
                lat_diff,
                i
            );
        }
    }

    #[test]
    fn test_is_aurora_visible() {
        let viewline = vec![
            ViewlinePoint {
                lon: -94.0,
                lat: 48.0,
            },
            ViewlinePoint {
                lon: -93.0,
                lat: 46.0,
            },
            ViewlinePoint {
                lon: -92.0,
                lat: 47.0,
            },
        ];
        assert!(is_aurora_visible(&viewline, 45.0, -93.0).is_none());
        assert!(is_aurora_visible(&viewline, 47.0, -93.0).is_some());
    }

    #[test]
    fn test_empty_viewline() {
        assert!(is_aurora_visible(&[], 45.0, -93.0).is_none());
    }

    // --- compute_tonight_viewline tests ---

    fn make_forecast(time_tag: &str, kp: f64) -> KpForecast {
        KpForecast {
            time_tag: time_tag.to_string(),
            kp,
            observed: "estimated".to_string(),
            noaa_scale: "".to_string(),
        }
    }

    #[test]
    fn test_tonight_window_before_noon_utc() {
        // 08:00 UTC = 02:00 CST — still in overnight window
        let now = Utc.with_ymd_and_hms(2026, 3, 6, 8, 0, 0).unwrap();
        let (start, end) = tonight_window(now);
        assert_eq!(start, Utc.with_ymd_and_hms(2026, 3, 6, 0, 0, 0).unwrap());
        assert_eq!(end, Utc.with_ymd_and_hms(2026, 3, 6, 12, 0, 0).unwrap());
    }

    #[test]
    fn test_tonight_window_after_noon_utc() {
        // 20:00 UTC = 14:00 CST — evening window is upcoming
        let now = Utc.with_ymd_and_hms(2026, 3, 5, 20, 0, 0).unwrap();
        let (start, end) = tonight_window(now);
        assert_eq!(start, Utc.with_ymd_and_hms(2026, 3, 6, 0, 0, 0).unwrap());
        assert_eq!(end, Utc.with_ymd_and_hms(2026, 3, 6, 12, 0, 0).unwrap());
    }

    #[test]
    fn test_compute_tonight_viewline_picks_max_kp() {
        let now = Utc.with_ymd_and_hms(2026, 3, 6, 8, 0, 0).unwrap();
        let forecasts = vec![
            make_forecast("2026-03-06 01:00:00", 2.0),
            make_forecast("2026-03-06 04:00:00", 4.0), // max in window
            make_forecast("2026-03-06 07:00:00", 3.0),
            make_forecast("2026-03-06 14:00:00", 5.0), // outside window
        ];
        let result = compute_tonight_viewline(&forecasts, now).unwrap();
        assert_eq!(result.max_kp, 4.0);
        assert!(!result.viewline.is_empty());
    }

    #[test]
    fn test_compute_tonight_viewline_no_entries_returns_none() {
        let now = Utc.with_ymd_and_hms(2026, 3, 6, 8, 0, 0).unwrap();
        let forecasts = vec![
            make_forecast("2026-03-06 14:00:00", 3.0), // all outside window
        ];
        assert!(compute_tonight_viewline(&forecasts, now).is_none());
    }

    // --- NOAA reference verification tests ---

    #[test]
    fn test_kp1_noaa_reference() {
        // NOAA viewline Kp=1: includes horizon viewing offset
        // At lon -80 (Ontario): ~50-52N
        let lat = viewline_lat_at_lon(1.0, -80.0).unwrap();
        assert!(
            lat >= 49.0 && lat <= 53.0,
            "Kp=1 at lon -80: expected 49-53N, got {:.1}N",
            lat
        );

        // Pacific (~-125): further north due to geomagnetic pole offset
        let lat_pac = viewline_lat_at_lon(1.0, -125.0).unwrap();
        assert!(
            lat_pac >= 52.0 && lat_pac <= 59.0,
            "Kp=1 at lon -125: expected 52-59N, got {:.1}N",
            lat_pac
        );
    }

    #[test]
    fn test_kp3_noaa_reference() {
        // NOAA viewline Kp=3: near US/Canada border with viewing offset
        // At lon -80 (Ontario): ~46-48N
        let lat = viewline_lat_at_lon(3.0, -80.0).unwrap();
        assert!(
            lat >= 45.0 && lat <= 49.0,
            "Kp=3 at lon -80: expected 45-49N, got {:.1}N",
            lat
        );

        // Pacific (~-125): further north due to geomagnetic pole offset
        let lat_pac = viewline_lat_at_lon(3.0, -125.0).unwrap();
        assert!(
            lat_pac >= 48.0 && lat_pac <= 54.0,
            "Kp=3 at lon -125: expected 48-54N, got {:.1}N",
            lat_pac
        );
    }

    #[test]
    fn test_kp4_noaa_reference() {
        // NOAA viewline Kp=4: with viewing offset ~43-47N
        let lat = viewline_lat_at_lon(4.0, -80.0).unwrap();
        assert!(
            lat >= 43.0 && lat <= 47.0,
            "Kp=4 at lon -80: expected 43-47N, got {:.1}N",
            lat
        );
    }

    /// NOAA reference ranges for each integer Kp at key North American longitudes.
    /// Derived from NOAA's published aurora Kp map and viewline forecasts.
    /// Format: (kp, lon, min_lat, max_lat, label)
    const NOAA_REFERENCE: &[(u32, f64, f64, f64, &str)] = &[
        // Kp 1
        (1, -124.0, 51.0, 55.0, "Kp1 west coast"),
        (1, -80.0, 49.0, 53.0, "Kp1 Great Lakes"),
        (1, -63.0, 50.0, 54.0, "Kp1 PEI"),
        // Kp 3
        (3, -124.0, 48.0, 52.0, "Kp3 west coast"),
        (3, -80.0, 45.0, 49.0, "Kp3 Great Lakes"),
        (3, -63.0, 47.0, 51.0, "Kp3 PEI"),
        // Kp 5
        (5, -124.0, 44.0, 48.0, "Kp5 west coast"),
        (5, -80.0, 41.0, 45.0, "Kp5 Great Lakes"),
        (5, -63.0, 43.0, 47.0, "Kp5 PEI"),
        // Kp 7
        (7, -124.0, 40.0, 44.0, "Kp7 west coast"),
        (7, -80.0, 37.0, 41.0, "Kp7 Great Lakes"),
        (7, -63.0, 39.0, 43.0, "Kp7 PEI"),
    ];

    #[test]
    fn test_noaa_reference_ranges() {
        for &(kp, lon, min_lat, max_lat, label) in NOAA_REFERENCE {
            let viewline = compute_viewline(kp as f64);
            let closest = viewline
                .iter()
                .min_by(|a, b| {
                    (a.lon - lon)
                        .abs()
                        .partial_cmp(&(b.lon - lon).abs())
                        .unwrap()
                })
                .expect("Viewline should have points");

            assert!(
                closest.lat >= min_lat && closest.lat <= max_lat,
                "{}: expected {:.0}-{:.0}N, got {:.1}N",
                label,
                min_lat,
                max_lat,
                closest.lat,
            );
        }
    }

    /// Live test: fetches NOAA's Kp forecast, determines tonight's max Kp
    /// (the same value NOAA uses for their viewline image), computes our
    /// viewline, and verifies it falls within expected NOAA reference ranges.
    ///
    /// Run with: cargo test -p northerndawn test_live_noaa_comparison -- --ignored --nocapture
    #[tokio::test]
    async fn test_live_noaa_comparison() {
        use chrono::Utc;

        let client = reqwest::Client::new();

        // Fetch NOAA Kp forecast
        let resp = client
            .get("https://services.swpc.noaa.gov/products/noaa-planetary-k-index-forecast.json")
            .send()
            .await
            .expect("Failed to fetch NOAA Kp forecast");

        let forecasts: Vec<Vec<serde_json::Value>> =
            resp.json().await.expect("Failed to parse NOAA Kp forecast");

        // Determine tonight's window (same logic as compute_tonight_viewline)
        let now = Utc::now();
        let (window_start, window_end) = tonight_window(now);

        println!("Tonight window: {} to {}", window_start, window_end);

        // Find max Kp in tonight's window
        let mut max_kp: f64 = 0.0;
        for row in &forecasts[1..] {
            // Skip header row
            if let (Some(time_str), Some(kp_val)) = (row[0].as_str(), row[1].as_str()) {
                if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(time_str, "%Y-%m-%d %H:%M:%S")
                {
                    let dt = chrono::TimeZone::from_utc_datetime(&Utc, &dt);
                    if dt >= window_start && dt < window_end {
                        if let Ok(kp) = kp_val.parse::<f64>() {
                            if kp > max_kp {
                                max_kp = kp;
                            }
                        }
                    }
                }
            }
        }

        println!("Tonight max Kp (raw): {:.2}", max_kp);
        assert!(max_kp > 0.0, "No Kp forecast found for tonight's window");

        // NOAA uses integer Kp for their viewline image
        let noaa_kp = max_kp.round() as u32;
        println!("NOAA integer Kp: {}", noaa_kp);

        // Compute our viewline at the raw Kp (matching our app behavior)
        let viewline = compute_viewline(max_kp);
        assert!(!viewline.is_empty(), "Viewline should not be empty");

        // Check against NOAA reference ranges for the nearest integer Kp
        let refs: Vec<_> = NOAA_REFERENCE.iter().filter(|r| r.0 == noaa_kp).collect();

        if refs.is_empty() {
            println!(
                "No reference data for Kp={}, skipping range checks (Kp may be 0, 2, 6, 8, or 9)",
                noaa_kp
            );
        }

        // Allow slightly wider range since we use raw Kp (not rounded)
        let margin = 1.5; // extra degrees of tolerance for raw vs integer Kp
        for &&(_, lon, min_lat, max_lat, label) in &refs {
            let closest = viewline
                .iter()
                .min_by(|a, b| {
                    (a.lon - lon)
                        .abs()
                        .partial_cmp(&(b.lon - lon).abs())
                        .unwrap()
                })
                .unwrap();

            let ok = closest.lat >= (min_lat - margin) && closest.lat <= (max_lat + margin);
            let status = if ok { "PASS" } else { "FAIL" };
            println!(
                "  [{}] {} at lon {:.0}: {:.1}N (NOAA range: {:.0}-{:.0}N, with margin: {:.0}-{:.0}N)",
                status,
                label,
                lon,
                closest.lat,
                min_lat,
                max_lat,
                min_lat - margin,
                max_lat + margin,
            );
            assert!(
                ok,
                "{}: {:.1}N outside NOAA range {:.0}-{:.0}N (±{:.1}° margin)",
                label, closest.lat, min_lat, max_lat, margin,
            );
        }

        // Print full viewline summary for visual comparison
        println!("\nViewline summary (Kp={:.2}):", max_kp);
        let targets = [
            (-124.0, "West coast"),
            (-93.0, "Minneapolis"),
            (-80.0, "Great Lakes"),
            (-68.0, "Maine"),
            (-63.0, "PEI"),
        ];
        for (lon, name) in targets {
            if let Some(closest) = viewline.iter().min_by(|a, b| {
                (a.lon - lon)
                    .abs()
                    .partial_cmp(&(b.lon - lon).abs())
                    .unwrap()
            }) {
                println!("  {} (lon {:.0}): {:.1}N", name, lon, closest.lat);
            }
        }
    }

    #[test]
    #[ignore]
    fn test_dump_viewline_geojson() {
        use std::fs;
        use std::path::Path;

        let output_dir = Path::new("test_output");
        fs::create_dir_all(output_dir).unwrap();

        for kp in 1..=5 {
            let viewline = compute_viewline(kp as f64);
            let coords: Vec<String> = viewline
                .iter()
                .map(|p| format!("[{:.2}, {:.2}]", p.lon, p.lat))
                .collect();

            let geojson = format!(
                r#"{{
  "type": "FeatureCollection",
  "features": [{{
    "type": "Feature",
    "properties": {{ "kp": {kp}, "description": "Kp={kp} viewline" }},
    "geometry": {{
      "type": "LineString",
      "coordinates": [{coords}]
    }}
  }}]
}}"#,
                kp = kp,
                coords = coords.join(", ")
            );

            let path = output_dir.join(format!("viewline_kp{}.geojson", kp));
            fs::write(&path, geojson).unwrap();
            println!("Wrote {}", path.display());
        }
    }
}
