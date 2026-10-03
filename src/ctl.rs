//! Yönetim komutları (eski ctl.py). Bu adımda: dhcp-olay, yukle, oturumlar. Diğerleri (gun-kapat, yedekle, ara...) sonraki adımlarda.

use crate::ayar::Config;
use crate::ortak::{self, Row, Runner};
use std::process::ExitCode;

/// dnsmasq `dhcp-script` çağrısı: `<add|old|del> <mac> <ip> [hostname]` (Debian dnsmasq biçimi).
/// 5651 iç IP dağıtım kaydını (`dhcp.csv`) yazar; atama/yenilemede kayıtlı cihazın oturumunu yeni IP'ye taşır.
pub fn dhcp_olay(cfg: &Config, action: &str, mac: &str, ip: &str, host: &str, now: f64, runner: &Runner) -> &'static str {
    let olay = match action {
        "add" => "DHCP_ATAMA",
        "old" => "DHCP_YENILEME",
        "del" => "DHCP_BIRAKMA",
        _ => return "yok sayildi", // tftp, init vb.
    };
    let (Some(mac), true) = (ortak::norm_mac(mac), ortak::in_subnet(ip, &cfg.main.subnet)) else { return "gecersiz" };
    let host: String = host.chars().filter(|c| c.is_ascii_alphanumeric() || "._-".contains(*c)).take(63).collect();
    let zaman = ortak::now_iso(now);
    let row = Row::new(olay, &zaman).set("mac", &mac).set("ic_ip", ip).set("alan_adi", host);
    let path = ortak::day_file(&cfg.main.log_root, ortak::day_of(&zaman), "dhcp.csv");
    if let Err(e) = ortak::append_rows(&path, &[row]) {
        eprintln!("dhcp-olay: {} yazılamadı: {e}", path.display());
    }
    if action == "del" {
        return "kaydedildi";
    }
    // Cihaz eski IP'sinde ısrar etmez; havuzdan ne aldıysa oturum oraya taşınır, IP'yi tutan başka oturum beklemeye alınır.
    let m = &cfg.main;
    let Ok(_g) = ortak::state_lock(&m.state_root) else { return "kilit hatasi" };
    let mut ses = ortak::load_sessions(&m.state_root);
    let movable = ses.get(&mac).is_some_and(|s| s.expires_epoch > now && s.ip != ip);
    let result = if movable {
        if !ortak::move_session(&m.log_root, &mut ses, &mac, ip, now, runner) {
            return "nft hatasi";
        }
        "tasindi"
    } else if ses.iter().any(|(k, v)| *k != mac && v.ip == ip) {
        ortak::park_ip_holders(&m.log_root, &mut ses, &mac, ip, now, runner);
        "bosaltildi"
    } else {
        return "degisiklik yok";
    };
    if let Err(e) = ortak::save_sessions(&m.state_root, &ses) {
        eprintln!("dhcp-olay: oturum dosyası yazılamadı: {e}");
    }
    result
}

/// Açılışta / güvenlik duvarı yeniden yüklenince: izinli ve yasaklı kümeleri doldurur, süresi dolmamış oturumları
/// kalan süreyle nft'ye geri ekler (müşteri tekrar SMS istemez), süresi dolanları kapatır.
pub fn yukle(cfg: &Config, now: f64, runner: &Runner) -> usize {
    for mac in Config::devices(&cfg.allow).keys() {
        ortak::run_opt(runner, ortak::nft_set_cmd("add", "allow_mac", mac));
    }
    for mac in Config::devices(&cfg.ban).keys() {
        ortak::run_opt(runner, ortak::nft_set_cmd("add", "ban_mac", mac));
    }
    let m = &cfg.main;
    let Ok(_g) = ortak::state_lock(&m.state_root) else { return 0 };
    let mut ses = ortak::load_sessions(&m.state_root);
    let mut restored = 0;
    for mac in ses.keys().cloned().collect::<Vec<_>>() {
        let s = ses[&mac].clone();
        let left = s.expires_epoch - now;
        if left <= 5.0 {
            ortak::close_session(&m.log_root, &mut ses, &mac, "sure_doldu", now, runner);
        } else if s.ip.is_empty() {
            continue; // IP'siz beklemede: cihaz dönünce DHCP olayı yeni IP'ye taşır
        } else if ortak::run_opt(runner, ortak::nft_add_cmd(&mac, &s.ip, left)) {
            restored += 1;
        } else {
            ortak::close_session(&m.log_root, &mut ses, &mac, "yukleme_hatasi", now, runner);
        }
    }
    if let Err(e) = ortak::save_sessions(&m.state_root, &ses) {
        eprintln!("yukle: oturum dosyası yazılamadı: {e}");
    }
    restored
}

fn oturumlar(cfg: &Config, now: f64) {
    let ses = ortak::load_sessions(&cfg.main.state_root);
    if ses.is_empty() {
        println!("Aktif oturum yok");
        return;
    }
    println!("{:<11} {:<24} {:<17} {:<15} {:<16} KALAN", "TELEFON", "AD SOYAD", "MAC", "IP", "BASLANGIC");
    let mut list: Vec<_> = ses.iter().collect();
    list.sort_by(|a, b| a.1.start_epoch.total_cmp(&b.1.start_epoch));
    for (mac, s) in list {
        let left = ((s.expires_epoch - now).max(0.0) as u64) / 60;
        let name: String = format!("{} {}", s.ad, s.soyad).chars().take(24).collect();
        let start = s.start.get(..16).unwrap_or("").replace('T', " ");
        let ip = if s.ip.is_empty() { "(beklemede)" } else { &s.ip };
        println!("{:<11} {name:<24} {mac:<17} {ip:<15} {start:<16} {}g {}sa {}dk", s.phone, left / 1440, left / 60 % 24, left % 60);
    }
}

/// Parolayı ekrana yazdırmadan okur (terminal değilse düz satır: kurulum betikleri için).
fn read_secret(prompt: &str) -> String {
    use std::io::Write;
    eprint!("{prompt}");
    let _ = std::io::stderr().flush();
    // SAFETY: termios yalnızca stdin için okunur/yazılır, okuma bitince eski hali geri konur.
    let mut old: libc::termios = unsafe { std::mem::zeroed() };
    let tty = unsafe { libc::tcgetattr(0, &mut old) } == 0;
    if tty {
        let mut t = old;
        t.c_lflag &= !libc::ECHO;
        unsafe { libc::tcsetattr(0, libc::TCSANOW, &t) };
    }
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    if tty {
        unsafe { libc::tcsetattr(0, libc::TCSANOW, &old) };
        eprintln!();
    }
    line.trim_end_matches(['\r', '\n']).to_string()
}

/// `admin` (hizmet sağlayıcı) parolası: yalnızca cihaz konsolundan / SSH'tan, root olarak.
fn admin_parola(path: &str) -> ExitCode {
    let pw = read_secret("Yeni admin parolası: ");
    if let Some(e) = crate::hesap::password_problem(&pw) {
        eprintln!("{e}");
        return ExitCode::from(1);
    }
    if read_secret("Tekrar: ") != pw {
        eprintln!("Parolalar aynı değil.");
        return ExitCode::from(1);
    }
    match crate::hesap::Hesaplar::new(path).set_admin(&pw) {
        Ok(()) => {
            println!("admin parolası kaydedildi.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

pub fn run(cfg: Config, args: &[String]) -> ExitCode {
    let runner = |c: &[String]| ortak::run(c);
    let now = ortak::wall();
    match args.first().map(String::as_str) {
        Some("dhcp-olay") if args.len() >= 4 => {
            let host = args.get(4).map(String::as_str).unwrap_or("");
            println!("{}", dhcp_olay(&cfg, &args[1], &args[2], &args[3], host, now, &runner));
            ExitCode::SUCCESS
        }
        Some("yukle") => {
            println!("{} oturum geri yüklendi", yukle(&cfg, now, &runner));
            ExitCode::SUCCESS
        }
        Some("oturumlar") => {
            oturumlar(&cfg, now);
            ExitCode::SUCCESS
        }
        Some("gun-kapat") => {
            let day = args.iter().skip(1).find(|a| !a.starts_with("--")).map(String::as_str);
            let out = crate::muhur::gun_kapat(&cfg, day, now, args.iter().any(|a| a == "--zorla"));
            println!("{}", if out.is_empty() { "Kapatılacak gün yok".to_string() } else { out.join("\n") });
            if out.iter().any(|l| l.contains("HATA")) { ExitCode::from(1) } else { ExitCode::SUCCESS }
        }
        Some("dogrula") => {
            let last = args.iter().position(|a| a == "--son").and_then(|i| args.get(i + 1)).and_then(|n| n.parse().ok());
            let r = crate::muhur::verify(&cfg, last);
            println!("{}", crate::muhur::format_verify(&r));
            if r.iter().any(|(_, p, _)| !p.is_empty()) { ExitCode::from(1) } else { ExitCode::SUCCESS }
        }
        Some("temizle") => {
            let out = crate::muhur::prune(&cfg, now, args.iter().any(|a| a == "--kuru"));
            println!("{}", if out.is_empty() { "Silinecek kayıt yok".to_string() } else { out.join("\n") });
            ExitCode::SUCCESS
        }
        Some("admin-parola") => admin_parola(crate::hesap::PATH),
        Some(c @ ("ag-uygula" | "ag-gecis" | "ag-onayla" | "ag-geri-al" | "ag-acilis")) => {
            use crate::ag;
            let y = ag::Yollar::sistem();
            let report = |r: Result<Vec<String>, String>| match r {
                Ok(errs) if errs.is_empty() => ExitCode::SUCCESS,
                Ok(errs) => {
                    eprintln!("{}", errs.join("; "));
                    ExitCode::from(1)
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(1)
                }
            };
            match c {
                // Kurulumda / elle: ag.toml'dan dosyaları üret ve ağı geçir (geri alma zamanlayıcısı yok)
                "ag-uygula" => report(Ok(ag::switch(&cfg, &ag::load(&y.ag), &y, &runner))),
                "ag-gecis" => match args.get(1) {
                    Some(f) => report(ag::gecis(&cfg, &y, std::path::Path::new(f), now, &runner)),
                    None => report(Err("Kullanım: wificorrect ctl ag-gecis <yeni-ag.toml>".into())),
                },
                "ag-onayla" => report(ag::onayla(&cfg, &y, &runner).map(|_| vec![])),
                "ag-geri-al" => report(ag::geri_al(&cfg, &y, args.get(1).map_or("elle", String::as_str), &runner)),
                _ => report(ag::geri_al(&cfg, &y, "acilis", &runner)),
            }
        }
        Some("ara") => {
            let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
            let Some((tur, deger)) = ["ic-ip", "nat-port", "hedef-ip", "telefon", "mac"].iter().find_map(|t| opt(&format!("--{t}")).map(|v| (*t, v))) else {
                eprintln!("Kullanım: wificorrect ctl ara (--ic-ip IP | --nat-port PORT | --hedef-ip IP) --zaman \"2026-09-24 14:30\" [--tolerans SN]");
                eprintln!("          wificorrect ctl ara --telefon NUMARA | --mac MAC [--zaman ...]");
                return ExitCode::from(2);
            };
            let tol = opt("--tolerans").and_then(|v| v.parse().ok()).unwrap_or(120.0);
            match crate::kayit::ara(&cfg, tur, deger, opt("--zaman").unwrap_or(""), tol, now) {
                Ok(sections) => {
                    for (title, rows) in sections {
                        println!("{title}:\n{}\n", crate::kayit::format_rows(&rows));
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(2)
                }
            }
        }
        Some("disa-aktar") => {
            let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
            let (Some(bas), Some(bit), Some(out)) = (opt("--baslangic"), opt("--bitis"), opt("--cikti")) else {
                eprintln!("Kullanım: wificorrect ctl disa-aktar --baslangic YYYY-AA-GG --bitis YYYY-AA-GG --cikti /tmp/talep.tar");
                return ExitCode::from(2);
            };
            match crate::kayit::talep_paketi(&cfg, bas, bit, std::path::Path::new(out)) {
                Ok(n) => {
                    println!("Paket: {out} ({n} gün)");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(1)
                }
            }
        }
        Some("yedekle") => {
            // ponytail: bir gün 1 saatte gitmezse YEDEK_HATA; ertesi gece kaldığı yerden devam eder
            let msg = crate::muhur::backup(&cfg, now, &|c: &[String]| ortak::run_timeout(c, 3600));
            println!("{msg}");
            if msg.contains("HATA") { ExitCode::from(1) } else { ExitCode::SUCCESS }
        }
        _ => {
            eprintln!(
                "Kullanım: wificorrect ctl <komut>\n  dhcp-olay <add|old|del> <mac> <ip> [ad]\n  yukle\n  oturumlar\n  \
                 gun-kapat [YYYY-AA-GG] [--zorla]\n  dogrula [--son N]\n  temizle [--kuru]\n  yedekle\n  ara ...  (ayrıntı: ctl ara)\n  disa-aktar --baslangic G --bitis G --cikti DOSYA\n  ag-uygula | ag-gecis DOSYA | ag-onayla | ag-geri-al\n  admin-parola"
            );
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    const M1: &str = "aa:bb:cc:dd:ee:01";
    const M2: &str = "aa:bb:cc:dd:ee:02";

    fn cfg() -> (Config, std::path::PathBuf) {
        static N: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!("wfc-ctl-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&root);
        let text = format!(
            "[main]\nlog_root = '{0}/5651'\nstate_root = '{0}/state'\n[[allow]]\nmac = 'aa:bb:cc:dd:ee:99'\n[[ban]]\nmac = 'aa:bb:cc:dd:ee:66'\n",
            root.display()
        );
        (Config::parse(&text).unwrap(), root)
    }

    fn sess(ip: &str, expires: f64) -> ortak::Session {
        ortak::Session {
            phone: "5334553132".into(),
            ad: "A".into(),
            soyad: "B".into(),
            ip: ip.into(),
            session_id: "s1".into(),
            start: ortak::now_iso(1000.0),
            start_epoch: 1000.0,
            expires_epoch: expires,
        }
    }

    const NOW: f64 = 1_790_705_134.0;

    #[test]
    fn dhcp_event_logs_and_moves_session() {
        let (c, root) = cfg();
        let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let c2 = calls.clone();
        let runner = move |x: &[String]| {
            c2.lock().unwrap().push(x.to_vec());
            true
        };
        let mut ses = ortak::Sessions::new();
        ses.insert(M1.into(), sess("10.50.0.23", NOW + 3600.0));
        ses.insert(M2.into(), sess("10.50.0.30", NOW + 3600.0));
        ortak::save_sessions(&c.main.state_root, &ses).unwrap();
        assert_eq!(dhcp_olay(&c, "add", "AA:BB:CC:DD:EE:01", "10.50.0.30", "iPhone;=x", NOW, &runner), "tasindi");
        let ses = ortak::load_sessions(&c.main.state_root);
        assert_eq!((ses[M1].ip.as_str(), ses[M2].ip.as_str()), ("10.50.0.30", "")); // eski sahibi beklemeye
        let dhcp = std::fs::read_to_string(root.join("5651/gunluk/2026-09-29/dhcp.csv")).unwrap();
        assert!(dhcp.contains("2026-09-29T21:05:34+03:00;DHCP_ATAMA;;;;aa:bb:cc:dd:ee:01;10.50.0.30;;;;;;;iPhonex;"));
        assert!(calls.lock().unwrap().iter().any(|c| c[1] == "add" && c[6].contains("10.50.0.30")));
        assert_eq!(dhcp_olay(&c, "old", M1, "10.50.0.30", "", NOW, &runner), "degisiklik yok");
        assert_eq!(dhcp_olay(&c, "del", M1, "10.50.0.30", "", NOW, &runner), "kaydedildi");
        assert_eq!(dhcp_olay(&c, "add", "bozuk", "10.50.0.30", "", NOW, &runner), "gecersiz");
        assert_eq!(dhcp_olay(&c, "add", M1, "192.168.1.5", "", NOW, &runner), "gecersiz");
        assert_eq!(dhcp_olay(&c, "tftp", M1, "10.50.0.30", "", NOW, &runner), "yok sayildi");
        let dhcp = std::fs::read_to_string(root.join("5651/gunluk/2026-09-29/dhcp.csv")).unwrap();
        assert!(dhcp.contains(";DHCP_YENILEME;") && dhcp.contains(";DHCP_BIRAKMA;"));
    }

    #[test]
    fn restore_sessions_and_sets() {
        let (c, _root) = cfg();
        let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let c2 = calls.clone();
        let runner = move |x: &[String]| {
            c2.lock().unwrap().push(x.to_vec());
            !x.iter().any(|a| a.contains("10.50.0.99"))
        };
        let mut ses = ortak::Sessions::new();
        ses.insert("aa:bb:cc:dd:ee:01".into(), sess("10.50.0.21", NOW + 7200.0)); // geri yüklenir
        ses.insert("aa:bb:cc:dd:ee:02".into(), sess("10.50.0.22", NOW + 3.0)); // süresi doldu
        ses.insert("aa:bb:cc:dd:ee:03".into(), sess("", NOW + 7200.0)); // IP'siz beklemede
        ses.insert("aa:bb:cc:dd:ee:04".into(), sess("10.50.0.99", NOW + 7200.0)); // nft hatası
        ortak::save_sessions(&c.main.state_root, &ses).unwrap();
        assert_eq!(yukle(&c, NOW, &runner), 1);
        let ses = ortak::load_sessions(&c.main.state_root);
        assert_eq!(ses.keys().cloned().collect::<Vec<_>>(), vec!["aa:bb:cc:dd:ee:01", "aa:bb:cc:dd:ee:03"]);
        let calls = calls.lock().unwrap();
        assert!(calls.iter().any(|c| c[5] == "allow_mac" && c[6] == "{ aa:bb:cc:dd:ee:99 }"));
        assert!(calls.iter().any(|c| c[5] == "ban_mac" && c[6] == "{ aa:bb:cc:dd:ee:66 }"));
        assert!(calls.iter().any(|c| c[6] == "{ aa:bb:cc:dd:ee:01 . 10.50.0.21 timeout 7200s }"));
    }
}
