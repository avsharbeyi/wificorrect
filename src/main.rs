//! WifiCorrect — tek program, alt komutlarla çalışır (docs/RUST_YENIDEN_YAZIM.md, B/C bölümleri).

mod ayar;
mod ortak;
mod portal;
mod sms;

use std::process::ExitCode;

const USAGE: &str = "Kullanım: wificorrect <komut>

Komutlar:
  portal      Müşteri giriş portalı (SMS doğrulama)
  kaydedici   5651 bağlantı / DNS / oturum kaydı
  panel       Yönetim paneli (HTTPS)
  ctl         Yönetim komutları (durum, oturumlar, gun-kapat, yedekle ...)
  surum       Sürümü yazar

Ayar dosyası: /etc/wificorrect/ayarlar.toml (WFC_AYAR ortam değişkeniyle değiştirilebilir)";

fn config() -> Result<ayar::Config, ExitCode> {
    let path = std::env::var("WFC_AYAR").unwrap_or_else(|_| ayar::PATH.to_string());
    ayar::Config::load(&path).map_err(|e| {
        eprintln!("{e}");
        ExitCode::from(1)
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("surum") => {
            println!("wificorrect {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("portal") => match config() {
            Ok(cfg) => portal::run(cfg),
            Err(code) => code,
        },
        Some(cmd @ ("kaydedici" | "panel" | "ctl")) => {
            eprintln!("'{cmd}' henüz yazılmadı");
            ExitCode::from(2)
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
