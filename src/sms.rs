//! NetGSM OTP SMS gönderimi (eski netgsm.py; docs/MASTER_ENGINEERING.md §12). Twilio sonraki adımda.

use crate::ayar::Config;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub struct Sonuc {
    pub ok: bool,
    /// NetGSM kodu ("0" başarılı) ya da "AG" (ağ hatası) / "PARSE" (anlaşılmayan cevap).
    pub kod: String,
    pub is: Option<String>,
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

pub fn send_otp(cfg: &Config, phone: &str, code: &str) -> Sonuc {
    let n = &cfg.netgsm;
    if n.mock {
        eprintln!("MOCK OTP {phone} -> {code}");
        return Sonuc { ok: true, kod: "0".into(), is: Some("MOCK".into()) };
    }
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
    fn mock_sends_nothing() {
        let r = send_otp(&Config::default(), "5334553132", "123456");
        assert_eq!(r, Sonuc { ok: true, kod: "0".into(), is: Some("MOCK".into()) });
    }
}
