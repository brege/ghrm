use anyhow::Result;
use clap::Parser;
use ghrm_stat::{Tool, resolve};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about = "Small repo-stat probe for ghrm")]
struct Cli {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long, value_enum)]
    tool: Vec<Tool>,
    #[arg(long)]
    json: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let report = resolve(&cli.path, &cli.tool)?;

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("root\t{}", report.root.display());
    for section in report.sections {
        println!();
        println!("{:?}", section.tool);
        for row in section.rows {
            println!("{}\t{}", row.key, row.value);
        }
    }

    Ok(())
}
