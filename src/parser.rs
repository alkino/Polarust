use anyhow::{Context, Result};
use std::fs;
use std::path::{Path};

use crate::model::{GpsPoint, LocationsFile, Trip};

pub fn parse_trip(archive_dir: &Path) -> Result<Trip> {
    let json_path = {
        let p = archive_dir.join("trip.json");
        p.exists().then_some(p)
    }.context("impossible de trouver 'trip.json'.")?;

    let content =
        fs::read_to_string(&json_path).with_context(|| format!("Lecture de {:?}", json_path))?;

    serde_json::from_str(&content).context("Parsing trip.json échoué")
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
