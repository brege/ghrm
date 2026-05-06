use anyhow::Result;
use clap::Parser;
use ghrm_stat::{Config, Tool, resolve_with_config};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about = "Small repo-stat probe for ghrm")]
struct Cli {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long, value_enum)]
    tool: Vec<Tool>,
    #[arg(long, default_value_t = 6)]
    max_languages: usize,
    #[arg(long, default_value_t = 3)]
    max_authors: usize,
    #[arg(long, default_value_t = 3)]
    max_churn: usize,
    #[arg(long, default_value_t = 30)]
    churn_limit: usize,
    #[arg(long)]
    include_hidden: bool,
    #[arg(long)]
    json: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let report = resolve_with_config(
        &cli.path,
        Config {
            tools: cli.tool,
            max_languages: cli.max_languages,
            max_authors: cli.max_authors,
            max_churn: cli.max_churn,
            churn_limit: cli.churn_limit,
            include_hidden: cli.include_hidden,
            ..Config::default()
        },
    )?;

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
