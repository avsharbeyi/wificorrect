//! Cihaz başına hız sınırı: MAC → sınır kaydı, nft sınıf zinciri metni ve HTB tc komutları.
//! Saf metin üretimi; dosya IO'su yalnızca `oku`/`kaydet`.
#![allow(dead_code)]

use crate::ortak;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const HIZLAR: [u32; 5] = [1, 2, 5, 10, 20];
/// Uygulanmış planın parmak izi (yeniden başlatmada silinir; bkz. `uygula`).
pub const UYGULANAN_YOLU: &str = "/run/wificorrect/hiz_uygulanan";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Sinir {
    /// Mb/sn
    pub hiz: u32,
    pub ad: String,
    pub zaman: String,
    pub kim: String,
}

/// mac → sınır
pub type Sinirlar = BTreeMap<String, Sinir>;

fn yol(state_root: &str) -> PathBuf {
    Path::new(state_root).join("hiz_sinir.json")
}

/// Dosya yok/bozuk → boş.
pub fn oku(state_root: &str) -> Sinirlar {
    std::fs::read_to_string(yol(state_root))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn kaydet(state_root: &str, s: &Sinirlar) -> std::io::Result<()> {
    let data = serde_json::to_vec_pretty(s).map_err(std::io::Error::other)?;
    ortak::write_atomic(&yol(state_root), &data)
}

pub struct Plan {
    pub nft: String,
    pub tc: Vec<Vec<String>>,
}

fn komut(s: String) -> Vec<String> {
    s.split_whitespace().map(String::from).collect()
}

/// `sinirli`: IP'ye göre sıralı (ip, Mb/sn). Sınıf numarası 0x10 + sıra (nft/tc'de onaltılık).
pub fn plan(lan: &str, wan: &str, sinirli: &[(String, u32)]) -> Plan {
    let mut nft = String::from("flush chain inet wfc_hiz sinif\n");
    for (i, (ip, _)) in sinirli.iter().enumerate() {
        let n = 0x10 + i;
        nft += &format!("add rule inet wfc_hiz sinif oifname \"{lan}\" ip daddr {ip} meta priority set 1:{n:x}\n");
        nft += &format!("add rule inet wfc_hiz sinif oifname \"{wan}\" ip saddr {ip} meta priority set 1:{n:x}\n");
    }
    let mut tc = Vec::new();
    for d in [lan, wan] {
        tc.push(komut(format!("tc qdisc replace dev {d} root handle 1: htb default 1")));
        tc.push(komut(format!("tc class replace dev {d} parent 1: classid 1:1 htb rate 1gbit")));
        tc.push(komut(format!("tc qdisc replace dev {d} parent 1:1 handle 2: fq_codel")));
        for (i, (_, mb)) in sinirli.iter().enumerate() {
            let n = 0x10 + i;
            tc.push(komut(format!("tc class replace dev {d} parent 1: classid 1:{n:x} htb rate {mb}mbit ceil {mb}mbit")));
            tc.push(komut(format!("tc qdisc replace dev {d} parent 1:{n:x} handle {n:x}: fq_codel")));
        }
    }
    Plan { nft, tc }
}

/// nft + tc'nin birleşik metni; uygulanmış durumla karşılaştırmak için.
pub fn parmak_izi(p: &Plan) -> String {
    let mut s = p.nft.clone();
    for c in &p.tc {
        s += &c.join(" ");
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tc_say(p: &Plan) -> Vec<String> {
        p.tc.iter().map(|c| c.join(" ")).collect()
    }

    #[test]
    fn plan_bos_yalniz_kok_ve_varsayilan() {
        let p = plan("br-hotspot", "enp3s0", &[]);
        assert_eq!(p.nft, "flush chain inet wfc_hiz sinif\n");
        let beklenen: Vec<String> = ["br-hotspot", "enp3s0"]
            .iter()
            .flat_map(|d| {
                [
                    format!("tc qdisc replace dev {d} root handle 1: htb default 1"),
                    format!("tc class replace dev {d} parent 1: classid 1:1 htb rate 1gbit"),
                    format!("tc qdisc replace dev {d} parent 1:1 handle 2: fq_codel"),
                ]
            })
            .collect();
        assert_eq!(tc_say(&p), beklenen);
    }

    #[test]
    fn plan_iki_sinir() {
        let s = vec![("10.50.0.57".to_string(), 5), ("10.50.0.93".to_string(), 2)];
        let p = plan("br-hotspot", "enp3s0", &s);
        let nft: Vec<&str> = p.nft.lines().collect();
        assert_eq!(
            nft,
            [
                "flush chain inet wfc_hiz sinif",
                r#"add rule inet wfc_hiz sinif oifname "br-hotspot" ip daddr 10.50.0.57 meta priority set 1:10"#,
                r#"add rule inet wfc_hiz sinif oifname "enp3s0" ip saddr 10.50.0.57 meta priority set 1:10"#,
                r#"add rule inet wfc_hiz sinif oifname "br-hotspot" ip daddr 10.50.0.93 meta priority set 1:11"#,
                r#"add rule inet wfc_hiz sinif oifname "enp3s0" ip saddr 10.50.0.93 meta priority set 1:11"#,
            ]
        );
        assert!(p.nft.ends_with('\n'));
        let tc = tc_say(&p);
        for d in ["br-hotspot", "enp3s0"] {
            let kendi: Vec<&String> = tc.iter().filter(|c| c.contains(&format!("dev {d} "))).collect();
            assert_eq!(kendi.len(), 7);
            assert_eq!(kendi[3], &format!("tc class replace dev {d} parent 1: classid 1:10 htb rate 5mbit ceil 5mbit"));
            assert_eq!(kendi[4], &format!("tc qdisc replace dev {d} parent 1:10 handle 10: fq_codel"));
            assert_eq!(kendi[5], &format!("tc class replace dev {d} parent 1: classid 1:11 htb rate 2mbit ceil 2mbit"));
            assert_eq!(kendi[6], &format!("tc qdisc replace dev {d} parent 1:11 handle 11: fq_codel"));
        }
        assert!(!tc.iter().any(|c| c.contains(" del ")));
    }

    #[test]
    fn sinif_numarasi_onaltilik() {
        let s: Vec<(String, u32)> = (0..8).map(|i| (format!("10.50.0.{}", 10 + i), 1)).collect();
        let p = plan("a", "b", &s);
        assert!(p.nft.contains("ip daddr 10.50.0.17 meta priority set 1:17\n"));
        let s: Vec<(String, u32)> = (0..11).map(|i| (format!("10.50.0.{}", 10 + i), 1)).collect();
        let p = plan("a", "b", &s);
        assert!(p.nft.contains("ip daddr 10.50.0.20 meta priority set 1:1a\n"));
        assert!(tc_say(&p).contains(&"tc qdisc replace dev a parent 1:1a handle 1a: fq_codel".to_string()));
    }

    #[test]
    fn parmak_izi_planla_degisir() {
        let a = plan("a", "b", &[]);
        let b = plan("a", "b", &[("10.50.0.5".to_string(), 1)]);
        assert_ne!(parmak_izi(&a), parmak_izi(&b));
        assert_eq!(parmak_izi(&a), parmak_izi(&plan("a", "b", &[])));
    }

    #[test]
    fn oku_kaydet_gidis_donus() {
        let dir = std::env::temp_dir().join(format!("hiz-test-{}", std::process::id()));
        let root = dir.to_str().unwrap();
        let mut s = Sinirlar::new();
        s.insert(
            "aa:bb:cc:dd:ee:01".into(),
            Sinir { hiz: 5, ad: "Telefon".into(), zaman: "2026-10-10T12:00:00".into(), kim: "yonetici".into() },
        );
        kaydet(root, &s).unwrap();
        assert_eq!(oku(root), s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bozuk_dosya_bos() {
        let dir = std::env::temp_dir().join(format!("hiz-bozuk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let root = dir.to_str().unwrap();
        assert!(oku(root).is_empty());
        std::fs::write(dir.join("hiz_sinir.json"), "{bozuk").unwrap();
        assert!(oku(root).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
