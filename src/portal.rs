//! Müşteri giriş portalı (eski portal.py; docs/MASTER_ENGINEERING.md §11, RUST_YENIDEN_YAZIM.md A4).
//! Akış: form → hız sınırları → SMS kodu → doğrulama → nft yetkisi + oturum + 5651 kayıtları.

use crate::ayar::Config;
use crate::ortak::{self, Row, Runner, Session};
use crate::sms::{self, Saglayici, Sonuc};
use crate::ulkeler;
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

const NO_MAC: &str = "Cihazınız tanınamadı. Wi-Fi bağlantısını kapatıp yeniden açın; sorun sürerse personele başvurun.";
const AFTER_LOGIN_URL: &str = "http://www.google.com/"; // istenen sayfa bilinmiyorsa başarıdan sonra

pub type Form = HashMap<String, String>;
pub type Clock = dyn Fn() -> f64 + Send + Sync;
/// (ayar, sağlayıcı, numara E.164, kod) → sonuç. Twilio gerçek modda kodu kendisi üretir.
pub type SmsFn = dyn Fn(&Config, Saglayici, &str, &str) -> Sonuc + Send + Sync;
/// Twilio Verify uzaktan doğrulama: (onaylandı, ağ hatası).
pub type CheckFn = dyn Fn(&Config, &str, &str) -> (bool, Option<String>) + Send + Sync;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner()) // bir istekteki panik diğerlerini kilitlemesin
}

/// Yalnızca http(s) ve ≤ 2048 karakter (açık yönlendirme koruması).
pub fn safe_dst(dst: &str) -> String {
    let d = dst.trim();
    if d.len() <= 2048 && (d.starts_with("http://") || d.starts_with("https://")) {
        d.to_string()
    } else {
        String::new()
    }
}

/// Kayan pencere sayacı. ponytail: anahtarlar bellekten silinmez; kafe ölçeğinde önemsiz, gerekirse periyodik temizlik.
#[derive(Default)]
pub struct RateLimiter(Mutex<HashMap<String, VecDeque<f64>>>);

impl RateLimiter {
    pub fn count(&self, key: &str, window: f64, now: f64) -> usize {
        let mut m = lock(&self.0);
        let q = m.entry(key.to_string()).or_default();
        while q.front().is_some_and(|&t| t <= now - window) {
            q.pop_front();
        }
        q.len()
    }

    pub fn hit(&self, key: &str, now: f64) {
        lock(&self.0).entry(key.to_string()).or_default().push_back(now);
    }
}

#[derive(Debug)]
pub struct Page {
    pub tpl: &'static str,
    pub ctx: Vec<(&'static str, String)>,
}

impl Page {
    fn new(tpl: &'static str, ctx: Vec<(&'static str, String)>) -> Page {
        Page { tpl, ctx }
    }

    #[cfg(test)]
    pub fn get(&self, key: &str) -> &str {
        self.ctx.iter().find(|(k, _)| *k == key).map(|(_, v)| v.as_str()).unwrap_or("")
    }
}

#[derive(Clone, Debug)]
struct Pending {
    phone: String,
    ad: String,
    soyad: String,
    code: String,
    dst: String,
    expires: f64,
    attempts: u32,
    sent_at: f64,
    provider: Saglayici,
    /// Uzaktan doğrulama sürerken kod yenilendiyse sonuç eski koda uygulanmasın
    nonce: u64,
}

pub struct Portal {
    pub cfg: Config,
    pending: Mutex<HashMap<String, Pending>>,
    rl: RateLimiter,
    clock: Box<Clock>,
    wall: Box<Clock>,
    runner: Box<Runner>,
    send_sms: Box<SmsFn>,
    check_sms: Box<CheckFn>,
    nonce: AtomicU64,
    pub host: String,
    pub base_url: String,
}

fn template(name: &str) -> &'static str {
    match name {
        "_sayfa" => include_str!("sablon/_sayfa.html"),
        "giris" => include_str!("sablon/giris.html"),
        "kod" => include_str!("sablon/kod.html"),
        "basarili" => include_str!("sablon/basarili.html"),
        "hata" => include_str!("sablon/hata.html"),
        "kvkk" => include_str!("sablon/kvkk.html"),
        _ => "",
    }
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#x27;")
}

/// `$ad` yer tutucularını doldurur; bilinmeyeni olduğu gibi bırakır (Python string.Template.safe_substitute).
pub fn substitute(tpl: &str, values: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(tpl.len());
    let mut rest = tpl;
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        if let Some(stripped) = after.strip_prefix('$') {
            out.push('$');
            rest = stripped;
            continue;
        }
        let n = after.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(after.len());
        let name = &after[..n];
        match values.get(name) {
            Some(v) if !name.is_empty() && !name.starts_with(|c: char| c.is_ascii_digit()) => out.push_str(v),
            _ => {
                out.push('$');
                out.push_str(name);
            }
        }
        rest = &after[n..];
    }
    out.push_str(rest);
    out
}

/// Sabit sürede karşılaştırma (zamanlama ile kod tahminini önler).
fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn otp_code() -> String {
    loop {
        let mut b = [0u8; 4];
        getrandom::getrandom(&mut b).expect("rastgele sayı üretilemedi");
        let x = u32::from_le_bytes(b);
        if x < 4_294_000_000 {
            // eşit dağılım için 10^6'nın katının üstü atılır
            return format!("{:06}", x % 1_000_000);
        }
    }
}

impl Portal {
    pub fn new(cfg: Config, clock: Box<Clock>, wall: Box<Clock>, runner: Box<Runner>, send_sms: Box<SmsFn>, check_sms: Box<CheckFn>) -> Portal {
        let host = format!("{}:{}", cfg.main.router_ip, cfg.main.portal_port);
        Portal {
            base_url: format!("http://{host}"),
            host,
            cfg,
            pending: Mutex::default(),
            rl: RateLimiter::default(),
            clock,
            wall,
            runner,
            send_sms,
            check_sms,
            nonce: AtomicU64::new(1),
        }
    }

    /// Bekleyen koddan formu yeniden doldurmak için (numara `+` ile, ülke seçimi yok sayılır).
    fn prefill(p: &Pending) -> Form {
        [("ad", p.ad.clone()), ("soyad", p.soyad.clone()), ("telefon", format!("+{}", p.phone)), ("kvkk", "1".into()), ("dst", p.dst.clone())]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }

    // --- sayfalar
    pub fn render(&self, page: &Page) -> String {
        let mut values: HashMap<&str, String> = HashMap::new();
        values.insert("site", self.cfg.main.site_name.clone());
        values.insert("portal", self.base_url.clone());
        for (k, v) in &page.ctx {
            values.insert(k, v.clone());
        }
        let safe: HashMap<&str, String> =
            values.into_iter().map(|(k, v)| (k, if k.ends_with("_html") { v } else { html_escape(&v) })).collect();
        let body = substitute(template(page.tpl), &safe);
        let mut outer = HashMap::new();
        outer.insert("icerik_html", body);
        outer.insert("site", safe["site"].clone());
        outer.insert("bas_html", safe.get("bas_html").cloned().unwrap_or_default());
        substitute(template("_sayfa"), &outer)
    }

    fn form_page(&self, form: &Form, errors: Vec<(&'static str, String)>, dst: &str) -> Page {
        let f = |k: &str| form.get(k).cloned().unwrap_or_default();
        let mut ctx = vec![
            ("ad", f("ad")),
            ("soyad", f("soyad")),
            ("telefon", f("telefon")),
            ("kvkk_checked", if f("kvkk") == "1" { "checked".into() } else { String::new() }),
            ("dst", dst.to_string()),
            ("ulke_secenekleri_html", ulkeler::options_html(form.get("ulke").map_or("TR", String::as_str))),
        ];
        for k in ["hata_genel", "hata_ad", "hata_soyad", "hata_telefon", "hata_kvkk"] {
            let v = errors.iter().find(|(e, _)| *e == k).map(|(_, v)| v.clone()).unwrap_or_default();
            ctx.push((k, v));
        }
        Page::new("giris", ctx)
    }

    fn code_page(&self, p: &Pending, note: &str, error: &str) -> Page {
        let dakika = (self.cfg.limits.otp_ttl_sec / 60).max(1);
        Page::new(
            "kod",
            vec![
                ("telefon_maskeli", ulkeler::mask_phone(&p.phone)),
                ("dakika", dakika.to_string()),
                ("not", note.into()),
                ("hata", error.into()),
            ],
        )
    }

    fn error_page(&self, msg: &str) -> Page {
        Page::new("hata", vec![("mesaj", msg.into())])
    }

    /// JS yok (CSP): 1 sn sonra yönlen. Telefonların giriş penceresi interneti görünce kendini kapatır.
    fn success_page(&self, dst: &str) -> Page {
        let target = html_escape(if dst.is_empty() { AFTER_LOGIN_URL } else { dst });
        Page::new("basarili", vec![("bas_html", format!("<meta http-equiv=\"refresh\" content=\"1;url={target}\">"))])
    }

    fn audit(&self, olay: &str, ip: &str, mac: &str, phone: &str, ek: &str) {
        let row = Row::new(olay, &ortak::now_iso((self.wall)()))
            .set("telefon", phone)
            .set("mac", mac)
            .set("ic_ip", ip)
            .set("ek", ek);
        ortak::audit(&self.cfg.main.log_root, row);
    }

    fn session_for(&self, ip: &str, mac: &str) -> bool {
        ortak::load_sessions(&self.cfg.main.state_root)
            .get(mac)
            .is_some_and(|s| s.ip == ip && s.expires_epoch > (self.wall)())
    }

    // --- akış
    pub fn home(&self, ip: &str, mac: Option<&str>, dst: &str) -> Page {
        let Some(mac) = mac else { return self.error_page(NO_MAC) };
        if self.session_for(ip, mac) || self.rehome(ip, mac) {
            self.success_page(dst)
        } else {
            self.form_page(&Form::new(), vec![], dst)
        }
    }

    /// Oturumu süren cihaz DHCP'den yeni IP ile döndü: SMS istemeden oturumu yeni IP'ye taşır.
    fn rehome(&self, ip: &str, mac: &str) -> bool {
        let root = &self.cfg.main;
        let Ok(_g) = ortak::state_lock(&root.state_root) else { return false };
        let mut ses = ortak::load_sessions(&root.state_root);
        let now = (self.wall)();
        let movable = ses.get(mac).is_some_and(|s| s.expires_epoch > now && s.ip != ip);
        if !movable || !ortak::move_session(&root.log_root, &mut ses, mac, ip, now, &*self.runner) {
            return false;
        }
        ortak::save_sessions(&root.state_root, &ses).is_ok()
    }

    fn refusal(&self, pending: &HashMap<String, Pending>, mac: &str, phone: &str, now: f64, day: &str) -> Option<(&'static str, String)> {
        let l = &self.cfg.limits;
        let cooldown = l.resend_cooldown_sec as f64;
        if let Some(p) = pending.get(mac) {
            if now - p.sent_at < cooldown {
                let wait = (cooldown - (now - p.sent_at)) as i64 + 1;
                return Some(("bekleme", format!("Yeni kod için {wait} saniye bekleyin.")));
            }
        }
        let tel = format!("tel:{phone}");
        if self.rl.count(&tel, 900.0, now) >= l.sms_per_phone_15min {
            return Some(("tel_15dk", "Bu numaraya çok fazla kod gönderildi. 15 dakika sonra tekrar deneyin.".into()));
        }
        if self.rl.count(&tel, 86400.0, now) >= l.sms_per_phone_day {
            return Some(("tel_gun", "Bu numara için bugünkü kod sınırı doldu.".into()));
        }
        if self.rl.count(&format!("mac:{mac}"), 3600.0, now) >= l.sms_per_mac_hour {
            return Some(("mac_saat", "Bu cihazdan çok fazla kod istendi. Bir saat sonra tekrar deneyin.".into()));
        }
        if ortak::sms_count(&self.cfg.main.state_root, day) >= l.sms_global_day {
            return Some(("tavan", "Şu an SMS gönderilemiyor, lütfen personele başvurun.".into()));
        }
        None
    }

    pub fn send_code(&self, ip: &str, mac: Option<&str>, form: &Form) -> Page {
        let get = |k: &str| form.get(k).map(String::as_str).unwrap_or("");
        let dst = safe_dst(get("dst"));
        let Some(mac) = mac else { return self.error_page(NO_MAC) };
        let ulke = if get("ulke").is_empty() { "TR" } else { get("ulke") };
        let (ad, soyad, phone) = (ortak::clean_name(get("ad")), ortak::clean_name(get("soyad")), ulkeler::normalize_phone(ulke, get("telefon")));
        let mut errors = vec![];
        if ad.is_none() {
            errors.push(("hata_ad", "Adınızı harflerle yazın (2-40 karakter).".to_string()));
        }
        if soyad.is_none() {
            errors.push(("hata_soyad", "Soyadınızı harflerle yazın (2-40 karakter).".to_string()));
        }
        if phone.is_none() {
            let msg = if ulke == "TR" && !get("telefon").trim_start().starts_with('+') {
                "Geçerli bir cep telefonu numarası girin (5XX XXX XX XX)."
            } else {
                "Geçerli bir telefon numarası girin (ülke kodunu kontrol edin)."
            };
            errors.push(("hata_telefon", msg.to_string()));
        }
        if get("kvkk") != "1" {
            errors.push(("hata_kvkk", "Devam etmek için aydınlatma metnini onaylayın.".to_string()));
        }
        let (Some(ad), Some(soyad), Some(phone), true) = (ad, soyad, phone, errors.is_empty()) else {
            return self.form_page(form, errors, &dst);
        };
        let provider = match sms::route(&self.cfg, &phone) {
            Ok(p) => p,
            Err(msg) => {
                self.audit("OTP_ISTEK", ip, mac, &phone, "red=yabanci_numara");
                return self.form_page(form, vec![("hata_genel", msg.to_string())], &dst);
            }
        };
        let now = (self.clock)();
        let day = ortak::day_of(&ortak::now_iso((self.wall)())).to_string();
        let cooldown = self.cfg.limits.resend_cooldown_sec as f64;
        let code = {
            let pending = lock(&self.pending);
            if let Some(p) = pending.get(mac) {
                if p.phone == phone && now - p.sent_at < cooldown && now < p.expires {
                    let wait = (cooldown - (now - p.sent_at)) as i64 + 1;
                    return self.code_page(p, &format!("Kod az önce gönderildi. Yeni kod için {wait} saniye bekleyin."), "");
                }
            }
            if let Some((kind, msg)) = self.refusal(&pending, mac, &phone, now, &day) {
                drop(pending);
                let olay = if kind == "tavan" { "SMS_TAVAN" } else { "OTP_ISTEK" };
                self.audit(olay, ip, mac, &phone, &format!("red={kind}"));
                return self.form_page(form, vec![("hata_genel", msg)], &dst);
            }
            self.rl.hit(&format!("tel:{phone}"), now);
            self.rl.hit(&format!("mac:{mac}"), now);
            ortak::sms_count_inc(&self.cfg.main.state_root, &day);
            otp_code()
        };
        let r = (self.send_sms)(&self.cfg, provider, &phone, &code); // ağ çağrısı kilit dışında
        if !r.ok {
            self.audit("OTP_HATA", ip, mac, &phone, &format!("{}={}", provider.ad(), r.kod));
            return self.form_page(form, vec![("hata_genel", sms::message_for(provider, &r.kod).to_string())], &dst);
        }
        let p = Pending {
            phone: phone.clone(),
            ad,
            soyad,
            code,
            dst,
            expires: now + self.cfg.limits.otp_ttl_sec as f64,
            attempts: 0,
            sent_at: now,
            provider,
            nonce: self.nonce.fetch_add(1, Ordering::Relaxed),
        };
        let page = self.code_page(&p, "", "");
        lock(&self.pending).insert(mac.to_string(), p);
        let job = r.is.as_deref().unwrap_or("None");
        self.audit("OTP_GONDERILDI", ip, mac, &phone, &format!("{}={job}", provider.ad()));
        page
    }

    pub fn resend(&self, ip: &str, mac: Option<&str>) -> Page {
        let p = mac.and_then(|m| lock(&self.pending).get(m).cloned());
        let Some(p) = p else {
            return self.form_page(&Form::new(), vec![("hata_genel", "Kod bulunamadı, lütfen bilgilerinizi yeniden girin.".into())], "");
        };
        self.send_code(ip, mac, &Self::prefill(&p))
    }

    pub fn verify(&self, ip: &str, mac: Option<&str>, form: &Form) -> Page {
        let Some(mac) = mac else { return self.error_page(NO_MAC) };
        let now = (self.clock)();
        let fail_lock = self.cfg.limits.verify_fail_lock_min * 60;
        let fail_key = format!("hata:{mac}");
        if self.rl.count(&fail_key, fail_lock as f64, now) >= 10 {
            return self.error_page(&format!(
                "Çok fazla hatalı deneme yapıldı. Lütfen {} dakika sonra tekrar deneyin.",
                fail_lock / 60
            ));
        }
        let not_found = || self.form_page(&Form::new(), vec![("hata_genel", "Kod bulunamadı, lütfen yeni kod isteyin.".into())], "");
        let code: String = form.get("kod").map(|k| k.chars().filter(char::is_ascii_digit).collect()).unwrap_or_default();
        // 1) bekleyen kod
        let p = {
            let mut pending = lock(&self.pending);
            let Some(p) = pending.get(mac).cloned() else { return not_found() };
            if now >= p.expires {
                pending.remove(mac);
                return self.form_page(&Self::prefill(&p), vec![("hata_genel", "Kodun süresi doldu. Yeni kod isteyin.".into())], &p.dst);
            }
            p
        };
        // 2) karşılaştır: Twilio gerçek modda kodu Twilio doğrular (ağ çağrısı kilit dışında); diğerlerinde yerelde
        let matched = if p.provider == Saglayici::Twilio && !self.cfg.sms.mock {
            let (ok, err) = (self.check_sms)(&self.cfg, &p.phone, &code);
            if err.is_some() {
                return self.code_page(&p, "", "Kod şu an doğrulanamadı, lütfen birkaç saniye sonra tekrar deneyin.");
            }
            ok
        } else {
            ct_eq(&code, &p.code)
        };
        // 3) sonucu uygula — bu arada kod yenilendiyse eski sonuç uygulanmaz
        let wrong_page = {
            let mut pending = lock(&self.pending);
            let Some(cur) = pending.get_mut(mac).filter(|c| c.nonce == p.nonce) else { return not_found() };
            if matched {
                pending.remove(mac);
                None
            } else {
                cur.attempts += 1;
                self.rl.hit(&fail_key, now);
                let left = self.cfg.limits.otp_max_attempts.saturating_sub(cur.attempts);
                Some(if left == 0 {
                    pending.remove(mac);
                    self.form_page(&Self::prefill(&p), vec![("hata_genel", "Çok fazla hatalı deneme. Yeni kod isteyin.".into())], &p.dst)
                } else {
                    self.code_page(&p, "", &format!("Kod hatalı. Kalan deneme hakkı: {left}"))
                })
            }
        };
        if let Some(page) = wrong_page {
            self.audit("OTP_HATALI_KOD", ip, mac, &p.phone, "");
            return page;
        }
        let Some(s) = self.open_session(ip, mac, &p) else {
            return self.error_page("Bağlantı açılamadı, lütfen tekrar deneyin.");
        };
        self.audit("OTP_BASARILI", ip, mac, &p.phone, &format!("oturum={}", s.session_id));
        self.success_page(&p.dst)
    }

    /// §11.7 sırası. nft başarısızsa yeni oturum yazılmaz.
    fn open_session(&self, ip: &str, mac: &str, p: &Pending) -> Option<Session> {
        let m = &self.cfg.main;
        let runner = &*self.runner;
        let _g = ortak::state_lock(&m.state_root).ok()?;
        let mut ses = ortak::load_sessions(&m.state_root);
        let now = (self.wall)();
        if ses.contains_key(mac) {
            ortak::close_session(&m.log_root, &mut ses, mac, "yeniden_giris", now, runner);
        }
        let mut same: Vec<(f64, String)> =
            ses.iter().filter(|(_, s)| s.phone == p.phone).map(|(k, s)| (s.start_epoch, k.clone())).collect();
        same.sort_by(|a, b| a.0.total_cmp(&b.0));
        while !same.is_empty() && same.len() >= m.max_devices_per_phone {
            let (_, old) = same.remove(0);
            ortak::close_session(&m.log_root, &mut ses, &old, "cihaz_limiti", now, runner);
        }
        let secs = (m.session_minutes * 60) as f64;
        if !ortak::nft_add_cmd(mac, ip, secs).is_some_and(|c| runner(&c)) {
            let _ = ortak::save_sessions(&m.state_root, &ses);
            return None;
        }
        ortak::park_ip_holders(&m.log_root, &mut ses, mac, ip, now, runner);
        let zaman = ortak::now_iso(now);
        let s = Session {
            phone: p.phone.clone(),
            ad: p.ad.clone(),
            soyad: p.soyad.clone(),
            ip: ip.into(),
            session_id: ortak::random_hex(6),
            start: zaman.clone(),
            start_epoch: now,
            expires_epoch: now + secs,
        };
        ses.insert(mac.into(), s.clone());
        if let Err(e) = ortak::save_sessions(&m.state_root, &ses) {
            eprintln!("oturum dosyası yazılamadı: {e}");
        }
        let basla = Row::new("OTURUM_BASLA", &zaman)
            .set("telefon", &s.phone)
            .set("ad", &s.ad)
            .set("soyad", &s.soyad)
            .set("mac", mac)
            .set("ic_ip", ip)
            .set("oturum_id", &s.session_id)
            .set("ek", format!("bitis={}", ortak::now_iso(now + secs)));
        let day = ortak::day_of(&zaman).to_string();
        let written = ortak::append_user_rows(&m.log_root, &s, mac, &[basla.clone()])
            .and_then(|_| ortak::append_rows(&ortak::day_file(&m.log_root, &day, "oturum.csv"), &[basla]))
            .and_then(|_| ortak::upsert_index(&m.log_root, &s.phone, &s.ad, &s.soyad, &zaman));
        if let Err(e) = written {
            eprintln!("oturum kaydı yazılamadı: {e}");
        }
        Some(s)
    }
}

// ---------------------------------------------------------------- HTTP
fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(v) => {
                        out.push(v);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `a=1&b=2` → ilk değer kazanır; boş değerler atlanır (Python parse_qs gibi).
pub fn parse_query(s: &str) -> Form {
    let mut out = Form::new();
    for pair in s.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let (k, v) = (pct_decode(k), pct_decode(v));
        if !k.is_empty() && !v.is_empty() {
            out.entry(k).or_insert(v);
        }
    }
    out
}

fn pct_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn respond(req: tiny_http::Request, code: u16, body: String, ctype: &str, location: Option<String>, base_url: &str) {
    let csp = format!("default-src 'none'; style-src 'unsafe-inline'; img-src data:; form-action {base_url}");
    let mut headers = vec![
        ("Content-Type", ctype.to_string()),
        ("Cache-Control", "no-store".into()),
        ("X-Frame-Options", "DENY".into()),
        ("Content-Security-Policy", csp),
        ("Server", "hotspot".into()),
    ];
    if let Some(l) = location {
        headers.push(("Location", l));
    }
    let mut resp = tiny_http::Response::from_string(body).with_status_code(code);
    for (k, v) in headers {
        if let Ok(h) = tiny_http::Header::from_bytes(k.as_bytes(), v.as_bytes()) {
            resp.add_header(h);
        }
    }
    let _ = req.respond(resp);
}

fn handle(p: &Portal, per_ip: &RateLimiter, mut req: tiny_http::Request) {
    let base = p.base_url.clone();
    let text = |req, code, msg: &str| respond(req, code, msg.to_string(), "text/plain; charset=utf-8", None, &base);
    let ip = req.remote_addr().map(|a| a.ip().to_string()).unwrap_or_default();
    if !ortak::in_subnet(&ip, &p.cfg.main.subnet) {
        return text(req, 403, "Yasak");
    }
    let now = ortak::monotonic();
    if per_ip.count(&ip, 60.0, now) >= 30 {
        return text(req, 429, "Çok fazla istek. Biraz bekleyip tekrar deneyin.");
    }
    per_ip.hit(&ip, now);
    let host = req.headers().iter().find(|h| h.field.equiv("Host")).map(|h| h.value.as_str().to_string()).unwrap_or_default();
    if host != p.host {
        // nft'nin 80 → 8080 yönlendirdiği yabancı istek: portala gönder, gitmek istediği adresi hatırla
        let dst = if host.is_empty() { String::new() } else { safe_dst(&format!("http://{host}{}", req.url())) };
        let q = if dst.is_empty() { String::new() } else { format!("?dst={}", pct_encode(&dst)) };
        return respond(req, 302, String::new(), "text/html; charset=utf-8", Some(format!("{base}/{q}")), &base);
    }
    let url = req.url().to_string();
    let (path, query) = url.split_once('?').unwrap_or((url.as_str(), ""));
    let mac = ortak::lookup_mac(&ip, &p.cfg.main.leases_file);
    let mac = mac.as_deref();
    let method = req.method().clone();
    let page = match method {
        tiny_http::Method::Get | tiny_http::Method::Head => match path {
            "/" => p.home(&ip, mac, &safe_dst(parse_query(query).get("dst").map(String::as_str).unwrap_or(""))),
            "/kvkk" => Page::new("kvkk", vec![]),
            _ => return respond(req, 302, String::new(), "text/html; charset=utf-8", Some(format!("{base}/")), &base),
        },
        tiny_http::Method::Post => {
            if req.body_length().unwrap_or(0) > 4096 {
                return text(req, 413, "İstek çok büyük.");
            }
            let mut raw = String::new();
            if req.as_reader().take(4096).read_to_string(&mut raw).is_err() {
                return text(req, 400, "Geçersiz istek.");
            }
            let form = parse_query(&raw);
            match path {
                "/kod-gonder" => p.send_code(&ip, mac, &form),
                "/dogrula" => p.verify(&ip, mac, &form),
                "/tekrar-gonder" => p.resend(&ip, mac),
                _ => return respond(req, 302, String::new(), "text/html; charset=utf-8", Some(format!("{base}/")), &base),
            }
        }
        _ => return text(req, 405, "Desteklenmeyen istek."),
    };
    let html = p.render(&page);
    respond(req, 200, html, "text/html; charset=utf-8", None, &base);
}

pub fn run(cfg: Config) -> ExitCode {
    let missing = cfg.sms_missing();
    if !missing.is_empty() {
        eprintln!("portal: SMS ayarları eksik: {}", missing.join(", "));
        return ExitCode::from(1);
    }
    let addr = format!("{}:{}", cfg.main.router_ip, cfg.main.portal_port);
    let server = match tiny_http::Server::http(&addr) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("portal: {addr} dinlenemedi: {e}");
            return ExitCode::from(1);
        }
    };
    let portal = Arc::new(Portal::new(
        cfg,
        Box::new(ortak::monotonic),
        Box::new(ortak::wall),
        Box::new(|c: &[String]| ortak::run(c)),
        Box::new(sms::send),
        Box::new(crate::twilio::check),
    ));
    portal.audit("SERVIS_BASLADI", "", "", "", "portal");
    eprintln!("portal: {addr} dinleniyor (SMS deneme modu: {})", if portal.cfg.sms.mock { "açık" } else { "kapalı" });
    let per_ip = Arc::new(RateLimiter::default());
    // ponytail: 8 işçi + gövde ≤ 4 KB; yavaş istemci bir işçiyi en çok gövde okuma süresince tutar. Kafe ölçeğinde yeterli.
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let (server, portal, per_ip) = (server.clone(), portal.clone(), per_ip.clone());
            std::thread::spawn(move || loop {
                match server.recv() {
                    Ok(req) => {
                        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle(&portal, &per_ip, req)));
                        if r.is_err() {
                            eprintln!("portal: istek işlenirken beklenmeyen hata");
                        }
                    }
                    Err(e) => {
                        eprintln!("portal: bağlantı hatası: {e}");
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            })
        })
        .collect();
    for w in workers {
        let _ = w.join();
    }
    ExitCode::from(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct T {
        p: Portal,
        mono: Arc<Mutex<f64>>,
        wall: Arc<Mutex<f64>>,
        calls: Arc<Mutex<Vec<Vec<String>>>>,
        sms: Arc<Mutex<Vec<(String, String, &'static str)>>>,
        root: std::path::PathBuf,
    }

    const MAC: &str = "aa:bb:cc:dd:ee:01";
    const IP: &str = "10.50.0.23";
    const WALL0: f64 = 1_790_705_134.0; // 2026-09-29 21:05:34 +03

    fn setup(tweak: impl FnOnce(&mut Config)) -> T {
        static N: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!("wfc-portal-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&root);
        let mut cfg = Config::default();
        cfg.main.site_name = "Bocafe".into();
        cfg.main.log_root = root.join("5651").to_string_lossy().into();
        cfg.main.state_root = root.join("state").to_string_lossy().into();
        tweak(&mut cfg);
        let (mono, wall) = (Arc::new(Mutex::new(100.0)), Arc::new(Mutex::new(WALL0)));
        let calls = Arc::new(Mutex::new(vec![]));
        let sms_log = Arc::new(Mutex::new(vec![]));
        let (m2, w2, c2, s2) = (mono.clone(), wall.clone(), calls.clone(), sms_log.clone());
        let p = Portal::new(
            cfg,
            Box::new(move || *m2.lock().unwrap()),
            Box::new(move || *w2.lock().unwrap()),
            Box::new(move |c: &[String]| {
                c2.lock().unwrap().push(c.to_vec());
                !c.iter().any(|a| a.contains("10.50.0.99")) // .99'a nft ekleme başarısız sayılır
            }),
            Box::new(move |_c: &Config, prov: Saglayici, phone: &str, code: &str| {
                s2.lock().unwrap().push((phone.into(), code.into(), prov.ad()));
                Sonuc { ok: true, kod: "0".into(), is: Some("J1".into()) }
            }),
            // Twilio uzaktan doğrulama: 424242 onaylı, 999999 ağ hatası
            Box::new(|_c: &Config, _phone: &str, code: &str| match code {
                "424242" => (true, None),
                "999999" => (false, Some("AG".into())),
                _ => (false, None),
            }),
        );
        T { p, mono, wall, calls, sms: sms_log, root }
    }

    impl T {
        fn advance(&self, secs: f64) {
            *self.mono.lock().unwrap() += secs;
            *self.wall.lock().unwrap() += secs;
        }
        fn last_code(&self) -> String {
            self.sms.lock().unwrap().last().unwrap().1.clone()
        }
        fn sms_count(&self) -> usize {
            self.sms.lock().unwrap().len()
        }
        fn read(&self, rel: &str) -> String {
            std::fs::read_to_string(self.root.join(rel)).unwrap_or_default()
        }
        fn audit_text(&self) -> String {
            self.read("5651/gunluk/2026-09-29/denetim.csv")
        }
    }

    fn form(pairs: &[(&str, &str)]) -> Form {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn good(phone: &str) -> Form {
        form(&[("ad", "Ayşe"), ("soyad", "Yılmaz"), ("telefon", phone), ("kvkk", "1"), ("dst", "http://neverssl.com/")])
    }

    #[test]
    fn form_errors_send_nothing() {
        let t = setup(|_| {});
        let pg = t.p.send_code(IP, Some(MAC), &form(&[("ad", "A"), ("soyad", "Ali1"), ("telefon", "212 455 31 32")]));
        assert_eq!(pg.tpl, "giris");
        for k in ["hata_ad", "hata_soyad", "hata_telefon", "hata_kvkk"] {
            assert!(!pg.get(k).is_empty(), "{k}");
        }
        assert_eq!(pg.get("telefon"), "212 455 31 32"); // girilen değer korunur
        assert_eq!(t.sms_count(), 0);
        assert_eq!(t.p.send_code(IP, None, &good("5334553132")).tpl, "hata"); // MAC bilinmiyor
    }

    #[test]
    fn full_flow_opens_session_and_writes_records() {
        let t = setup(|_| {});
        let pg = t.p.send_code(IP, Some(MAC), &good("0533 455 31 32"));
        assert_eq!(pg.tpl, "kod");
        assert_eq!(pg.get("telefon_maskeli"), "+90 5XX XXX XX 32");
        assert_eq!(t.sms.lock().unwrap()[0].0, "905334553132");
        assert_eq!(t.sms.lock().unwrap()[0].2, "netgsm");
        let code = t.last_code();
        assert_eq!(code.len(), 6);
        let wrong = if code == "000000" { "111111" } else { "000000" };
        let pg = t.p.verify(IP, Some(MAC), &form(&[("kod", wrong)]));
        assert_eq!((pg.tpl, pg.get("hata")), ("kod", "Kod hatalı. Kalan deneme hakkı: 4"));
        let pg = t.p.verify(IP, Some(MAC), &form(&[("kod", &format!(" {code} "))]));
        assert_eq!(pg.tpl, "basarili");
        assert!(pg.get("bas_html").contains("url=http://neverssl.com/"));
        let calls = t.calls.lock().unwrap().clone();
        assert!(calls.iter().any(|c| c.last().unwrap() == &format!("{{ {MAC} . {IP} timeout 2592000s }}")));
        let ses = ortak::load_sessions(&t.p.cfg.main.state_root);
        assert_eq!(ses[MAC].ip, IP);
        assert_eq!(ses[MAC].phone, "905334553132");
        let user = t.read("5651/gunluk/2026-09-29/kullanicilar/905334553132.csv");
        assert!(user.starts_with('\u{feff}'));
        assert!(user.contains(";KAYIT;905334553132;Ayşe;Yılmaz;aa:bb:cc:dd:ee:01;10.50.0.23;"));
        assert!(user.contains(";OTURUM_BASLA;"));
        assert!(t.read("5651/gunluk/2026-09-29/oturum.csv").contains("OTURUM_BASLA"));
        assert!(t.read("5651/kullanicilar/index.csv").contains("905334553132;Ayşe;Yılmaz;2026-09-29T21:05:34+03:00"));
        let a = t.audit_text();
        for olay in ["OTP_GONDERILDI", "OTP_HATALI_KOD", "OTP_BASARILI"] {
            assert!(a.contains(olay), "{olay}");
        }
        assert!(!a.contains(&code), "kod kayda yazılmaz");
        assert_eq!(t.p.home(IP, Some(MAC), "").tpl, "basarili"); // zaten bağlı
    }

    #[test]
    fn double_submit_does_not_resend() {
        let t = setup(|_| {});
        t.p.send_code(IP, Some(MAC), &good("5334553132"));
        t.advance(5.0);
        let pg = t.p.send_code(IP, Some(MAC), &good("5334553132"));
        assert_eq!(pg.tpl, "kod");
        assert!(pg.get("not").starts_with("Kod az önce gönderildi"));
        assert_eq!(t.sms_count(), 1);
        assert!(t.p.resend(IP, Some(MAC)).get("not").starts_with("Kod az önce gönderildi"));
        t.advance(61.0);
        assert_eq!(t.p.resend(IP, Some(MAC)).tpl, "kod");
        assert_eq!(t.sms_count(), 2);
    }

    #[test]
    fn five_wrong_codes_cancel_and_expiry() {
        let t = setup(|_| {});
        t.p.send_code(IP, Some(MAC), &good("5334553132"));
        let wrong = if t.last_code() == "000000" { "111111" } else { "000000" };
        for _ in 0..4 {
            assert_eq!(t.p.verify(IP, Some(MAC), &form(&[("kod", wrong)])).tpl, "kod");
        }
        let pg = t.p.verify(IP, Some(MAC), &form(&[("kod", wrong)]));
        assert_eq!((pg.tpl, pg.get("hata_genel")), ("giris", "Çok fazla hatalı deneme. Yeni kod isteyin."));
        assert_eq!(pg.get("telefon"), "+905334553132");
        assert!(t.p.verify(IP, Some(MAC), &form(&[("kod", wrong)])).get("hata_genel").contains("Kod bulunamadı"));

        t.advance(61.0);
        t.p.send_code(IP, Some(MAC), &good("5334553132"));
        let code = t.last_code();
        t.advance(181.0);
        let pg = t.p.verify(IP, Some(MAC), &form(&[("kod", &code)]));
        assert_eq!(pg.get("hata_genel"), "Kodun süresi doldu. Yeni kod isteyin.");
    }

    #[test]
    fn rate_limits_and_global_cap() {
        let t = setup(|_| {});
        for i in 0..3 {
            assert_eq!(t.p.send_code(IP, Some(MAC), &good("5334553132")).tpl, "kod", "gönderim {i}");
            t.advance(61.0);
        }
        let pg = t.p.send_code(IP, Some(MAC), &good("5334553132"));
        assert!(pg.get("hata_genel").contains("15 dakika"));
        assert!(t.audit_text().contains("red=tel_15dk"));
        assert_eq!(t.sms_count(), 3);

        let t = setup(|c| c.limits.sms_global_day = 1);
        assert_eq!(t.p.send_code(IP, Some(MAC), &good("5334553132")).tpl, "kod");
        let pg = t.p.send_code("10.50.0.24", Some("aa:bb:cc:dd:ee:02"), &good("5321112233"));
        assert_eq!(pg.get("hata_genel"), "Şu an SMS gönderilemiyor, lütfen personele başvurun.");
        assert!(t.audit_text().contains("SMS_TAVAN"));
    }

    fn login(t: &T, mac: &str, ip: &str, phone: &str) -> Page {
        t.p.send_code(ip, Some(mac), &good(phone));
        let code = t.last_code();
        let pg = t.p.verify(ip, Some(mac), &form(&[("kod", &code)]));
        t.advance(61.0);
        pg
    }

    #[test]
    fn ip_change_moves_session_without_sms() {
        let t = setup(|_| {});
        login(&t, MAC, IP, "5334553132");
        let before = t.sms_count();
        assert_eq!(t.p.home("10.50.0.50", Some(MAC), "").tpl, "basarili");
        assert_eq!(t.sms_count(), before);
        let calls = t.calls.lock().unwrap().clone();
        assert!(calls.iter().any(|c| c[1] == "add" && c.last().unwrap().contains("10.50.0.50")));
        assert!(calls.iter().any(|c| c[1] == "delete" && c.last().unwrap().contains(IP)));
        assert_eq!(ortak::load_sessions(&t.p.cfg.main.state_root)[MAC].ip, "10.50.0.50");
        assert!(t.read("5651/gunluk/2026-09-29/oturum.csv").contains("OTURUM_IP_DEGISTI"));
    }

    #[test]
    fn device_limit_closes_oldest() {
        let t = setup(|c| c.main.max_devices_per_phone = 2);
        login(&t, "aa:bb:cc:dd:ee:01", "10.50.0.21", "5334553132");
        login(&t, "aa:bb:cc:dd:ee:02", "10.50.0.22", "5334553132");
        login(&t, "aa:bb:cc:dd:ee:03", "10.50.0.23", "5334553132");
        let ses = ortak::load_sessions(&t.p.cfg.main.state_root);
        assert_eq!(ses.keys().cloned().collect::<Vec<_>>(), vec!["aa:bb:cc:dd:ee:02", "aa:bb:cc:dd:ee:03"]);
        assert!(t.read("5651/gunluk/2026-09-29/oturum.csv").contains("neden=cihaz_limiti"));
    }

    #[test]
    fn same_ip_holder_is_parked() {
        let t = setup(|_| {});
        login(&t, "aa:bb:cc:dd:ee:01", IP, "5334553132");
        login(&t, "aa:bb:cc:dd:ee:02", IP, "5321112233"); // DHCP aynı IP'yi yeni cihaza verdi
        let ses = ortak::load_sessions(&t.p.cfg.main.state_root);
        assert_eq!(ses["aa:bb:cc:dd:ee:01"].ip, "");
        assert_eq!(ses["aa:bb:cc:dd:ee:02"].ip, IP);
    }

    #[test]
    fn nft_failure_writes_no_session() {
        let t = setup(|_| {});
        let pg = login(&t, MAC, "10.50.0.99", "5334553132");
        assert_eq!((pg.tpl, pg.get("mesaj")), ("hata", "Bağlantı açılamadı, lütfen tekrar deneyin."));
        assert!(ortak::load_sessions(&t.p.cfg.main.state_root).is_empty());
    }

    #[test]
    fn rendering_escapes_and_safe_redirects() {
        let t = setup(|_| {});
        let pg = t.p.send_code(IP, Some(MAC), &form(&[("ad", "<script>x</script>"), ("dst", "javascript:alert(1)")]));
        let html = t.p.render(&pg);
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>x"));
        assert!(html.contains("Bocafe'ye hoş geldiniz"));
        assert!(html.contains("name=\"dst\" value=\"\""));
        assert_eq!(safe_dst("https://a.com/x"), "https://a.com/x");
        assert_eq!(safe_dst(&format!("http://{}", "a".repeat(2050))), "");
        assert_eq!(parse_query("a=1&a=2&b=&c=%C5%9F+x&d=%zz"), form(&[("a", "1"), ("c", "ş x"), ("d", "%zz")]));
        assert_eq!(pct_encode("http://a.com/?q=1"), "http%3A%2F%2Fa.com%2F%3Fq%3D1");
        let mut v = HashMap::new();
        v.insert("site", "X".to_string());
        assert_eq!(substitute("$site'ye $bilinmeyen $$5", &v), "X'ye $bilinmeyen $5");
    }

    #[test]
    fn foreign_numbers_and_twilio() {
        // Twilio kapalı: yabancı numaraya SMS yok
        let t = setup(|_| {});
        let f = form(&[("ad", "John"), ("soyad", "Smith"), ("ulke", "DE"), ("telefon", "0151 2345 6789"), ("kvkk", "1")]);
        let pg = t.p.send_code(IP, Some(MAC), &f);
        assert_eq!(pg.get("hata_genel"), sms::YABANCI_YOK);
        assert!(t.p.render(&pg).contains("<option value=\"DE\" selected>")); // seçim korunur
        assert_eq!(t.sms_count(), 0);
        // Twilio açık, deneme modu: yabancı numara Twilio'dan, kod yerelde doğrulanır
        let t = setup(|c| c.twilio.enabled = true);
        let pg = t.p.send_code(IP, Some(MAC), &f);
        assert_eq!((pg.tpl, pg.get("telefon_maskeli")), ("kod", "+49 XXXXXXXXX89"));
        let code = t.last_code();
        assert_eq!(t.sms.lock().unwrap()[0], ("4915123456789".to_string(), code.clone(), "twilio"));
        assert_eq!(t.p.verify(IP, Some(MAC), &form(&[("kod", &code)])).tpl, "basarili");
        assert_eq!(ortak::load_sessions(&t.p.cfg.main.state_root)[MAC].phone, "4915123456789");
        // Türk numarası Twilio açıkken de NetGSM'den
        let t = setup(|c| c.twilio.enabled = true);
        t.p.send_code(IP, Some(MAC), &good("5334553132"));
        assert_eq!(t.sms.lock().unwrap()[0].2, "netgsm");
    }

    #[test]
    fn twilio_real_mode_checks_remotely() {
        let t = setup(|c| {
            c.sms.mock = false;
            c.sms.provider = "twilio".into();
        });
        assert_eq!(t.p.send_code(IP, Some(MAC), &good("5334553132")).tpl, "kod");
        // ağ hatası: deneme hakkı düşmez
        let pg = t.p.verify(IP, Some(MAC), &form(&[("kod", "999999")]));
        assert_eq!(pg.get("hata"), "Kod şu an doğrulanamadı, lütfen birkaç saniye sonra tekrar deneyin.");
        let pg = t.p.verify(IP, Some(MAC), &form(&[("kod", "111111")]));
        assert_eq!(pg.get("hata"), "Kod hatalı. Kalan deneme hakkı: 4");
        assert_eq!(t.p.verify(IP, Some(MAC), &form(&[("kod", "424242")])).tpl, "basarili");
    }
}
