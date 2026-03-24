use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use image::imageops::FilterType;
use indicatif::{ProgressBar, ProgressStyle};
use minijinja::{context, Environment};
use rayon::prelude::*;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::{EnrichedStep, GpsPoint, Media, MediaKind, Trip};

// Structs de contexte pour les templates
#[derive(Serialize)]
struct StepContext<'a> {
    step: &'a crate::model::Step,
    thumb: String,
    date: i64,
    location: String,
    media: &'a Vec<Media>,
    weather: String,
}

#[derive(Serialize)]
struct MapMarker {
    id: u64,
    lat: f64,
    lon: f64,
    thumb: String,
    location: String,
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
        env.set_loader(minijinja::path_loader("src/assets"));

        // Register a filter to format timestamp in the template directly
        env.add_filter("format_date", |ts: i64| -> String {
            Utc.timestamp_opt(ts, 0)
                .single()
                .map(|dt: DateTime<Utc>| dt.format("%d %B %Y").to_string())
                .unwrap_or_else(|| "?".to_string())
        });

        env.add_filter("tojson", |value: minijinja::Value| -> String {
            serde_json::to_string(&value).unwrap_or_else(|_| "[]".to_string())
        });

        Self {
            output_dir: output_dir.to_path_buf(),
            archive_root: archive_root.to_path_buf(),
            env,
        }
    }

    pub fn generate_index(&self, all_trips: &[Trip]) -> Result<()> {
        self.write_css()?;
        self.write_js()?;

        let tmpl = self.env.get_template("trips.html")?;
        let html = tmpl.render(context! {
            trips => all_trips,
        })?;

        fs::write(self.output_dir.join("index.html"), html)?;
        Ok(())
    }

    pub fn generate_trip(
        &self,
        trip: &Trip,
        steps: &[EnrichedStep],
        gps: &[GpsPoint],
    ) -> Result<()> {
        self.prepare_dirs(&trip)?;
        if let Some(url) = &trip.cover_photo_path {
            let dest = self.output_dir.join(&trip.slug).join("cover.jpg");
            self.download_cover(url, &dest)?;
        }
        self.copy_media(trip, steps)?;
        self.write_gallery_page(trip, steps)?;
        self.write_index(trip, steps, gps)?;
        for i in 0..steps.len() {
            let prev = if i > 0 { steps.get(i - 1) } else { None };
            let next = steps.get(i + 1);
            self.write_step_page(&trip, i, &steps[i], prev, next, steps.len())?;
        }
        tracing::info!("✅ Site généré dans {:?}", self.output_dir.join(&trip.slug));
        Ok(())
    }

    fn download_cover(&self, url: &str, dest: &Path) -> Result<()> {
        if dest.exists() {
            return Ok(());
        }

        tracing::info!("    🖼️  Téléchargement cover : {}", url);
        match ureq::get(url).call() {
            Ok(response) => {
                let bytes = response.into_body().read_to_vec()?;
                fs::write(dest, bytes)?;
            }
            Err(e) => {
                tracing::warn!("⚠️  Cover inaccessible (URL expirée ?) : {}", e);
            }
        }
        Ok(())
    }

    fn write_index(&self, trip: &Trip, steps: &[EnrichedStep], gps: &[GpsPoint]) -> Result<()> {
        let steps_ctx: Vec<StepContext> = steps
            .iter()
            .map(|es| StepContext {
                step: &es.step,
                thumb: es
                    .media
                    .iter()
                    .find(|m| matches!(m.kind, MediaKind::Photo))
                    .map(|p| format!("thumbnails/{}", p.relative_path))
                    .unwrap_or_default(),
                date: es.step.start_time,
                location: es.location.clone(),
                media: &es.media,
                weather: es.weather.clone(),
            })
            .collect();

        let map_markers: Vec<MapMarker> = steps
            .iter()
            .filter_map(|es| {
                let loc = es.step.location.as_ref()?;
                Some(MapMarker {
                    id: es.step.id,
                    lat: loc.lat?,
                    lon: loc.lon?,
                    thumb: es
                        .media
                        .iter()
                        .find(|m| matches!(m.kind, MediaKind::Photo))
                        .map(|p| format!("thumbnails/{}", p.relative_path))
                        .unwrap_or_default(),
                    location: es.location.clone(),
                })
            })
            .collect();

        let polyline: Vec<[f64; 2]> = gps.iter().map(|p| [p.lat, p.lon]).collect();

        let (first_lat, first_lon) = steps
            .iter()
            .find_map(|es| {
                let loc = es.step.location.as_ref()?;
                Some((loc.lat?, loc.lon?))
            })
            .unwrap_or((48.8566, 2.3522));

        let tmpl = self.env.get_template("trip.html")?;
        let html = tmpl.render(context! {
            trip     => trip,
            steps    => steps_ctx,
            map_markers => map_markers,
            polyline => serde_json::to_string(&polyline)?,
            gps_len  => gps.len(),
            first_lat => first_lat,
            first_lon => first_lon,
        })?;

        fs::write(self.output_dir.join(&trip.slug).join("index.html"), html)?;
        Ok(())
    }

    fn write_step_page(
        &self,
        trip: &Trip,
        i: usize,
        es: &EnrichedStep,
        prev: Option<&EnrichedStep>,
        next: Option<&EnrichedStep>,
        steps_len: usize,
    ) -> Result<()> {
        let make_nav = |e: &EnrichedStep| NavStep {
            id: e.step.id,
            name: e
                .step
                .display_name
                .clone()
                .unwrap_or_else(|| "Étape".into()),
        };

        let map_link = es
            .step
            .location
            .as_ref()
            .and_then(|l| l.lat.zip(l.lon))
            .map(|(lat, lon)| {
                format!("https://www.openstreetmap.org/?mlat={lat}&mlon={lon}#map=13/{lat}/{lon}")
            });

        let tmpl = self.env.get_template("step.html")?;
        let html = tmpl.render(context! {
            trip        => trip,
            i           => i + 1,
            title       => es.step.display_name.as_deref().unwrap_or("Étape sans titre"),
            date        => es.step.start_time,
            location    => es.location,
            weather     => es.weather,
            description => es.step.description.as_deref().unwrap_or(""),
            media       => &es.media,
            map_link    => map_link,
            prev        => prev.map(make_nav),
            next        => next.map(make_nav),
            steps_len   => steps_len,
        })?;

        fs::write(
            self.output_dir
                .join(&trip.slug)
                .join("steps")
                .join(format!("step_{}.html", es.step.id)),
            html,
        )?;
        Ok(())
    }

    fn write_gallery_page(&self, trip: &Trip, steps: &[EnrichedStep]) -> Result<()> {
        let tmpl = self.env.get_template("gallery.html")?;
        let html = tmpl.render(context! {
            trip => trip,
            title => "Galerie",
            steps => steps,
        })?;

        fs::write(self.output_dir.join(&trip.slug).join("gallery.html"), html)?;
        Ok(())
    }

    fn write_js(&self) -> Result<()> {
        fs::write(
            self.output_dir.join("trip.js"),
            include_str!("assets/trip.js"),
        )?;
        Ok(())
    }

    fn write_css(&self) -> Result<()> {
        fs::write(
            self.output_dir.join("style.css"),
            include_str!("assets/style.css"),
        )?;
        Ok(())
    }

    fn prepare_dirs(&self, trip: &Trip) -> Result<()> {
        fs::create_dir_all(self.output_dir.join(&trip.slug).join("media"))?;
        fs::create_dir_all(self.output_dir.join(&trip.slug).join("thumbnails"))?;
        fs::create_dir_all(self.output_dir.join(&trip.slug).join("steps"))?;
        Ok(())
    }

    fn copy_media(&self, trip: &Trip, steps: &[EnrichedStep]) -> Result<()> {
        let thumb_dir = self.output_dir.join(&trip.slug).join("thumbnails");
        let dst = self.output_dir.join(&trip.slug).join("media");
        let trip_key = format!("{}_{}", trip.slug, trip.id);

        let all_media: Vec<(&EnrichedStep, &Media)> = steps
            .iter()
            .flat_map(|es| es.media.iter().map(move |m| (es, m)))
            .collect();

        let total = all_media.len();
        let pb = ProgressBar::new(total as u64);
        pb.set_style(
            ProgressStyle::default_bar().template("    📷 Copying Media [{bar:40}] {pos}/{len}")?,
        );

        all_media
            .par_iter()
            .try_for_each(|(es, media)| -> Result<()> {
                let src_subdir = match media.kind {
                    MediaKind::Photo => "photos",
                    MediaKind::Video => "videos",
                };
                let src = self
                    .archive_root
                    .join(&trip_key)
                    .join(&es.dir_name)
                    .join(src_subdir)
                    .join(&media.relative_path);
                let dst = dst.join(&media.relative_path);

                if src.exists() {
                    if !dst.exists() {
                        fs::copy(&src, &dst)?;
                    }

                    if matches!(media.kind, MediaKind::Photo) {
                        let thumb = thumb_dir.join(&media.relative_path);
                        if !thumb.exists() {
                            let data = fs::read(&src)?;
                            let img: image::RgbImage = turbojpeg::decompress_image(&data)?;
                            let resized = image::DynamicImage::ImageRgb8(img).resize(
                                400,
                                300,
                                FilterType::Triangle,
                            );
                            let rgb = resized.to_rgb8();
                            let compressed =
                                turbojpeg::compress_image(&rgb, 75, turbojpeg::Subsamp::Sub2x2)?;
                            fs::write(&thumb, compressed)?;
                        }
                    }
                }
                pb.inc(1);
                Ok(())
            })?;
        pb.finish();
        Ok(())
    }
}
