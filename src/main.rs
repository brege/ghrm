mod config;
mod render;
mod repo;
mod server;
mod theme;
mod tmpl;
mod vendor;
mod walk;
mod watch;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use crate::walk::Scope;

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

    #[arg(
        short = 'I',
        long,
        help = "Ignore .gitignore, .git/info/exclude, and global gitignore rules"
    )]
    no_ignore: bool,

    #[arg(short = 'a', long, help = "Default the explorer to all files on load")]
    all: bool,

    #[arg(long, help = "Clear cached frontend assets before startup")]
    clean: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ghrm=info,warn".into()),
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

    let target = cli
        .target
        .ok_or_else(|| anyhow::anyhow!("missing target"))?;
    let abs = target.canonicalize()?;

    let port = cli.port.or(cfg.port).unwrap_or(1331);
    let bind = cli
        .bind
        .or(cfg.bind)
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let no_ignore = cli.no_ignore || cfg.no_ignore.unwrap_or(false);
    let open = match std::env::var("GHRM_OPEN").as_deref() {
        Ok("0") => false,
        Ok(_) => true,
        Err(_) => cfg.open.unwrap_or(true),
    };
    let default_scope = if cli.all { Scope::All } else { Scope::Md };
    let exclude_names = cfg
        .walk
        .exclude_names
        .unwrap_or_else(config::default_exclude_names);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(server::run(
        bind,
        port,
        open,
        abs,
        !no_ignore,
        default_scope,
        exclude_names,
    ))
}
