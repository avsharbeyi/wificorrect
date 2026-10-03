//! Uzak erişim (2026-10-03, kullanıcı kararı: kendi sunucumuz + WireGuard). Cihaz NAT arkasından sunucuya tünel açar ve
//! canlı tutar; admin aynı sunucuya bağlanıp cihazın tünel adresinden (10.99.0.x) panele ve SSH'a girer.
//! Tünel yalnızca VPN ağını taşır (internet trafiği tünelden geçmez); güvenlik duvarı tünelden yalnızca 22 ve 8443'ü açar.
//! Gizli anahtar cihazda üretilir, cihazdan çıkmaz; açık anahtar Admin ayarları'nda görünür, sunucuya o eklenir.

use crate::ayar::Config;
use crate::ortak::{self, Row, Runner};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub const KEY: &str = "/etc/wificorrect/wg.key";
/// Yedek (rsync/SSH) anahtarı: cihazda üretilir, açık anahtarı sunucudaki yalnızca-yazma hesaba eklenir
pub const YEDEK_KEY: &str = "/etc/wificorrect/yedek_anahtar";
pub const CONF: &str = "/etc/wireguard/wfc.conf";
pub const IFACE: &str = "wfc";
pub const UNIT: &str = "wg-quick@wfc.service";
/// VPN ağı: sunucu .1, yönetici bilgisayarları ve cihazlar diğer adresler (ponytail: sabit; guvenlik.nft de bunu açar)
pub const AG: &str = "10.99.0.0/24";

pub fn key_ok(s: &str) -> bool {
    s.len() == 44 && s.ends_with('=') && s[..43].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
}

/// `vpn.ornek.com:51820` ya da `203.0.113.5:51820`
pub fn endpoint_ok(s: &str) -> bool {
    let Some((host, port)) = s.rsplit_once(':') else { return false };
    let host_ok = !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|l| (1..=63).contains(&l.len()) && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') && !l.starts_with('-'));
    host_ok && port.parse::<u16>().is_ok_and(|p| p > 0)
}

/// Cihazın tünel adresi: 10.99.0.2 – 10.99.0.254 (.1 sunucu)
pub fn adres_ok(s: &str) -> bool {
    s.parse::<std::net::Ipv4Addr>().is_ok_and(|ip| ortak::in_subnet(s, AG) && (2..=254).contains(&ip.octets()[3]))
}

/// Gizli anahtar yoksa üretir (600). Dönen: gizli anahtar.
pub fn ensure_key(path: &Path) -> Result<String, String> {
    if let Ok(k) = std::fs::read_to_string(path) {
        if key_ok(k.trim()) {
            return Ok(k.trim().to_string());
        }
    }
    let out = Command::new("wg").arg("genkey").output().map_err(|e| format!("wg genkey: {e}"))?;
    let k = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !key_ok(&k) {
        return Err("anahtar üretilemedi".into());
    }
    ortak::write_atomic(path, format!("{k}\n").as_bytes()).map_err(|e| e.to_string())?;
    Ok(k)
}

pub fn public_key(private: &str) -> Result<String, String> {
    let mut child = Command::new("wg").arg("pubkey").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().map_err(|e| format!("wg pubkey: {e}"))?;
    child.stdin.take().ok_or("stdin")?.write_all(format!("{private}\n").as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let k = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if key_ok(&k) { Ok(k) } else { Err("açık anahtar hesaplanamadı".into()) }
}

/// Yedek SSH anahtarı yoksa üretir; dönen: açık anahtar satırı (ssh-ed25519 …).
pub fn yedek_anahtari(path: &Path) -> Result<String, String> {
    if !path.exists() {
        let host = ortak::capture(&["hostname"]).trim().to_string();
        let ok = Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-C", &format!("wificorrect-{host}"), "-f"])
            .arg(path)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            return Err("yedek anahtarı üretilemedi (ssh-keygen)".into());
        }
    }
    let pub_path = format!("{}.pub", path.display());
    std::fs::read_to_string(&pub_path).map(|s| s.trim().to_string()).map_err(|e| format!("{pub_path}: {e}"))
}

/// İşletme adından sunucu kayıt adı: "Bocafe Kadıköy" → "bocafe-kadikoy" (küçük harf, rakam, tire; en çok 31).
pub fn kayit_adi(site: &str) -> String {
    let mut out = String::new();
    for c in site.chars() {
        let c = match c {
            'ç' | 'Ç' => 'c',
            'ğ' | 'Ğ' => 'g',
            'ı' | 'I' | 'İ' | 'i' => 'i',
            'ö' | 'Ö' => 'o',
            'ş' | 'Ş' => 's',
            'ü' | 'Ü' => 'u',
            c if c.is_ascii_alphanumeric() => c.to_ascii_lowercase(),
            _ => '-',
        };
        if c != '-' || !out.ends_with('-') && !out.is_empty() {
            out.push(c);
        }
    }
    let s: String = out.trim_end_matches('-').chars().take(31).collect();
    let s = s.trim_end_matches('-').to_string();
    if s.is_empty() { "isletme".into() } else { s }
}

/// Merkez sunucuda bu cihazı tanıtan komut (panelde kopyalanmak üzere gösterilir).
pub fn sunucu_komutu(site: &str, wg_pub: &str, ssh_pub: &str) -> String {
    format!("sudo wificorrect-sunucu cihaz-ekle {} {wg_pub} \"{ssh_pub}\"", kayit_adi(site))
}

pub fn eksik(cfg: &Config) -> Option<&'static str> {
    let u = &cfg.uzak;
    if !endpoint_ok(&u.sunucu) {
        Some("sunucu adresi (adres:port)")
    } else if !key_ok(&u.sunucu_anahtar) {
        Some("sunucunun açık anahtarı")
    } else if !adres_ok(&u.adres) {
        Some("cihazın tünel adresi (10.99.0.2–254)")
    } else {
        None
    }
}

pub fn conf_text(cfg: &Config, private: &str) -> String {
    let u = &cfg.uzak;
    format!(
        "# WifiCorrect üretir (panel → Admin ayarları → Uzak erişim). Elle değiştirmeyin.\n\
         [Interface]\nPrivateKey = {private}\nAddress = {}/24\n\n\
         [Peer]\nPublicKey = {}\nEndpoint = {}\nAllowedIPs = {AG}\nPersistentKeepalive = 25\n",
        u.adres, u.sunucu_anahtar, u.sunucu
    )
}

/// Ayara göre tüneli açar ya da kapatır.
pub fn uygula(cfg: &Config, key: &Path, conf: &Path, runner: &Runner) -> Result<String, String> {
    let cmd = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    if !cfg.uzak.enabled {
        runner(&cmd(&["systemctl", "disable", "--now", UNIT]));
        let _ = std::fs::remove_file(conf);
        return Ok("uzak erişim kapalı".into());
    }
    if let Some(e) = eksik(cfg) {
        return Err(format!("eksik ya da geçersiz: {e}"));
    }
    let private = ensure_key(key)?;
    if let Some(d) = conf.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    ortak::write_atomic(conf, conf_text(cfg, &private).as_bytes()).map_err(|e| format!("{}: {e}", conf.display()))?;
    runner(&cmd(&["systemctl", "enable", UNIT]));
    if !runner(&cmd(&["systemctl", "restart", UNIT])) {
        return Err("tünel başlatılamadı (sunucu adresi çözülemedi ya da ayar hatalı olabilir)".into());
    }
    Ok(format!("tünel kuruldu: {} → {}", cfg.uzak.adres, cfg.uzak.sunucu))
}

/// `wg show wfc latest-handshakes` çıktısından son el sıkışmanın zamanı (epoch).
pub fn last_handshake(wg_show: &str) -> Option<f64> {
    wg_show.lines().filter_map(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok()).filter(|&t| t > 0.0).reduce(f64::max)
}

/// Panelde gösterilen durum.
pub fn durum(cfg: &Config, now: f64) -> String {
    if !cfg.uzak.enabled {
        return "kapalı".into();
    }
    match last_handshake(&ortak::capture(&["wg", "show", IFACE, "latest-handshakes"])) {
        Some(t) if now - t < 180.0 => format!("bağlı (son el sıkışma {} sn önce)", (now - t).max(0.0) as u64),
        Some(t) => format!("BAĞLANTI YOK (son el sıkışma {} dk önce)", ((now - t) / 60.0) as u64),
        None => "BAĞLANTI YOK (sunucuyla hiç el sıkışılmadı)".into(),
    }
}

pub fn audit(cfg: &Config, now: f64, r: &Result<String, String>) {
    let ek = match r {
        Ok(m) => format!("sonuc={m}"),
        Err(e) => format!("hata={e}"),
    };
    ortak::audit(&cfg.main.log_root, Row::new("UZAK_ERISIM", &ortak::now_iso(now)).set("ek", ek));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    const K: &str = "aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789+/abcdE=";

    #[test]
    fn validation_and_config() {
        assert!(key_ok(K) && !key_ok("kısa") && !key_ok(&K.replace('=', "A")));
        assert!(endpoint_ok("vpn.wificorrect.com:51820") && endpoint_ok("203.0.113.5:51820"));
        assert!(!endpoint_ok("vpn.wificorrect.com") && !endpoint_ok(":51820") && !endpoint_ok("a b:1") && !endpoint_ok("x.com:0"));
        assert!(adres_ok("10.99.0.17") && !adres_ok("10.99.0.1") && !adres_ok("10.98.0.17") && !adres_ok("10.99.0.255"));
        let mut cfg = Config::default();
        cfg.uzak.enabled = true;
        assert_eq!(eksik(&cfg), Some("sunucu adresi (adres:port)"));
        cfg.uzak.sunucu = "vpn.wificorrect.com:51820".into();
        cfg.uzak.sunucu_anahtar = K.into();
        cfg.uzak.adres = "10.99.0.17".into();
        assert_eq!(eksik(&cfg), None);
        let t = conf_text(&cfg, "GIZLI");
        assert!(t.contains("PrivateKey = GIZLI\nAddress = 10.99.0.17/24") && t.contains("AllowedIPs = 10.99.0.0/24") && t.contains("PersistentKeepalive = 25"));
        assert_eq!(last_handshake(&format!("{K}\t1791000000\n{K}\t0\n")), Some(1_791_000_000.0));
        assert_eq!(last_handshake(&format!("{K}\t0\n")), None);
        assert_eq!(kayit_adi("Bocafe Kadıköy Şubesi"), "bocafe-kadikoy-subesi");
        assert_eq!(kayit_adi("  Çay & Ötesi!! "), "cay-otesi");
        assert_eq!(kayit_adi("İşletme"), "isletme");
        assert_eq!(kayit_adi("???"), "isletme");
        assert_eq!(kayit_adi(&"a".repeat(40)).len(), 31);
        assert_eq!(sunucu_komutu("Bocafe", "W=", "ssh-ed25519 AAAA x"), "sudo wificorrect-sunucu cihaz-ekle bocafe W= \"ssh-ed25519 AAAA x\"");
    }

    #[test]
    fn apply_enable_disable() {
        let dir = std::env::temp_dir().join(format!("wfc-uzak-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (key, conf) = (dir.join("wg.key"), dir.join("wireguard/wfc.conf"));
        std::fs::write(&key, format!("{K}\n")).unwrap(); // var olan anahtar korunur
        let calls: Arc<Mutex<Vec<String>>> = Arc::default();
        let c2 = calls.clone();
        let runner = move |c: &[String]| {
            c2.lock().unwrap().push(c.join(" "));
            true
        };
        let mut cfg = Config::default();
        cfg.uzak.enabled = true;
        assert!(uygula(&cfg, &key, &conf, &runner).unwrap_err().contains("eksik"));
        cfg.uzak.sunucu = "vpn.wificorrect.com:51820".into();
        cfg.uzak.sunucu_anahtar = K.into();
        cfg.uzak.adres = "10.99.0.17".into();
        uygula(&cfg, &key, &conf, &runner).unwrap();
        assert!(std::fs::read_to_string(&conf).unwrap().contains(&format!("PrivateKey = {K}")));
        assert!(calls.lock().unwrap().iter().any(|c| c == "systemctl restart wg-quick@wfc.service"));
        cfg.uzak.enabled = false;
        uygula(&cfg, &key, &conf, &runner).unwrap();
        assert!(!conf.exists() && calls.lock().unwrap().iter().any(|c| c == "systemctl disable --now wg-quick@wfc.service"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
