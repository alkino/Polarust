use serde::{Deserialize, Deserializer, Serialize};

fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    f64::deserialize(deserializer).map(|f| f as i64)
}

fn deserialize_timestamp_opt<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<f64>::deserialize(deserializer).map(|opt| opt.map(|f| f as i64))
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Trip {
    pub id: u64,
    pub slug: String,
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_timestamp_opt")]
    pub start_date: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_timestamp_opt")]
    pub end_date: Option<i64>,
    pub summary: Option<String>,
    pub cover_photo_path: Option<String>,
    #[serde(rename = "all_steps")]
    pub steps: Vec<Step>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Step {
    pub id: u64,
    pub display_name: Option<String>,
    pub description: Option<String>,
    #[serde(deserialize_with = "deserialize_timestamp")]
    pub start_time: i64,
    pub location: Option<Location>,
    pub slug: Option<String>,
    pub weather_condition: Option<String>,
    pub weather_temperature: Option<i8>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Location {
    pub name: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub detail: Option<String>,
    pub country_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Photo,
    Video,
}

#[derive(Debug, Clone, Serialize)]
pub struct Media {
    pub kind: MediaKind,
    pub relative_path: String,
}

/// Représente un step enrichi avec ses photos (post-parsing)
#[derive(Debug, Serialize)]
pub struct EnrichedStep {
    pub step: Step,
    pub media: Vec<Media>, // chemins relatifs vers output/photos/
    pub dir_name: String,
    pub location: String,
    pub weather: String,
}

#[derive(Debug, Deserialize)]
pub struct GpsPoint {
    pub lat: f64,
    pub lon: f64,
    #[serde(rename = "time")]
    #[serde(deserialize_with = "deserialize_timestamp")]
    pub timestamp: i64,
}

#[derive(Debug, Deserialize)]
pub struct LocationsFile {
    pub locations: Vec<GpsPoint>,
}
