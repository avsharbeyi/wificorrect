//! WifiCorrect — tek program, alt komutlarla çalışır (docs/RUST_YENIDEN_YAZIM.md, B/C bölümleri).

use std::process::ExitCode;

const USAGE: &str = "Kullanım: wificorrect <komut>

Komutlar:
  portal      Müşteri giriş portalı (SMS doğrulama)
  kaydedici   5651 bağlantı / DNS / oturum kaydı
  panel       Yönetim paneli (HTTPS)
  ctl         Yönetim komutları (durum, oturumlar, gun-kapat, yedekle ...)
  surum       Sürümü yazar";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("surum") => {
            println!("wificorrect {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(cmd @ ("portal" | "kaydedici" | "panel" | "ctl")) => {
            eprintln!("'{cmd}' henüz yazılmadı");
            ExitCode::from(2)
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
