mod model;
mod parser;
mod generator;

use std::path::PathBuf;
use clap::Parser;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "polarust-static", about)]
struct Cli {
    archive: PathBuf,

    #[arg(short, long, default_value = "site")]
    output: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    println!("📂 Lecture de l'archive : {:?}", cli.archive);
    let trip = parser::parse_trip(&cli.archive)?;
    println!("🗺  Voyage : {} ({} étapes)", trip.name, trip.steps.len());

    let gps = parser::parse_locations(&cli.archive)?;
    println!("📍 {} points GPS", gps.len());

    let (trip, enriched) = parser::enrich_steps(&cli.archive, trip)?;

    let generator = generator::SiteGenerator::new(&cli.output, &cli.archive);
    generator.generate(&trip, &enriched, &gps)?;

    println!("🌐 Ouvre {:?}/index.html dans ton navigateur", cli.output);
    Ok(())
}

