//! Kayıtları okuma, resmi talep araması ve talep paketi (eski ctl.py `ara` / `disa-aktar`, panel Loglar / Kullanıcılar / Resmi talep).
//! Yalnızca okur: mühürlü `.csv.gz` dosyalar açılıp okunur, hiçbir kayıt değiştirilmez.

use crate::ayar::Config;
use crate::ortak::{self, CSV_HEADER};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Bir CSV satırı (19 sütun, §14.1).
pub type Rec = Vec<String>;

pub fn col<'a>(r: &'a Rec, name: &str) -> &'a str {
    CSV_HEADER.iter().position(|k| *k == name).and_then(|i| r.get(i)).map_or("", String::as_str)
}

// ---------------------------------------------------------------- zaman
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    // Howard Hinnant; ortak::civil_from_days'in tersi
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// "2026-09-24 14:30", "2026-09-24 14:30:05", ISO "2026-09-24T14:30:05+03:00" ya da yalnızca gün → epoch (Türkiye saati).
pub fn parse_time(s: &str) -> Option<f64> {
    let s = s.trim();
    let num = |a: usize, b: usize| s.get(a..b).filter(|x| x.bytes().all(|c| c.is_ascii_digit())).and_then(|x| x.parse::<i64>().ok());
    if s.len() < 10 || s.as_bytes()[4] != b'-' || s.as_bytes()[7] != b'-' {
        return None;
    }
    let (y, m, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let (mut h, mut mi, mut se) = (0, 0, 0);
    if s.len() > 10 {
        if !matches!(s.as_bytes()[10], b' ' | b'T') || s.len() < 16 || s.as_bytes()[13] != b':' {
            return None;
        }
        h = num(11, 13)?;
        mi = num(14, 16)?;
        if s.len() >= 19 && s.as_bytes()[16] == b':' {
            se = num(17, 19)?;
        }
        if h > 23 || mi > 59 || se > 59 {
            return None;
        }
    }
    Some((days_from_civil(y, m as u32, d as u32) * 86400 + h * 3600 + mi * 60 + se - 3 * 3600) as f64)
}

/// t1..t2 arasındaki günler (her ikisi dahil).
pub fn days_range(t1: f64, t2: f64) -> Vec<String> {
    let mut out = vec![];
    let mut t = t1;
    while ortak::day_of(&ortak::now_iso(t)) <= ortak::day_of(&ortak::now_iso(t2)) {
        out.push(ortak::day_of(&ortak::now_iso(t)).to_string());
        t += 86400.0;
    }
    out
}

pub fn valid_day(s: &str) -> bool {
    s.len() == 10 && parse_time(s).is_some()
}

// ---------------------------------------------------------------- dosyalar
/// Gün klasöründe izin verilen dosya adı mı: `trafik.csv(.gz)`, `kullanicilar/905….csv(.gz)`. Başka yol yok (dışarı çıkılamaz).
pub fn safe_file(root: &str, day: &str, rel: &str) -> Option<PathBuf> {
    if !valid_day(day) {
        return None;
    }
    let base = rel.strip_suffix(".gz").unwrap_or(rel);
    let stem = base.strip_suffix(".csv")?;
    let ok = match stem.strip_prefix("kullanicilar/") {
        Some(phone) => (6..=15).contains(&phone.len()) && phone.bytes().all(|b| b.is_ascii_digit()),
        None => !stem.is_empty() && stem.bytes().all(|b| b.is_ascii_lowercase()),
    };
    let p = ortak::day_file(root, day, rel);
    (ok && p.is_file()).then_some(p)
}

/// Dosya metni (mühürlü `.gz` açılır), BOM'suz.
pub fn read_text(path: &Path) -> String {
    let mut out = String::new();
    let ok = match fs::File::open(path) {
        Ok(f) if path.extension().is_some_and(|e| e == "gz") => flate2::read::GzDecoder::new(f).read_to_string(&mut out).is_ok(),
        Ok(mut f) => f.read_to_string(&mut out).is_ok(),
        Err(_) => false,
    };
    if !ok {
        out.clear();
    }
    out.trim_start_matches('\u{feff}').to_string()
}

pub fn parse_csv(text: &str) -> Vec<Rec> {
    text.lines()
        .filter(|l| !l.is_empty() && !l.starts_with("zaman;"))
        .map(|l| {
            let mut v: Rec = l.split(';').map(String::from).collect();
            v.resize(CSV_HEADER.len(), String::new());
            v
        })
        .collect()
}

/// Günün bir dosyasının satırları: mühürlü `.gz` + (varsa) mühürden sonra düşmüş `.csv`.
pub fn day_rows(root: &str, day: &str, name: &str) -> Vec<Rec> {
    let p = ortak::day_file(root, day, name);
    let mut rows = parse_csv(&read_text(&PathBuf::from(format!("{}.gz", p.display()))));
    rows.extend(parse_csv(&read_text(&p)));
    rows
}

pub struct DayInfo {
    pub day: String,
    pub sealed: bool,
    pub backed_up: bool,
    /// (göreli yol, bayt)
    pub files: Vec<(String, u64)>,
}

pub fn day_info(root: &str, day: &str) -> Option<DayInfo> {
    let d = Path::new(root).join("gunluk").join(day);
    if !valid_day(day) || !d.is_dir() {
        return None;
    }
    let mut files: Vec<(String, u64)> = crate::muhur::day_files(&d, ".csv")
        .into_iter()
        .chain(crate::muhur::day_files(&d, ".csv.gz"))
        .map(|rel| {
            let size = fs::metadata(d.join(&rel)).map_or(0, |m| m.len());
            (rel, size)
        })
        .collect();
    files.sort();
    Some(DayInfo { day: day.into(), sealed: d.join("MANIFEST.sha256").exists(), backed_up: d.join(".yedeklendi").exists(), files })
}

/// Cihazdaki günler, yeniden eskiye.
pub fn days_desc(root: &str) -> Vec<String> {
    let mut v = crate::muhur::days(root);
    v.reverse();
    v.retain(|d| valid_day(d));
    v
}

/// Kişinin dosyası olan günler (eskiden yeniye) ve yolları.
pub fn user_days(root: &str, phone: &str) -> Vec<(String, String)> {
    crate::muhur::days(root)
        .into_iter()
        .filter_map(|day| {
            ["csv", "csv.gz"].iter().find_map(|ext| {
                let rel = format!("kullanicilar/{phone}.{ext}");
                safe_file(root, &day, &rel).map(|_| (day.clone(), rel))
            })
        })
        .collect()
}

// ---------------------------------------------------------------- arama
pub fn by_phone(root: &str, phone: &str) -> Vec<Rec> {
    user_days(root, phone)
        .into_iter()
        .flat_map(|(day, _)| day_rows(root, &day, &format!("kullanicilar/{phone}.csv")))
        .filter(|r| col(r, "olay").starts_with("OTURUM_") || col(r, "olay") == "KAYIT")
        .collect()
}

pub fn by_mac(root: &str, mac: &str, t1: f64, t2: f64) -> Vec<Rec> {
    let mut rows: Vec<Rec> = days_range(t1, t2)
        .iter()
        .flat_map(|day| ["dhcp.csv", "oturum.csv"].iter().flat_map(move |n| day_rows(root, day, n)))
        .filter(|r| col(r, "mac") == mac)
        .collect();
    rows.sort_by(|a, b| a[0].cmp(&b[0]));
    rows
}

/// t anında `ip`'yi tutan oturum(lar): oturumun t'ye kadarki son olayına bakılır (başlangıç, IP değişimi, bitiş).
pub fn sessions_at(cfg: &Config, ip: &str, t: f64) -> Vec<Rec> {
    let span = cfg.main.session_minutes as f64 * 60.0 + 86400.0;
    let mut by_id: std::collections::BTreeMap<String, Vec<(f64, Rec)>> = Default::default();
    for day in days_range(t - span, t) {
        for r in day_rows(&cfg.main.log_root, &day, "oturum.csv") {
            if let Some(ts) = parse_time(col(&r, "zaman")).filter(|&ts| ts <= t) {
                by_id.entry(col(&r, "oturum_id").to_string()).or_default().push((ts, r));
            }
        }
    }
    let mut out = vec![];
    for (_, mut ev) in by_id {
        ev.sort_by(|a, b| a.0.total_cmp(&b.0));
        let started = ev.iter().find(|(_, r)| col(r, "olay") == "OTURUM_BASLA").map(|(_, r)| r.clone());
        if let (Some((_, last)), Some(start)) = (ev.last(), started) {
            if col(last, "olay") != "OTURUM_BITIS" && col(last, "ic_ip") == ip {
                out.push(start);
            }
        }
    }
    out
}

/// t anına kadarki son DHCP kaydı (kira 2 saat; bir gün geriye bakılır).
pub fn dhcp_at(cfg: &Config, ip: &str, t: f64) -> Option<Rec> {
    days_range(t - 86400.0, t)
        .iter()
        .flat_map(|d| day_rows(&cfg.main.log_root, d, "dhcp.csv"))
        .filter(|r| col(r, "ic_ip") == ip && parse_time(col(r, "zaman")).is_some_and(|ts| ts <= t))
        .last()
}

/// `field` = değer olan ve [t-tol, t+tol] ile çakışan bağlantılar (`nat_port`, `hedef_ip`).
pub fn flows(cfg: &Config, field: &str, value: &str, t: f64, tol: f64) -> Vec<Rec> {
    days_range(t - tol - 86400.0, t + tol) // uzun süren akışlar için bir gün geriden
        .iter()
        .flat_map(|d| day_rows(&cfg.main.log_root, d, "trafik.csv"))
        .filter(|r| {
            if col(r, field) != value {
                return false;
            }
            let Some(end) = parse_time(col(r, "zaman")) else { return false };
            let start = if col(r, "olay") == "BAGLANTI_BITIS" { end - col(r, "sure_sn").parse::<f64>().unwrap_or(0.0) } else { end };
            start - tol <= t && t <= end + tol
        })
        .collect()
}

/// Talep araması. tur: ic-ip | nat-port | hedef-ip | telefon | mac. Dönen: (başlık, satırlar) bölümleri ya da hata.
pub fn ara(cfg: &Config, tur: &str, deger: &str, zaman: &str, tol: f64, now: f64) -> Result<Vec<(String, Vec<Rec>)>, String> {
    let root = &cfg.main.log_root;
    let deger = deger.trim();
    let t = || parse_time(zaman).ok_or_else(|| "Zaman geçersiz (ör. 2026-09-24 14:30).".to_string());
    let ip_ok = |s: &str| s.parse::<std::net::Ipv4Addr>().is_ok();
    match tur {
        "telefon" => {
            let phone = crate::ulkeler::normalize_phone("TR", deger).ok_or("Telefon numarası geçersiz.")?;
            Ok(vec![(format!("+{phone} oturumları"), by_phone(root, &phone))])
        }
        "mac" => {
            let mac = ortak::norm_mac(deger).ok_or("MAC adresi geçersiz.")?;
            let (t1, t2) = match parse_time(zaman) {
                Some(t) => (t - tol.max(86400.0), t + tol.max(86400.0)),
                None => (now - 30.0 * 86400.0, now), // zaman yoksa son 30 gün
            };
            Ok(vec![(format!("{mac} DHCP ve oturum geçmişi"), by_mac(root, &mac, t1, t2))])
        }
        "ic-ip" => {
            if !ip_ok(deger) {
                return Err("İç IP geçersiz.".into());
            }
            let t = t()?;
            Ok(vec![
                ("O andaki DHCP kaydı".into(), dhcp_at(cfg, deger, t).into_iter().collect()),
                ("O anda açık oturum".into(), sessions_at(cfg, deger, t)),
            ])
        }
        "nat-port" => {
            if deger.parse::<u16>().is_err() {
                return Err("Port geçersiz.".into());
            }
            Ok(vec![("Bağlantılar".into(), flows(cfg, "nat_port", deger, t()?, tol))])
        }
        "hedef-ip" => {
            if !ip_ok(deger) {
                return Err("Hedef IP geçersiz.".into());
            }
            Ok(vec![("Bağlantılar".into(), flows(cfg, "hedef_ip", deger, t()?, tol))])
        }
        _ => Err("Arama türü geçersiz.".into()),
    }
}

// ---------------------------------------------------------------- paketler
/// Resmi talep paketi: günlerin klasörleri olduğu gibi (mühürlü dosyalar, MANIFEST), zincir.txt ve doğrulama çıktısı.
/// Orijinallere dokunulmaz. ponytail: sistemdeki GNU tar kullanılır.
pub fn talep_paketi(cfg: &Config, start: &str, end: &str, out: &Path) -> Result<usize, String> {
    let (Some(t1), Some(t2)) = (parse_time(start), parse_time(end)) else { return Err("Tarih geçersiz (YYYY-AA-GG).".into()) };
    if t2 < t1 || t2 - t1 > 92.0 * 86400.0 {
        return Err("Aralık en fazla 92 gün olabilir ve bitiş başlangıçtan önce olamaz.".into());
    }
    let root = Path::new(&cfg.main.log_root);
    let days: Vec<String> = days_range(t1, t2).into_iter().filter(|d| root.join("gunluk").join(d).is_dir()).collect();
    if days.is_empty() {
        return Err("Bu aralıkta cihazda kayıt yok.".into());
    }
    let tmp = out.with_extension("dogrulama");
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let report = crate::muhur::format_verify(&crate::muhur::verify(cfg, None));
    fs::write(tmp.join("dogrulama.txt"), report).map_err(|e| e.to_string())?;
    let mut args: Vec<String> = ["tar", "--owner=0", "--group=0", "-cf"].map(String::from).to_vec();
    args.push(out.display().to_string());
    args.extend(["-C".into(), cfg.main.log_root.clone()]);
    args.extend(days.iter().map(|d| format!("gunluk/{d}")));
    if root.join("zincir.txt").exists() {
        args.push("zincir.txt".into());
    }
    args.extend(["-C".into(), tmp.display().to_string(), "dogrulama.txt".into()]);
    let ok = ortak::run_timeout(&args, 600);
    let _ = fs::remove_dir_all(&tmp);
    if ok { Ok(days.len()) } else { Err("Paket oluşturulamadı (tar).".into()) }
}

/// Komut satırı tablosu: yalnızca dolu sütunlar.
pub fn format_rows(rows: &[Rec]) -> String {
    if rows.is_empty() {
        return "Kayıt bulunamadı".into();
    }
    let cols: Vec<usize> = (0..CSV_HEADER.len()).filter(|&i| rows.iter().any(|r| !r[i].is_empty())).collect();
    let width: Vec<usize> = cols.iter().map(|&i| rows.iter().map(|r| r[i].chars().count()).max().unwrap_or(0).max(CSV_HEADER[i].len())).collect();
    let line = |cells: Vec<String>| cells.iter().zip(&width).map(|(c, w)| format!("{c:<w$}")).collect::<Vec<_>>().join("  ").trim_end().to_string();
    let mut out = vec![line(cols.iter().map(|&i| CSV_HEADER[i].to_uppercase()).collect())];
    out.extend(rows.iter().map(|r| line(cols.iter().map(|&i| r[i].clone()).collect())));
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ortak::Row;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn setup() -> (Config, PathBuf) {
        static N: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!("wfc-kayit-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&root);
        let text = format!("[main]\nlog_root = '{0}/5651'\nstate_root = '{0}/state'\nsession_minutes = 1440\n", root.display());
        (Config::parse(&text).unwrap(), root)
    }

    pub fn ses_row(olay: &str, zaman: &str, ip: &str, sid: &str) -> Row {
        Row::new(olay, zaman).set("telefon", "905334553132").set("ad", "Ayşe").set("soyad", "Yılmaz").set("mac", "aa:bb:cc:dd:ee:01").set("ic_ip", ip).set("oturum_id", sid)
    }

    #[test]
    fn time_parsing() {
        assert_eq!(parse_time("2026-09-29 21:05:34"), Some(1_790_705_134.0));
        assert_eq!(parse_time("2026-09-29T21:05:34+03:00"), Some(1_790_705_134.0));
        assert_eq!(parse_time("2026-09-29 21:05"), Some(1_790_705_100.0));
        assert_eq!(ortak::now_iso(parse_time("2024-02-29").unwrap()), "2024-02-29T00:00:00+03:00");
        assert!(parse_time("2026-13-01").is_none() && parse_time("dün").is_none() && parse_time("2026-09-29 25:00").is_none());
        assert_eq!(days_range(parse_time("2026-09-30 23:00").unwrap(), parse_time("2026-10-02 01:00").unwrap()), vec!["2026-09-30", "2026-10-01", "2026-10-02"]);
    }

    #[test]
    fn safe_paths_only() {
        let (cfg, _) = setup();
        let root = &cfg.main.log_root;
        ortak::append_rows(&ortak::day_file(root, "2026-09-28", "dhcp.csv"), &[Row::new("DHCP_ATAMA", "2026-09-28T10:00:00+03:00")]).unwrap();
        ortak::append_rows(&ortak::user_file(root, "2026-09-28", "905334553132"), &[Row::new("KAYIT", "2026-09-28T10:00:00+03:00")]).unwrap();
        assert!(safe_file(root, "2026-09-28", "dhcp.csv").is_some());
        assert!(safe_file(root, "2026-09-28", "kullanicilar/905334553132.csv").is_some());
        for bad in ["../../zincir.txt", "kullanicilar/../dhcp.csv", "/etc/passwd", "dhcp.csv/..", "MANIFEST.sha256", "yok.csv"] {
            assert!(safe_file(root, "2026-09-28", bad).is_none(), "{bad}");
        }
        assert!(safe_file(root, "../2026-09-28", "dhcp.csv").is_none());
        assert_eq!(user_days(root, "905334553132"), vec![("2026-09-28".to_string(), "kullanicilar/905334553132.csv".to_string())]);
    }

    #[test]
    fn search_sealed_days_ip_moves_and_flows() {
        let (cfg, root) = setup();
        let r = &cfg.main.log_root;
        let d1 = "2026-09-27";
        ortak::append_rows(&ortak::day_file(r, d1, "oturum.csv"), &[
            ses_row("OTURUM_BASLA", "2026-09-27T10:00:00+03:00", "10.50.0.23", "s1"),
            ses_row("OTURUM_IP_DEGISTI", "2026-09-27T12:00:00+03:00", "10.50.0.40", "s1").set("ek", "eski_ip=10.50.0.23"),
        ]).unwrap();
        ortak::append_rows(&ortak::day_file(r, d1, "dhcp.csv"), &[Row::new("DHCP_ATAMA", "2026-09-27T09:59:00+03:00").set("mac", "aa:bb:cc:dd:ee:01").set("ic_ip", "10.50.0.23")]).unwrap();
        ortak::append_rows(&ortak::day_file(r, d1, "trafik.csv"), &[
            Row::new("BAGLANTI_BITIS", "2026-09-27T11:05:00+03:00").set("telefon", "905334553132").set("ic_ip", "10.50.0.23").set("hedef_ip", "142.250.187.110").set("nat_port", "40123").set("sure_sn", "300"),
        ]).unwrap();
        ortak::append_rows(&ortak::user_file(r, d1, "905334553132"), &[ses_row("OTURUM_BASLA", "2026-09-27T10:00:00+03:00", "10.50.0.23", "s1")]).unwrap();
        crate::muhur::close_day(&cfg, d1, parse_time("2026-09-28 01:00").unwrap(), true).unwrap(); // mühürlü (.gz) gün de okunur
        assert!(Path::new(r).join("gunluk").join(d1).join("oturum.csv.gz").exists());
        let d2 = "2026-09-28";
        ortak::append_rows(&ortak::day_file(r, d2, "oturum.csv"), &[ses_row("OTURUM_BITIS", "2026-09-28T09:00:00+03:00", "10.50.0.40", "s1")]).unwrap();

        let at = |ip: &str, z: &str| sessions_at(&cfg, ip, parse_time(z).unwrap()).len();
        assert_eq!((at("10.50.0.23", "2026-09-27 11:00"), at("10.50.0.40", "2026-09-27 11:00")), (1, 0));
        assert_eq!((at("10.50.0.23", "2026-09-27 13:00"), at("10.50.0.40", "2026-09-27 13:00")), (0, 1)); // IP değişimi izlenir
        assert_eq!(at("10.50.0.40", "2026-09-28 10:00"), 0); // oturum bitti
        assert_eq!(col(&dhcp_at(&cfg, "10.50.0.23", parse_time("2026-09-27 11:00").unwrap()).unwrap(), "mac"), "aa:bb:cc:dd:ee:01");

        let t = parse_time("2026-09-27 11:02").unwrap();
        assert_eq!(flows(&cfg, "nat_port", "40123", t, 0.0).len(), 1); // 11:00–11:05 arasında açık
        assert_eq!(flows(&cfg, "nat_port", "40123", parse_time("2026-09-27 10:50").unwrap(), 60.0).len(), 0);
        assert_eq!(flows(&cfg, "hedef_ip", "142.250.187.110", parse_time("2026-09-27 10:59").unwrap(), 120.0).len(), 1);

        let res = ara(&cfg, "telefon", "0533 455 31 32", "", 0.0, 0.0).unwrap();
        assert_eq!(res[0].1.len(), 1);
        assert!(ara(&cfg, "ic-ip", "10.50.0.23", "dün", 0.0, 0.0).is_err());
        assert!(ara(&cfg, "ic-ip", "10.50.0.999", "2026-09-27 11:00", 0.0, 0.0).is_err());
        let mac = ara(&cfg, "mac", "AA-BB-CC-DD-EE-01", "2026-09-27 12:00", 0.0, 0.0).unwrap();
        assert_eq!(mac[0].1.len(), 4); // ±1 gün: DHCP + başla + IP değişimi + ertesi günkü bitiş
        assert!(format_rows(&mac[0].1).starts_with("ZAMAN"));

        let out = root.join("talep.tar");
        assert_eq!(talep_paketi(&cfg, "2026-09-26", "2026-09-28", &out), Ok(2));
        let list = String::from_utf8(std::process::Command::new("tar").arg("-tf").arg(&out).output().unwrap().stdout).unwrap();
        assert!(list.contains("gunluk/2026-09-27/MANIFEST.sha256") && list.contains("gunluk/2026-09-27/oturum.csv.gz"));
        assert!(list.contains("gunluk/2026-09-28/oturum.csv") && list.contains("zincir.txt") && list.contains("dogrulama.txt"));
        assert!(talep_paketi(&cfg, "2026-01-01", "2026-01-02", &out).is_err());
    }
}
