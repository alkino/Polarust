mod generator;
mod model;
mod parser;

use anyhow::Result;
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "polarust", about)]
struct Cli {
    input: PathBuf,

    #[arg(short, long, default_value = "site")]
    output: PathBuf,

    #[arg(long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(level))
        .init();

    let voyage_dirs: Vec<PathBuf> = fs::read_dir(&cli.input)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join("trip.json").exists())
        .collect();

    tracing::info!("📂 {} voyage(s) trouvé(s)", voyage_dirs.len());

    let generator = generator::SiteGenerator::new(&cli.output, &cli.input);
    let mut all_trips: Vec<model::Trip> = vec![];

    for (i, dir) in voyage_dirs.iter().enumerate() {
        let trip = parser::parse_trip(dir)?;
        tracing::info!(
            "📂 {}/{} {} dans {:?}",
            i + 1,
            voyage_dirs.len(),
            trip.name,
            dir
        );

        let gps = parser::parse_locations(dir)?;
        tracing::info!("    📍 {} points GPS", gps.len());

        let (trip, enriched) = parser::enrich_steps(dir, trip)?;

        generator.generate_trip(&trip, &enriched, &gps)?;

        all_trips.push(trip);
    }

    generator.generate_index(&all_trips)?;

    tracing::info!(
        "🌐 Ouvre {:?} dans ton navigateur",
        cli.output.join("index.html")
    );
    Ok(())
}
