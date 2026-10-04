//! Yönetim merkezi bağı (spec 2026-10-04 §7): müşteri numarası + parola merkezde açılır; cihaz ilk girişte
//! `api.wificorrect.com`'a bağlanır, bağ `merkez.json`'da tutulur (internetsiz giriş yerel özetle). Her gün 06:00'dan
//! sonra eşitlenir; serbest bırakılınca fabrika akışı kayıtları teslim edip bağı siler. Cihaz anahtarı loglanmaz.

use crate::ayar::Config;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub const PATH: &str = "/etc/wificorrect/merkez.json";
pub const API: &str = "https://api.wificorrect.com";
const TR: f64 = 3.0 * 3600.0; // Türkiye UTC+3, yaz saati yok
const YENIDEN_SN: f64 = 30.0 * 60.0;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Merkez {
    pub numara: String,
    pub tuz: String,
    pub ozet: String,
    pub yineleme: u32,
    pub cihaz_anahtari: String,
    /// son başarılı eşitleme (epoch); ilk girişte giriş anı
    #[serde(default)]
    pub son_eslesme: f64,
    /// son eşitleme denemesi (epoch) — başarısızsa 30 dk sonra yeniden
    #[serde(default)]
    pub deneme: f64,
}

/// (yol, JSON gövde) → (HTTP durumu, gövde). Err: merkeze ulaşılamadı.
pub type Http = dyn Fn(&str, &str) -> Result<(u16, String), String> + Send + Sync;

/// Gerçek istemci: curl, sertifika doğrulanır, gövde standart girdiden (parola komut satırında görünmez).
pub fn curl(yol: &str, govde: &str) -> Result<(u16, String), String> {
    let mut c = Command::new("curl")
        .args(["-s", "--max-time", "40", "-X", "POST", "-H", "Content-Type: application/json", "--data-binary", "@-", "-w", "\n%{http_code}"])
        .arg(format!("{API}{yol}"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("curl: {e}"))?;
    c.stdin.take().ok_or("curl stdin")?.write_all(govde.as_bytes()).map_err(|e| e.to_string())?;
    let out = c.wait_with_output().map_err(|e| e.to_string())?;
    let s = String::from_utf8_lossy(&out.stdout);
    let (g, kod) = s.rsplit_once('\n').ok_or("merkezden yanıt yok")?;
    match kod.trim().parse::<u16>() {
        Ok(k) if k > 0 => Ok((k, g.to_string())),
        _ => Err("merkeze ulaşılamadı".into()),
    }
}

pub fn oku(path: &Path) -> Option<Merkez> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

pub fn kaydet(path: &Path, m: &Merkez) -> Result<(), String> {
    let data = serde_json::to_vec_pretty(m).map_err(|e| e.to_string())?;
    crate::ortak::write_atomic(path, &data).map_err(|e| format!("{}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn sil(path: &Path) {
    let _ = std::fs::remove_file(path);
}

pub fn numara_gecerli(s: &str) -> bool {
    s.len() == 7 && s.bytes().all(|b| b.is_ascii_digit()) && !s.starts_with('0')
}

pub fn dogrula(m: &Merkez, pw: &str) -> bool {
    crate::hesap::ct_eq(&crate::hesap::digest(pw, &m.tuz, m.yineleme), &m.ozet)
}

#[derive(Debug)]
pub enum GirisHata {
    /// merkezin Türkçe mesajı; bool: hatalı parola gibi kilit sayacına yazılmalı mı
    Mesaj(String, bool),
    Baglanti,
}

pub struct Giris {
    pub merkez: Merkez,
    pub tunel_ip: String,
    pub sunucu_pub: String,
    pub uc_nokta: String,
    pub yedek_hedefi: String,
}

fn metin(v: &Value, k: &str) -> Option<String> {
    v.get(k)?.as_str().map(String::from)
}

/// Yanıttan özet alanları (tuz, ozet, yineleme).
fn ozet_alanlari(v: &Value) -> Option<(String, String, u32)> {
    Some((metin(v, "tuz")?, metin(v, "ozet")?, u32::try_from(v.get("yineleme")?.as_u64()?).ok()?))
}

/// Hata yanıtı ise (mesaj, kod).
fn hata(v: &Value) -> Option<(String, String)> {
    Some((metin(v, "hata")?, metin(v, "kod").unwrap_or_default()))
}

fn cagir(http: &Http, yol: &str, govde: &Value) -> Result<(u16, Value), String> {
    let (d, g) = http(yol, &govde.to_string())?;
    let v: Value = serde_json::from_str(&g).map_err(|_| format!("merkezden geçersiz yanıt (HTTP {d})"))?;
    Ok((d, v))
}

pub fn giris(numara: &str, pw: &str, wg_pub: &str, ssh_pub: &str, isletme_adi: &str, unvan: &str, http: &Http, now: f64) -> Result<Giris, GirisHata> {
    let govde = json!({"numara": numara, "parola": pw, "wg_pub": wg_pub, "ssh_pub": ssh_pub,
                       "isletme_adi": isletme_adi, "unvan": unvan, "surum": env!("CARGO_PKG_VERSION")});
    let (d, v) = cagir(http, "/api/giris", &govde).map_err(|_| GirisHata::Baglanti)?;
    if d != 200 {
        return match hata(&v) {
            Some((m, kod)) => Err(GirisHata::Mesaj(m, kod == "hatali")),
            None => Err(GirisHata::Baglanti),
        };
    }
    let alanlar = (|| {
        let (tuz, ozet, yineleme) = ozet_alanlari(&v)?;
        Some(Giris {
            merkez: Merkez { numara: numara.into(), tuz, ozet, yineleme, cihaz_anahtari: metin(&v, "cihaz_anahtari")?, son_eslesme: now, deneme: 0.0 },
            tunel_ip: metin(&v, "tunel_ip")?,
            sunucu_pub: metin(&v, "sunucu_pub")?,
            uc_nokta: metin(&v, "uc_nokta")?,
            yedek_hedefi: metin(&v, "yedek_hedefi")?,
        })
    })();
    alanlar.ok_or(GirisHata::Baglanti)
}

/// Bağlanan cihazın uzak erişimi ve uzak yedeği merkezden gelen değerlerle açılır.
pub fn uygula(cfg: &mut Config, g: &Giris) {
    cfg.uzak.enabled = true;
    cfg.uzak.sunucu.clone_from(&g.uc_nokta);
    cfg.uzak.sunucu_anahtar.clone_from(&g.sunucu_pub);
    cfg.uzak.adres.clone_from(&g.tunel_ip);
    cfg.backup.enabled = true;
    cfg.backup.target.clone_from(&g.yedek_hedefi);
}

#[derive(Debug)]
pub enum Eslesme {
    Bagli { uyelik: String },
    Serbest,
    Taninmadi,
}

pub fn eslesme(m: &mut Merkez, isletme_adi: &str, unvan: &str, temizlendi: bool, http: &Http, now: f64) -> Result<Eslesme, String> {
    let mut govde = json!({"cihaz_anahtari": m.cihaz_anahtari, "isletme_adi": isletme_adi, "unvan": unvan, "surum": env!("CARGO_PKG_VERSION")});
    if temizlendi {
        govde["temizlendi"] = Value::Bool(true);
    }
    let (d, v) = cagir(http, "/api/eslesme", &govde)?;
    if d == 401 && hata(&v).is_some_and(|(_, k)| k == "taninmadi") {
        return Ok(Eslesme::Taninmadi);
    }
    if d != 200 {
        return Err(hata(&v).map_or(format!("HTTP {d}"), |(m, _)| m));
    }
    match metin(&v, "durum").as_deref() {
        Some("serbest") => Ok(Eslesme::Serbest),
        Some("bagli") => {
            let (tuz, ozet, yineleme) = ozet_alanlari(&v).ok_or("merkez yanıtında parola özeti yok")?;
            (m.tuz, m.ozet, m.yineleme, m.son_eslesme) = (tuz, ozet, yineleme, now);
            Ok(Eslesme::Bagli { uyelik: metin(&v, "uyelik").unwrap_or_default() })
        }
        _ => Err("merkezden geçersiz yanıt".into()),
    }
}

pub fn parola(m: &mut Merkez, eski: &str, yeni: &str, http: &Http) -> Result<(), String> {
    let govde = json!({"cihaz_anahtari": m.cihaz_anahtari, "eski": eski, "yeni": yeni});
    let (d, v) = cagir(http, "/api/parola", &govde).map_err(|_| "Parola değişimi için internet gerekli.".to_string())?;
    if d != 200 {
        return Err(hata(&v).map_or(format!("Merkez parola değişimini kabul etmedi (HTTP {d})."), |(m, _)| m));
    }
    let (tuz, ozet, yineleme) = ozet_alanlari(&v).ok_or("Merkezden geçersiz yanıt.")?;
    (m.tuz, m.ozet, m.yineleme) = (tuz, ozet, yineleme);
    Ok(())
}

/// Son başarılı eşitlemeden sonraki ilk 06:00 (TR) geçtiyse ve son denemenin üzerinden 30 dk geçtiyse.
pub fn zamani_geldi(m: &Merkez, now: f64) -> bool {
    let yerel = m.son_eslesme + TR;
    let mut alti = (yerel / 86_400.0).floor() * 86_400.0 + 6.0 * 3600.0;
    if yerel >= alti {
        alti += 86_400.0;
    }
    now >= alti - TR && now - m.deneme >= YENIDEN_SN
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    const N: &str = "4511643";

    fn tmp(ad: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("wfc-merkez-{}-{ad}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Sahte merkez: çağrıları kaydeder, sırayla verilen yanıtları döner.
    fn sahte(yanitlar: Vec<Result<(u16, &'static str), &'static str>>) -> (Box<Http>, Arc<Mutex<Vec<(String, serde_json::Value)>>>) {
        let cagrilar = Arc::new(Mutex::new(vec![]));
        let (c2, y) = (cagrilar.clone(), Arc::new(Mutex::new(yanitlar.into_iter())));
        let f = move |yol: &str, govde: &str| {
            c2.lock().unwrap().push((yol.to_string(), serde_json::from_str(govde).unwrap()));
            match y.lock().unwrap().next().expect("beklenmeyen çağrı") {
                Ok((d, g)) => Ok((d, g.to_string())),
                Err(e) => Err(e.to_string()),
            }
        };
        (Box::new(f), cagrilar)
    }

    fn ornek() -> Merkez {
        let tuz = "a1b2c3d4".to_string();
        Merkez { numara: N.into(), ozet: crate::hesap::digest("parola-12345", &tuz, 120_000), tuz, yineleme: 120_000,
                 cihaz_anahtari: "gizli-anahtar".into(), son_eslesme: 0.0, deneme: 0.0 }
    }

    const GIRIS_OK: &str = r#"{"tuz":"a1b2c3d4","ozet":"ee38d08b3d6573ecc281263ad6259d2e7ee39f37d6b8958b3180906580a44495","yineleme":120000,
        "cihaz_anahtari":"k-123","tunel_ip":"10.99.0.12","sunucu_pub":"S","uc_nokta":"vpn.wificorrect.com:51820","yedek_hedefi":"wfc-4511643@10.99.0.1:"}"#;

    #[test]
    fn numara_ozet_ve_dosya() {
        assert!(numara_gecerli(N) && !numara_gecerli("451164") && !numara_gecerli("0451164") && !numara_gecerli("45116434") && !numara_gecerli("45116٤3"));
        assert_eq!(crate::hesap::digest("parola-12345", "a1b2c3d4", 120_000), "ee38d08b3d6573ecc281263ad6259d2e7ee39f37d6b8958b3180906580a44495");
        let m = ornek();
        assert!(dogrula(&m, "parola-12345") && !dogrula(&m, "parola-12346"));
        let p = tmp("dosya").join("merkez.json");
        assert!(oku(&p).is_none());
        kaydet(&p, &m).unwrap();
        assert_eq!(oku(&p), Some(m));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        }
        std::fs::write(&p, "{bozuk").unwrap();
        assert!(oku(&p).is_none());
        sil(&p);
        assert!(!p.exists());
    }

    #[test]
    fn giris_basarili_ve_hatalar() {
        let (h, c) = sahte(vec![Ok((200, GIRIS_OK))]);
        let g = giris(N, "parola-12345", "WG=", "ssh-ed25519 AAAA x", "Bocafe", "Boca Ltd.", &*h, 1000.0).unwrap();
        assert_eq!((g.tunel_ip.as_str(), g.yedek_hedefi.as_str(), g.merkez.cihaz_anahtari.as_str()), ("10.99.0.12", "wfc-4511643@10.99.0.1:", "k-123"));
        assert!(dogrula(&g.merkez, "parola-12345") && g.merkez.son_eslesme == 1000.0);
        let (yol, govde) = c.lock().unwrap()[0].clone();
        assert_eq!(yol, "/api/giris");
        assert_eq!((govde["numara"].as_str(), govde["parola"].as_str(), govde["wg_pub"].as_str()), (Some(N), Some("parola-12345"), Some("WG=")));
        let (h, _) = sahte(vec![Ok((401, r#"{"hata":"Müşteri numarası veya parola hatalı.","kod":"hatali"}"#))]);
        assert!(matches!(giris(N, "x", "W", "S", "", "", &*h, 0.0), Err(GirisHata::Mesaj(m, true)) if m.contains("hatalı")));
        let (h, _) = sahte(vec![Ok((409, r#"{"hata":"Bu numara başka bir cihazda kullanılıyor.","kod":"baska_cihaz"}"#))]);
        assert!(matches!(giris(N, "x", "W", "S", "", "", &*h, 0.0), Err(GirisHata::Mesaj(m, false)) if m.contains("başka")));
        let (h, _) = sahte(vec![Err("bağlanılamadı")]);
        assert!(matches!(giris(N, "x", "W", "S", "", "", &*h, 0.0), Err(GirisHata::Baglanti)));
        let (h, _) = sahte(vec![Ok((502, "<html>"))]);
        assert!(matches!(giris(N, "x", "W", "S", "", "", &*h, 0.0), Err(GirisHata::Baglanti)));
        let (h, _) = sahte(vec![Ok((200, r#"{"tuz":"a"}"#))]); // eksik alan
        assert!(matches!(giris(N, "x", "W", "S", "", "", &*h, 0.0), Err(GirisHata::Baglanti)));
    }

    #[test]
    fn uygula_uzak_ve_yedek() {
        let (h, _) = sahte(vec![Ok((200, GIRIS_OK))]);
        let g = giris(N, "parola-12345", "W", "S", "", "", &*h, 0.0).unwrap();
        let mut cfg = Config::default();
        uygula(&mut cfg, &g);
        assert!(cfg.uzak.enabled && cfg.backup.enabled);
        assert_eq!((cfg.uzak.sunucu.as_str(), cfg.uzak.sunucu_anahtar.as_str(), cfg.uzak.adres.as_str()), ("vpn.wificorrect.com:51820", "S", "10.99.0.12"));
        assert_eq!(cfg.backup.target, "wfc-4511643@10.99.0.1:");
    }

    #[test]
    fn eslesme_bagli_serbest_taninmadi() {
        let mut m = ornek();
        let yeni = crate::hesap::digest("yeni-parola-1", "ff", 120_000);
        let govde: &'static str = Box::leak(format!(r#"{{"durum":"bagli","tuz":"ff","ozet":"{yeni}","yineleme":120000,"uyelik":"aktif"}}"#).into_boxed_str());
        let (h, c) = sahte(vec![Ok((200, govde)), Ok((200, r#"{"durum":"serbest"}"#)), Ok((401, r#"{"hata":"Cihaz tanınmadı.","kod":"taninmadi"}"#)), Err("yok")]);
        assert!(matches!(eslesme(&mut m, "Bocafe", "Ltd", false, &*h, 5000.0), Ok(Eslesme::Bagli { uyelik }) if uyelik == "aktif"));
        assert!(dogrula(&m, "yeni-parola-1") && m.son_eslesme == 5000.0);
        let g = c.lock().unwrap()[0].1.clone(); // kopya: kilit tutulursa sonraki sahte çağrı kilitlenir
        assert_eq!((g["cihaz_anahtari"].as_str(), g["isletme_adi"].as_str(), g.get("temizlendi")), (Some("gizli-anahtar"), Some("Bocafe"), None));
        assert!(matches!(eslesme(&mut m, "", "", true, &*h, 0.0), Ok(Eslesme::Serbest)));
        assert_eq!(c.lock().unwrap()[1].1["temizlendi"], serde_json::Value::Bool(true));
        assert!(matches!(eslesme(&mut m, "", "", false, &*h, 0.0), Ok(Eslesme::Taninmadi)));
        assert!(eslesme(&mut m, "", "", false, &*h, 0.0).is_err());
        assert!(dogrula(&m, "yeni-parola-1")); // hata yanıtları özeti değiştirmez
    }

    #[test]
    fn parola_degisimi() {
        let mut m = ornek();
        let yeni = crate::hesap::digest("yeni-parola-1", "ee", 120_000);
        let govde: &'static str = Box::leak(format!(r#"{{"tuz":"ee","ozet":"{yeni}","yineleme":120000}}"#).into_boxed_str());
        let (h, c) = sahte(vec![Ok((401, r#"{"hata":"Mevcut parola yanlış.","kod":"hatali"}"#)), Err("yok"), Ok((200, govde))]);
        assert_eq!(parola(&mut m, "x", "yeni-parola-1", &*h).unwrap_err(), "Mevcut parola yanlış.");
        assert_eq!(parola(&mut m, "parola-12345", "yeni-parola-1", &*h).unwrap_err(), "Parola değişimi için internet gerekli.");
        parola(&mut m, "parola-12345", "yeni-parola-1", &*h).unwrap();
        assert!(dogrula(&m, "yeni-parola-1") && !dogrula(&m, "parola-12345"));
        assert_eq!(c.lock().unwrap()[2].0, "/api/parola");
    }

    #[test]
    fn eslesme_zamani_0600_ve_30_dk() {
        // 2026-10-04 05:00 TR = 02:00 UTC
        let gun = 1_791_072_000.0; // 2026-10-04 00:00 UTC
        let tr = |saat: f64| gun + (saat - 3.0) * 3600.0;
        let mut m = ornek();
        m.son_eslesme = tr(5.0); // 05:00'te bağlandı
        assert!(!zamani_geldi(&m, tr(5.9)) && zamani_geldi(&m, tr(6.0)));
        m.son_eslesme = tr(6.5); // 06:30'da eşitlendi
        assert!(!zamani_geldi(&m, tr(23.0)) && !zamani_geldi(&m, tr(29.9)) && zamani_geldi(&m, tr(30.0))); // ertesi gün 06:00
        m.deneme = tr(30.0); // 06:00 denemesi başarısız
        assert!(!zamani_geldi(&m, tr(30.4)) && zamani_geldi(&m, tr(30.5)));
    }
}
