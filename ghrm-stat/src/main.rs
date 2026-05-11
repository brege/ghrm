use anyhow::Result;
use clap::Parser;
use ghrm_stat::{
    Config, Tool,
    filesystem::{self, FsConfig},
    resolve_with_config,
};
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
    filesystem: bool,
    #[arg(long)]
    no_ignore: bool,
    #[arg(long)]
    show_excludes: bool,
    #[arg(long = "exclude-name")]
    exclude_names: Vec<String>,
    #[arg(long)]
    json: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.filesystem {
        let report = filesystem::scan(
            &cli.path,
            &FsConfig {
                hidden: cli.include_hidden,
                use_ignore: !cli.no_ignore,
                show_excludes: cli.show_excludes,
                exclude_names: cli.exclude_names,
                same_file_system: true,
                filter_groups: Vec::new(),
            },
        )?;
        if cli.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
            return Ok(());
        }

        println!("root\t{}", report.root.display());
        println!("filesystem\t{}", report.file_system.unwrap_or_default());
        println!("files\t{}", report.totals.files);
        println!("dirs\t{}", report.totals.dirs);
        println!("symlinks\t{}", report.totals.symlinks);
        println!("size\t{}", filesystem::format_bytes(report.totals.bytes));
        println!("depth\t{}", report.max_depth);
        return Ok(());
    }

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
            print!("{}", row.key);
            if !row.value.is_empty() {
                print!("\t{}", row.value);
            }
            for metric in row.metrics {
                print!("\t{}={}", metric.key, metric.value);
            }
            println!();
        }
    }

    Ok(())
}
