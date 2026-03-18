mod model;
mod parser;
mod generator;

use std::fs;
use std::path::PathBuf;
use clap::Parser;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "polarust", about)]
struct Cli {
    input: PathBuf,

    #[arg(short, long, default_value = "site")]
    output: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let voyage_dirs: Vec<PathBuf> = fs::read_dir(&cli.input)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join("trip.json").exists())
        .collect();

    println!("📂 {} voyage(s) trouvé(s)", voyage_dirs.len());

    let generator = generator::SiteGenerator::new(&cli.output, &cli.input);
    let mut all_trips: Vec<model::Trip> = vec![];

    for (i, dir) in voyage_dirs.iter().enumerate() {
        let trip = parser::parse_trip(dir)?;
        println!("📂 {}/{} {} dans {:?}", i + 1, voyage_dirs.len(), trip.name, dir);

        let gps = parser::parse_locations(dir)?;
        println!("    📍 {} points GPS", gps.len());

        let (trip, enriched) = parser::enrich_steps(dir, trip)?;

        generator.generate_trip(&trip, &enriched, &gps)?;

        all_trips.push(trip);
    }

    generator.generate_index(&all_trips)?;

    println!("🌐 Ouvre {:?} dans ton navigateur", cli.output.join("index.html"));
    Ok(())
}

