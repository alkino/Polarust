use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::{EnrichedStep, GpsPoint, LocationsFile, Media, MediaKind, Step, Trip};

pub fn parse_trip(archive_dir: &Path) -> Result<Trip> {
    let json_path = {
        let p = archive_dir.join("trip.json");
        p.exists().then_some(p)
    }.context("impossible de trouver 'trip.json'.")?;

    let content =
        fs::read_to_string(&json_path).with_context(|| format!("Lecture de {:?}", json_path))?;

    serde_json::from_str(&content).context("Parsing trip.json échoué")
}

pub fn country_flag(iso_code: Option<&str>) -> String {
    let Some(iso_code) = iso_code else {
        return "🌍".to_string();
    };
    let chars: Vec<char> = iso_code.chars().collect();

    if chars.len() != 2 || !chars.iter().all(|c| c.is_ascii_alphabetic()) {
        return "🌍".to_string();
    }

    const OFFSET: u32 = '🇦' as u32 - 'A' as u32;

    let flag1 = char::from_u32(chars[0] as u32 + OFFSET).unwrap();
    let flag2 = char::from_u32(chars[1] as u32 + OFFSET).unwrap();

    [flag1, flag2].iter().collect()
}

pub fn weather_icon(condition: Option<&str>) -> &'static str {
    match condition {
        Some("clear-day") => "☀️",
        Some("cloudy") => "☁️",
        Some("partly-cloudy-day") => "🌤️",
        Some("rain") => "🌧️",
        Some("snow") => "❄️",
        _ => "🌡️",
    }
}

fn generate_location(s: &Step) -> String {
    let country = country_flag(s.location.as_ref().and_then(|l| l.country_code.as_deref()));
    let location = s
        .location
        .as_ref()
        .and_then(|l| l.name.as_deref())
        .unwrap_or("Lieu inconnu")
        .to_string();
    format!("{} {}", country, location)
}

pub fn enrich_steps(archive_dir: &Path, trip: Trip) -> Result<(Trip, Vec<EnrichedStep>)> {
    let enriched = trip
        .steps
        .iter()
        .map(|step| {
            let dir_name = format!("{}_{}", step.slug, step.id);
            let media = load_step_media(&archive_dir.join(&dir_name));
            let location = generate_location(step);
            let weather = format!(
                "{} {}",
                weather_icon(step.weather_condition.as_deref()),
                step.weather_temperature
                    .map(|t| format!("{}°C", t))
                    .unwrap_or("-".to_string())
            );

            EnrichedStep {
                dir_name,
                step: step.clone(),
                media,
                location,
                weather,
            }
        })
        .collect();

    Ok((trip, enriched))
}

fn load_step_media(root: &Path) -> Vec<Media> {
    let mut media: Vec<(PathBuf, MediaKind)> = vec![];

    let photo_dir = root.join("photos");
    if photo_dir.exists() {
        match fs::read_dir(&photo_dir) {
            Ok(entries) => {
                let mut photos: Vec<_> = entries
                    .flatten()
                    .map(|e| (e.path(), MediaKind::Photo))
                    .collect();
                media.append(&mut photos);
            }
            Err(e) => tracing::warn!("Impossible de lire {:?} : {}", photo_dir, e),
        }
    }

    let video_dir = root.join("videos");
    if video_dir.exists() {
        match fs::read_dir(&video_dir) {
            Ok(entries) => {
                let mut videos: Vec<_> = entries
                    .flatten()
                    .map(|e| (e.path(), MediaKind::Video))
                    .collect();
                media.append(&mut videos);
            }
            Err(e) => tracing::warn!("Impossible de lire {:?} : {}", video_dir, e),
        }
    }

    media.sort_by(|(a, _), (b, _)| {
        a.file_name()
            .unwrap_or_default()
            .cmp(b.file_name().unwrap_or_default())
    });

    media
        .into_iter()
        .map(|(p, kind)| Media {
            kind,
            relative_path: p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        })
        .collect()
}

pub fn parse_locations(archive_dir: &Path) -> Result<Vec<GpsPoint>> {
    let path = archive_dir.join("locations.json");
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(&path)?;
    let mut points: Vec<GpsPoint> = if let Ok(p) = serde_json::from_str(&content) {
        p
    } else {
        let file: LocationsFile = serde_json::from_str(&content)?;
        file.locations
    };

    points.sort_by_key(|p| p.timestamp);
    Ok(points)
}
