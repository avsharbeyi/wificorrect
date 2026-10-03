//! Panel: Yasaklı siteler ve kelimeler (iki rol de görür). Kaydedince hemen uygulanır (src/filtre.rs).

use super::*;
use crate::filtre;

const LISTELER: &[(&str, &str, &str)] = &[
    ("site", "Yasaklı siteler", "Alan adı ve bütün alt adları engellenir (ör. bet365.com → www.bet365.com, m.bet365.com)."),
    ("kelime", "Yasaklı kelimeler", "Alan adının herhangi bir yerinde geçerse engellenir (ör. bet → superbet.com.tr). Nokta içeremez."),
    ("istisna", "İstisnalar", "Kelime filtresine takılan meşru siteler için: bu kelime geçen alan adı kelime filtresinden geçer (ör. alphabet, better)."),
];

fn list_mut<'a>(cfg: &'a mut Config, liste: &str) -> Option<&'a mut Vec<String>> {
    match liste {
        "site" => Some(&mut cfg.filtre.siteler),
        "kelime" => Some(&mut cfg.filtre.kelimeler),
        "istisna" => Some(&mut cfg.filtre.istisnalar),
        _ => None,
    }
}

impl Panel {
    pub(super) fn filtre_sayfa(&self, cfg: &Config, req: &Req, o: &Oturum) -> Resp {
        let hits = filtre::counters(&ortak::capture(&["iptables", "-L", "WFC_DNS", "-v", "-n", "-x"]));
        let mut cards = String::new();
        for (key, title, note) in LISTELER {
            let list = match *key {
                "site" => &cfg.filtre.siteler,
                "kelime" => &cfg.filtre.kelimeler,
                _ => &cfg.filtre.istisnalar,
            };
            let rows: Vec<Vec<String>> = list
                .iter()
                .map(|v| {
                    let mut r = vec![format!("<code>{}</code>", h(v))];
                    if *key != "site" {
                        r.push(hits.get(v).map_or("0".into(), |n| n.to_string()));
                    }
                    r.push(post_button(o, "/yasakli-siteler/kaldir", "Kaldır", &[("liste", key), ("deger", v)], "ikincil"));
                    r
                })
                .collect();
            let headers: &[&str] = if *key == "site" { &["Site", ""] } else { &["Kelime", "Eşleşen sorgu", ""] };
            cards.push_str(&format!(
                "<section class=\"kart\"><h2>{}</h2><p class=\"not\">{}</p>\
                 <form class=\"satir\" method=\"post\" action=\"/yasakli-siteler/ekle\">{}<input type=\"hidden\" name=\"liste\" value=\"{key}\">\
                 <div><label for=\"d-{key}\">Ekle</label><input type=\"text\" id=\"d-{key}\" name=\"deger\" required autocomplete=\"off\"></div><button>Ekle</button></form>{}</section>",
                h(title),
                h(note),
                csrf_input(o),
                table(headers, &rows, "Liste boş")
            ));
        }
        let ad = req.query.get("ad").map_or("", |s| s.trim());
        let sonuc = if ad.is_empty() {
            String::new()
        } else {
            match filtre::check(cfg, ad) {
                Some(why) => format!("<p class=\"hata\"><b>{}</b> engelli ({}).</p>", h(ad), h(&why)),
                None => format!("<p><b>{}</b> engelli değil.</p>", h(ad)),
            }
        };
        let body = format!(
            "<p class=\"not\">Müşteri ağında engellenen adresler \"site bulunamadı\" olarak görünür. Bütün DNS cihaza yönlendirildiği için \
             başka DNS yazmak filtreyi atlatmaz; ancak telefonda \"Özel DNS\" ya da tarayıcıda \"Güvenli DNS\" (şifreli DNS) açıksa atlatılır.</p>\
             <section class=\"kart\"><h2>Bu adres engelli mi?</h2><form class=\"satir\" method=\"get\" action=\"/yasakli-siteler\">\
             <div><label for=\"ad\">Alan adı</label><input type=\"text\" id=\"ad\" name=\"ad\" value=\"{}\" placeholder=\"ör. www.ornek.com\"></div>\
             <button class=\"ikincil\">Dene</button></form>{sonuc}</section>{cards}",
            h(ad)
        );
        self.page(cfg, req, Some(o), "Yasaklı siteler", &body)
    }

    pub(super) fn filtre_degistir(&self, mut cfg: Config, req: &Req, o: &Oturum, ekle: bool) -> Resp {
        let liste = req.form.get("liste").map_or("", String::as_str);
        let raw = req.form.get("deger").map_or("", String::as_str);
        let value = if liste == "site" { filtre::normalize_site(raw) } else { filtre::normalize_word(raw) };
        let Some(value) = value else {
            let why = if liste == "site" { "Geçersiz alan adı (ör. ornek.com)." } else { "Geçersiz kelime: 2-63 karakter, yalnızca harf (Türkçe karakter olmadan), rakam, tire." };
            return redirect("/yasakli-siteler", Some((why, true)));
        };
        let Some(list) = list_mut(&mut cfg, liste) else { return redirect("/yasakli-siteler", Some(("Geçersiz liste.", true))) };
        if ekle {
            if list.contains(&value) {
                return redirect("/yasakli-siteler", Some(("Zaten listede.", true)));
            }
            list.push(value.clone());
            list.sort();
        } else if let Some(i) = list.iter().position(|v| *v == value) {
            list.remove(i);
        } else {
            return redirect("/yasakli-siteler", Some(("Listede yok.", true)));
        }
        if let Err(e) = cfg.save(&self.cfg_path) {
            return redirect("/yasakli-siteler", Some((&e, true)));
        }
        self.audit(&cfg, req, Some(o), if ekle { "PANEL_FILTRE_EKLE" } else { "PANEL_FILTRE_KALDIR" }, &format!("liste={liste} deger={value}"));
        match filtre::uygula(&cfg, &self.filtre_conf, &*self.runner) {
            Ok(()) => redirect("/yasakli-siteler", Some((if ekle { "Eklendi, hemen geçerli." } else { "Kaldırıldı." }, false))),
            Err(e) => redirect("/yasakli-siteler", Some((&format!("Kaydedildi ama uygulanamadı: {e}"), true))),
        }
    }
}
