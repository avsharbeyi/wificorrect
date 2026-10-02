# Kurulum günlüğü — Debian 13 (ilk cihaz: Bocafe donanımı)

Donanım: Intel Celeron J1900, 1,85 GB RAM, 32 GB SSD (sda1 EFI, sda2 kök 27 GB ext4, sda3 swap), Wi-Fi Atheros AR9287 (ath9k).
Sistem: Debian 13 "trixie", çekirdek 6.12.111, cihaz adı `wificorrect`. Yönetim: `root` yalnızca SSH anahtarıyla, `wificorrect` kullanıcısı (sudo).

## Arayüz adları
| Debian | Eski (OpenWrt) | Fiziksel | MAC | Görev |
|---|---|---|---|---|
| `enp3s0` | `eth1` | sağ, "Ethernet 1" | 00:0e:c4:ce:a0:9b | internet alır (modem DHCP, modemde **192.168.1.110**'a sabit) |
| `enp1s0` | `eth0` | sol, "Ethernet 2" | 00:0e:c4:ce:a0:9a | müşteri tarafı, `br-hotspot` köprüsünde (AP buraya takılacak) |
| `wlp2s0` | `phy0-ap0` | AR9287 | 90:00:4e:b5:b3:c5 | **kapalı**; ileride panelden açılacak |

## Adım 1 — Temel sistem (2026-10-02)
- Paketler: `dnsmasq hostapd iw wireless-regdb conntrack rsync curl unattended-upgrades sudo bridge-utils`.
- Kurulmayanlar (kullanıcı kararı): `firmware-atheros` (USB Wi-Fi yok), `iptables` (bet/porn filtresi yerine "yasaklı kelimeler ve siteler" modülü yazılacak).
- `hostapd` maskeli (Wi-Fi kapalı), `dnsmasq` Adım 3'e kadar kapalıydı.
- Otomatik güncelleme yalnızca `trixie-security`, kendiliğinden yeniden başlatma yok (`deploy/debian/etc/apt/apt.conf.d/`).
- NTP: `0.tr.pool.ntp.org 1.tr.pool.ntp.org time.google.com`, saat dilimi Europe/Istanbul.
- `/srv/5651`, `/srv/hotspot/state` (700). Ayrı `/srv` bölümü yok; Debian'da sysupgrade riski olmadığı için kökte.
- Açık karar: SSH parola girişi (`wificorrect` kullanıcısı için) hâlâ açık.

## Adım 2 — Köprü (2026-10-02)
- `/etc/network/interfaces.d/hotspot`: `br-hotspot` = `enp1s0`, 10.50.0.1/24, müşteri tarafında IPv6 kapalı. Ölü adam anahtarı + yeni SSH oturumuyla doğrulandı.

## Adım 3 — Güvenlik duvarı + DHCP/DNS (2026-10-02)
- `wificorrect-guvenlik.service` (sysinit, ağdan önce): `guvenlik.nft` (temel duvar, fw4'ün karşılığı) + `kapi.nft` (giriş kapısı: auth / allow_mac / ban_mac setleri, DNS yönlendirme, portala yönlendirme, 0x5651 işareti, köprü yalıtımı). Yönlendirme yalnızca kurallar yüklenince açılır; durdurmak kuralları silmez (kapalıyken güvenli). Debian'ın `nftables.service`'i kapalı.
- dnsmasq: yalnızca `br-hotspot`, havuz 10.50.0.20–249, kira 2 saat, `log-queries=extra`.
- Doğrulama (cihaz içinde sanal müşteri, `ip netns` + veth): DHCP ✔, DNS yönlendirme ✔ (30–60 ms), girişsiz HTTPS anında red ✔, girişsiz HTTP → portala ✔, izinli müşteri HTTPS/HTTP ✔, dükkân ağı / DoT 853 / IPv6 engelli ✔, yeniden başlatmada hepsi kendiliğinden ✔ (~25 sn).
- Tuzak: **example.com** bu hattan neredeyse hiç çözülmüyor (PC'den de 7 sn). DNS testlerinde başka ad kullanın.
