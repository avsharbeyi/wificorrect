//! Panel 8b: Kayıtlar, Kullanıcılar, Resmi talep (iki rol de görür; 2026-10-03 kullanıcı kararı).
//! Yalnızca okur ve indirir; her görüntüleme / indirme / arama denetime yazılır.

use super::*;
use crate::kayit::{self, Rec};
use crate::ortak::CSV_HEADER;

const SAYFA: usize = 500;
const TALEP_EN_FAZLA: usize = 2000;

fn size(b: u64) -> String {
    match b {
        0..=1023 => format!("{b} B"),
        1024..=1_048_575 => format!("{:.1} KB", b as f64 / 1024.0),
        _ => format!("{:.1} MB", b as f64 / 1_048_576.0),
    }
}

fn q<'a>(req: &'a Req, k: &str) -> &'a str {
    req.query.get(k).map_or("", |v| v.as_str())
}

fn link(path: &str, params: &[(&str, &str)]) -> String {
    let qs: Vec<String> = params.iter().map(|(k, v)| format!("{k}={}", pct(v))).collect();
    format!("{path}?{}", qs.join("&"))
}

/// Satırlar tablosu: yalnızca dolu sütunlar.
fn rec_table(rows: &[Rec], empty: &str) -> String {
    let cols: Vec<usize> = (0..CSV_HEADER.len()).filter(|&i| rows.iter().any(|r| !r[i].is_empty())).collect();
    let body: Vec<Vec<String>> = rows.iter().map(|r| cols.iter().map(|&i| h(&r[i])).collect()).collect();
    table(&cols.iter().map(|&i| CSV_HEADER[i]).collect::<Vec<_>>(), &body, empty)
}

fn download_name(day: &str, rel: &str) -> String {
    format!("{day}_{}", rel.trim_end_matches(".gz").replace('/', "_"))
}

fn file_resp(file: std::fs::File, name: &str, ctype: &str) -> Resp {
    Resp {
        status: 200,
        body: String::new(),
        headers: vec![
            ("Content-Type".into(), ctype.into()),
            ("Content-Disposition".into(), format!("attachment; filename=\"{name}\"")),
        ],
        file: Some(file),
    }
}

/// Geçici dosyaya üretilen paketi açar, adını siler (açık tanıtıcı kalır) ve indirme yanıtı yapar.
fn tar_resp(cfg: &Config, start: &str, end: &str, name: &str) -> Result<Resp, String> {
    let tmp = std::env::temp_dir().join(format!("wfc-{}.tar", ortak::random_hex(8)));
    let made = kayit::talep_paketi(cfg, start, end, &tmp);
    let file = made.and_then(|_| std::fs::File::open(&tmp).map_err(|e| e.to_string()));
    let _ = std::fs::remove_file(&tmp);
    file.map(|f| file_resp(f, name, "application/x-tar"))
}

impl Panel {
    pub(super) fn kayitlar(&self, cfg: &Config, req: &Req, o: &Oturum) -> Resp {
        let root = &cfg.main.log_root;
        let rows: Vec<Vec<String>> = kayit::days_desc(root)
            .iter()
            .filter_map(|d| kayit::day_info(root, d))
            .map(|i| {
                let durum = match (i.sealed, i.backed_up) {
                    (false, _) => "açık",
                    (true, false) => "mühürlü",
                    (true, true) => "mühürlü, yedeklendi",
                };
                let people = i.files.iter().filter(|(r, _)| r.starts_with("kullanicilar/")).count();
                vec![
                    format!("<a href=\"{}\">{}</a>", h(&link("/kayitlar/gun", &[("gun", &i.day)])), h(&i.day)),
                    h(durum),
                    people.to_string(),
                    h(&size(i.files.iter().map(|(_, s)| s).sum())),
                    format!("<a class=\"dugme ikincil\" href=\"{}\">Günü indir</a>", h(&link("/kayitlar/gun-indir", &[("gun", &i.day)]))),
                ]
            })
            .collect();
        let body = format!(
            "<p class=\"not\">Bu sayfadaki her görüntüleme, arama ve indirme kimin yaptığıyla birlikte kaydedilir ve hizmet sağlayıcı tarafından denetlenir.</p><p class=\"not\">Her gün gece 00:15'te mühürlenir (sıkıştırılır, özeti zincire eklenir). Kayıtlar yalnızca okunur; \
             panelden silinemez. \"Günü indir\" mühürlü dosyaları, zinciri ve doğrulama çıktısını tek pakette verir.</p>{}",
            table(&["Gün", "Durum", "Kişi", "Boyut", ""], &rows, "Henüz kayıt yok")
        );
        self.page(cfg, req, Some(o), "Kayıtlar", &body)
    }

    pub(super) fn kayit_gun(&self, cfg: &Config, req: &Req, o: &Oturum) -> Resp {
        let day = q(req, "gun");
        let Some(info) = kayit::day_info(&cfg.main.log_root, day) else { return text(404, "Gün bulunamadı") };
        self.audit(cfg, req, Some(o), "PANEL_KAYIT_GUN", &format!("gun={day}"));
        let names = ortak::read_index(&cfg.main.log_root);
        let rows: Vec<Vec<String>> = info
            .files
            .iter()
            .map(|(rel, sz)| {
                let who = rel
                    .strip_prefix("kullanicilar/")
                    .and_then(|f| f.split('.').next())
                    .and_then(|p| names.get(p))
                    .map_or(String::new(), |r| h(&format!("{} {}", r[1], r[2])));
                vec![
                    format!("<a href=\"{}\">{}</a>", h(&link("/kayitlar/dosya", &[("gun", day), ("dosya", rel)])), h(rel)),
                    who,
                    h(&size(*sz)),
                    format!("<a class=\"dugme ikincil\" href=\"{}\">İndir</a>", h(&link("/kayitlar/indir", &[("gun", day), ("dosya", rel)]))),
                ]
            })
            .collect();
        let body = format!(
            "<p><a href=\"/kayitlar\">← Kayıtlar</a></p><p class=\"not\">{}. İndirilen dosya Excel'de açılır (mühürlü dosyanın açılmış kopyası; \
             orijinal değişmez).</p>{}<p style=\"margin-top:20px\"><a class=\"dugme\" href=\"{}\">Günü indir (paket)</a></p>",
            if info.sealed { "Mühürlü gün" } else { "Açık gün: yazılmaya devam ediyor" },
            table(&["Dosya", "Kişi", "Boyut", ""], &rows, "Dosya yok"),
            h(&link("/kayitlar/gun-indir", &[("gun", day)]))
        );
        self.page(cfg, req, Some(o), day, &body)
    }

    pub(super) fn kayit_dosya(&self, cfg: &Config, req: &Req, o: &Oturum) -> Resp {
        let (day, rel, filter) = (q(req, "gun"), q(req, "dosya"), q(req, "q").trim().to_lowercase());
        let Some(path) = kayit::safe_file(&cfg.main.log_root, day, rel) else { return text(404, "Dosya bulunamadı") };
        let rows: Vec<Rec> = kayit::parse_csv(&kayit::read_text(&path))
            .into_iter()
            .filter(|r| filter.is_empty() || r.iter().any(|c| c.to_lowercase().contains(&filter)))
            .collect();
        let pages = rows.len().div_ceil(SAYFA).max(1);
        let page = q(req, "s").parse::<usize>().unwrap_or(1).clamp(1, pages);
        let shown = &rows[(page - 1) * SAYFA..(page * SAYFA).min(rows.len())];
        let nav = |p: usize, label: &str| {
            format!("<a class=\"dugme ikincil\" href=\"{}\">{label}</a>", h(&link("/kayitlar/dosya", &[("gun", day), ("dosya", rel), ("q", &filter), ("s", &p.to_string())])))
        };
        let mut pager = format!("<span class=\"not\">{} satır · sayfa {page} / {pages}</span>", rows.len());
        if page > 1 {
            pager.push_str(&nav(page - 1, "← Önceki"));
        }
        if page < pages {
            pager.push_str(&nav(page + 1, "Sonraki →"));
        }
        self.audit(cfg, req, Some(o), "PANEL_KAYIT_GORUNTULE", &format!("gun={day} dosya={rel} sayfa={page}{}", if filter.is_empty() { String::new() } else { format!(" ara={filter}") }));
        let body = format!(
            "<p><a href=\"{}\">← {}</a></p>\
             <form class=\"satir kart\" method=\"get\" action=\"/kayitlar/dosya\"><input type=\"hidden\" name=\"gun\" value=\"{}\"><input type=\"hidden\" name=\"dosya\" value=\"{}\">\
             <div><label for=\"q\">Satırlarda ara</label><input type=\"text\" id=\"q\" name=\"q\" value=\"{}\" placeholder=\"telefon, IP, alan adı…\"></div><button>Ara</button>\
             <a class=\"dugme ikincil\" href=\"{}\">İndir</a></form><div class=\"eylemler\" style=\"align-items:center\">{pager}</div>{}",
            h(&link("/kayitlar/gun", &[("gun", day)])),
            h(day),
            h(day),
            h(rel),
            h(&filter),
            h(&link("/kayitlar/indir", &[("gun", day), ("dosya", rel)])),
            rec_table(shown, "Satır yok"),
        );
        self.page(cfg, req, Some(o), rel, &body)
    }

    pub(super) fn kayit_indir(&self, cfg: &Config, req: &Req, o: &Oturum) -> Resp {
        let (day, rel) = (q(req, "gun"), q(req, "dosya"));
        let Some(path) = kayit::safe_file(&cfg.main.log_root, day, rel) else { return text(404, "Dosya bulunamadı") };
        self.audit(cfg, req, Some(o), "PANEL_KAYIT_INDIR", &format!("gun={day} dosya={rel}"));
        let name = download_name(day, rel);
        let ctype = "text/csv; charset=utf-8";
        if rel.ends_with(".gz") {
            // ponytail: mühürlü dosya bellekte açılır (günlük dosya birkaç MB); çok büyürse geçici dosyaya akıt
            return file_resp_text(format!("\u{feff}{}", kayit::read_text(&path)), &name, ctype);
        }
        match std::fs::File::open(&path) {
            Ok(f) => file_resp(f, &name, ctype),
            Err(_) => text(404, "Dosya bulunamadı"),
        }
    }

    pub(super) fn kayit_gun_indir(&self, cfg: &Config, req: &Req, o: &Oturum) -> Resp {
        let day = q(req, "gun");
        if kayit::day_info(&cfg.main.log_root, day).is_none() {
            return text(404, "Gün bulunamadı");
        }
        self.audit(cfg, req, Some(o), "PANEL_KAYIT_INDIR", &format!("gun={day} paket"));
        tar_resp(cfg, day, day, &format!("kayit_{day}.tar")).unwrap_or_else(|e| redirect("/kayitlar", Some((&e, true))))
    }

    pub(super) fn kullanicilar(&self, cfg: &Config, req: &Req, o: &Oturum) -> Resp {
        let filter = q(req, "q").trim().to_lowercase();
        let mut list: Vec<[String; 5]> = ortak::read_index(&cfg.main.log_root)
            .into_values()
            .filter(|r| filter.is_empty() || r.iter().take(3).any(|c| c.to_lowercase().contains(&filter)))
            .collect();
        list.sort_by(|a, b| b[4].cmp(&a[4])); // son oturum yeniden eskiye
        let total = list.len();
        let rows: Vec<Vec<String>> = list
            .iter()
            .take(SAYFA)
            .map(|r| {
                vec![
                    format!("<a href=\"{}\">+{}</a>", h(&link("/kullanici", &[("tel", &r[0])])), h(&r[0])),
                    h(&format!("{} {}", r[1], r[2])),
                    h(&r[3].get(..16).unwrap_or("").replace('T', " ")),
                    h(&r[4].get(..16).unwrap_or("").replace('T', " ")),
                ]
            })
            .collect();
        if filter.is_empty() {
            self.audit(cfg, req, Some(o), "PANEL_KULLANICILAR", "");
        } else {
            self.audit(cfg, req, Some(o), "PANEL_KULLANICI_ARA", &format!("ara={filter}"));
        }
        let body = format!(
            "<form class=\"satir kart\" method=\"get\" action=\"/kullanicilar\"><div><label for=\"q\">Telefon ya da ad</label>\
             <input type=\"text\" id=\"q\" name=\"q\" value=\"{}\"></div><button>Ara</button></form><p class=\"not\">Bu sayfadaki her görüntüleme, arama ve indirme kimin yaptığıyla birlikte kaydedilir ve hizmet sağlayıcı tarafından denetlenir.</p><p class=\"not\">{total} kişi{}</p>{}",
            h(&filter),
            if total > SAYFA { format!(", ilk {SAYFA} gösteriliyor") } else { String::new() },
            table(&["Telefon", "Ad soyad", "İlk kayıt", "Son oturum"], &rows, "Kayıtlı kişi yok")
        );
        self.page(cfg, req, Some(o), "Kullanıcılar", &body)
    }

    pub(super) fn kullanici(&self, cfg: &Config, req: &Req, o: &Oturum) -> Resp {
        let phone = q(req, "tel");
        if !(6..=15).contains(&phone.len()) || !phone.bytes().all(|b| b.is_ascii_digit()) {
            return text(404, "Kişi bulunamadı");
        }
        let root = &cfg.main.log_root;
        let idx = ortak::read_index(root).get(phone).cloned();
        let days = kayit::user_days(root, phone);
        if idx.is_none() && days.is_empty() {
            return text(404, "Kişi bulunamadı");
        }
        self.audit(cfg, req, Some(o), "PANEL_KULLANICI", &format!("tel={phone}"));
        let info = idx.map_or(String::new(), |r| {
            facts(&[
                ("Ad soyad", h(&format!("{} {}", r[1], r[2]))),
                ("İlk kayıt", h(&r[3].replace('T', " "))),
                ("Son oturum", h(&r[4].replace('T', " "))),
            ])
        });
        let day_rows: Vec<Vec<String>> = days
            .iter()
            .rev()
            .map(|(d, rel)| {
                vec![
                    format!("<a href=\"{}\">{}</a>", h(&link("/kayitlar/dosya", &[("gun", d), ("dosya", rel)])), h(d)),
                    format!("<a class=\"dugme ikincil\" href=\"{}\">İndir</a>", h(&link("/kayitlar/indir", &[("gun", d), ("dosya", rel)]))),
                ]
            })
            .collect();
        let body = format!(
            "<p><a href=\"/kullanicilar\">← Kullanıcılar</a></p><div class=\"kart\">{info}</div>\
             <div class=\"kart\"><h2>Oturumlar</h2>{}</div><div class=\"kart\"><h2>Kaydı olan günler</h2>{}</div>",
            rec_table(&kayit::by_phone(root, phone), "Oturum kaydı yok"),
            table(&["Gün", ""], &day_rows, "Cihazda gün yok")
        );
        self.page(cfg, req, Some(o), &format!("+{phone}"), &body)
    }

    pub(super) fn talep(&self, cfg: &Config, req: &Req, o: &Oturum, now: f64) -> Resp {
        let (tur, deger, zaman, tol) = (q(req, "tur"), q(req, "deger"), q(req, "zaman"), q(req, "tol"));
        let tol_s = tol.parse::<f64>().unwrap_or(120.0).clamp(0.0, 86400.0);
        let tur_sec = [("ic-ip", "İç IP + zaman"), ("nat-port", "NAT portu + zaman"), ("hedef-ip", "Hedef IP + zaman"), ("telefon", "Telefon"), ("mac", "MAC adresi")]
            .iter()
            .map(|(k, l)| format!("<option value=\"{k}\"{}>{l}</option>", if *k == tur { " selected" } else { "" }))
            .collect::<String>();
        let mut results = String::new();
        if !tur.is_empty() {
            match kayit::ara(cfg, tur, deger, zaman, tol_s, now) {
                Err(e) => results = format!("<p class=\"hata\">{}</p>", h(&e)),
                Ok(sections) => {
                    self.audit(cfg, req, Some(o), "PANEL_TALEP_ARA", &format!("tur={tur} deger={deger} zaman={zaman} tolerans={tol_s}"));
                    for (title, rows) in sections {
                        let note = if rows.len() > TALEP_EN_FAZLA { format!("<p class=\"not\">{} satır; ilk {TALEP_EN_FAZLA} gösteriliyor.</p>", rows.len()) } else { String::new() };
                        let shown = &rows[..rows.len().min(TALEP_EN_FAZLA)];
                        results.push_str(&format!("<div class=\"kart\"><h2>{}</h2>{note}{}</div>", h(&title), rec_table(shown, "Kayıt bulunamadı")));
                    }
                }
            }
        }
        let today = ortak::day_of(&ortak::now_iso(now)).to_string();
        let body = format!(
            "<p class=\"not\">Bu sayfadaki her görüntüleme, arama ve indirme kimin yaptığıyla birlikte kaydedilir ve hizmet sağlayıcı tarafından denetlenir.</p><p class=\"not\">Resmi talepler genellikle tarih-saat + IP (+ port) ile gelir. Çift NAT'ta dış port modemde değişir; \
             o durumda zaman + hedef IP ile arayın. Zaman biçimi: 2026-09-24 14:30.</p>\
             <form class=\"satir kart\" method=\"get\" action=\"/talep\">\
             <div><label for=\"tur\">Arama</label><select id=\"tur\" name=\"tur\">{tur_sec}</select></div>\
             <div><label for=\"deger\">Değer</label><input type=\"text\" id=\"deger\" name=\"deger\" value=\"{}\" required></div>\
             <div><label for=\"zaman\">Zaman</label><input type=\"text\" id=\"zaman\" name=\"zaman\" value=\"{}\" placeholder=\"YYYY-AA-GG SS:DD\"></div>\
             <div><label for=\"tol\">Tolerans (sn)</label><input type=\"number\" id=\"tol\" name=\"tol\" value=\"{tol_s}\"></div><button>Ara</button></form>\
             {results}\
             <div class=\"kart\"><h2>Talep paketi</h2><p class=\"not\">Seçilen günlerin mühürlü dosyaları, MANIFEST, zincir.txt ve bütünlük doğrulama \
             çıktısı tek .tar dosyasında (en fazla 92 gün). Yalnızca resmi makama, hukukçu yönlendirmesiyle teslim edin.</p>\
             <form class=\"satir\" method=\"get\" action=\"/talep/paket\">\
             <div><label for=\"bas\">Başlangıç günü</label><input type=\"text\" id=\"bas\" name=\"bas\" value=\"{today}\" required></div>\
             <div><label for=\"bit\">Bitiş günü</label><input type=\"text\" id=\"bit\" name=\"bit\" value=\"{today}\" required></div>\
             <button>Paketi indir</button></form></div>",
            h(deger),
            h(zaman),
        );
        self.page(cfg, req, Some(o), "Resmi talep", &body)
    }

    /// Girilmemesi gereken bir siteye / IP'ye kim girdi: kişiler (en çok girenden) + ayrıntı satırları.
    pub(super) fn site_ara(&self, cfg: &Config, req: &Req, o: &Oturum, now: f64) -> Resp {
        let gun = |t: f64| ortak::day_of(&ortak::now_iso(t)).to_string();
        let aranan = q(req, "q").trim();
        let bas = if q(req, "bas").is_empty() { gun(now - 6.0 * 86400.0) } else { q(req, "bas").trim().to_string() };
        let bit = if q(req, "bit").is_empty() { gun(now) } else { q(req, "bit").trim().to_string() };
        let mut sonuc = String::new();
        if !aranan.is_empty() {
            match kayit::site_ara(cfg, aranan, &bas, &bit) {
                Err(e) => sonuc = format!("<p class=\"hata\">{}</p>", h(&e)),
                Ok(s) => {
                    self.audit(cfg, req, Some(o), "PANEL_SITE_ARA", &format!("aranan={aranan} bas={bas} bit={bit} kisi={}", s.kisiler.len()));
                    let kisiler: Vec<Vec<String>> = s
                        .kisiler
                        .iter()
                        .map(|k| {
                            let tel = if k.telefon.is_empty() {
                                "<span class=\"not\">izinli cihaz</span>".into()
                            } else {
                                format!("<a href=\"{}\">+{}</a>", h(&link("/kullanici", &[("tel", &k.telefon)])), h(&k.telefon))
                            };
                            let zaman = |z: &str| h(&z.get(..16).unwrap_or(z).replace('T', " "));
                            vec![tel, h(&k.ad), format!("<code>{}</code>", h(&k.macler.join(", "))), zaman(&k.ilk), zaman(&k.son), k.sayi.to_string()]
                        })
                        .collect();
                    let goster = &s.satirlar[..s.satirlar.len().min(TALEP_EN_FAZLA)];
                    let not = if s.satirlar.len() > goster.len() { format!("<p class=\"not\">{} satır; ilk {TALEP_EN_FAZLA} gösteriliyor.</p>", s.satirlar.len()) } else { String::new() };
                    sonuc = format!(
                        "<div class=\"kart\"><h2>Kimler girdi</h2>{}</div><div class=\"kart\"><h2>Ayrıntı</h2>{not}{}</div>",
                        table(&["Telefon", "Ad soyad", "MAC", "İlk", "Son", "Kaç kez"], &kisiler, "Bu aralıkta kimse girmemiş"),
                        rec_table(goster, "Kayıt bulunamadı")
                    );
                }
            }
        }
        let body = format!(
            "<p class=\"not\">Girilmemesi gereken bir siteye ya da IP adresine kimin girdiğini bulur. Site adı DNS kayıtlarında \
             (\"bet365\", \"bet365.com\" ya da adres çubuğundan kopyalanmış adres), IP adresi bağlantı kayıtlarında aranır. \
             Yasaklı kelime filtresinin engellediği siteler kayda düşmez; tarayıcının şifreli DNS'i (DoH) kullanılırsa site adı \
             görünmez — o durumda sitenin IP adresiyle arayın.</p>\
             <form class=\"satir kart\" method=\"get\" action=\"/ara\">\
             <div><label for=\"ara-q\">Site adı ya da IP</label><input type=\"text\" id=\"ara-q\" name=\"q\" value=\"{}\" required autocomplete=\"off\"></div>\
             <div><label for=\"ara-bas\">Başlangıç günü</label><input type=\"date\" id=\"ara-bas\" name=\"bas\" value=\"{}\" required></div>\
             <div><label for=\"ara-bit\">Bitiş günü</label><input type=\"date\" id=\"ara-bit\" name=\"bit\" value=\"{}\" required></div>\
             <button>Ara</button></form>{sonuc}",
            h(aranan),
            h(&bas),
            h(&bit)
        );
        self.page(cfg, req, Some(o), "Site / IP arama", &body)
    }

    pub(super) fn talep_paket(&self, cfg: &Config, req: &Req, o: &Oturum) -> Resp {
        let (bas, bit) = (q(req, "bas").trim(), q(req, "bit").trim());
        if !kayit::valid_day(bas) || !kayit::valid_day(bit) {
            return redirect("/talep", Some(("Günler YYYY-AA-GG biçiminde olmalı.", true)));
        }
        self.audit(cfg, req, Some(o), "PANEL_TALEP_PAKET", &format!("bas={bas} bit={bit}"));
        tar_resp(cfg, bas, bit, &format!("talep_{bas}_{bit}.tar")).unwrap_or_else(|e| redirect("/talep", Some((&e, true))))
    }
}

fn file_resp_text(body: String, name: &str, ctype: &str) -> Resp {
    Resp {
        status: 200,
        body,
        headers: vec![
            ("Content-Type".into(), ctype.into()),
            ("Content-Disposition".into(), format!("attachment; filename=\"{name}\"")),
        ],
        file: None,
    }
}
