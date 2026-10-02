//! Twilio Verify ile OTP (eski twilio.py). Kodu Twilio üretir ve doğrular; portal yalnızca başlatır ve sorar.
//! API: POST /v2/Services/{VA}/Verifications (To, Channel=sms, Locale) ve /VerificationCheck (To, Code).

use crate::ayar::Config;
use crate::sms::Sonuc;
use std::time::Duration;

const BASE: &str = "https://verify.twilio.com/v2/Services";

pub fn user_message(kod: &str) -> &'static str {
    match kod {
        "60200" | "21211" | "60205" => "Bu numaraya SMS gönderilemiyor, lütfen numarayı kontrol edin.",
        "60203" | "20429" => "Çok fazla deneme yapıldı, lütfen biraz sonra tekrar deneyin.",
        "AG" => "SMS servisine ulaşılamadı, lütfen personele başvurun.",
        _ => "SMS gönderilemedi, lütfen personele başvurun.",
    }
}

fn base64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in input.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= c.len() { T[(n >> shift & 63) as usize] as char } else { '=' });
        }
    }
    out
}

fn form(fields: &[(&str, &str)]) -> String {
    let enc = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    };
    fields.iter().map(|(k, v)| format!("{}={}", enc(k), enc(v))).collect::<Vec<_>>().join("&")
}

/// (HTTP durumu, JSON gövde). Ağa ulaşılamazsa None.
fn request(cfg: &Config, path: &str, fields: &[(&str, &str)]) -> Option<(u16, serde_json::Value)> {
    let t = &cfg.twilio;
    let (user, secret) = if !t.api_key_sid.is_empty() && !t.api_key_secret.is_empty() {
        (&t.api_key_sid, &t.api_key_secret)
    } else {
        (&t.account_sid, &t.auth_token)
    };
    let resp = ureq::post(&format!("{BASE}/{}/{path}", t.verify_sid))
        .timeout(Duration::from_secs(t.timeout_sec.max(1)))
        .set("Authorization", &format!("Basic {}", base64(format!("{user}:{secret}").as_bytes())))
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(&form(fields));
    let (status, r) = match resp {
        Ok(r) => (r.status(), r),
        Err(ureq::Error::Status(code, r)) => (code, r),
        Err(e) => {
            eprintln!("Twilio erişim hatası: {}", e.kind()); // kimlik bilgisi asla yazılmaz
            return None;
        }
    };
    let body = r.into_string().ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(serde_json::Value::Null);
    Some((status, body))
}

/// Doğrulamayı başlatır (kodu Twilio üretir). `phone`: uluslararası biçim, + olmadan.
pub fn send(cfg: &Config, phone: &str) -> Sonuc {
    let to = format!("+{phone}");
    let locale = if crate::ulkeler::is_tr(phone) { "tr" } else { "en" };
    match request(cfg, "Verifications", &[("To", &to), ("Channel", "sms"), ("Locale", locale)]) {
        None => Sonuc { ok: false, kod: "AG".into(), is: None },
        Some((st, b)) if st < 300 && b["status"] == "pending" => {
            Sonuc { ok: true, kod: "0".into(), is: b["sid"].as_str().map(String::from) }
        }
        Some((st, b)) => Sonuc { ok: false, kod: b["code"].as_u64().map_or(st.to_string(), |c| c.to_string()), is: None },
    }
}

/// (onaylandı_mı, hata). Hata yalnızca Twilio'ya ulaşılamazsa "AG"; yanlış/süresi dolmuş kod → (false, None).
pub fn check(cfg: &Config, phone: &str, code: &str) -> (bool, Option<String>) {
    match request(cfg, "VerificationCheck", &[("To", &format!("+{phone}")), ("Code", code)]) {
        None => (false, Some("AG".into())),
        Some((st, b)) => (st < 300 && b["status"] == "approved", None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_and_messages() {
        assert_eq!(base64(b"AC1:tok"), "QUMxOnRvaw==");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b"abc"), "YWJj");
        assert_eq!(form(&[("To", "+905334553132"), ("Channel", "sms")]), "To=%2B905334553132&Channel=sms");
        assert_eq!(user_message("60200"), "Bu numaraya SMS gönderilemiyor, lütfen numarayı kontrol edin.");
        assert_eq!(user_message("x"), "SMS gönderilemedi, lütfen personele başvurun.");
    }
}
