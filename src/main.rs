mod config;
mod lyrics;
mod model;
mod server;
mod theme;
mod ui;

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};

struct Options {
    bind: String,
    port: u16,
    print: bool,
    theme: Option<usize>,
    token: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let options = match parse_args()? {
        Some(options) => options,
        None => return Ok(()),
    };

    let playback = Arc::new(Mutex::new(None));
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", options.bind, options.port))
        .await
        .with_context(|| {
            format!(
                "failed to bind {}:{} (is the port already in use?)",
                options.bind, options.port
            )
        })?;
    let address = listener.local_addr()?;

    let app = server::router(
        playback.clone(),
        env!("CARGO_PKG_VERSION"),
        options.token.clone(),
    );
    eprintln!(
        "ytlrc v{} — daemon listening on http://{} (bind {})",
        env!("CARGO_PKG_VERSION"),
        address,
        options.bind
    );
    eprintln!("POST /state from the extension · GET /health for status");
    if options.token.is_some() {
        eprintln!("API token enabled (Authorization: Bearer <token>)");
    } else if !is_loopback(&options.bind) {
        eprintln!(
            "warning: {} is reachable from other devices and no token is set.",
            options.bind
        );
        eprintln!(
            "         add --token <secret> here and the same TOKEN in extension/background.js"
        );
    }

    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("server stopped: {error}");
        }
    });

    let cache = lyrics::cache::Cache::new()?;
    eprintln!("lyrics cache: {}", cache.dir().display());
    let pruned = cache.prune();
    if pruned > 0 {
        eprintln!("cache: removed {pruned} stale entries");
    }

    if options.print {
        eprintln!("--print mode active (Ctrl+C to stop)");
        ui::run_print(playback, cache).await?;
    } else {
        eprintln!("press q to quit · t to switch theme");
        ui::run(playback, cache, options.theme).await?;
    }

    Ok(())
}

fn parse_args() -> Result<Option<Options>> {
    let mut bind = "127.0.0.1".to_string();
    let mut port = 8765u16;
    let mut print = false;
    let mut theme = None;
    let mut token = std::env::var("YTLRC_TOKEN")
        .ok()
        .filter(|v| !v.trim().is_empty());

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("ytlrc {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "--print" => print = true,
            "--bind" => {
                bind = args
                    .next()
                    .context("--bind needs an address, for example 127.0.0.1")?;
            }
            "--port" => {
                let value = args.next().context("--port needs a number")?;
                port = value
                    .parse::<u16>()
                    .with_context(|| format!("invalid port: {value}"))?;
            }
            "--theme" => {
                let value = args
                    .next()
                    .context("--theme needs a name or a number between 1 and 5")?;
                theme = Some(theme::by_name(&value).with_context(|| {
                    format!("unknown theme: {value}. Available: {}", theme::names())
                })?);
            }
            "--token" => {
                let value = args.next().context("--token needs a secret value")?;
                if value.trim().is_empty() {
                    anyhow::bail!("--token must not be empty");
                }
                token = Some(value);
            }
            other => anyhow::bail!("unknown argument: {other} (see --help)"),
        }
    }

    Ok(Some(Options {
        bind,
        port,
        print,
        theme,
        token,
    }))
}

fn is_loopback(bind: &str) -> bool {
    matches!(bind, "127.0.0.1" | "localhost" | "::1")
}

fn print_help() {
    println!(
        "ytlrc {} — YouTube lyrics in your terminal\n\n\
         Usage:\n  ytlrc [options]\n\n\
         Options:\n  --print        no TUI, print the active lyric line (debug/scripts)\n  --bind <addr>  listen address (default 127.0.0.1; use 0.0.0.0 from WSL)\n  --port <port>  daemon port (default 8765)\n  --token <s>    require this token from clients (or set YTLRC_TOKEN)\n  --theme <t>    initial theme: {} — or a number from 1 to 5\n  -h, --help     show this help\n  -V, --version  show the version\n\n\
         Themes (switch while running):\n  t              next theme\n  1-5            pick a theme directly\n  Available: {}\n\n\
         TUI keybindings:\n  q / Esc        quit\n  + / -          shift lyrics by 0.25 seconds\n  0              reset offset\n  p              toggle synced / plain\n  r              refetch lyrics now (bypass cache)\n  m              search manually \"Artist - Title\"\n  ↑ / ↓ / PgUp / PgDn  scroll in plain mode\n\n\
         Automatic lookups: ytlrc searches for lyrics whenever the song changes.\n\
         If a lookup fails it keeps retrying on its own (8s, 25s, 90s, 240s, then\n\
         every 5 minutes) with different query strategies — no key press needed.\n\n\
         Security: the API binds loopback by default. When using --bind 0.0.0.0,\n\
         set --token and the same TOKEN in extension/background.js.\n\n\
         Lyrics come from LRCLIB (https://lrclib.net) and require `curl` on PATH.",
        env!("CARGO_PKG_VERSION"),
        theme::names(),
        theme::names()
    );
}
