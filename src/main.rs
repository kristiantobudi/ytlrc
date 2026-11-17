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
        .with_context(|| format!("gagal bind {}:{} (port dipakai?)", options.bind, options.port))?;
    let address = listener.local_addr()?;

    let app = server::router(playback.clone(), env!("CARGO_PKG_VERSION"));
    eprintln!(
        "ytlrc v{} — daemon siap di http://{} (bind {})",
        env!("CARGO_PKG_VERSION"),
        address,
        options.bind
    );
    eprintln!("POST /state dari extension · GET /health untuk cek status");

    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("server berhenti: {error}");
        }
    });

    let cache = lyrics::cache::Cache::new()?;
    eprintln!("cache lirik: {}", cache.dir().display());

    if options.print {
        eprintln!("mode --print aktif (Ctrl+C untuk berhenti)");
        ui::run_print(playback, cache).await?;
    } else {
        eprintln!("tekan q untuk keluar · t untuk ganti tema");
        ui::run(playback, cache, options.theme).await?;
    }

    Ok(())
}

fn parse_args() -> Result<Option<Options>> {
    let mut bind = "0.0.0.0".to_string();
    let mut port = 8765u16;
    let mut print = false;
    let mut theme = None;

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
                bind = args.next().context("--bind butuh alamat, contoh 127.0.0.1")?;
            }
            "--port" => {
                let value = args.next().context("--port butuh angka")?;
                port = value
                    .parse::<u16>()
                    .with_context(|| format!("port tidak valid: {value}"))?;
            }
            "--theme" => {
                let value = args.next().context("--theme butuh nama atau nomor 1-5")?;
                theme = Some(theme::by_name(&value).with_context(|| {
                    format!(
                        "tema tidak dikenal: {value}. Pilihan: {}",
                        theme::names()
                    )
                })?);
            }
            other => anyhow::bail!("argumen tidak dikenal: {other} (lihat --help)"),
        }
    }

    Ok(Some(Options {
        bind,
        port,
        print,
        theme,
    }))
}

fn print_help() {
    println!(
        "ytlrc {} — lirik YouTube di terminal\n\n\
         Penggunaan:\n  ytlrc [opsi]\n\n\
         Opsi:\n  --print        tanpa TUI, cetak baris lirik aktif (debug/script)\n  --bind <addr>  alamat bind (default 0.0.0.0, supaya bisa diakses dari Windows)\n  --port <port>  port daemon (default 8765)\n  --theme <t>    tema awal: {} — atau nomor 1-5\n  -h, --help     bantuan\n  -V, --version  versi\n\n\
         Theme (bisa diganti saat berjalan):\n  t              tema berikutnya\n  1-5            pilih tema langsung\n  Pilihan: {}\n\n\
         Keybinding TUI:\n  q / Esc        keluar\n  + / -          geser offset lirik 0.25 detik\n  0              reset offset\n  p              toggle synced / plain\n  r              ambil ulang lirik sekarang (abaikan cache)\n  m              cari manual \"Artis - Judul\"\n  ↑ / ↓ / PgUp / PgDn  scroll mode plain\n\n\
         Pencarian lirik otomatis: saat lagu berganti, ytlrc mencari sendiri.\n\
         Kalau gagal, ia mencoba lagi otomatis (8s, 25s, 90s, 240s, lalu tiap 5 menit)\n\
         dengan strategi query berbeda — tanpa perlu menekan tombol.\n\n\
         Lirik berasal dari LRCLIB (https://lrclib.net), butuh `curl` di PATH.",
        env!("CARGO_PKG_VERSION"),
        theme::names(),
        theme::names()
    );
}
