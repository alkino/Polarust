use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::{EnrichedStep, GpsPoint, LocationsFile, Media, MediaKind, Step, Trip};

pub fn parse_trip(archive_dir: &Path) -> Result<Trip> {
    // Cherche trip.json dans le premier sous-dossier ou à la racine
    let json_path =
        find_trip_json(archive_dir).context("Impossible de trouver trip.json dans l'archive")?;

    let content =
        fs::read_to_string(&json_path).with_context(|| format!("Lecture de {:?}", json_path))?;

    serde_json::from_str(&content).context("Parsing trip.json échoué")
}

fn find_trip_json(dir: &Path) -> Option<PathBuf> {
    let direct = dir.join("trip.json");
    if direct.exists() {
        return Some(direct);
    }
    // Cherche dans les sous-dossiers (archive dézippée avec nom de voyage)
    fs::read_dir(dir).ok()?.find_map(|entry| {
        let path = entry.ok()?.path();
        if path.is_dir() {
            let candidate = path.join("trip.json");
            candidate.exists().then_some(candidate)
        } else {
            None
        }
    })
}

pub fn enrich_steps(archive_dir: &Path, trip: Trip) -> Result<(Trip, Vec<EnrichedStep>)> {
    // Trouve le répertoire racine contenant les step_<id>/
    let root = find_trip_root(archive_dir);

    let enriched = trip
        .steps
        .iter()
        .map(|step| {
            let dir_name = step_dir_name(step);
            let media = load_step_media(&root, step);
            EnrichedStep {
                dir_name,
                step: step.clone(),
                media,
            }
        })
        .collect();

    Ok((trip, enriched))
}

fn find_trip_root(archive_dir: &Path) -> PathBuf {
    // Si un sous-dossier contient step_* → c'est là
    if let Ok(entries) = fs::read_dir(archive_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if has_step_dirs(&p) {
                    return p;
                }
            }
        }
    }
    archive_dir.to_path_buf()
}

pub fn has_step_dirs(dir: &Path) -> bool {
    fs::read_dir(dir)
        .ok()
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with("step_"))
        })
        .unwrap_or(false)
}

pub fn step_dir_name(step: &Step) -> String {
    match &step.slug {
        Some(slug) => format!("{}_{}", slug, step.id),
        None => format!("step_{}", step.id),
    }
}

fn load_step_media(root: &Path, step: &Step) -> Vec<Media> {
    let dir_name = step_dir_name(step);

    let mut media: Vec<(PathBuf, MediaKind)> = vec![];

    let photo_dir = root.join(&dir_name).join("photos");
    if photo_dir.exists() {
        let mut photos: Vec<_> = fs::read_dir(&photo_dir)
            .unwrap_or_else(|_| panic!("Lecture {:?}", photo_dir))
            .flatten()
            .map(|e| (e.path(), MediaKind::Photo))
            .collect();
        media.append(&mut photos);
    }

    let video_dir = root.join(&dir_name).join("videos");
    if video_dir.exists() {
        let mut videos: Vec<_> = fs::read_dir(&video_dir)
            .unwrap_or_else(|_| panic!("Lecture {:?}", video_dir))
            .flatten()
            .map(|e| (e.path(), MediaKind::Video))
            .collect();
        media.append(&mut videos);
    }

    media.sort_by(|(a, _), (b, _)| {
        a.file_name()
            .unwrap_or_default()
            .cmp(b.file_name().unwrap_or_default())
    });

    media
        .into_iter()
        .map(|(p, kind)| Media {
            kind: kind,
            relative_path: p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        })
        .collect()
}

pub fn parse_locations(archive_dir: &Path) -> Result<Vec<GpsPoint>> {
    let path = find_trip_root(archive_dir).join("locations.json");
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
