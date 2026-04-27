mod auth;
mod config;
mod render;
mod repo;
mod search;
mod server;
mod theme;
mod tmpl;
mod vendor;
mod walk;
mod watch;

use crate::auth::AuthConfig;
use anyhow::Result;
use clap::Parser;
use std::collections::BTreeSet;
use std::net::IpAddr;
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
    let auth = match (cfg.auth.password, cfg.auth.password_hash) {
        (Some(_), Some(_)) => {
            anyhow::bail!("auth.password and auth.password_hash are mutually exclusive")
        }
        (Some(password), None) => Some(AuthConfig {
            username: cfg.auth.username.unwrap_or_else(|| "admin".to_string()),
            password,
        }),
        (None, Some(_)) => anyhow::bail!("auth.password_hash is not supported"),
        (None, None) => {
            if cfg.auth.username.is_some() {
                anyhow::bail!("auth.username requires auth.password");
            }
            None
        }
    };
    if bind_requires_auth(&bind) && auth.is_none() {
        anyhow::bail!("non-loopback bind requires auth.password");
    }
    let no_ignore = cli.no_ignore || cfg.walk.no_ignore.unwrap_or(false);
    let open = !cli.no_browser
        && match std::env::var("GHRM_OPEN").as_deref() {
            Ok("0") => false,
            Ok(_) => true,
            Err(_) => cfg.open.unwrap_or(true),
        };
    let has_explicit_ext_filter = !cli.extensions.is_empty()
        || cfg
            .walk
            .extensions
            .as_ref()
            .is_some_and(|extensions| !extensions.is_empty());
    let extensions = if cli.extensions.is_empty() {
        normalize_extensions(cfg.walk.extensions.unwrap_or_default())?
    } else {
        normalize_extensions(cli.extensions)?
    };
    let extensions = if extensions.is_empty() {
        vec!["md".to_string()]
    } else {
        extensions
    };
    let exclude_names = cfg
        .walk
        .exclude_names
        .unwrap_or_else(config::default_exclude_names);
    let no_excludes = cli.no_excludes || cfg.walk.no_excludes.unwrap_or(false);
    let max_rows = cli.max_rows.or(cfg.search.max_rows).unwrap_or(1000);
    if max_rows == 0 {
        anyhow::bail!("max search rows must be greater than zero");
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(server::run(server::Options {
        bind,
        port,
        open,
        target: abs,
        use_ignore: !no_ignore,
        default_hidden: cli.hidden || cfg.walk.hidden.unwrap_or(false),
        default_filter_ext: has_explicit_ext_filter,
        extensions,
        exclude_names,
        no_excludes,
        search_max_rows: max_rows,
        auth,
    }))
}

fn normalize_extensions(raw: Vec<String>) -> Result<Vec<String>> {
    let mut extensions = BTreeSet::new();
    for ext in raw {
        let ext = ext.trim().trim_start_matches('.').to_lowercase();
        if ext.is_empty() {
            anyhow::bail!("empty extension filter");
        }
        extensions.insert(ext);
    }
    Ok(extensions.into_iter().collect())
}

fn bind_requires_auth(bind: &str) -> bool {
    if bind.eq_ignore_ascii_case("localhost") {
        return false;
    }
    match bind.parse::<IpAddr>() {
        Ok(addr) => !addr.is_loopback(),
        Err(_) => true,
    }
}
