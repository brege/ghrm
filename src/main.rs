mod assets;
mod config;
mod render;
mod server;
mod tmpl;
mod vendor;
mod walk;
mod watch;

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

    #[arg(long = "static", help = "Render to stdout and exit")]
    render_static: bool,

    #[arg(
        long,
        help = "Ignore .gitignore, .git/info/exclude, and global gitignore rules"
    )]
    no_ignore: bool,

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
    }
    vendor::ensure()?;

    let target = cli
        .target
        .ok_or_else(|| anyhow::anyhow!("missing target"))?;
    let abs = target.canonicalize()?;

    if cli.render_static {
        let md = std::fs::read_to_string(&abs)?;
        let root = abs.parent().unwrap_or(&abs);
        let rendered = render::render_at(&md, Some(render::RenderPath { root, src: &abs }));
        let page = tmpl::page(&rendered.html);
        let html = tmpl::base(tmpl::PageShell {
            title: &rendered.title,
            body: &page,
            live_reload: false,
        });
        print!("{}", html);
        return Ok(());
    }

    let port = cli.port.or(cfg.port).unwrap_or(1313);
    let bind = cli
        .bind
        .or(cfg.bind)
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let open = match std::env::var("GHRM_OPEN").as_deref() {
        Ok("0") => false,
        Ok(_) => true,
        Err(_) => cfg.open.unwrap_or(true),
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(server::run(bind, port, open, abs, !cli.no_ignore))
}
