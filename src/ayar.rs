//! Ayar dosyası (/etc/wificorrect/ayarlar.toml). Eski UCI `hotspot` yapılandırmasının karşılığı;
//! bölüm ve anahtar adları docs/MASTER_ENGINEERING.md §8 ile aynı. Eksik anahtar varsayılanı alır.

use serde::Deserialize;

pub const PATH: &str = "/etc/wificorrect/ayarlar.toml";

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Main {
    pub site_name: String,
    pub iface: String,
    pub router_ip: String,
    pub subnet: String,
    pub portal_port: u16,
    pub session_minutes: u64,
    pub max_devices_per_phone: usize,
    pub log_root: String,
    pub state_root: String,
    pub leases_file: String,
}

impl Default for Main {
    fn default() -> Self {
        Main {
            site_name: "Kafe".into(),
            iface: "br-hotspot".into(),
            router_ip: "10.50.0.1".into(),
            subnet: "10.50.0.0/24".into(),
            portal_port: 8080,
            session_minutes: 43200,
            max_devices_per_phone: 3,
            log_root: "/srv/5651".into(),
            state_root: "/srv/hotspot/state".into(),
            leases_file: "/var/lib/misc/dnsmasq.leases".into(),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Netgsm {
    pub mock: bool,
    pub url: String,
    pub usercode: String,
    pub password: String,
    pub msgheader: String,
    pub appkey: String,
    pub message: String,
    pub timeout_sec: u64,
}

impl Default for Netgsm {
    fn default() -> Self {
        Netgsm {
            mock: true,
            url: "https://api.netgsm.com.tr/sms/send/otp".into(),
            usercode: String::new(),
            password: String::new(),
            msgheader: String::new(),
            appkey: String::new(),
            message: "WiFi dogrulama kodunuz: {kod}. Kod 3 dakika gecerlidir.".into(),
            timeout_sec: 10,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Limits {
    pub otp_ttl_sec: u64,
    pub otp_max_attempts: u32,
    pub resend_cooldown_sec: u64,
    pub sms_per_phone_15min: usize,
    pub sms_per_phone_day: usize,
    pub sms_per_mac_hour: usize,
    pub sms_global_day: u64,
    pub verify_fail_lock_min: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            otp_ttl_sec: 180,
            otp_max_attempts: 5,
            resend_cooldown_sec: 60,
            sms_per_phone_15min: 3,
            sms_per_phone_day: 10,
            sms_per_mac_hour: 5,
            sms_global_day: 300,
            verify_fail_lock_min: 15,
        }
    }
}

/// Portalsız geçen (`[[allow]]`, ör. AP, personel; trafiği yine kaydedilir) ya da yasaklı (`[[ban]]`) cihaz.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct Device {
    pub mac: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Config {
    pub main: Main,
    pub netgsm: Netgsm,
    pub limits: Limits,
    pub allow: Vec<Device>,
    pub ban: Vec<Device>,
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, String> {
        toml::from_str(text).map_err(|e| format!("ayar dosyası okunamadı: {e}"))
    }

    pub fn load(path: &str) -> Result<Config, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path} açılamadı: {e}"))?;
        Self::parse(&text)
    }

    /// Geçerli MAC'li girdiler: {mac: ad}.
    pub fn devices(list: &[Device]) -> std::collections::BTreeMap<String, String> {
        list.iter().filter_map(|d| crate::ortak::norm_mac(&d.mac).map(|m| (m, d.name.clone()))).collect()
    }

    /// Gerçek SMS modunda eksik NetGSM bilgileri (portal bunlarla başlamaz).
    pub fn netgsm_missing(&self) -> Vec<&'static str> {
        let n = &self.netgsm;
        if n.mock {
            return vec![];
        }
        [("usercode", &n.usercode), ("password", &n.password), ("msgheader", &n.msgheader)]
            .into_iter()
            .filter(|(_, v)| v.is_empty())
            .map(|(k, _)| k)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_partial_file() {
        let c = Config::parse("[main]\nsite_name = 'Bocafe'\n[netgsm]\nmock = false\nusercode = '850'\n").unwrap();
        assert_eq!(c.main.site_name, "Bocafe");
        assert_eq!(c.main.session_minutes, 43200);
        assert_eq!(c.limits.otp_ttl_sec, 180);
        assert_eq!(c.netgsm_missing(), vec!["password", "msgheader"]);
        assert!(Config::parse("").unwrap().netgsm_missing().is_empty()); // varsayılan: deneme modu
        let c = Config::parse("[[allow]]\nmac = 'AA-BB-CC-DD-EE-99'\nname = 'AP'\n[[allow]]\nmac = 'bozuk'\n").unwrap();
        assert_eq!(Config::devices(&c.allow).into_iter().collect::<Vec<_>>(), vec![("aa:bb:cc:dd:ee:99".into(), "AP".into())]);
        assert!(Config::parse("[main\n").is_err());
    }
}
