use anyhow::{Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use crate::model::{EnrichedStep, Media, MediaKind, Step, Trip};

fn weather_icon(condition: Option<&str>) -> &'static str {
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
    let location = s.location.as_ref();

    let country: String = {
        let iso_code = location.and_then(|l| l.country_code.as_deref());
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
    };

    let location = location
        .and_then(|l| l.name.as_deref())
        .unwrap_or("Lieu inconnu")
        .to_string();
    format!("{} {}", country, location)
}

fn get_media(dir: PathBuf, kind: MediaKind) -> Vec<Media> {
    if !dir.exists() {
        return vec![];
    }
    match fs::read_dir(&dir) {
        Ok(entries) => entries
                .flatten()
                .filter_map(|e| {
                    Some(Media {
                        kind,
                        relative_path: e.path()
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                })
                .collect(),
        Err(e) => {
            tracing::warn!("Impossible de lire {:?} : {}", dir, e);
            vec![]
        }
    }
}

fn scan_archive(archive_dir: &Path) -> HashMap<String, Vec<Media>> {
    let mut map: HashMap<String, Vec<Media>> = HashMap::new();

    let Ok(entries) = fs::read_dir(archive_dir) else {
        return map;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let mut media = get_media(path.join("photos"), MediaKind::Photo);
        media.extend(get_media(path.join("videos"), MediaKind::Video));
        media.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        map.insert(dir_name, media);
    }
    map
}

pub fn enrich_steps(archive_dir: &Path, trip: Trip) -> Result<(Trip, Vec<EnrichedStep>)> {
    let media_map = scan_archive(archive_dir);

    let enriched = trip
        .steps
        .iter()
        .map(|step| {
            let dir_name = format!("{}_{}", step.slug, step.id);
            let media = media_map.get(&dir_name).cloned().unwrap_or_default();
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
