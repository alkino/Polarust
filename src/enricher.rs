use anyhow::{Result};
use std::fs;
use std::path::{Path, PathBuf};

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
