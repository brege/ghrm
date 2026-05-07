mod config;
mod dirs;
mod explorer;
mod http;
mod options;
mod paths;
mod render;
mod repo;
mod runtime;
mod search;
#[cfg(test)]
mod testutil;
mod tmpl;

use crate::explorer::{column, filter};
use crate::http::{server, theme, vendor};
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "ghrm", version, about = "GitHub-flavored markdown preview")]
struct Cli {
    target: Option<PathBuf>,

    #[arg(short = 'c', long)]
    config: Option<PathBuf>,

    #[arg(short = 'p', long)]
    port: Option<u16>,

    #[arg(short = 'b', long)]
    bind: Option<String>,

    #[arg(short = 'O', long, help = "Do not open a browser on startup")]
    no_browser: bool,

    #[arg(
        short = 'I',
        long,
        help = "Ignore .gitignore, .git/info/exclude, and global gitignore rules"
    )]
    no_ignore: bool,

    #[arg(
        short = 'H',
        long,
        help = "Default the explorer to include hidden paths"
    )]
    hidden: bool,

    #[arg(
        short = 'e',
        long = "extension",
        value_name = "EXT",
        help = "Default the explorer to files with this extension"
    )]
    extensions: Vec<String>,

    #[arg(
        short = 'E',
        long,
        help = "Do not hide excluded directories (.git, node_modules, etc.) in explorer"
    )]
    no_excludes: bool,

    #[arg(
        short = 'm',
        long = "max-rows",
        value_name = "ROWS",
        help = "Maximum number of search result rows"
    )]
    max_rows: Option<usize>,

    #[arg(long, help = "Clear cached frontend assets before startup")]
    clean: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,ghrm=info,tokei::language::language_type=off".into()),
        )
        .init();

    let cli = Cli::parse();
    let cfg = config::Config::load(cli.config.as_deref())?;
    if cli.clean {
        vendor::clean()?;
        theme::clean()?;
        if cli.target.is_none() {
            return Ok(());
        }
    }
    vendor::ensure()?;
    theme::ensure()?;

    let resolved = options::resolve(
        options::Input {
            target: cli.target,
            config_path: cli.config.as_deref(),
            port: cli.port,
            bind: cli.bind,
            no_browser: cli.no_browser,
            no_ignore: cli.no_ignore,
            hidden: cli.hidden,
            extensions: cli.extensions,
            no_excludes: cli.no_excludes,
            max_rows: cli.max_rows,
            ghrm_open: std::env::var("GHRM_OPEN").ok(),
        },
        &cfg,
    )?;

    let filters = filter::Set::resolve(&cfg.walk.filter)?;
    let default_filter_ext = resolved.filter_ext || filters.default_enabled();
    let default_columns = column::Set::from_defaults(|def| cfg.explorer.columns.default_for(def));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(server::run(server::Options {
        bind: resolved.bind,
        port: resolved.port,
        exact_port: resolved.exact_port,
        open: resolved.open,
        target: resolved.target,
        use_ignore: resolved.use_ignore,
        default_hidden: resolved.show_hidden,
        default_filter_ext,
        default_columns,
        extensions: resolved.extensions,
        filters,
        exclude_names: resolved.exclude_names,
        show_excludes: resolved.show_excludes,
        search_max_rows: resolved.max_rows,
        config_path: resolved.config_path,
        stats: resolved.stats,
        auth: resolved.auth,
    }))
}
