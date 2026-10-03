//! Fabrika ayarlarına dönüş (2026-10-03, kullanıcı isteği; panel → Admin ayarları): cihaz ISO'dan kurulduktan hemen
//! sonraki haline döner — ayarlar ürün varsayılanı, admin dışındaki hesaplar silinir (açılışta kurulum ekranı gelir),
//! portlar varsayılan (Ethernet 1 internet alır, Ethernet 2 verir, Wi-Fi kapalı), yasaklı listeler boş, açık oturumlar kapanır.
//! 5651 kayıtları (/srv/5651) SİLİNMEZ: yasal delil, saklama süresi boyunca durmak zorunda.

use crate::ag::{self, Ag, Yollar};
use crate::ayar::Config;
use crate::hesap::Hesaplar;
use crate::ortak::{self, Row, Runner};
use std::path::Path;

pub fn fabrika(cfg: &Config, cfg_path: &str, hesap_path: &str, filtre_conf: &Path, y: &Yollar, now: f64, runner: &Runner) -> Vec<String> {
    let mut errs = vec![];
    // 1) açık müşteri oturumları kapanır (kayda OTURUM_BITIS neden=fabrika)
    let m = &cfg.main;
    match ortak::state_lock(&m.state_root) {
        Ok(_g) => {
            let mut ses = ortak::load_sessions(&m.state_root);
            let macs: Vec<String> = ses.keys().cloned().collect();
            for mac in macs {
                ortak::close_session(&m.log_root, &mut ses, &mac, "fabrika", now, runner);
            }
            if let Err(e) = ortak::save_sessions(&m.state_root, &ses) {
                errs.push(format!("oturumlar: {e}"));
            }
        }
        Err(e) => errs.push(format!("oturum kilidi: {e}")),
    }
    // 2) yalnızca admin kalır → panel kurulum ekranını açar
    if let Err(e) = Hesaplar::new(hesap_path).keep_only_admin() {
        errs.push(e);
    }
    // 3) ayarlar ürün varsayılanı (SMS bilgileri, işletme adı, metinler, yedek dahil)
    // sistem yolları ve müşteri ağı adresleri kullanıcı ayarı değil: korunur (ISO'dakiyle aynı)
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
    // 4) yasaklı site / kelime listeleri boş
    if let Err(e) = crate::filtre::uygula(&fresh, filtre_conf, runner) {
        errs.push(e);
    }
    // 5) portlar varsayılan; bekleyen port değişikliği ve yedeği silinir
    for p in [&y.ag, &y.yeni()] {
        let _ = std::fs::remove_file(p);
    }
    for f in ["ag-bekliyor", "ag-yedek.toml"] {
        let _ = std::fs::remove_file(y.durum.join(f));
    }
    errs.extend(ag::switch(&fresh, &Ag::default(), y, runner));
    // 6) servisler yeni ayarla
    if !runner(&["systemctl", "restart", "wificorrect-portal", "wificorrect-kaydedici", "wificorrect-panel"].map(String::from)) {
        errs.push("servisler yeniden başlatılamadı".into());
    }
    let ek = if errs.is_empty() { String::new() } else { format!("hata={}", errs.join(",")) };
    ortak::audit(&fresh.main.log_root, Row::new("FABRIKA_AYARI", &ortak::now_iso(now)).set("ek", ek));
    errs
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
        let errs = fabrika(&cfg, &cfg_path.to_string_lossy(), &root.join("hesaplar.json").to_string_lossy(), &root.join("yasak.conf"), &y, 1_791_000_000.0, &runner);
        assert!(errs.is_empty(), "{errs:?}");
        // ayarlar ürün varsayılanı
        let c = Config::load(&cfg_path.to_string_lossy()).unwrap();
        assert_eq!((c.main.site_name.as_str(), c.main.unvan.as_str(), c.netgsm.password.as_str(), c.backup.enabled, c.filtre.kelimeler.len()), ("İşletme", "", "", false, 0));
        // yalnızca admin kaldı → kurulum ekranı
        let m = h.load().unwrap();
        assert_eq!((m.len(), m.get(ADMIN).map(|x| x.rol)), (1, Some(Rol::Hizmet)));
        assert!(h.needs_setup() && h.verify(ADMIN, "admin-parola-123").is_some());
        // oturum kapandı, kayda yazıldı; 5651 kayıtları duruyor
        assert!(ortak::load_sessions(&root.join("state").to_string_lossy()).is_empty());
        let oturum = std::fs::read_to_string(root.join("5651/gunluk/2026-10-03/oturum.csv")).unwrap();
        assert!(oturum.contains("neden=fabrika"));
        // portlar varsayılan, ağ yeniden kuruldu
        assert!(std::fs::read_to_string(&y.nft).unwrap().contains("\"enp3s0\"") && !y.ag.exists());
        let v = calls.lock().unwrap();
        assert!(v.iter().any(|x| x == "ifup enp3s0") && v.iter().any(|x| x.starts_with("systemctl restart wificorrect-portal")));
        let denetim = std::fs::read_to_string(root.join("5651/gunluk/2026-10-03/denetim.csv")).unwrap();
        assert!(denetim.contains("FABRIKA_AYARI"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
