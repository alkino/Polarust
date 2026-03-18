use std::path::{Path, PathBuf};
use std::fs;
use anyhow::Result;
use chrono::{DateTime, Utc, TimeZone};
use minijinja::{Environment, context };
use serde::Serialize;
use image::imageops::FilterType;
use rayon::prelude::*;

use crate::model::{Trip, EnrichedStep, GpsPoint, GalleryPhoto};

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
            "trips.html".into(),
            include_str!("assets/trips.html").into()
        ).unwrap();
        env.add_template(
            "index.html".into(),
            include_str!("assets/index.html").into()
        ).unwrap();
        env.add_template(
            "step.html".into(),
            include_str!("assets/step.html").into()
        ).unwrap();
        env.add_template(
            "gallery.html".into(),
            include_str!("assets/gallery.html").into()
        ).unwrap();

        Self {
            output_dir: output_dir.to_path_buf(),
            archive_root: archive_root.to_path_buf(),
            env,
        }
    }

    pub fn generate_index(&self, all_trips: &Vec<Trip>) -> Result<()> {
        let tmpl = self.env.get_template("trips.html")?;
        let html = tmpl.render(context! {
            trips => all_trips,
        })?;

        fs::write(self.output_dir.join("index.html"), html)?;
        Ok(())
    }

    pub fn generate_trip(&self, trip: &Trip, steps: &[EnrichedStep], gps: &[GpsPoint]) -> Result<()> {
        self.prepare_dirs(&trip)?;
        self.copy_photos(trip, steps)?;
        self.write_gallery_page(trip, steps)?;
        self.write_css()?;
        self.write_index(trip, steps, gps)?;
        for i in 0..steps.len() {
            let prev = if i > 0 { steps.get(i - 1) } else { None };
            let next = steps.get(i + 1);
            self.write_step_page(&trip, &steps[i], prev, next, steps.len())?;
        }
        println!("✅ Site généré dans {:?}", self.output_dir.join(&trip.slug));
        Ok(())
    }

    fn write_index(&self, trip: &Trip, steps: &[EnrichedStep], gps: &[GpsPoint]) -> Result<()> {
        let steps_ctx: Vec<StepContext> = steps.iter().map(|es| StepContext {
            step: &es.step,
            thumb: es.photos.first()
                .map(|p| format!("thumbnails/{}", p))
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
                    .map(|p| format!("thumbnails/{}", p))
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


        fs::write(self.output_dir.join(&trip.slug).join("index.html"), html)?;
        Ok(())
    }

    fn write_step_page(
        &self,
        trip: &Trip,
        es: &EnrichedStep,
        prev: Option<&EnrichedStep>,
        next: Option<&EnrichedStep>,
        steps_len: usize,
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

        let cover_url = trip.cover_photo_path.as_deref()
            .unwrap_or_default();

        let tmpl = self.env.get_template("step.html")?;
        let html = tmpl.render(context! {
            trip        => trip,
            title       => es.step.display_name.as_deref().unwrap_or("Étape sans titre"),
            date        => format_ts(es.step.start_time),
            location    => es.step.location.as_ref().and_then(|l| l.name.as_deref()).unwrap_or(""),
            description => es.step.description.as_deref().unwrap_or(""),
            photos      => &es.photos,
            map_link    => map_link,
            prev        => prev.map(make_nav),
            next        => next.map(make_nav),
            cover_url   => cover_url,
            steps_len   => steps_len,
        })?;

        fs::write(
            self.output_dir.join(&trip.slug).join("steps").join(format!("step_{}.html", es.step.id)),
            html,
        )?;
        Ok(())
    }

    fn write_gallery_page(&self, trip: &Trip, steps: &[EnrichedStep]) -> Result<()> {
        let photos: Vec<GalleryPhoto> = steps.iter().flat_map(|es| {
            let name = es.step.display_name.clone().unwrap_or_else(|| "Étape".into());
            es.photos.iter().map(move |p| GalleryPhoto {
                src: format!("photos/{p}"),
                thumb: format!("thumbnails/{p}"),
                step_id: es.step.id,
                step_name: name.clone(),
            })
        }).collect();

        let cover_url = trip.cover_photo_path.as_deref()
            .unwrap_or_default();

        let tmpl = self.env.get_template("gallery.html")?;
        let html = tmpl.render(context! {
            trip => trip,
            title => "Galerie",
            photos => &photos,
            cover_url => cover_url,
            steps => steps.len(),
        })?;

        fs::write(self.output_dir.join(&trip.slug).join("gallery.html"), html)?;
        Ok(())
    }

    fn write_css(&self) -> Result<()> {
        fs::write(self.output_dir.join("style.css"), include_str!("assets/style.css"))?;
        Ok(())
    }

    fn prepare_dirs(&self, trip: &Trip) -> Result<()> {
        fs::create_dir_all(self.output_dir.join(&trip.slug).join("photos"))?;
        fs::create_dir_all(self.output_dir.join(&trip.slug).join("thumbnails"))?;
        fs::create_dir_all(self.output_dir.join(&trip.slug).join("steps"))?;
        Ok(())
    }

    fn copy_photos(&self, trip: &Trip, steps: &[EnrichedStep]) -> Result<()> {
        eprintln!("    📷 Copying photos");
        let thumb_dir = self.output_dir.join(&trip.slug).join("thumbnails");

        steps.par_iter().try_for_each(|es| -> Result<()> {
            let trip_key = format!("{}_{}", trip.slug, trip.id);
            let src_dir = self.archive_root.join(trip_key).join(&es.dir_name).join("photos");
            es.photos.par_iter().try_for_each(|photo| -> Result<()> {
                let src = src_dir.join(photo);
                let dst = self.output_dir.join(&trip.slug).join("photos").join(photo);
                let thumb = thumb_dir.join(photo);

                if src.exists() {
                    if !dst.exists() {
                        fs::copy(&src, &dst)?;
                    }
                    if !thumb.exists() {
                        let data = fs::read(&src)?;
                        let img: image::RgbImage = turbojpeg::decompress_image(&data)?;
                        let resized = image::DynamicImage::ImageRgb8(img)
                            .resize(400, 300, FilterType::Triangle);
                        let rgb = resized.to_rgb8();
                        let compressed = turbojpeg::compress_image(
                            &rgb,
                            75,
                            turbojpeg::Subsamp::Sub2x2
                        )?;
                        fs::write(&thumb, compressed)?;
                    }
                }
                Ok(())
            })
        })?;
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
