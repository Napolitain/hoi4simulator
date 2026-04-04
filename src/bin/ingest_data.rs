use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use hoi4simulator::data::{DataProfilePaths, ingest_profile};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mut profile = "vanilla".to_string();
    let mut game_dir = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => {
                profile = args
                    .next()
                    .ok_or_else(|| "missing value for --profile".to_string())?;
            }
            "--game-dir" => {
                game_dir = Some(
                    args.next()
                        .ok_or_else(|| "missing value for --game-dir".to_string())?,
                );
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => {
                return Err(format!("unknown argument: {other}\n\n{}", usage_text()));
            }
        }
    }

    let game_dir =
        PathBuf::from(game_dir.ok_or_else(|| format!("missing --game-dir\n\n{}", usage_text()))?);
    let paths = DataProfilePaths::new(env!("CARGO_MANIFEST_DIR"), profile.clone());
    let manifest = ingest_profile(&paths, &game_dir).map_err(|error| error.to_string())?;

    println!("profile: {}", profile);
    println!("game dir: {}", game_dir.display());
    println!("mirrored files: {}", manifest.mirrored_files.len());
    println!("raw root: {}", paths.raw_root().display());
    println!("structured root: {}", paths.structured_root().display());
    if manifest.warnings.is_empty() {
        println!("warnings: none");
    } else {
        println!("warnings:");
        for warning in manifest.warnings {
            println!("  - {warning}");
        }
    }

    Ok(())
}

fn print_usage() {
    println!("{}", usage_text());
}

fn usage_text() -> &'static str {
    "Usage: cargo run --bin ingest_data -- --game-dir <PATH> [--profile <NAME>]\n\nCopies exact local HOI4 data into data/raw/<profile>/ and writes normalized Apache Fory data to data/structured/<profile>/."
}
