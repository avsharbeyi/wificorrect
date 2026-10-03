//! Fabrika ayarlarına dönüş (2026-10-03, kullanıcı isteği; panel → Admin ayarları): cihaz ISO'dan kurulduktan hemen
//! sonraki haline döner — ayarlar ürün varsayılanı, admin dışındaki hesaplar silinir (açılışta kurulum ekranı gelir),
//! portlar varsayılan (Ethernet 1 internet alır, Ethernet 2 verir, Wi-Fi kapalı), yasaklı listeler boş, açık oturumlar kapanır.
//! 5651 kayıtları (kullanıcı kararı): bugün dahil bütün günler mühürlenip uzak sunucuya gönderilir, ancak hepsi gönderildiyse
//! cihazdan silinir (işletme sahibi kayıtlarını sunucudaki panelden görmeye devam eder). Uzak yedek kapalıysa ya da bir gün
//! gönderilemezse işlem İPTAL olur: kayıt silinmez, ayarlara dokunulmaz.

use crate::ag::{self, Ag, Yollar};
use crate::ayar::Config;
use crate::hesap::Hesaplar;
use crate::ortak::{self, Row, Runner};
use std::path::Path;

fn cmd(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

fn audit(root: &str, olay: &str, now: f64, ek: &str) {
    ortak::audit(root, Row::new(olay, &ortak::now_iso(now)).set("ek", ek));
}

/// Fabrika dönüşü yapılabilir mi (panel düğmeye basılınca da sorar).
pub fn engel(cfg: &Config) -> Option<&'static str> {
    (!cfg.backup.enabled || cfg.backup.target.trim().is_empty())
        .then_some("Uzak yedek kapalı: kayıtlar sunucuya gönderilmeden fabrika ayarlarına dönülmez. Önce Admin ayarları'nda uzak yedeği açın.")
}

/// Kayıtları mühürle, gönder, sonra yerelden sil. Hata: hiçbir kayıt silinmedi.
fn kayitlari_teslim_et(cfg: &Config, now: f64, runner: &Runner) -> Result<(), String> {
    let root = &cfg.main.log_root;
    let g = Path::new(root).join("gunluk");
    for day in crate::muhur::days(root) {
        if !g.join(&day).join("MANIFEST.sha256").exists() {
            crate::muhur::close_day(cfg, &day, now, true).map_err(|e| format!("{day} mühürlenemedi: {e}"))?;
        }
    }
    crate::muhur::backup(cfg, now, runner)?;
    let unsent: Vec<String> = crate::muhur::days(root).into_iter().filter(|d| !g.join(d).join(".yedeklendi").exists()).collect();
    if !unsent.is_empty() {
        return Err(format!("gönderilmemiş gün: {}", unsent.join(", ")));
    }
    // ponytail: gönderimden sonra yazılan denetim satırları (YEDEK) da silinir; asıl kayıtlar sunucuda
    for day in crate::muhur::days(root) {
        std::fs::remove_dir_all(g.join(&day)).map_err(|e| format!("{day} silinemedi: {e}"))?;
    }
    for f in ["zincir.txt", "kullanicilar/index.csv"] {
        let _ = std::fs::remove_file(Path::new(root).join(f));
    }
    Ok(())
}

pub fn fabrika(cfg: &Config, cfg_path: &str, hesap_path: &str, filtre_conf: &Path, y: &Yollar, now: f64, runner: &Runner) -> Result<Vec<String>, String> {
    let m = &cfg.main;
    if let Some(e) = engel(cfg) {
        audit(&m.log_root, "FABRIKA_IPTAL", now, "neden=yedek_kapali");
        return Err(e.into());
    }
    // 1) kayıt yazanlar durur; açık müşteri oturumları kapanır (OTURUM_BITIS neden=fabrika, gönderilecek kayda girer)
    runner(&cmd(&["systemctl", "stop", "wificorrect-portal", "wificorrect-kaydedici", "dnsmasq"]));
    if let Ok(_g) = ortak::state_lock(&m.state_root) {
        let mut ses = ortak::load_sessions(&m.state_root);
        let macs: Vec<String> = ses.keys().cloned().collect();
        for mac in macs {
            ortak::close_session(&m.log_root, &mut ses, &mac, "fabrika", now, runner);
        }
        let _ = ortak::save_sessions(&m.state_root, &ses);
    }
    // 2) kayıtlar sunucuya; olmazsa iptal (servisler geri açılır, ayarlara dokunulmaz)
    if let Err(e) = kayitlari_teslim_et(cfg, now, runner) {
        runner(&cmd(&["systemctl", "start", "dnsmasq", "wificorrect-kaydedici", "wificorrect-portal"]));
        audit(&m.log_root, "FABRIKA_IPTAL", now, &format!("neden={e}"));
        return Err(format!("Fabrika ayarlarına dönülmedi, kayıtlar sunucuya gönderilemedi: {e}"));
    }
    let mut errs = vec![];
    // 3) yalnızca admin kalır → panel kurulum ekranını açar
    if let Err(e) = Hesaplar::new(hesap_path).keep_only_admin() {
        errs.push(e);
    }
    // 4) ayarlar ürün varsayılanı; sistem yolları ve müşteri ağı adresleri kullanıcı ayarı değil, korunur
    let mut fresh = Config::default();
    for (dst, src) in [
        (&mut fresh.main.log_root, &m.log_root),
        (&mut fresh.main.state_root, &m.state_root),
        (&mut fresh.main.leases_file, &m.leases_file),
        (&mut fresh.main.iface, &m.iface),
        (&mut fresh.main.router_ip, &m.router_ip),
        (&mut fresh.main.subnet, &m.subnet),
    ] {
        dst.clone_from(src);
    }
    fresh.main.portal_port = m.portal_port;
    if let Err(e) = fresh.save(cfg_path) {
        errs.push(e);
    }
    // 5) yasaklı site / kelime listeleri boş
    if let Err(e) = crate::filtre::uygula(&fresh, filtre_conf, runner) {
        errs.push(e);
    }
    // 6) portlar varsayılan; bekleyen port değişikliği ve yedeği silinir
    for p in [&y.ag, &y.yeni()] {
        let _ = std::fs::remove_file(p);
    }
    for f in ["ag-bekliyor", "ag-yedek.toml"] {
        let _ = std::fs::remove_file(y.durum.join(f));
    }
    errs.extend(ag::switch(&fresh, &Ag::default(), y, runner));
    // 7) servisler yeni ayarla (ağ geçişi dnsmasq'ı zaten başlattı)
    if !runner(&cmd(&["systemctl", "restart", "wificorrect-portal", "wificorrect-kaydedici", "wificorrect-panel"])) {
        errs.push("servisler yeniden başlatılamadı".into());
    }
    let ek = if errs.is_empty() { String::new() } else { format!("hata={}", errs.join(",")) };
    audit(&fresh.main.log_root, "FABRIKA_AYARI", now, &ek); // yeni işletmenin kaydının ilk satırı
    Ok(errs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hesap::{Rol, ADMIN};
    use std::sync::{Arc, Mutex};

    #[test]
    fn factory_reset_returns_to_fresh_install() {
        let root = std::env::temp_dir().join(format!("wfc-fabrika-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        // eskimiş cihaz: işletme ayarlı, SMS bilgisi girilmiş, sahip hesabı ve açık oturum var, portlar değişmiş
        let cfg_path = root.join("ayarlar.toml");
        let text = format!(
            "[main]\nsite_name = 'Bocafe'\nunvan = 'Boca Gıda Ltd.'\nlog_root = '{0}/5651'\nstate_root = '{0}/state'\n[netgsm]\npassword = 'gizli'\n[backup]\nenabled = true\ntarget = 'kafe-x@sunucu:'\n[filtre]\nkelimeler = ['bet']\n",
            root.display()
        );
        std::fs::write(&cfg_path, &text).unwrap();
        let cfg = Config::parse(&text).unwrap();
        let h = Hesaplar::new(root.join("hesaplar.json"));
        h.set_admin("admin-parola-123").unwrap();
        h.setup("mudur", "sahip-parola-12").unwrap();
        let mut ses = ortak::Sessions::new();
        ses.insert("aa:bb:cc:dd:ee:01".into(), ortak::Session {
            phone: "905334553132".into(), ad: "Ayşe".into(), soyad: "Yılmaz".into(), ip: "10.50.0.23".into(),
            session_id: "s1".into(), start: "2026-10-03T10:00:00+03:00".into(), start_epoch: 1.0, expires_epoch: 9e9,
        });
        std::fs::create_dir_all(root.join("state")).unwrap();
        ortak::save_sessions(&cfg.main.state_root, &ses).unwrap();
        let y = Yollar {
            ag: root.join("ag.toml"),
            interfaces: root.join("interfaces"),
            nft: root.join("arayuzler.nft"),
            hostapd: root.join("hostapd.conf"),
            issue: root.join("issue"),
            durum: root.join("durum"),
        };
        ag::save(&y.ag, &Ag { wan: "enp1s0".into(), lan: vec!["enp3s0".into()], ..Ag::default() }).unwrap();
        let calls: Arc<Mutex<Vec<String>>> = Arc::default();
        let c2 = calls.clone();
        let runner = move |c: &[String]| {
            c2.lock().unwrap().push(c.join(" "));
            true
        };
        // eski bir gün ve bugünün açık kaydı
        ortak::append_rows(&ortak::day_file(&cfg.main.log_root, "2026-10-01", "dhcp.csv"), &[Row::new("DHCP_ATAMA", "2026-10-01T10:00:00+03:00")]).unwrap();
        // sunucuya gönderilemezse iptal: hiçbir şey silinmez, ayarlar aynı, servisler geri açılır
        let fail = |c: &[String]| c[0] != "rsync";
        let r = fabrika(&cfg, &cfg_path.to_string_lossy(), &root.join("hesaplar.json").to_string_lossy(), &root.join("yasak.conf"), &y, 1_791_000_000.0, &fail);
        assert!(r.unwrap_err().contains("gönderilemedi"));
        assert!(root.join("5651/gunluk/2026-10-01").exists() && h.load().unwrap().len() == 2);
        assert_eq!(Config::load(&cfg_path.to_string_lossy()).unwrap().main.site_name, "Bocafe");
        // gönderilince: kayıtlar mühürlü + yedeklendi işaretli olarak sunucuya, sonra cihazdan silinir
        let errs = fabrika(&cfg, &cfg_path.to_string_lossy(), &root.join("hesaplar.json").to_string_lossy(), &root.join("yasak.conf"), &y, 1_791_000_000.0, &runner).unwrap();
        assert!(errs.is_empty(), "{errs:?}");
        {
            let v = calls.lock().unwrap();
            assert!(v.iter().filter(|x| x.starts_with("rsync") && x.contains("gunluk/2026-10-0")).count() >= 2); // eski gün + bugün
            assert!(v.iter().any(|x| x.starts_with("rsync") && x.contains("zincir.txt")));
        }
        assert!(!root.join("5651/gunluk/2026-10-01").exists() && !root.join("5651/zincir.txt").exists());
        // ayarlar ürün varsayılanı
        let c = Config::load(&cfg_path.to_string_lossy()).unwrap();
        assert_eq!((c.main.site_name.as_str(), c.main.unvan.as_str(), c.netgsm.password.as_str(), c.backup.enabled, c.filtre.kelimeler.len()), ("İşletme", "", "", false, 0));
        // yalnızca admin kaldı → kurulum ekranı
        let m = h.load().unwrap();
        assert_eq!((m.len(), m.get(ADMIN).map(|x| x.rol)), (1, Some(Rol::Hizmet)));
        assert!(h.needs_setup() && h.verify(ADMIN, "admin-parola-123").is_some());
        // oturum kapandı (OTURUM_BITIS sunucuya giden kayda girdi)
        assert!(ortak::load_sessions(&root.join("state").to_string_lossy()).is_empty());
        // portlar varsayılan, ağ yeniden kuruldu
        assert!(std::fs::read_to_string(&y.nft).unwrap().contains("\"enp3s0\"") && !y.ag.exists());
        let v = calls.lock().unwrap();
        assert!(v.iter().any(|x| x == "systemctl restart ifup@enp3s0.service") && v.iter().any(|x| x.starts_with("systemctl restart wificorrect-portal")));
        // yeni işletmenin kaydı FABRIKA_AYARI ile başlar
        let denetim = std::fs::read_to_string(root.join("5651/gunluk/2026-10-03/denetim.csv")).unwrap();
        assert!(denetim.contains("FABRIKA_AYARI") && !denetim.contains("FABRIKA_IPTAL"));
        // yedek kapalıysa hiç başlamaz
        let mut kapali = Config::load(&cfg_path.to_string_lossy()).unwrap();
        kapali.backup.enabled = false;
        assert!(engel(&kapali).is_some());
        let _ = std::fs::remove_dir_all(&root);
    }
}
