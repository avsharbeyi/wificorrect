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

## Hedef: kurulum kalıbı (ISO) — bütün adımlarda geçerli kurallar
Son ürün, aynı donanımlı boş bir cihaza yüklenen bir ISO olacak; ilk açılışta **sahiplendirme ekranı** (kurulum sihirbazı) gelecek.
- Cihaza elle yapılan her ayar repoda (`deploy/debian/`) dosya olarak durur; kalıp yalnızca repodan üretilir.
- Kalıba cihaza/kafeye özel hiçbir şey girmez: MAC, IP sabitlemesi, SSH sunucu anahtarları (ilk açılışta yeniden üretilir), root parolası, panel hesapları, SMS bilgileri, yedek anahtarı. Hepsini sahiplendirme ekranı sorar veya üretir.
- Uygulama `.deb` paketi olarak paketlenir; ürün paketi GitHub Actions'ta derlenir. **Kalıpta derleyici yoktur.**

## Adım 4 — Rust geliştirme ortamı (2026-10-02)
- WSL kullanılamadı (varsayılan dağıtımın diski yok; diğerleri başka projelerin). Geliştirme döneminde **cihazın kendisinde** derleniyor: `cargo`, `rustc` 1.85.1, `gcc`, `libc6-dev`, `pkg-config` (~1,7 GB kök alanı). Ürün aşamasında kaldırılacak.
- `scripts/gelistir.sh`: kaynağı `tar | ssh` ile `/root/rza`'ya gönderir, `cargo build --release`, `/usr/local/bin/wificorrect`'e kurar (~30 sn). `Cargo.lock` geri alınıp repoda tutulur.
- İskelet: tek program, alt komutlar `portal`, `kaydedici`, `panel`, `ctl`, `surum`.

## Adım 5 — Giriş portalı (Rust) (2026-10-02)
- `src/portal.rs` (eski portal.py), `src/ortak.rs` (common.py'nin portal kısmı), `src/sms.rs` (NetGSM + deneme modu), `src/ayar.rs` (`/etc/wificorrect/ayarlar.toml`, eski UCI'nin karşılığı; örnek: `deploy/debian/etc/wificorrect/ayarlar.ornek.toml`). Şablonlar eski sistemden aynen (`src/sablon/`, programın içine gömülü).
- Kütüphaneler: tiny_http, ureq (rustls), serde/serde_json/toml, libc (flock), getrandom. Debian Rust 1.85 için `.cargo/config.toml` → MSRV'ye uygun sürüm seçimi.
- `wificorrect-portal.service`: Restart=always, `/srv` bağlı değilse açılmaz, yalnızca /srv'ye yazabilir (ProtectSystem=full).
- 21 birim testi geçti (eski test_portal / test_common / test_netgsm karşılıkları).
- Cihazda uçtan uca (sanal müşteri): girişsiz HTTPS anında red ✔, telefon bağlantı kontrolü → 302 portal ✔, form hataları ✔, kod gönderme (deneme modu, kod günlükte) ✔, yanlış kod sayacı ✔, doğru kod → "Bağlandınız" + 1 sn sonra istenen sayfa ✔, nft yetkisi 30 gün ✔, HTTPS çıkış ✔, KAYIT/OTURUM_BASLA/denetim/index kayıtları ✔.
- Ölçüm: program 2,4 MB, portal bellekte 3,8 MB (Python portalı ~20 MB). İlk derleme 4 dk 40 sn.
- Not: test sırasında `/srv/5651/gunluk/2026-10-02/` altına uydurma numarayla (5550000000, "Deneme Musteri") test kayıtları yazıldı; kayıtlar yalnızca eklenerek tutulduğu için silinmedi.

## Adım 6 — Kaydedici, DHCP kaydı, oturum geri yükleme (2026-10-02)
- `src/kaydedici.rs` (eski logger.py): `conntrack -E … -s 10.50.0.0/24` → `trafik.csv`; dnsmasq sorguları `journalctl -f -n 0 -o short-unix -u dnsmasq` → `dns.csv` (satırın kendi zamanıyla); her ikisi kişinin günlük dosyasına. 30 sn'de bir oturum bakımı (süre dolumu, IP değişimi → taşıma, IP başkasına verildi → beklemeye alma), disk %80 uyarı / %95 kişi kopyası durur. Okuyucu ölürse `LOG_BOSLUK`, ENOBUFS'ta kuyruk ve döngü gecikmesiyle `LOG_BOSLUK`. Wi-Fi bekçisi Wi-Fi açılınca eklenecek.
- Eski sistemden iki iyileştirme: (1) IP oturum listesinde yoksa dosyalar o an yeniden okunur — giriş/taşıma sonrası ilk olay telefonsuz yazılmaz; (2) bağlantı kapanışı (BAGLANTI_BITIS) bağlantıyı **başlatan** oturuma yazılır, oturum bu arada başka IP'ye taşınsa bile.
- `src/ctl.rs`: `dhcp-olay` (dnsmasq `dhcp-script`: `add|old|del mac ip [ad]` → `dhcp.csv` + oturum taşıma), `yukle` (açılışta / güvenlik duvarı yenilenince izinli-yasaklı kümeler + oturumlar kalan süreyle), `oturumlar`.
- Sistem: `/etc/wificorrect/conntrack.sysctl` (acct=1, timestamp=1 — Debian'da ikisi de varsayılan kapalıydı; güvenlik duvarı servisi nft'den sonra uygular), `wificorrect-guvenlik.service` → `ctl yukle`, `/usr/local/lib/wificorrect/dhcp-script`, dnsmasq günlük hız sınırı kapalı (`dnsmasq.service.d/wificorrect.conf`), journald 500 MB / 14 gün (DNS sorguları kişisel veri), `wificorrect-kaydedici.service`.
- 35 birim testi geçti (sahadan alınmış gerçek conntrack/dnsmasq satırlarıyla, `tests/fixtures/`).
- Cihazda uçtan uca: DHCP_ATAMA/YENILEME ✔, giriş → trafik + DNS satırları telefonla ✔, IP değişimi → SMS'siz taşıma + `OTURUM_IP_DEGISTI` + yeni IP'nin ilk isteği bile telefonla ✔, kapanış satırları bayt + süreyle ✔, yeniden başlatmada `1 oturum geri yüklendi` + nft'de kalan süre ✔.
- Bellek: portal 4,5 MB, kaydedici 8,5 MB (Python: 20,3 + 11,5 MB).
