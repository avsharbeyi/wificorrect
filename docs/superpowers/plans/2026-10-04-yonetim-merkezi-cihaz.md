# Yönetim Merkezi — Cihaz (Rust) Tarafı Uygulama Planı

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cihazda kurulum ekranı kalksın; müşteri 7 haneli numarası ve parolasıyla giriş yapsın, ilk girişte cihaz merkeze
bağlanıp tüneli ve yedeği kendisi kursun; cihaz her gün 06:00'da merkezle eşitlensin, serbest bırakılınca kayıtlarını
teslim edip kendini temizlesin.

**Architecture:** Yeni `src/merkez.rs` merkezle konuşmanın tamamını taşır (bağ dosyası `merkez.json`, `curl` ile
`https://api.wificorrect.com/api/{giris,eslesme,parola}`, yerel parola doğrulama, 06:00 zamanlaması). Panel girişte
bunu kullanır; kaydedici zamanı gelince `ctl merkez-eslesme`'yi arka planda başlatır; fabrika dönüşü kayıtlar teslim
edildikten sonra merkeze `temizlendi` bildirir ve bağı siler. HTTP çağrısı enjekte edilebilir (`merkez::Http`), testler ağ kullanmaz.

**Tech Stack:** Rust (mevcut crate'ler: serde, serde_json, pbkdf2, sha2, tiny_http), `curl` (cihazda kurulu), systemd-run.

**Spec:** `docs/superpowers/specs/2026-10-04-yonetim-merkezi-design.md` §7, §9. Sunucu tarafı kurulu ve çalışıyor
(`docs/superpowers/plans/2026-10-04-yonetim-merkezi-sunucu.md`, PR avsharbeyi/rza#1).

## Global Constraints

- Müşteri numarası 7 hane, `^[1-9][0-9]{6}$` (yalnızca ASCII). `admin` kullanıcı adı ayrılmış, yerel doğrulanır.
- Parola özeti sunucuyla aynı: PBKDF2-HMAC-SHA256, tuz metin baytları, kayıttaki yineleme, hex (`hesap::digest`).
  Test vektörü: `digest("parola-12345", "a1b2c3d4", 120000) == "ee38d08b3d6573ecc281263ad6259d2e7ee39f37d6b8958b3180906580a44495"`.
- API: `https://api.wificorrect.com/api/...`, JSON; hata yanıtı `{"hata": "<Türkçe>", "kod": "<kod>"}`
  (kodlar: `gecersiz`, `kilit`, `hatali`, `uyelik`, `baska_cihaz`, `kayit`, `taninmadi`, `hiz`, `yok`).
  `giris` 200 → `tuz, ozet, yineleme, cihaz_anahtari, tunel_ip, sunucu_pub, uc_nokta, yedek_hedefi`;
  `eslesme` 200 → `durum: bagli` + `tuz, ozet, yineleme, uyelik` ya da `durum: serbest`; `parola` 200 → `tuz, ozet, yineleme`.
- `merkez.json` `/etc/wificorrect/merkez.json`, izin 600, atomik yazım. Cihaz anahtarı hiçbir yere loglanmaz.
- Parola `curl` komut satırına girmez: gövde standart girdiden (`--data-binary @-`).
- Eşitleme: her gün 06:00 (TR, UTC+3) sonrası ilk tur; başarılıysa ertesi 06:00'a kadar yok; başarısızsa 30 dk'da bir.
- Merkez "tanınmadı" derse cihaz kendini SİLMEZ (yalnızca denetime yazar) — merkezdeki bir hata bütün cihazları sıfırlamasın.
- Testler: `scripts/test.sh` (cihazda, bellek sınırlı); tek test: `scripts/test.sh wificorrect <süzgeç>`.
- Kullanıcıya görünen metin Türkçe; parolalar ve cihaz anahtarı repoya/sohbete girmez.

## Review Focus

1. İnternet yokken bağlı cihazda müşteri girişi yerelde çalışır; bağlı olmayan cihazda "Merkeze ulaşılamadı" der ve admin yine girer → Görev 2 testi.
2. Merkez eşitlemesi yeni parola özeti getirince eski özetle açılmış panel oturumları düşer → Görev 2 testi.
3. Serbest bırakılan cihazda kayıt teslimi başarısız olursa bağ (`merkez.json`) ve kayıtlar silinmez, sonraki denemede tekrar edilir → Görev 3 testi.
4. Merkez "tanınmadı" (401) derse cihaz silinmez → Görev 3 testi.
5. Bağlı cihaza admin dışında başka bir numara/kullanıcı adıyla girilemez ("başka müşteriye ait") → Görev 2 testi.

## Dosya Haritası

| Dosya | Değişiklik |
|---|---|
| `src/merkez.rs` | **yeni**: bağ dosyası, HTTP (curl), giriş/eşitleme/parola, zamanlama, ayarlara uygulama |
| `src/main.rs` | `mod merkez;` |
| `src/hesap.rs` | `digest` → `pub(crate)`; `setup`, `needs_setup`, `add`, `remove` kalkar; `Oturum.surum`, `Oturumlar::create(user, rol, now, surum)` |
| `src/panel.rs` | kurulum ekranı ve Hesaplar sayfası kalkar; giriş merkeze bağlı; Sahip oturumu `merkez.json` özetine bağlı; Sahip parola değişimi API'den; testler |
| `src/fabrika.rs` | kayıt tesliminden sonra merkeze `temizlendi` bildirimi ve `merkez.json` silme |
| `src/ctl.rs` | `merkez-eslesme`; `fabrika` bildirimi bağlar |
| `src/kaydedici.rs` | `check_merkez` (zamanı gelince `ctl merkez-eslesme`) |
| `docs/KURULUM_GUNLUGU.md`, `docs/RUST_YENIDEN_YAZIM.md` | geçiş ve rol değişikliği |

---

### Task 1: `src/merkez.rs` — bağ dosyası, HTTP, giriş/eşitleme/parola, zamanlama

**Files:**
- Create: `src/merkez.rs`
- Modify: `src/main.rs` (`mod merkez;`), `src/hesap.rs:44` (`fn digest` → `pub(crate) fn digest`)

**Interfaces:**
- Produces:
  - `pub const PATH: &str = "/etc/wificorrect/merkez.json"`, `pub const API: &str = "https://api.wificorrect.com"`, `pub const YEDEK_SUNUCU_SSH`
  - `pub struct Merkez { numara, tuz, ozet: String, yineleme: u32, cihaz_anahtari: String, son_eslesme: f64, deneme: f64 }` (Serialize/Deserialize/Clone/Debug/PartialEq)
  - `pub type Http = dyn Fn(&str, &str) -> Result<(u16, String), String> + Send + Sync;` (yol, json gövde) → (HTTP durumu, gövde); Err = merkeze ulaşılamadı
  - `pub fn curl(yol: &str, govde: &str) -> Result<(u16, String), String>`
  - `pub fn oku(path: &Path) -> Option<Merkez>`, `pub fn kaydet(path: &Path, m: &Merkez) -> Result<(), String>`, `pub fn sil(path: &Path)`
  - `pub fn numara_gecerli(s: &str) -> bool`, `pub fn dogrula(m: &Merkez, pw: &str) -> bool`
  - `pub enum GirisHata { Mesaj(String, bool) /* (metin, kilit sayacına yazılsın mı) */, Baglanti }`
  - `pub struct Giris { pub merkez: Merkez, pub tunel_ip, pub sunucu_pub, pub uc_nokta, pub yedek_hedefi: String }`
  - `pub fn giris(numara, pw, wg_pub, ssh_pub, isletme_adi, unvan: &str, http: &Http, now: f64) -> Result<Giris, GirisHata>`
  - `pub fn uygula(cfg: &mut Config, g: &Giris)` — uzak erişim + yedek ayarları
  - `pub enum Eslesme { Bagli { uyelik: String }, Serbest, Taninmadi }`
  - `pub fn eslesme(m: &mut Merkez, isletme_adi, unvan: &str, temizlendi: bool, http: &Http, now: f64) -> Result<Eslesme, String>` (Bagli'de `m`'nin özet alanları ve `son_eslesme` güncellenir)
  - `pub fn parola(m: &mut Merkez, eski, yeni: &str, http: &Http) -> Result<(), String>` (Err metni kullanıcıya gösterilir)
  - `pub fn zamani_geldi(m: &Merkez, now: f64) -> bool`

- [ ] **Step 1: Failing tests** — `src/merkez.rs` içine (dosya henüz yoksa yalnızca test modülüyle başlayarak derleme hatası görülür):

```rust
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
        let g = &c.lock().unwrap()[0].1;
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
```

- [ ] **Step 2: Run** `scripts/test.sh wificorrect merkez` → derleme hatası (modül/işlevler yok).

- [ ] **Step 3: Implement** `src/merkez.rs` (testlerin üstüne):

```rust
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
```

`src/main.rs`'e `mod merkez;` ekle (diğer `mod` satırlarıyla alfabetik). `src/hesap.rs:44` `fn digest` → `pub(crate) fn digest`.
(`numara_gecerli` testindeki Arapça-Hint rakamı `٤` bilerek: `is_ascii_digit` onu reddeder.)

- [ ] **Step 4: Run** `scripts/test.sh wificorrect merkez` → 6 test geçer; `scripts/test.sh` → bütün testler geçer (uyarı yok;
`Eslesme`/`Giris` henüz kullanılmadığı için çıkan `dead_code` uyarısı Görev 2–3'te kaybolur — bu görevde `#[allow(dead_code)]`
EKLEME, uyarıyı not et).
- [ ] **Step 5: Commit** `git add src/merkez.rs src/main.rs src/hesap.rs && git commit -m "Cihaz: yönetim merkezi bağı (merkez.rs)"`

---

### Task 2: Panel — kurulum ekranı kalkar, giriş merkeze bağlı, Hesaplar sayfası kalkar

**Files:**
- Modify: `src/panel.rs` (yapı alanları, `handle`, `page`, `kurulum*`, `giris_post`, `sifre_post`, `hesaplar*`, `may_manage`, menü, testler), `src/hesap.rs`

**Interfaces:**
- Consumes: `merkez::{oku, kaydet, dogrula, numara_gecerli, giris, uygula, parola, GirisHata, Http, curl, PATH}`
- Produces: `Panel.merkez_path: PathBuf`, `Panel.http: Box<merkez::Http>` (testte değiştirilir);
  `hesap::Oturumlar::create(&self, user: &str, rol: Rol, now: f64, surum: &str) -> String`, `Oturum.surum: String`;
  `Hesaplar::{setup, needs_setup, add, remove}` kaldırılır.

- [ ] **Step 1: Test yardımcısını yeni modele çevir** — `src/panel.rs` testlerinde:

```rust
    const MUSTERI: &str = "4511643";

    /// Sahte merkez: yalnızca bağ testlerinde çağrılır; varsayılan "ulaşılamadı".
    fn merkez_yok() -> Box<crate::merkez::Http> {
        Box::new(|_: &str, _: &str| Err("ağ yok".into()))
    }

    /// Cihaz müşteriye bağlı (merkez.json) ve admin parolası kurulu; `user` "mudur" ise müşteri numarasıyla girer.
    fn setup_and_login(e: &Env, user: &str, pw: &str) -> (String, String) {
        if crate::merkez::oku(&e.p.merkez_path).is_none() {
            e.p.hesaplar.set_admin("hizmet-parola-1").unwrap();
            let tuz = "0011aabb".to_string();
            let m = crate::merkez::Merkez { numara: MUSTERI.into(), ozet: crate::hesap::digest("sahip-parola-12", &tuz, 120_000), tuz,
                                            yineleme: 120_000, cihaz_anahtari: "k".into(), son_eslesme: 1_790_705_134.0, deneme: 0.0 };
            crate::merkez::kaydet(&e.p.merkez_path, &m).unwrap();
            let mut c = Config::load(&e.p.cfg_path).unwrap();
            (c.main.site_name, c.main.unvan) = ("Bocafe Göztepe".into(), "Boca Gıda Tic. Ltd. Şti.".into());
            c.save(&e.p.cfg_path).unwrap();
        }
        let user = if user == "mudur" { MUSTERI } else { user };
        let r = e.p.handle(&req("POST", "/giris", &[("kullanici", user), ("parola", pw)], None));
        let cookie = r.headers.iter().find(|(k, _)| k == "Set-Cookie").map(|(_, v)| v.clone()).expect("çerez");
        let token = cookie.trim_start_matches("wfc=").split(';').next().unwrap().to_string();
        let page = e.p.handle(&req("GET", "/sifre", &[], Some(&token))).body;
        let csrf = page.split("name=\"csrf\" value=\"").nth(1).unwrap().split('"').next().unwrap().to_string();
        (token, csrf)
    }
```
`env()` içinde `p.merkez_path = root.join("merkez.json"); p.http = merkez_yok();` ekle. `setup_and_login_admin_only` silinir.

- [ ] **Step 2: Yeni/değişen testler**

`setup_then_login_lock_and_customer_network_denied` yerine:

```rust
    #[test]
    fn giris_ekrani_ilk_giris_merkezden() {
        let mut e = env();
        e.p.hesaplar.set_admin("hizmet-parola-1").unwrap();
        assert_eq!(loc(&e.p.handle(&req("GET", "/", &[], None))), "/giris"); // kurulum ekranı yok
        assert_eq!(e.p.handle(&req("GET", "/kurulum", &[], None)).status, 404);
        let page = e.p.handle(&req("GET", "/giris", &[], None)).body;
        assert!(page.contains("WifiCorrect") && !page.contains("müşteriye bağlanmadı"));
        // merkeze ulaşılamıyor: müşteri giremez, admin girer
        let r = e.p.handle(&req("POST", "/giris", &[("kullanici", MUSTERI), ("parola", "sahip-parola-12")], None));
        assert!(r.body.contains("Merkeze ulaşılamadı"));
        assert_eq!(e.p.handle(&req("POST", "/giris", &[("kullanici", "admin"), ("parola", "hizmet-parola-1")], None)).status, 303);
        // numara biçimsizse merkeze sorulmaz
        assert!(e.p.handle(&req("POST", "/giris", &[("kullanici", "mudur"), ("parola", "x")], None)).body.contains("hatalı"));
        // merkez: önce hatalı parola, sonra başarı
        let cagri = Arc::new(Mutex::new(0));
        let c2 = cagri.clone();
        e.p.http = Box::new(move |yol: &str, _: &str| {
            assert_eq!(yol, "/api/giris");
            *c2.lock().unwrap() += 1;
            Ok(if *c2.lock().unwrap() == 1 {
                (401, r#"{"hata":"Müşteri numarası veya parola hatalı.","kod":"hatali"}"#.to_string())
            } else {
                (200, format!(r#"{{"tuz":"0011aabb","ozet":"{}","yineleme":120000,"cihaz_anahtari":"k-1","tunel_ip":"10.99.0.12","sunucu_pub":"S","uc_nokta":"vpn.wificorrect.com:51820","yedek_hedefi":"wfc-{MUSTERI}@10.99.0.1:"}}"#,
                              crate::hesap::digest("sahip-parola-12", "0011aabb", 120_000)))
            })
        });
        assert!(e.p.handle(&req("POST", "/giris", &[("kullanici", MUSTERI), ("parola", "yanlis-parola")], None)).body.contains("hatalı"));
        let r = e.p.handle(&req("POST", "/giris", &[("kullanici", MUSTERI), ("parola", "sahip-parola-12")], None));
        assert_eq!(r.status, 303);
        let m = crate::merkez::oku(&e.p.merkez_path).unwrap();
        assert_eq!((m.numara.as_str(), m.cihaz_anahtari.as_str()), (MUSTERI, "k-1"));
        let c = Config::load(&e.p.cfg_path).unwrap();
        assert!(c.uzak.enabled && c.uzak.adres == "10.99.0.12" && c.backup.target == format!("wfc-{MUSTERI}@10.99.0.1:"));
        assert!(e.calls.lock().unwrap().iter().any(|c| c.join(" ").contains("ctl uzak-uygula")));
        // bağlandıktan sonra: merkez çağrılmaz, yerel doğrulama; başka numara reddedilir
        e.p.http = merkez_yok();
        assert_eq!(e.p.handle(&req("POST", "/giris", &[("kullanici", MUSTERI), ("parola", "sahip-parola-12")], None)).status, 303);
        assert!(e.p.handle(&req("POST", "/giris", &[("kullanici", "7654321"), ("parola", "sahip-parola-12")], None)).body.contains("başka bir müşteriye ait"));
        // kilit ve müşteri ağı
        for _ in 0..5 {
            e.p.handle(&req("POST", "/giris", &[("kullanici", MUSTERI), ("parola", "yanlis-parola")], None));
        }
        assert!(e.p.handle(&req("POST", "/giris", &[("kullanici", MUSTERI), ("parola", "sahip-parola-12")], None)).body.contains("Çok fazla"));
        let mut r = req("GET", "/giris", &[], None);
        r.ip = "10.50.0.23".into();
        assert_eq!(e.p.handle(&r).status, 403);
        let audit = std::fs::read_to_string(e.root.join("5651/gunluk/2026-09-29/denetim.csv")).unwrap();
        assert!(audit.contains("PANEL_MERKEZ_BAGLANDI") && audit.contains("PANEL_GIRIS_HATA") && audit.contains("PANEL_GIRIS_KILIT"));
        assert!(!audit.contains("k-1")); // cihaz anahtarı denetime yazılmaz
    }

    #[test]
    fn merkezden_yeni_ozet_gelince_eski_oturum_duser() {
        let e = env();
        let (tok, _) = setup_and_login(&e, "mudur", "sahip-parola-12");
        assert_eq!(e.p.handle(&req("GET", "/", &[], Some(&tok))).status, 200);
        let mut m = crate::merkez::oku(&e.p.merkez_path).unwrap();
        m.ozet = crate::hesap::digest("baska-parola-1", &m.tuz, m.yineleme); // 06:00 eşitlemesi sıfırlanmış parolayı getirdi
        crate::merkez::kaydet(&e.p.merkez_path, &m).unwrap();
        assert_eq!(loc(&e.p.handle(&req("GET", "/", &[], Some(&tok)))), "/giris");
        crate::merkez::sil(&e.p.merkez_path); // fabrika: bağ silindi
        let (tok2, _) = setup_and_login(&e, "mudur", "sahip-parola-12");
        crate::merkez::sil(&e.p.merkez_path);
        assert_eq!(loc(&e.p.handle(&req("GET", "/", &[], Some(&tok2)))), "/giris");
    }
```

`csrf_required_and_roles`: `/hesaplar` satırlarını (`page.contains("href=\"/hesaplar\"")` dahil 6 satır: 1381'deki `&& page.contains("href=\"/hesaplar\"")`
parçası ve 1382–1387) şununla değiştir:

```rust
        assert!(!page.contains("href=\"/admin-ayarlari\"") && !page.contains("href=\"/hesaplar\""));
        assert_eq!(e.p.handle(&req("GET", "/hesaplar", &[], Some(&tok))).status, 404); // yerel ek hesap yok
```
`provider_sets_secrets_write_only_and_validates`: `/hesaplar` sayfası ve `/hesaplar/sil` satırlarını (1405–1407) sil.

`password_change_logs_out_other_sessions` yerine:

```rust
    #[test]
    fn password_change_via_center_logs_out_other_sessions() {
        let mut e = env();
        let (t1, csrf) = setup_and_login(&e, "mudur", "sahip-parola-12");
        let (t2, _) = setup_and_login(&e, "mudur", "sahip-parola-12");
        let f = |eski: &str| [("csrf", csrf.clone()), ("eski", eski.to_string()), ("yeni", "yepyeni-parola".into()), ("yeni2", "yepyeni-parola".into())];
        let post = |e: &Env, eski: &str| {
            let a = f(eski);
            let v: Vec<(&str, &str)> = a.iter().map(|(k, v)| (*k, v.as_str())).collect();
            e.p.handle(&req("POST", "/sifre", &v, Some(&t1)))
        };
        assert!(post(&e, "yanlis-parola").body.contains("hatalı")); // yerel kontrol, merkeze gitmez
        assert!(post(&e, "sahip-parola-12").body.contains("internet gerekli")); // merkez yok
        let yeni = crate::hesap::digest("yepyeni-parola", "cc", 120_000);
        e.p.http = Box::new(move |yol: &str, govde: &str| {
            assert!(yol == "/api/parola" && govde.contains("\"eski\":\"sahip-parola-12\""));
            Ok((200, format!(r#"{{"tuz":"cc","ozet":"{yeni}","yineleme":120000}}"#)))
        });
        let r = post(&e, "sahip-parola-12");
        assert_eq!(r.status, 303);
        assert!(crate::merkez::dogrula(&crate::merkez::oku(&e.p.merkez_path).unwrap(), "yepyeni-parola"));
        assert_eq!(loc(&e.p.handle(&req("GET", "/", &[], Some(&t2)))), "/giris"); // diğer oturum düştü
        let cookie = r.headers.iter().find(|(k, _)| k == "Set-Cookie").map(|(_, v)| v.clone()).expect("yeni çerez");
        let t3 = cookie.trim_start_matches("wfc=").split(';').next().unwrap().to_string();
        assert_eq!(e.p.handle(&req("GET", "/", &[], Some(&t3))).status, 200); // değiştiren yeni oturumla devam eder
        // admin parolası yerel değişir
        let (a1, acsrf) = setup_and_login(&e, "admin", "hizmet-parola-1");
        e.p.http = merkez_yok();
        let r = e.p.handle(&req("POST", "/sifre", &[("csrf", &acsrf), ("eski", "hizmet-parola-1"), ("yeni", "yeni-admin-12"), ("yeni2", "yeni-admin-12")], Some(&a1)));
        assert_eq!(r.status, 303);
        assert!(e.p.hesaplar.verify("admin", "yeni-admin-12").is_some());
    }
```

`owner_activity_is_logged_and_only_admin_sees_it`: `("kim", "mudur")` → `("kim", MUSTERI)`; `audit.contains("kullanici=mudur rol=sahip ip=")`
→ `audit.contains(&format!("kullanici={MUSTERI} rol=sahip ip="))`; yorumdaki "mudur" → "müşteri".
`admin_settings_page_and_factory_reset` ve `admin_remote_access_settings` içinde `needs_setup` / `hesaplar` kullanan satır varsa
`crate::merkez::oku(&e.p.merkez_path)` ile aynı anlamda değiştir (fabrika panelde yalnızca `ctl fabrika`'yı başlatır; bağın
silinmesi Görev 3'te `fabrika.rs` testinde).

`src/hesap.rs` testlerinde `setup`/`add`/`remove`/`needs_setup` kullanan test(ler)i `set_admin` + `verify` + `keep_only_admin`
+ `set_password` ile yeniden yaz; `Oturumlar::create` çağrılarına 4. argüman `""`.

- [ ] **Step 3: Run** `scripts/test.sh wificorrect panel` → derleme hatası / başarısız (alanlar ve davranış yok).

- [ ] **Step 4: Implement**

`src/hesap.rs`:
- `Oturum`'a `pub surum: String` (doc: "Sahip oturumunda girişteki merkez parola özeti; değişince oturum düşer").
- `pub fn create(&self, user: &str, rol: Rol, now: f64, surum: &str) -> String` (`surum: surum.into()`).
- `setup`, `needs_setup`, `add`, `remove` ve yalnızca onların kullandığı `username_problem` çağrıları silinir
  (`username_problem` başka yerde kullanılmıyorsa o da silinir). Modül başı yorumu: "İşletme sahibi hesabı merkezdedir
  (merkez.rs); bu dosyada yalnızca `admin`."

`src/panel.rs`:
- `Panel` yapısına `pub merkez_path: std::path::PathBuf` ve `pub http: Box<crate::merkez::Http>`; `new` içinde
  `merkez_path: crate::merkez::PATH.into()`, `http: Box::new(crate::merkez::curl)`.
- `page`: `v.insert("site", h(if crate::merkez::oku(&self.merkez_path).is_none() { "WifiCorrect" } else { &cfg.main.site_name }));`
- `handle`: `setup` değişkeni ve `/kurulum` dalları silinir (`/kurulum` artık 404). Oturum yoksa `redirect("/giris", None)`.
  Oturum alındıktan hemen sonra:

```rust
        if o.rol == Rol::Sahip && !self.sahip_oturumu_gecerli(&o) {
            if let Some(t) = &req.token {
                self.oturumlar.remove(t);
            }
            return redirect("/giris", None);
        }
```

```rust
    /// Sahip oturumu: cihaz hâlâ bu numaraya bağlı ve parola özeti girişteki gibi (merkezden sıfırlanmadı).
    fn sahip_oturumu_gecerli(&self, o: &Oturum) -> bool {
        crate::merkez::oku(&self.merkez_path).is_some_and(|m| m.numara == o.user && hesap::ct_eq(&m.ozet, &o.surum))
    }
```
- `kurulum`, `kurulum_post` silinir (ve yalnızca onların kullandığı `unvan_ok`/yardımcılar başka yerde kullanılmıyorsa).
- `giris` formu: etiket "Kullanıcı adı / müşteri numarası".
- `giris_post` gövdesi:

```rust
    fn giris_post(&self, cfg: &Config, req: &Req, now: f64) -> Resp {
        let user = req.form.get("kullanici").map_or("", String::as_str).trim().to_lowercase();
        let pw = req.form.get("parola").map_or("", String::as_str);
        if !self.guard.allowed(&req.ip, &user, now) {
            self.audit(cfg, req, None, "PANEL_GIRIS_KILIT", &format!("kullanici_adi={user}"));
            return self.giris(cfg, req, "Çok fazla hatalı deneme. 15 dakika sonra tekrar deneyin.");
        }
        let hatali = |e: &str| {
            self.guard.failed(&req.ip, &user, now);
            self.audit(cfg, req, None, "PANEL_GIRIS_HATA", &format!("kullanici_adi={user}"));
            self.giris(cfg, req, e)
        };
        let (rol, surum) = if user == hesap::ADMIN {
            match self.hesaplar.verify(&user, pw) {
                Some(r) => (r, String::new()),
                None => return hatali("Kullanıcı adı veya parola hatalı."),
            }
        } else if let Some(m) = crate::merkez::oku(&self.merkez_path) {
            if user != m.numara {
                return hatali(if crate::merkez::numara_gecerli(&user) { "Bu cihaz başka bir müşteriye ait." } else { "Kullanıcı adı veya parola hatalı." });
            }
            if !crate::merkez::dogrula(&m, pw) {
                return hatali("Kullanıcı adı veya parola hatalı.");
            }
            (Rol::Sahip, m.ozet)
        } else if !crate::merkez::numara_gecerli(&user) {
            return hatali("Kullanıcı adı veya parola hatalı.");
        } else {
            match self.merkeze_baglan(cfg, req, &user, pw, now) {
                Ok(ozet) => (Rol::Sahip, ozet),
                Err((mesaj, kilit)) => {
                    return if kilit { hatali(&mesaj) } else { self.giris(cfg, req, &mesaj) };
                }
            }
        };
        self.guard.succeeded(&req.ip, &user);
        let token = self.oturumlar.create(&user, rol, now, &surum);
        let o = self.oturumlar.get(&token, now);
        self.audit(cfg, req, o.as_ref(), "PANEL_GIRIS", "");
        let mut r = redirect("/", None);
        r.headers.push(("Set-Cookie".into(), format!("wfc={token}; Path=/; Secure; HttpOnly; SameSite=Strict")));
        r
    }

    /// Bağlı olmayan cihazda ilk müşteri girişi: merkez parolayı doğrular, cihazı bağlar; tünel ve yedek ayarlanır.
    /// Dönen: parola özeti (oturum sürümü) ya da (mesaj, kilit sayacına yazılsın mı).
    fn merkeze_baglan(&self, cfg: &Config, req: &Req, numara: &str, pw: &str, now: f64) -> Result<String, (String, bool)> {
        let wg = crate::uzak::ensure_key(&self.uzak_key).and_then(|k| crate::uzak::public_key(&k));
        let ssh = crate::uzak::yedek_anahtari(&self.yedek_key);
        let (wg, ssh) = match (wg, ssh) {
            (Ok(w), Ok(s)) => (w, s),
            (Err(e), _) | (_, Err(e)) => return Err((format!("Cihaz anahtarı üretilemedi: {e}"), false)),
        };
        let g = match crate::merkez::giris(numara, pw, &wg, &ssh, &cfg.main.site_name, &cfg.main.unvan, &*self.http, now) {
            Ok(g) => g,
            Err(crate::merkez::GirisHata::Baglanti) => return Err(("Merkeze ulaşılamadı, internet bağlantısını kontrol edin.".into(), false)),
            Err(crate::merkez::GirisHata::Mesaj(m, kilit)) => return Err((m, kilit)),
        };
        crate::merkez::kaydet(&self.merkez_path, &g.merkez).map_err(|e| (e, false))?;
        let mut yeni = cfg.clone();
        crate::merkez::uygula(&mut yeni, &g);
        yeni.save(&self.cfg_path).map_err(|e| (e, false))?;
        let unit = format!("wfc-uzak-{}", now as u64);
        (self.runner)(&cmd(&["systemd-run", "--unit", &unit, "--on-active", "1s", "/usr/local/bin/wificorrect", "ctl", "uzak-uygula"]));
        self.audit(&yeni, req, None, "PANEL_MERKEZ_BAGLANDI", &format!("numara={numara} tunel={}", g.tunel_ip));
        Ok(g.merkez.ozet)
    }
```
(`Config` `Clone` değilse `Config::load(&self.cfg_path)` ile yeniden oku ve onu değiştir.)
- `sifre_post` gövdesinin başı:

```rust
        if o.rol == Rol::Sahip {
            let Some(mut m) = crate::merkez::oku(&self.merkez_path) else { return redirect("/giris", None) };
            if !crate::merkez::dogrula(&m, &g("eski")) {
                return self.sifre(cfg, req, o, "Şu anki parola hatalı.");
            }
            if let Some(e) = hesap::password_problem(&g("yeni")) {
                return self.sifre(cfg, req, o, e);
            }
            if g("yeni") != g("yeni2") {
                return self.sifre(cfg, req, o, "Yeni parolalar aynı değil.");
            }
            if let Err(e) = crate::merkez::parola(&mut m, &g("eski"), &g("yeni"), &*self.http) {
                return self.sifre(cfg, req, o, &e);
            }
            if let Err(e) = crate::merkez::kaydet(&self.merkez_path, &m) {
                return self.sifre(cfg, req, o, &e);
            }
            self.oturumlar.remove_user(&o.user, None);
            let now = (self.clock)();
            let token = self.oturumlar.create(&o.user, Rol::Sahip, now, &m.ozet);
            self.audit(cfg, req, Some(o), "PANEL_SIFRE", "merkez");
            let mut r = redirect("/sifre", Some(("Parola değiştirildi.", false)));
            r.headers.push(("Set-Cookie".into(), format!("wfc={token}; Path=/; Secure; HttpOnly; SameSite=Strict")));
            return r;
        }
```
(geri kalanı — admin için yerel değişim — aynen kalır.)
- `hesaplar_sayfa`, `may_manage`, `hesap_ekle`, `hesap_sil`, `hesap_sifirla`, `/hesaplar*` dalları ve menüdeki `("/hesaplar", "Hesaplar", false)`
  satırı silinir.
- `run()` içindeki başlangıç mesajı: `panel.hesaplar.needs_setup()` yerine `crate::merkez::oku(Path::new(crate::merkez::PATH)).is_none()` → " (müşteri girişi bekleniyor)".

- [ ] **Step 5: Run** `scripts/test.sh` → bütün testler geçer, uyarı yok (`Eslesme` dead_code uyarısı Görev 3'e kadar kalabilir).
- [ ] **Step 6: Commit** `git commit -am "Panel: kurulum ekranı kalktı; müşteri girişi yönetim merkezine bağlı; yerel ek hesap yok"`

---

### Task 3: Günlük eşitleme, serbest bırakma ve fabrika bildirimi

**Files:**
- Modify: `src/fabrika.rs` (imza + bildirim + test), `src/ctl.rs` (`merkez-eslesme`, `fabrika`), `src/kaydedici.rs` (`check_merkez` + test)

**Interfaces:**
- Consumes: `merkez::{oku, kaydet, sil, eslesme, zamani_geldi, Eslesme, curl, PATH}`
- Produces: `fabrika::fabrika(cfg, cfg_path, hesap_path, merkez_path: &Path, bildir: &dyn Fn(&merkez::Merkez) -> Result<(), String>, filtre_conf, y, now, runner)`;
  `ctl merkez-eslesme`; `Kaydedici.merkez_path: PathBuf`

- [ ] **Step 1: Failing tests**

`src/fabrika.rs` — mevcut `factory_reset_returns_to_fresh_install` testini genişlet (aynı kurulum: gün kaydı, `Yollar`, runner):
- `h.setup("mudur", "sahip-parola-12").unwrap();` satırını sil (yerel sahip hesabı yok); yerine müşteri bağı:

```rust
        let mp = root.join("merkez.json");
        crate::merkez::kaydet(&mp, &crate::merkez::Merkez { numara: "4511643".into(), tuz: "t".into(), ozet: "o".into(), yineleme: 120_000,
                                                          cihaz_anahtari: "k".into(), son_eslesme: 0.0, deneme: 0.0 }).unwrap();
        let bildirimler: Arc<Mutex<Vec<String>>> = Arc::default();
        let b2 = bildirimler.clone();
        let bildir = move |m: &crate::merkez::Merkez| {
            b2.lock().unwrap().push(m.numara.clone());
            Ok(())
        };
```
- iki `fabrika(...)` çağrısına `hesaplar.json` argümanından sonra `&mp, &bildir,` ekle.
- iptal sonrası kontrol: `h.load().unwrap().len() == 2` → `h.load().unwrap().len() == 1 && mp.exists() && bildirimler.lock().unwrap().is_empty()`.
- başarı sonrası: `assert!(h.needs_setup() && …)` → `assert!(h.verify(ADMIN, "admin-parola-123").is_some());` ve ek olarak
  `assert!(!mp.exists() && *bildirimler.lock().unwrap() == vec!["4511643".to_string()]); // teslimden sonra merkeze bildirildi, bağ silindi`.

`src/kaydedici.rs` testine:

```rust
    #[test]
    fn merkez_eslesmesi_zamani_gelince_arka_planda() {
        let (mut k, root, _) = setup();
        let calls = Arc::new(Mutex::new(Vec::<String>::new()));
        let c2 = calls.clone();
        k.runner = Box::new(move |c: &[String]| {
            c2.lock().unwrap().push(c.join(" "));
            true
        });
        k.merkez_path = root.join("merkez.json");
        let n = |calls: &Arc<Mutex<Vec<String>>>| calls.lock().unwrap().iter().filter(|x| x.contains("ctl merkez-eslesme")).count();
        let gun = 1_791_072_000.0; // 2026-10-04 00:00 UTC
        let tr = |saat: f64| gun + (saat - 3.0) * 3600.0;
        k.check_merkez(tr(7.0));
        assert_eq!(n(&calls), 0); // bağ yok
        let m = crate::merkez::Merkez { numara: "4511643".into(), tuz: "t".into(), ozet: "o".into(), yineleme: 120_000, cihaz_anahtari: "k".into(), son_eslesme: tr(5.0), deneme: 0.0 };
        std::fs::create_dir_all(&root).unwrap();
        crate::merkez::kaydet(&k.merkez_path, &m).unwrap();
        k.check_merkez(tr(5.5));
        assert_eq!(n(&calls), 0); // 06:00 olmadı
        k.check_merkez(tr(6.1));
        assert_eq!(n(&calls), 1);
        k.check_merkez(tr(6.11));
        assert_eq!(n(&calls), 1); // aynı dakikada ikinci kez başlatılmaz
    }
```

- [ ] **Step 2: Run** `scripts/test.sh wificorrect fabrika` ve `scripts/test.sh wificorrect merkez_eslesmesi` → derleme hatası.

- [ ] **Step 3: Implement**

`src/fabrika.rs`: imza `pub fn fabrika(cfg: &Config, cfg_path: &str, hesap_path: &str, merkez_path: &Path, bildir: &dyn Fn(&crate::merkez::Merkez) -> Result<(), String>, filtre_conf: &Path, y: &Yollar, now: f64, runner: &Runner)`.
`kayitlari_teslim_et` başarılı olduktan hemen sonra (adım 3'ten önce):

```rust
    // 2b) yönetim merkezi: kayıtlar sunucuda → cihazın temizlendiği bildirilir, bağ silinir (yeni müşteri girebilir).
    // Bildirim gitmezse fabrika sürer (kayıtlar teslim edildi); admin yönetim merkezinden "zorla ayır" ile kapatır.
    if let Some(mz) = crate::merkez::oku(merkez_path) {
        if let Err(e) = bildir(&mz) {
            errs.push(format!("merkeze bildirilemedi: {e}"));
            audit(&m.log_root, "MERKEZ_BILDIRIM_HATA", now, &format!("hata={e}"));
        }
        crate::merkez::sil(merkez_path);
    }
```
(`let mut errs = vec![];` satırını bu bloğun üstüne taşı.) Modül başı yorumuna: "Müşteri bağı (merkez.json) da silinir; merkeze
`temizlendi` bildirilir."

`src/ctl.rs`: `fabrika` dalını yardımcıya çıkar ve `merkez-eslesme` ekle:

```rust
/// Fabrika dönüşü (panel ya da merkez serbest bırakması): kayıtlar teslim edilince merkeze `temizlendi` bildirilir.
fn fabrika_calistir(cfg: &Config, now: f64, runner: &Runner) -> ExitCode {
    let path = std::env::var("WFC_AYAR").unwrap_or_else(|_| crate::ayar::PATH.to_string());
    let bildir = |m: &crate::merkez::Merkez| {
        let mut m = m.clone();
        crate::merkez::eslesme(&mut m, "", "", true, &crate::merkez::curl, now).map(|_| ())
    };
    match crate::fabrika::fabrika(cfg, &path, crate::hesap::PATH, std::path::Path::new(crate::merkez::PATH), &bildir,
                                  std::path::Path::new(crate::filtre::DNSMASQ_CONF), &crate::ag::Yollar::sistem(), now, runner) {
        Ok(errs) if errs.is_empty() => {
            println!("Fabrika ayarlarına dönüldü.");
            ExitCode::SUCCESS
        }
        Ok(errs) => {
            eprintln!("Fabrika ayarlarına dönüldü, uyarılar: {}", errs.join("; "));
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}
```
`Some("fabrika") => fabrika_calistir(&cfg, now, &runner),` ve:

```rust
        // Kaydedici zamanı gelince başlatır (her gün 06:00 sonrası; hata olursa 30 dk sonra yeniden)
        Some("merkez-eslesme") => {
            let p = std::path::Path::new(crate::merkez::PATH);
            let Some(mut m) = crate::merkez::oku(p) else {
                println!("cihaz bir müşteriye bağlı değil");
                return ExitCode::SUCCESS;
            };
            m.deneme = now;
            let _ = crate::merkez::kaydet(p, &m);
            let denetim = |olay: &str, ek: String| ortak::audit(&cfg.main.log_root, Row::new(olay, &ortak::now_iso(now)).set("ek", ek));
            match crate::merkez::eslesme(&mut m, &cfg.main.site_name, &cfg.main.unvan, false, &crate::merkez::curl, now) {
                Ok(crate::merkez::Eslesme::Bagli { uyelik }) => {
                    if let Err(e) = crate::merkez::kaydet(p, &m) {
                        eprintln!("{e}");
                        return ExitCode::from(1);
                    }
                    denetim("MERKEZ_ESLESME", format!("uyelik={uyelik}"));
                    ExitCode::SUCCESS
                }
                Ok(crate::merkez::Eslesme::Serbest) => {
                    denetim("MERKEZ_SERBEST", "fabrika=basladi".into());
                    fabrika_calistir(&cfg, now, &runner) // teslim olmazsa bağ kalır, 30 dk sonra yeniden denenir
                }
                Ok(crate::merkez::Eslesme::Taninmadi) => {
                    // cihaz silinmez: merkezdeki bir hata bütün cihazları sıfırlamasın; admin yönetim merkezinden bakar
                    denetim("MERKEZ_TANINMADI", String::new());
                    ExitCode::from(1)
                }
                Err(e) => {
                    denetim("MERKEZ_ESLESME_HATA", format!("hata={e}"));
                    ExitCode::from(1)
                }
            }
        }
```
Kullanım metnine `merkez-eslesme` satırını ekle (dosyadaki biçimde).

`src/kaydedici.rs`: struct'a `pub merkez_path: std::path::PathBuf, merkez_son: f64`; `new`'de
`merkez_path: crate::merkez::PATH.into(), merkez_son: f64::NEG_INFINITY`; `check_uzak(now)` çağrısından sonra `self.check_merkez(now);`:

```rust
    /// Yönetim merkezi eşitlemesi: zamanı geldiyse (her gün 06:00 sonrası, hatada 30 dk sonra) arka planda
    /// `ctl merkez-eslesme` başlatılır (ağ çağrısı kaydediciyi bekletmesin). En çok dakikada bir.
    fn check_merkez(&mut self, now: f64) {
        if now - self.merkez_son < 60.0 {
            return;
        }
        let Some(m) = crate::merkez::oku(&self.merkez_path) else { return };
        if !crate::merkez::zamani_geldi(&m, now) {
            return;
        }
        self.merkez_son = now;
        let unit = format!("wfc-merkez-eslesme-{}", now as u64);
        let c: Vec<String> = ["systemd-run", "--unit", &unit, "--on-active", "1s", "/usr/local/bin/wificorrect", "ctl", "merkez-eslesme"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        (self.runner)(&c);
    }
```

- [ ] **Step 4: Run** `scripts/test.sh` → bütün testler geçer, uyarı yok.
- [ ] **Step 5: Commit** `git commit -am "Cihaz: günlük merkez eşitlemesi; serbest bırakılınca kayıt teslimi, temizlendi bildirimi ve bağın silinmesi"`

---

### Task 4: Cihaza kurulum, test cihazının geçişi, uçtan uca

**Files:** Modify: `docs/KURULUM_GUNLUGU.md`, `docs/RUST_YENIDEN_YAZIM.md`

- [ ] **Step 1: Cihazda derle ve kur** — `scripts/gelistir.sh` (mevcut iş akışı: release derleme bellek sınırlı; derleme
başarısızsa kurmaz). Doğrula: `ssh wificorrect 'wificorrect surum; systemctl is-active wificorrect-panel wificorrect-kaydedici wificorrect-portal'`.
- [ ] **Step 2: Eski elle eklenmiş tüneli kapat (sunucu, kullanıcı sudo parolasını girer)** — terminal panelinde betik:
`ssh -t -i ~/.ssh/gbserver gbserver@192.168.1.109 'sudo wificorrect-sunucu cihaz-kapat bocafe-test'`.
(Aynı WireGuard anahtarı yeni numarayla eklenebilsin diye şart; cihazın şimdiki tüneli ve yedeği bu anda durur.)
- [ ] **Step 3: Müşteri girişi (kullanıcı)** — `https://192.168.1.110:8443/giris` → Göztepe Bilgisayar numarası + parolası
(kullanıcı yazar). Beklenen: panel açılır; `ssh wificorrect 'cat /etc/wificorrect/merkez.json | grep numara; grep -A3 "\[uzak\]" /etc/wificorrect/ayarlar.toml; wg show wfc latest-handshakes'`
→ numara doğru, uzak.adres 10.99.0.x, el sıkışma birkaç saniye içinde. Yönetim merkezinde Göztepe Bilgisayar → Cihaz:
tünel adresi ve "çevrimiçi".
- [ ] **Step 4: Eşitleme ve yedek** — `ssh wificorrect 'wificorrect ctl merkez-eslesme; wificorrect ctl yedekle'` → `MERKEZ_ESLESME uyelik=aktif`,
yedek "zincir gönderildi"; sunucuda `ls /srv/wificorrect-arsiv/<numara>/veri` → `zincir.txt`. Yönetimde "son eşitleme" dolu.
- [ ] **Step 5: Günlükler** — KURULUM_GUNLUGU'na: sürüm, `bocafe-test` kapatıldı, cihaz Göztepe Bilgisayar numarasıyla bağlandı
(tünel adresi), ilk eşitleme ve yedek. RUST_YENIDEN_YAZIM rol bölümünde: "İşletme sahibi hesabı merkezde (yönetim merkezi);
cihazda kurulum ekranı yok, giriş müşteri numarasıyla; yerelde yalnızca admin."
- [ ] **Step 6: Commit** `git commit -am "Kurulum günlüğü: cihaz yönetim merkezine bağlandı"`
