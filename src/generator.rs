use std::path::{Path, PathBuf};
use std::fs;
use anyhow::Result;
use chrono::{DateTime, Utc, TimeZone};
use minijinja::{Environment, context };
use serde::Serialize;

use crate::model::{Trip, EnrichedStep, GpsPoint};

// Structs de contexte pour les templates
#[derive(Serialize)]
struct StepContext<'a> {
    step: &'a crate::model::Step,
    thumb: String,
    date: String,
    location: String,
    photos: &'a Vec<String>,
}

#[derive(Serialize)]
struct MapMarker {
    id: u64,
    lat: f64,
    lon: f64,
    title: String,
    thumb: String,
}

#[derive(Serialize)]
struct NavStep {
    id: u64,
    name: String,
}

pub struct SiteGenerator {
    output_dir: PathBuf,
    archive_root: PathBuf,
    env: Environment<'static>,
}

impl SiteGenerator {
    pub fn new(output_dir: &Path, archive_root: &Path) -> Self {
        let mut env = Environment::new();
        env.add_template(
            "index.html".into(),
            include_str!("assets/index.html").into()
        ).unwrap();
        env.add_template(
            "step.html".into(),
            include_str!("assets/step.html").into()
        ).unwrap();

        Self {
            output_dir: output_dir.to_path_buf(),
            archive_root: archive_root.to_path_buf(),
            env,
        }
    }

    pub fn generate(&self, trip: &Trip, steps: &[EnrichedStep], gps: &[GpsPoint]) -> Result<()> {
        self.prepare_dirs()?;
        self.copy_photos(steps)?;
        self.write_css()?;
        self.write_index(trip, steps, gps)?;
        for i in 0..steps.len() {
            let prev = if i > 0 { steps.get(i - 1) } else { None };
            let next = steps.get(i + 1);
            self.write_step_page(&steps[i], prev, next)?;
        }
        println!("✅ Site généré dans {:?}", self.output_dir);
        Ok(())
    }

    fn write_index(&self, trip: &Trip, steps: &[EnrichedStep], gps: &[GpsPoint]) -> Result<()> {
        let steps_ctx: Vec<StepContext> = steps.iter().map(|es| StepContext {
            step: &es.step,
            thumb: es.photos.first()
                .map(|p| format!("photos/{}", p))
                .unwrap_or_default(),
            date: format_ts(es.step.start_time),
            location: es.step.location.as_ref()
                .and_then(|l| l.name.as_deref())
                .unwrap_or("Lieu inconnu")
                .to_string(),
            photos: &es.photos,
        }).collect();

        let map_markers: Vec<MapMarker> = steps.iter().filter_map(|es| {
            let loc = es.step.location.as_ref()?;
            Some(MapMarker {
                id: es.step.id,
                lat: loc.lat?,
                lon: loc.lon?,
                title: escape_html(es.step.display_name.as_deref().unwrap_or("?")),
                thumb: es.photos.first()
                    .map(|p| format!("photos/{}", p))
                    .unwrap_or_default(),
            })
        }).collect();

        let polyline: Vec<[f64; 2]> = gps.iter()
            .map(|p| [p.lat, p.lon])
            .collect();

        let (first_lat, first_lon) = steps.iter()
            .find_map(|es| {
                let loc = es.step.location.as_ref()?;
                Some((loc.lat?, loc.lon?))
            })
            .unwrap_or((48.8566, 2.3522));

        let cover_url = trip.cover_photo_path.as_deref()
            .unwrap_or_default();

        let tmpl = self.env.get_template("index.html")?;
        let html = tmpl.render(context! {
            trip     => trip,
            steps    => steps_ctx,
            map_markers => map_markers,
            polyline => serde_json::to_string(&polyline)?,
            gps_len  => gps.len(),
            first_lat => first_lat,
            first_lon => first_lon,
            cover_url => cover_url,
        })?;

        fs::write(self.output_dir.join("index.html"), html)?;
        Ok(())
    }

    fn write_step_page(
        &self,
        es: &EnrichedStep,
        prev: Option<&EnrichedStep>,
        next: Option<&EnrichedStep>,
    ) -> Result<()> {
        let make_nav = |e: &EnrichedStep| NavStep {
            id: e.step.id,
            name: e.step.display_name.clone().unwrap_or_else(|| "Étape".into()),
        };

        let map_link = es.step.location.as_ref()
            .and_then(|l| l.lat.zip(l.lon))
            .map(|(lat, lon)| format!(
                "https://www.openstreetmap.org/?mlat={lat}&mlon={lon}#map=13/{lat}/{lon}"
            ));

        let tmpl = self.env.get_template("step.html")?;
        let html = tmpl.render(context! {
            title       => es.step.display_name.as_deref().unwrap_or("Étape sans titre"),
            date        => format_ts(es.step.start_time),
            location    => es.step.location.as_ref().and_then(|l| l.name.as_deref()).unwrap_or(""),
            description => es.step.description.as_deref().unwrap_or(""),
            photos      => &es.photos,
            map_link    => map_link,
            prev        => prev.map(make_nav),
            next        => next.map(make_nav),
        })?;

        fs::write(
            self.output_dir.join("steps").join(format!("step_{}.html", es.step.id)),
            html,
        )?;
        Ok(())
    }

    fn write_css(&self) -> Result<()> {
        fs::write(self.output_dir.join("style.css"), include_str!("assets/style.css"))?;
        Ok(())
    }

    fn prepare_dirs(&self) -> Result<()> {
        fs::create_dir_all(self.output_dir.join("photos"))?;
        fs::create_dir_all(self.output_dir.join("steps"))?;
        Ok(())
    }

    fn copy_photos(&self, steps: &[EnrichedStep]) -> Result<()> {
        for es in steps {
            let src_dir = self.archive_root.join(&es.dir_name).join("photos");
            for photo in &es.photos {
                let src = src_dir.join(photo);
                let dst = self.output_dir.join("photos").join(photo);
                if src.exists() && !dst.exists() {
                    fs::copy(&src, &dst)?;
                }
            }
        }
        Ok(())
    }
}

fn format_ts(ts: i64) -> String {
    Utc.timestamp_opt(ts, 0)
        .single()
        .map(|dt: DateTime<Utc>| dt.format("%d %B %Y").to_string())
        .unwrap_or_else(|| "Date inconnue".to_string())
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&#39;")
}
