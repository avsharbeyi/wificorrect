//! Panel hesapları, giriş kilidi ve oturumlar (eski panel_auth.py; RUST_YENIDEN_YAZIM.md A10, A14).
//! Hesap dosyası `/etc/wificorrect/hesaplar.json` (600). Parola özeti PBKDF2-HMAC-SHA256, kullanıcı başına tuz.
//! Varsayılan hesap yok: ilk açılışta kurulum ekranı hizmet sağlayıcı ve kafe sahibi hesaplarını oluşturur.

use crate::ortak;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Mutex;

pub const PATH: &str = "/etc/wificorrect/hesaplar.json";
const ITER: u32 = 120_000;
const DUMMY_SALT: &str = "00000000000000000000000000000000";

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Rol {
    /// Hizmet sağlayıcı (Göztepe): her şey, SMS API bilgileri, kayıtlar
    Hizmet,
    /// Kafe sahibi: SMS API bilgileri ve deneme modu hariç bütün ayarlar; kişisel kayıtları görmez
    Sahip,
}

impl Rol {
    pub fn ad(self) -> &'static str {
        match self {
            Rol::Hizmet => "Hizmet sağlayıcı",
            Rol::Sahip => "Kafe sahibi",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Hesap {
    pub rol: Rol,
    pub tuz: String,
    pub ozet: String,
    pub yineleme: u32,
}

fn digest(pw: &str, salt: &str, iter: u32) -> String {
    let mut out = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(pw.as_bytes(), salt.as_bytes(), iter, &mut out);
    out.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn password_problem(pw: &str) -> Option<&'static str> {
    (pw.chars().count() < 10).then_some("Parola en az 10 karakter olmalı.")
}

pub fn username_problem(u: &str) -> Option<&'static str> {
    let ok = (3..=32).contains(&u.len()) && u.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b));
    (!ok).then_some("Kullanıcı adı 3-32 karakter olmalı; yalnızca küçük harf, rakam, nokta, alt çizgi, tire.")
}

pub struct Hesaplar {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Hesaplar {
    pub fn new(path: impl Into<PathBuf>) -> Hesaplar {
        Hesaplar { path: path.into(), lock: Mutex::new(()) }
    }

    /// Dosya yoksa boş. Bozuksa hata: boş sayılsaydı kurulum ekranı açılır, cihaz ele geçirilebilirdi.
    pub fn load(&self) -> Result<BTreeMap<String, Hesap>, String> {
        match std::fs::read(&self.path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(format!("hesap dosyası okunamadı: {e}")),
            Ok(b) => serde_json::from_slice(&b).map_err(|e| format!("hesap dosyası bozuk: {e}")),
        }
    }

    /// Kurulum gerekli mi (dosya yok ya da boş). Bozuk dosya kurulum açmaz.
    pub fn needs_setup(&self) -> bool {
        self.load().map(|m| m.is_empty()).unwrap_or(false)
    }

    fn save(&self, m: &BTreeMap<String, Hesap>) -> Result<(), String> {
        let data = serde_json::to_vec_pretty(m).map_err(|e| e.to_string())?;
        ortak::write_atomic(&self.path, &data).map_err(|e| format!("hesap dosyası yazılamadı: {e}"))
    }

    fn new_hesap(rol: Rol, pw: &str) -> Hesap {
        let tuz = ortak::random_hex(16);
        Hesap { rol, ozet: digest(pw, &tuz, ITER), tuz, yineleme: ITER }
    }

    /// Doğruysa rol. Bilinmeyen kullanıcıda da özet hesaplanır (süre farkından kullanıcı adı anlaşılmasın).
    pub fn verify(&self, user: &str, pw: &str) -> Option<Rol> {
        let m = self.load().ok()?;
        match m.get(user) {
            Some(h) => ct_eq(&digest(pw, &h.tuz, h.yineleme), &h.ozet).then_some(h.rol),
            None => {
                let _ = digest(pw, DUMMY_SALT, ITER);
                None
            }
        }
    }

    /// İlk kurulum: yalnızca hiç hesap yokken.
    pub fn setup(&self, hizmet: (&str, &str), sahip: (&str, &str)) -> Result<(), String> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        if !self.load()?.is_empty() {
            return Err("Kurulum zaten yapılmış.".into());
        }
        if hizmet.0 == sahip.0 {
            return Err("İki hesabın kullanıcı adı farklı olmalı.".into());
        }
        let mut m = BTreeMap::new();
        m.insert(hizmet.0.to_string(), Self::new_hesap(Rol::Hizmet, hizmet.1));
        m.insert(sahip.0.to_string(), Self::new_hesap(Rol::Sahip, sahip.1));
        self.save(&m)
    }

    pub fn add(&self, user: &str, rol: Rol, pw: &str) -> Result<(), String> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut m = self.load()?;
        if m.contains_key(user) {
            return Err("Bu kullanıcı adı zaten var.".into());
        }
        m.insert(user.into(), Self::new_hesap(rol, pw));
        self.save(&m)
    }

    pub fn set_password(&self, user: &str, pw: &str) -> Result<(), String> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut m = self.load()?;
        let rol = m.get(user).ok_or("Kullanıcı bulunamadı.")?.rol;
        m.insert(user.into(), Self::new_hesap(rol, pw));
        self.save(&m)
    }

    /// Son hizmet sağlayıcı hesabı silinemez (cihaz yönetilemez kalır).
    pub fn remove(&self, user: &str) -> Result<(), String> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut m = self.load()?;
        let h = m.get(user).ok_or("Kullanıcı bulunamadı.")?;
        if h.rol == Rol::Hizmet && m.values().filter(|x| x.rol == Rol::Hizmet).count() == 1 {
            return Err("Son hizmet sağlayıcı hesabı silinemez.".into());
        }
        m.remove(user);
        self.save(&m)
    }
}

/// Giriş kilidi: 15 dk içinde 5 hata → IP de kullanıcı adı da kilitlenir.
#[derive(Default)]
pub struct LoginGuard(Mutex<HashMap<String, VecDeque<f64>>>);

impl LoginGuard {
    const LIMIT: usize = 5;
    const WINDOW: f64 = 900.0;

    fn count(m: &mut HashMap<String, VecDeque<f64>>, key: &str, now: f64) -> usize {
        let q = m.entry(key.to_string()).or_default();
        while q.front().is_some_and(|&t| t <= now - Self::WINDOW) {
            q.pop_front();
        }
        q.len()
    }

    pub fn allowed(&self, ip: &str, user: &str, now: f64) -> bool {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        Self::count(&mut m, &format!("ip:{ip}"), now) < Self::LIMIT && Self::count(&mut m, &format!("u:{user}"), now) < Self::LIMIT
    }

    pub fn failed(&self, ip: &str, user: &str, now: f64) {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        for k in [format!("ip:{ip}"), format!("u:{user}")] {
            m.entry(k).or_default().push_back(now);
        }
    }

    pub fn succeeded(&self, ip: &str, user: &str) {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        m.remove(&format!("ip:{ip}"));
        m.remove(&format!("u:{user}"));
    }
}

#[derive(Clone, Debug)]
pub struct Oturum {
    pub user: String,
    pub rol: Rol,
    pub csrf: String,
    expires: f64,
}

/// Bellekte oturumlar (panel yeniden başlayınca herkes yeniden girer). 12 saat geçerli.
#[derive(Default)]
pub struct Oturumlar(Mutex<HashMap<String, Oturum>>);

impl Oturumlar {
    const TTL: f64 = 12.0 * 3600.0;

    pub fn create(&self, user: &str, rol: Rol, now: f64) -> String {
        let token = ortak::random_hex(32);
        let o = Oturum { user: user.into(), rol, csrf: ortak::random_hex(16), expires: now + Self::TTL };
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        m.retain(|_, v| v.expires > now);
        m.insert(token.clone(), o);
        token
    }

    pub fn get(&self, token: &str, now: f64) -> Option<Oturum> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).get(token).filter(|o| o.expires > now).cloned()
    }

    pub fn remove(&self, token: &str) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).remove(token);
    }

    /// Parola değişince / hesap silinince o kullanıcının bütün oturumları kapanır (`keep` hariç).
    pub fn remove_user(&self, user: &str, keep: Option<&str>) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).retain(|t, o| o.user != user || Some(t.as_str()) == keep);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn tmp() -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!("wfc-hesap-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&d);
        d.join("hesaplar.json")
    }

    #[test]
    fn setup_verify_and_manage() {
        let h = Hesaplar::new(tmp());
        assert!(h.needs_setup());
        h.setup(("goztepe", "hizmet-parola-1"), ("mudur", "sahip-parola-12")).unwrap();
        assert!(!h.needs_setup());
        assert!(h.setup(("a", "b"), ("c", "d")).is_err()); // ikinci kurulum yok
        assert_eq!(h.verify("goztepe", "hizmet-parola-1"), Some(Rol::Hizmet));
        assert_eq!(h.verify("mudur", "sahip-parola-12"), Some(Rol::Sahip));
        assert_eq!(h.verify("mudur", "yanlis-parola"), None);
        assert_eq!(h.verify("yok", "sahip-parola-12"), None);
        h.set_password("mudur", "yeni-parola-123").unwrap();
        assert_eq!(h.verify("mudur", "yeni-parola-123"), Some(Rol::Sahip));
        assert!(h.remove("goztepe").is_err()); // son hizmet sağlayıcı
        h.add("teknik", Rol::Hizmet, "teknik-parola-1").unwrap();
        h.remove("goztepe").unwrap();
        assert!(h.add("teknik", Rol::Sahip, "x".repeat(12).as_str()).is_err());
        let raw = std::fs::read_to_string(&h.path).unwrap();
        assert!(!raw.contains("yeni-parola-123")); // parola düz metin saklanmaz
    }

    #[test]
    fn corrupt_file_does_not_open_setup() {
        let p = tmp();
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "{bozuk").unwrap();
        let h = Hesaplar::new(&p);
        assert!(!h.needs_setup());
        assert!(h.setup(("goztepe", "hizmet-parola-1"), ("mudur", "sahip-parola-12")).is_err());
        assert_eq!(h.verify("goztepe", "hizmet-parola-1"), None);
    }

    #[test]
    fn rules_guard_sessions() {
        assert!(password_problem("kisa").is_some() && password_problem("onkarakter").is_none());
        assert!(username_problem("Mudur").is_some() && username_problem("ab").is_some() && username_problem("mudur.1").is_none());
        let g = LoginGuard::default();
        for _ in 0..5 {
            assert!(g.allowed("1.1.1.1", "mudur", 10.0));
            g.failed("1.1.1.1", "mudur", 10.0);
        }
        assert!(!g.allowed("1.1.1.1", "baska", 10.0)); // IP kilitli
        assert!(!g.allowed("2.2.2.2", "mudur", 10.0)); // kullanıcı kilitli
        assert!(g.allowed("1.1.1.1", "mudur", 10.0 + 901.0)); // 15 dk sonra açılır
        let s = Oturumlar::default();
        let t1 = s.create("mudur", Rol::Sahip, 0.0);
        let t2 = s.create("mudur", Rol::Sahip, 0.0);
        assert_eq!(s.get(&t1, 1.0).unwrap().rol, Rol::Sahip);
        assert!(s.get(&t1, 13.0 * 3600.0).is_none()); // süresi doldu
        s.remove_user("mudur", Some(&t2));
        assert!(s.get(&t1, 1.0).is_none() && s.get(&t2, 1.0).is_some());
    }
}
