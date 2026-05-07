use crate::config::Config;
use crate::http::auth::AuthConfig;

use anyhow::{Result, bail};
use std::collections::BTreeSet;
use std::net::IpAddr;
use std::path::PathBuf;

pub struct Resolved {
    pub target: PathBuf,
    pub config_path: Option<PathBuf>,
    pub bind: String,
    pub port: u16,
    pub exact_port: bool,
    pub open: bool,
    pub auth: Option<AuthConfig>,
    pub use_ignore: bool,
    pub show_hidden: bool,
    pub show_excludes: bool,
    pub filter_ext: bool,
    pub extensions: Vec<String>,
    pub exclude_names: Vec<String>,
    pub max_rows: usize,
    pub stats: ghrm_stat::Config,
}

pub struct Input<'a> {
    pub target: Option<PathBuf>,
    pub config_path: Option<&'a std::path::Path>,
    pub port: Option<u16>,
    pub bind: Option<String>,
    pub no_browser: bool,
    pub no_ignore: bool,
    pub hidden: bool,
    pub extensions: Vec<String>,
    pub no_excludes: bool,
    pub max_rows: Option<usize>,
    pub ghrm_open: Option<String>,
}

pub fn resolve(cli: Input<'_>, cfg: &Config) -> Result<Resolved> {
    let target = cli.target.unwrap_or(std::env::current_dir()?);
    let target = target.canonicalize()?;

    let config_path = crate::config::path(cli.config_path)?;
    let exact_port = cli.port.is_some();
    let port = cli.port.or(cfg.port).unwrap_or(1331);
    let bind = cli
        .bind
        .or_else(|| cfg.bind.clone())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let auth = resolve_auth(&cfg.auth)?;
    if bind_requires_auth(&bind) && auth.is_none() {
        bail!("non-loopback bind requires auth.password");
    }

    let no_ignore = cli.no_ignore || cfg.walk.no_ignore.unwrap_or(false);
    let use_ignore = !no_ignore;

    let open = resolve_open(cli.no_browser, cfg.open, cli.ghrm_open.as_deref());

    let has_explicit_ext_filter = !cli.extensions.is_empty()
        || cfg
            .walk
            .extensions
            .as_ref()
            .is_some_and(|extensions| !extensions.is_empty());

    let extensions = if cli.extensions.is_empty() {
        normalize_extensions(cfg.walk.extensions.clone().unwrap_or_default())?
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
        .clone()
        .unwrap_or_else(crate::config::default_exclude_names);
    let show_excludes = cli.no_excludes || cfg.walk.no_excludes.unwrap_or(false);

    let max_rows = cli.max_rows.or(cfg.search.max_rows).unwrap_or(1000);
    if max_rows == 0 {
        bail!("max search rows must be greater than zero");
    }

    let show_hidden = cli.hidden || cfg.walk.hidden.unwrap_or(false);
    let filter_ext = has_explicit_ext_filter || cfg.walk.filter.enabled.unwrap_or(false);

    Ok(Resolved {
        target,
        config_path,
        bind,
        port,
        exact_port,
        open,
        auth,
        use_ignore,
        show_hidden,
        show_excludes,
        filter_ext,
        extensions,
        exclude_names,
        max_rows,
        stats: cfg.stats.clone().resolve(),
    })
}

fn resolve_auth(auth: &crate::config::AuthConfig) -> Result<Option<AuthConfig>> {
    match (&auth.password, &auth.password_hash) {
        (Some(_), Some(_)) => {
            bail!("auth.password and auth.password_hash are mutually exclusive")
        }
        (Some(password), None) => Ok(Some(AuthConfig {
            username: auth.username.clone().unwrap_or_else(|| "admin".to_string()),
            password: password.clone(),
        })),
        (None, Some(_)) => bail!("auth.password_hash is not supported"),
        (None, None) => {
            if auth.username.is_some() {
                bail!("auth.username requires auth.password");
            }
            Ok(None)
        }
    }
}

fn resolve_open(no_browser: bool, config_open: Option<bool>, env_open: Option<&str>) -> bool {
    if no_browser {
        return false;
    }
    match env_open {
        Some("0") => false,
        Some(_) => true,
        None => config_open.unwrap_or(true),
    }
}

fn normalize_extensions(raw: Vec<String>) -> Result<Vec<String>> {
    let mut extensions = BTreeSet::new();
    for ext in raw {
        let ext = ext.trim().trim_start_matches('.').to_lowercase();
        if ext.is_empty() {
            bail!("empty extension filter");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn default_input() -> Input<'static> {
        Input {
            target: None,
            config_path: None,
            port: None,
            bind: None,
            no_browser: false,
            no_ignore: false,
            hidden: false,
            extensions: vec![],
            no_excludes: false,
            max_rows: None,
            ghrm_open: None,
        }
    }

    fn resolve_err(input: Input<'_>, cfg: &Config) -> String {
        match resolve(input, cfg) {
            Ok(_) => panic!("expected option resolution to fail"),
            Err(err) => err.to_string(),
        }
    }

    #[test]
    fn default_bind_port() {
        let resolved = resolve(default_input(), &Config::default()).unwrap();
        assert_eq!(resolved.bind, "127.0.0.1");
        assert_eq!(resolved.port, 1331);
        assert!(!resolved.exact_port);
    }

    #[test]
    fn cli_port_sets_exact_port() {
        let input = Input {
            port: Some(8080),
            ..default_input()
        };
        let resolved = resolve(input, &Config::default()).unwrap();
        assert_eq!(resolved.port, 8080);
        assert!(resolved.exact_port);
    }

    #[test]
    fn ghrm_open_zero_disables_browser() {
        let input = Input {
            ghrm_open: Some("0".to_string()),
            ..default_input()
        };
        let resolved = resolve(input, &Config::default()).unwrap();
        assert!(!resolved.open);
    }

    #[test]
    fn no_browser_flag_disables_browser() {
        let input = Input {
            no_browser: true,
            ..default_input()
        };
        let resolved = resolve(input, &Config::default()).unwrap();
        assert!(!resolved.open);
    }

    #[test]
    fn auth_password_required_with_username() {
        let cfg: Config = toml::from_str(
            r#"
            [auth]
            username = "admin"
            "#,
        )
        .unwrap();
        let err = resolve_err(default_input(), &cfg);
        assert!(err.contains("auth.username requires auth.password"));
    }

    #[test]
    fn auth_password_hash_rejected() {
        let cfg: Config = toml::from_str(
            r#"
            [auth]
            password_hash = "somehash"
            "#,
        )
        .unwrap();
        let err = resolve_err(default_input(), &cfg);
        assert!(err.contains("auth.password_hash is not supported"));
    }

    #[test]
    fn auth_password_and_hash_mutually_exclusive() {
        let cfg: Config = toml::from_str(
            r#"
            [auth]
            password = "secret"
            password_hash = "somehash"
            "#,
        )
        .unwrap();
        let err = resolve_err(default_input(), &cfg);
        assert!(err.contains("mutually exclusive"));
    }

    #[test]
    fn extension_normalization() {
        let input = Input {
            extensions: vec![".MD".to_string(), "  txt  ".to_string()],
            ..default_input()
        };
        let resolved = resolve(input, &Config::default()).unwrap();
        assert_eq!(resolved.extensions, vec!["md", "txt"]);
    }

    #[test]
    fn empty_extension_rejected() {
        let input = Input {
            extensions: vec!["".to_string()],
            ..default_input()
        };
        let err = resolve_err(input, &Config::default());
        assert!(err.contains("empty extension filter"));
    }

    #[test]
    fn positive_walk_options() {
        let input = Input {
            no_ignore: true,
            hidden: true,
            no_excludes: true,
            ..default_input()
        };
        let resolved = resolve(input, &Config::default()).unwrap();
        assert!(!resolved.use_ignore);
        assert!(resolved.show_hidden);
        assert!(resolved.show_excludes);
    }

    #[test]
    fn non_loopback_bind_requires_auth() {
        let input = Input {
            bind: Some("0.0.0.0".to_string()),
            ..default_input()
        };
        let err = resolve_err(input, &Config::default());
        assert!(err.contains("non-loopback bind requires auth.password"));
    }

    #[test]
    fn localhost_bind_does_not_require_auth() {
        let input = Input {
            bind: Some("localhost".to_string()),
            ..default_input()
        };
        let resolved = resolve(input, &Config::default()).unwrap();
        assert_eq!(resolved.bind, "localhost");
    }
}
