//! SMS: sağlayıcı seçimi + deneme modu + NetGSM OTP (eski netgsm.py; docs/MASTER_ENGINEERING.md §12). Twilio: twilio.rs.

use crate::ayar::Config;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub struct Sonuc {
    pub ok: bool,
    /// Sağlayıcı kodu ("0" başarılı) ya da "AG" (ağ hatası) / "PARSE" (anlaşılmayan cevap).
    pub kod: String,
    pub is: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Saglayici {
    Netgsm,
    Twilio,
}

impl Saglayici {
    pub fn ad(self) -> &'static str {
        match self {
            Saglayici::Netgsm => "netgsm",
            Saglayici::Twilio => "twilio",
        }
    }
}

pub const YABANCI_YOK: &str = "Yabancı numaralara şu an SMS gönderilemiyor, lütfen personele başvurun.";

/// Türk numaraları ayardaki sağlayıcıdan; yabancı numaralar Twilio'dan (açıksa). NetGSM yalnızca Türk numaralarına gönderir.
pub fn route(cfg: &Config, phone: &str) -> Result<Saglayici, &'static str> {
    if cfg.sms.provider == "twilio" {
        Ok(Saglayici::Twilio)
    } else if crate::ulkeler::is_tr(phone) {
        Ok(Saglayici::Netgsm)
    } else if cfg.twilio.enabled {
        Ok(Saglayici::Twilio)
    } else {
        Err(YABANCI_YOK)
    }
}

/// Deneme modunda hiçbir sağlayıcıya istek gitmez; kod sistem günlüğüne yazılır ve yerelde karşılaştırılır.
pub fn send(cfg: &Config, p: Saglayici, phone: &str, code: &str) -> Sonuc {
    if cfg.sms.mock {
        eprintln!("MOCK OTP {phone} -> {code}");
        return Sonuc { ok: true, kod: "0".into(), is: Some("MOCK".into()) };
    }
    match p {
        Saglayici::Netgsm => send_netgsm(cfg, phone.strip_prefix("90").unwrap_or(phone), code),
        Saglayici::Twilio => crate::twilio::send(cfg, phone),
    }
}

pub fn message_for(p: Saglayici, kod: &str) -> &'static str {
    match p {
        Saglayici::Netgsm => user_message(kod),
        Saglayici::Twilio => crate::twilio::user_message(kod),
    }
}

pub fn user_message(kod: &str) -> &'static str {
    match kod {
        "50" => "Bu numaraya SMS gönderilemiyor, lütfen numarayı kontrol edin.",
        "80" => "Çok fazla deneme yapıldı, lütfen biraz sonra tekrar deneyin.",
        "100" => "SMS servisi geçici olarak yanıt vermiyor, lütfen biraz sonra tekrar deneyin.",
        "AG" => "SMS servisine ulaşılamadı, lütfen personele başvurun.",
        _ => "SMS gönderilemedi, lütfen personele başvurun.",
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

pub fn build_xml(usercode: &str, password: &str, msgheader: &str, appkey: &str, msg: &str, phone: &str) -> String {
    let e = xml_escape;
    let appkey = if appkey.is_empty() { String::new() } else { format!("<appkey>{}</appkey>", e(appkey)) };
    format!(
        "<?xml version='1.0' encoding='utf-8'?>\n<mainbody><header><usercode>{}</usercode><password>{}</password>\
         <msgheader>{}</msgheader>{appkey}</header><body><msg>{}</msg><no>{}</no></body></mainbody>",
        e(usercode),
        e(password),
        e(msgheader),
        e(msg),
        e(phone)
    )
}

fn tag<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let start = text.find(&format!("<{name}>"))? + name.len() + 2;
    let end = text[start..].find(&format!("</{name}>"))? + start;
    Some(text[start..end].trim())
}

/// XML (`<code>`, `<jobID>`) ya da düz metin (`00 123456`) cevabı → (kod, iş no).
pub fn parse_response(text: &str) -> (String, Option<String>) {
    let text = text.trim();
    if let Some(code) = tag(text, "code") {
        let job = tag(text, "jobID").filter(|j| !j.is_empty()).map(String::from);
        return (code.to_string(), job);
    }
    let mut parts = text.split_whitespace();
    match parts.next() {
        Some(c) if c.bytes().all(|b| b.is_ascii_digit()) => (c.to_string(), parts.next().map(String::from)),
        _ => ("PARSE".into(), None),
    }
}

/// `national`: 10 haneli Türk numarası (5XXXXXXXXX).
fn send_netgsm(cfg: &Config, national: &str, code: &str) -> Sonuc {
    let n = &cfg.netgsm;
    let phone = national;
    let msg = n.message.replace("{kod}", code);
    let body = build_xml(&n.usercode, &n.password, &n.msgheader, &n.appkey, &msg, phone);
    // Yeniden deneme yok: çift SMS ve çift ücret riski (§12.1)
    let resp = ureq::post(&n.url)
        .timeout(Duration::from_secs(n.timeout_sec.max(1)))
        .set("Content-Type", "application/xml; charset=UTF-8")
        .send_string(&body);
    // İstek gövdesi (şifre) asla yazılmaz; yalnızca hata türü
    let text = match resp {
        Ok(r) => r.into_string().map_err(|e| eprintln!("NetGSM cevabı okunamadı: {}", e.kind())).ok(),
        Err(e) => {
            eprintln!("NetGSM erişim hatası: {}", e.kind());
            None
        }
    };
    match text {
        Some(t) => {
            let (kod, is) = parse_response(&t);
            Sonuc { ok: kod == "0" || kod == "00", kod, is }
        }
        None => Sonuc { ok: false, kod: "AG".into(), is: None },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_is_escaped_and_complete() {
        let x = build_xml("850", "p<&>'\"", "gztp.blgsyr", "", "WiFi kodunuz: 123456", "5334553132");
        assert!(x.contains("<password>p&lt;&amp;&gt;&apos;&quot;</password>"));
        assert!(x.contains("<msgheader>gztp.blgsyr</msgheader>"));
        assert!(x.contains("<no>5334553132</no>"));
        assert!(!x.contains("<appkey>"));
        assert!(build_xml("u", "p", "h", "k1", "m", "5").contains("<appkey>k1</appkey>"));
    }

    #[test]
    fn responses() {
        let ok = "<?xml version=\"1.0\"?><xml><main><code>0</code><jobID>179095243099687544310585884</jobID></main></xml>";
        assert_eq!(parse_response(ok), ("0".into(), Some("179095243099687544310585884".into())));
        assert_eq!(parse_response("<xml><main><code>30</code><jobID></jobID></main></xml>"), ("30".into(), None));
        assert_eq!(parse_response("00 123456"), ("00".into(), Some("123456".into())));
        assert_eq!(parse_response("<html>hata</html>"), ("PARSE".into(), None));
        assert_eq!(user_message("50"), "Bu numaraya SMS gönderilemiyor, lütfen numarayı kontrol edin.");
        assert_eq!(user_message("30"), "SMS gönderilemedi, lütfen personele başvurun.");
    }

    #[test]
    fn routing() {
        let mut c = Config::default();
        assert_eq!(route(&c, "905334553132"), Ok(Saglayici::Netgsm));
        assert_eq!(route(&c, "4915123456789"), Err(YABANCI_YOK)); // Twilio kapalı
        c.twilio.enabled = true;
        assert_eq!(route(&c, "4915123456789"), Ok(Saglayici::Twilio));
        assert_eq!(route(&c, "905334553132"), Ok(Saglayici::Netgsm));
        c.sms.provider = "twilio".into();
        assert_eq!(route(&c, "905334553132"), Ok(Saglayici::Twilio));
    }

    #[test]
    fn mock_sends_nothing() {
        let r = send(&Config::default(), Saglayici::Twilio, "4915123456789", "123456");
        assert_eq!(r, Sonuc { ok: true, kod: "0".into(), is: Some("MOCK".into()) });
    }
}
