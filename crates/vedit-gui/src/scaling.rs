use std::env;
use std::fs;
use std::path::Path;

/// Detects a reasonable UI scale factor based on common desktop environment variables.
/// Returns `None` if no override should be applied and we should rely on the compositor defaults.
pub fn detect_scale_factor() -> Option<f64> {
    parse_single_value("WINIT_HIDPI_FACTOR")
        .or_else(|| parse_single_value("QT_SCALE_FACTOR"))
        .or_else(|| parse_qt_screen_scale_factors())
        .or_else(|| parse_single_value("GDK_SCALE"))
        .or_else(|| parse_single_value("GDK_DPI_SCALE"))
        .or_else(detect_from_sysfs)
        .and_then(|value| if value > 0.0 { Some(value) } else { None })
}

fn parse_single_value(var: &str) -> Option<f64> {
    env::var(var)
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
}

fn parse_qt_screen_scale_factors() -> Option<f64> {
    env::var("QT_SCREEN_SCALE_FACTORS").ok().and_then(|value| {
        value
            .split(|c| c == ';' || c == ',')
            .filter_map(|entry| entry.split('=').last())
            .filter_map(|raw| raw.trim().parse::<f64>().ok())
            .find(|factor| *factor > 0.0)
    })
}

fn detect_from_sysfs() -> Option<f64> {
    let drm_path = Path::new("/sys/class/drm");
    let mut best_scale = None;

    let entries = fs::read_dir(drm_path).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Only consider connector directories with names like card0-HDMI-A-1
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.contains('-'))
            .unwrap_or(false)
        {
            continue;
        }

        let status_path = path.join("status");
        if let Ok(status) = fs::read_to_string(&status_path) {
            if !status.trim().eq_ignore_ascii_case("connected") {
                continue;
            }
        } else {
            continue;
        }

        let modes_path = path.join("modes");
        let modes = match fs::read_to_string(&modes_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if let Some(first_mode) = modes.lines().find(|line| !line.trim().is_empty()) {
            if let Some(scale) = parse_mode_scale(first_mode.trim()) {
                best_scale = Some(best_scale.map_or(scale, |current: f64| current.max(scale)));
            }
        }
    }

    best_scale
}

fn parse_mode_scale(mode: &str) -> Option<f64> {
    let mut parts = mode.split('x');
    let width = parts.next()?.trim().parse::<f64>().ok()?;
    let rest = parts.next()?;
    let height_str = rest.split(['@', ' ', '\t']).next()?;
    let height = height_str.trim().parse::<f64>().ok()?;

    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    const REF_WIDTH: f64 = 3456.0;
    const REF_HEIGHT: f64 = 1944.0;

    let width_scale = REF_WIDTH / width;
    let height_scale = REF_HEIGHT / height;
    let mut scale = width_scale.min(height_scale);

    scale = scale.clamp(0.5, 1.0);

    // Use a gentle rounding so we produce stable values like 0.8 or 0.6.
    let rounded = (scale * 10.0).round() / 10.0;

    if (rounded - 1.0).abs() < 0.05 {
        None
    } else {
        Some(rounded)
    }
}
