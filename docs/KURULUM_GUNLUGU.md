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

## Farklı modem ağları (2026-10-02)
- İnternet alan port DHCP istemcisi: modem hangi havuzu verirse (192.168.0.x, 10.x …) IP, ağ geçidi ve DNS kendiliğinden alınır.
- Yönetim erişimi (SSH 22, panel 8443) artık `192.168.1.0/24`'e değil, internet alan portun arkasındaki **bütün özel ağlara** açık (10/8, 172.16/12, 192.168/16). Cihaz doğrudan genel IP alırsa internetten yönetim kapalı kalır.
- Cihaz ekranındaki giriş istemi yönetim adresini gösterir (`/etc/issue.d/wificorrect.issue`, `\4{enp3s0}`).
- Bilinen sınır: modem ağı 10.50.0.0/24 ise müşteri ağıyla çakışır (çok nadir); müşteri ağı sahiplendirme ekranında seçilebilir yapılacak.

## Adım 7 — Mühürleme, doğrulama, saklama, yedek (2026-10-02)
- `src/muhur.rs` (eski ctl.py §15): `ctl gun-kapat [GÜN] [--zorla]` → bugünden önceki mühürsüz her gün (kaçanlar dahil): `*.csv` → `*.csv.gz` (sıkıştırılmış dosya baştan okunup doğrulanmadan orijinal silinmez), `MANIFEST.sha256` (kişi dosyaları dahil), `zincir.txt` hash zinciri (yarım satıra dayanıklı), dosyalar 400. Son 5 dk'da yazılmış dosya varsa `GUN_KAPAT_ERTELENDI`.
- `ctl dogrula [--son N]`: zincir her zaman baştan, dosya hash'leri; bozulan dosyayı adıyla söyler. `ctl temizle [--kuru]`: `retention_days` (730) aşan **ve yedeklenmiş** günler silinir, yedeksiz gün asla (`YEDEKSIZ_GUN`); kişi listesinden 2 yıldır gelmeyenler çıkar. `ctl yedekle`: rsync, günler `--ignore-existing`, zincir/index'in önceki sürümü sunucuda `eski/` altına (1 saat zaman aşımı).
- Zamanlayıcılar (cron yerine): `wificorrect-gun-kapat.timer` 00:15, `wificorrect-gece.timer` 02:00 (yedekle, sonra temizle); `Persistent=true` — cihaz kapalıysa açılışta çalışır.
- RFC 3161 zaman damgası henüz yok (sağlayıcı seçilince); hash zinciri her durumda çalışıyor.
- 38 birim testi geçti (mühürleme/erteleme/zincir, bir bayt değişince dosya adıyla yakalama, zincir kırılması, yalnızca yedeklenmiş eski günlerin silinmesi, yedek hata/yeniden deneme).
- Yedek: `[backup] enabled = false` — sunucu hesabı ve SSH anahtarı bekleniyor.

## Bekleyen işler (kullanıcı kararıyla ertelendi)
- **Uzak yedek — arşiv sunucusu kurulunca yapılacak** (2026-10-02 kullanıcı kararı): cihazda yeni SSH anahtarı üret (`/root/.ssh/yedek_anahtar`), sunucuda kafe hesabına (`kafe-<ad>`, salt-yazma, rrsync) ekle, `/etc/wificorrect/ayarlar.toml` → `[backup] enabled = true`, `target = "kafe-<ad>@<sunucu>:"`; `ctl yedekle` ile dene. O zamana kadar günler cihazda mühürlenip birikir; `temizle` yedeklenmemiş günü silmez.
- Eski OpenWrt cihazının kayıtları/ayarları yeni sisteme taşınmayacak (kullanıcı kararı).

## Twilio + ülke kodu seçimi (2026-10-02, kullanıcı isteği)
- Portal formunda telefonun yanında ülke seçimi: varsayılan 🇹🇷 +90, 159 ülke (`src/ulkeler.rs`; bayrak ISO kodundan üretilir, JS yok). Numara `+…`/`00…` ile yazılırsa seçim yok sayılır. Türkiye için 5XX XXX XX XX kuralı, diğerlerinde 4–14 haneli ulusal numara.
- **Telefon biçimi değişti:** her yerde uluslararası biçim (E.164, + olmadan): `905334553132`, `4915123456789` — oturum, CSV `telefon` sütunu, kişi dosyası adı, kişi listesi. Ekranda `+90 5XX XXX XX 32`.
- SMS yönlendirme (`sms::route`): Türk numaraları `[sms] provider` (netgsm|twilio); yabancı numaralar Twilio açıksa Twilio'dan, değilse "Yabancı numaralara şu an SMS gönderilemiyor". NetGSM'e 10 haneli ulusal numara gider. Deneme modu tek anahtar: `[sms] mock` (iki sağlayıcıyı da kapsar).
- `src/twilio.rs` (eski twilio.py): Verify API; gerçek modda kodu Twilio üretir/doğrular, ağ hatasında deneme hakkı düşmez; doğrulama sürerken kod yenilenirse sonuç eski koda uygulanmaz. Yerel ayar: Türk numarası `tr`, diğerleri `en`.
- KVKK aydınlatma metnine yurt dışı aktarım (Twilio, ABD) eklendi — **hukukçuya onaylatılmalı (KVKK m.9).**
- 44 birim testi geçti; cihazda: varsayılan +90 seçili, Almanya / `+49` numarası Twilio kapalıyken düzgün reddediliyor (`OTP_ISTEK red=yabanci_numara`), Türk numarası NetGSM (deneme) ile kod alıyor.

## Adım 8a — Yönetim paneli çekirdeği (2026-10-02)
- `wificorrect panel` → `wificorrect-panel.service`, https://<WAN IP>:8443 (yalnızca dükkân ağı; müşteri ağından istek 403). İlk açılışta `/etc/wificorrect/panel.{crt,key}` öz-imzalı (EC P-256, 10 yıl) cihazda üretilir; kalıba girmez.
- **Hesaplar (2026-10-03, kullanıcı kararı):** hizmet sağlayıcı hesabı sabit **`admin`** (root gibi; parolasını yalnızca hizmet sağlayıcı bilir). Parola cihaz konsolundan/SSH ile `wificorrect ctl admin-parola` (ekrana yazılmaz) ile konur; ISO'da kalıba gömülecek. `admin` silinemez, başka hesap bu adı alamaz. **İlk açılış = `/kurulum`:** yalnızca kafe adı (boş gelir) + kafe sahibinin kullanıcı adı ve parolası (≥ 10). Kurulum bitene kadar panelde kafe adı yerine "WifiCorrect" yazar; `admin` kurulumdan önce de `/giris`ten girebilir. Hesaplar `/etc/wificorrect/hesaplar.json` (600, PBKDF2-SHA256 120.000, kullanıcı başına tuz); bozuk dosya kurulum ekranını açmaz.
- Giriş: 5 hatalı deneme / 15 dk (IP ve kullanıcı başına) kilit; çerez `HttpOnly; Secure; SameSite=Strict`, 12 saat; her POST'ta CSRF. Parola değişince kullanıcının diğer oturumları kapanır.
- Sayfalar: Özet (bağlı cihaz, bugün farklı kullanıcı, SMS/tavan, disk, uyarılar) · Bağlı cihazlar (+ bağlantı kes) · Yasaklı / izinli cihazlar · Ayarlar · Sistem (servisler, günü kapat, bütünlüğü doğrula, yeniden başlat) · Hesaplar (yalnızca hizmet sağlayıcı) · Şifremi değiştir.
- Roller (2026-10-03, kullanıcı kararı; eski "sahip kayıt görmez" kuralını değiştirir): kafe sahibi her şeyi admin gibi görür ve yönetir (bağlı cihazlarda numara/ad açık, ileride kayıtlar da). Admin'in tek farkı **API ayarları** sayfası (`/api`: SMS deneme modu, sağlayıcı, NetGSM ve Twilio bilgileri). Kafe sahibi Hesaplar sayfasında kafe sahibi rolünde hesap açar/siler/sıfırlar; admin'e ve hizmet sağlayıcı hesaplarına dokunamaz, API yetkili hesap açamaz. Şifre alanları yalnızca yazılır (boş = aynı kalır), denetimde `***`. Her işlem `denetim.csv`'ye `PANEL_*`.
- Ayar kaydı `ayarlar.toml`'u yeniden yazar (yorumlar gider; örnek dosyada duruyor) ve portalı yeniden başlatır; bağlı müşteriler düşmez.

## Adım 8b — Kayıtlar, kullanıcılar, resmi talep (2026-10-03)
- Panel (iki rol de görür): **Kayıtlar** (gün listesi: açık / mühürlü / yedeklendi; dosyayı tabloda görüntüleme, 500 satırlık sayfalar, satır araması; tek dosya indirme — mühürlü `.gz` açılmış CSV olarak, Excel'de açılır; günü paket olarak indirme), **Kullanıcılar** (index.csv'den liste + arama, kişi sayfası: oturumlar ve kaydı olan günler), **Resmi talep** (iç IP / NAT portu / hedef IP + zaman ± tolerans, telefon, MAC; talep paketi: günlerin mühürlü dosyaları + MANIFEST + zincir.txt + doğrulama çıktısı, `.tar`, en fazla 92 gün).
- Komut satırı: `wificorrect ctl ara (--ic-ip IP | --nat-port P | --hedef-ip IP) --zaman "2026-10-02 21:20" [--tolerans SN]`, `ctl ara --telefon N | --mac M [--zaman …]`, `ctl disa-aktar --baslangic G --bitis G --cikti DOSYA`.
- İç IP araması oturumun IP değişimlerini izler (o anda IP'yi hangi oturum tutuyordu). Dosya yolları yalnızca tarih + izinli dosya adı kalıbıyla kabul edilir (dışarı çıkılamaz). Her görüntüleme / indirme / arama denetime: `PANEL_KAYIT_GORUNTULE`, `PANEL_KAYIT_INDIR`, `PANEL_KULLANICI(_ARA)`, `PANEL_TALEP_ARA`, `PANEL_TALEP_PAKET`.
- Cihazda doğrulandı: 2026-10-02 gecesi ilk otomatik mühür `✔ hash ✔ zincir`; gerçek kayıtta iç IP araması doğru oturumu buldu; talep paketi 2 günü içerdi.
- Not: 2026-10-02'deki deneme satırları eski 10 haneli numarayla (`5550000000`) yazılmış; yeni kayıtlar `90…` biçiminde.

## Adım 8c — Portlar ve Wi-Fi (2026-10-03)
- Tek kaynak `/etc/wificorrect/ag.toml` (yoksa varsayılan: enp3s0 alır, enp1s0 verir, Wi-Fi kapalı). Ondan üretilenler: `/etc/network/interfaces.d/wificorrect` (WAN `allow-hotplug … dhcp` + `hwaddress`, `br-hotspot` köprüsü), `/etc/wificorrect/arayuzler.nft` (`define WAN`, `guvenlik.nft` include eder), `/etc/wificorrect/hostapd.conf` (Wi-Fi açıksa; 600), `/etc/issue.d/wificorrect.issue`. `/etc/network/interfaces` artık yalnızca lo + `source`; dnsmasq'tan `no-dhcp-interface` kaldırıldı (zaten yalnızca köprüde dinliyor).
- Panel → **Portlar** (iki rol): her Ethernet için internet alır / verir / kapalı, canlı kablo durumu ve hız; Wi-Fi verir / kapalı, ağ adı, şifre (boş = şifresiz), kanal 1-13; sağ/sol etiketi değiştirme. Kural: tam bir Ethernet alır, en az bir port (Ethernet ya da Wi-Fi) verir.
- Uygulama: panel yeni ayarı `/var/lib/wificorrect/ag-yeni.toml`'a yazar, `systemd-run` ile 2 sn sonra `wificorrect ctl ag-gecis` çalışır (panel bağlantısı kopsa da). Önce 180 sn'lik geri alma zamanlayıcısı kurulur, sonra: `ifdown -a` (eski dosyalarla) → dosyalar → `ifup -a` + `ifup <wan>` → `nft -f guvenlik.nft` → dnsmasq → `wificorrect-wifi` (hostapd). Panelden **Onayla** denmezse eski ayar döner; **Hemen geri al** düğmesi de var. Bekleyen değişiklik varken cihaz yeniden açılırsa `wificorrect-ag-acilis.service` ağ kalkmadan eski dosyaları yazar. WAN başka porta geçince eski WAN'ın MAC'i yeni porta yazılır (modemdeki .110 rezervasyonu korunur); müşteri tarafına dönen port `permaddr` ile kendi MAC'ine döndürülür. Denetim: `PANEL_PORT`, `PANEL_PORT_ONAY`, `PANEL_PORT_GERI_AL`, `PANEL_PORT_ETIKET`, `AG_UYGULANDI`, `AG_ONAYLANDI`, `AG_GERI_ALINDI neden=sure|elle|acilis`.
- Cihazda doğrulandı (ölü adam anahtarlarıyla): elle yazılmış ağ dosyalarından üretilene geçiş (yeni SSH ile erişim, sahte istemci DHCP aldı, portala yönlendi, HTTPS reddedildi); Wi-Fi test yayını (`WifiCorrect-Test`, WPA2, kanal 6, köprüde) açıldı ve onaylanmayınca 3 dk sonra kendiliğinden kapandı; WAN bilerek kablosuz porta (enp1s0) taşındı → cihaz erişilemez oldu → 3 dk sonra eski ayar geri geldi, aynı IP (192.168.1.110) ile erişim döndü.
- Yapılmadı (gerekince): Wi-Fi'nin internet alması (istemci modu; `wpasupplicant` paketi gerekir), ath9k "bağlı ama internet yok" bekçisi (Wi-Fi gerçekten kullanılınca bakılacak).

## Yasaklı siteler ve kelimeler (2026-10-03)
- Panel → **Yasaklı siteler** (iki rol). Listeler boş başlar (kullanıcı kararı); kafe sahibi yazar. Ayar `ayarlar.toml` `[filtre]` (`siteler`, `kelimeler`, `istisnalar`).
- **Site** → `/etc/wificorrect/yasak-siteler.conf` (`address=/site/`, dnsmasq `conf-file` ile okur): alan adı ve bütün alt adları NXDOMAIN; sorgu DNS kaydına düşer.
- **Kelime** → `iptables` (nf_tables arka ucu) + `xt_string`: `INPUT -i br-hotspot -p udp|tcp --dport 53 -j WFC_DNS`; zincirde önce istisnalar `RETURN`, sonra kelimeler `REJECT` (UDP: port ulaşılamaz → "site bulunamadı"; TCP: sıfırlama), büyük/küçük harf duyarsız. Bütün DNS cihaza yönlendirildiği için başka DNS yazmak atlatmaz; DoH atlatır. Kelime başına eşleşme sayısı panelde.
- Paket: `iptables` kuruldu (Debian 13, 1.8.11, nf_tables). Açılışta `wificorrect-guvenlik` → `ctl filtre-uygula` (kurallar kalıcı değil, her açılışta yeniden).
- Panelde "Bu adres engelli mi?" denemesi; değişiklik hemen geçerli (site listesi değişince dnsmasq yeniden başlar, kelimede yalnızca zincir). Denetim: `PANEL_FILTRE_EKLE`, `PANEL_FILTRE_KALDIR`.
- Cihazda sahte istemciyle doğrulandı (sorgu 8.8.8.8'e gönderilse de): site → NXDOMAIN, `bet` kelimesi → `superbet.com.tr`, `m.bet365.com` reddedildi, `alphabet` istisnası açık, `google.com` açık. Test sonrası listeler boşaltıldı.

## AP (erişim noktası) — 2026-10-03
- Kuralı (kullanıcı): her kafede AP farklı olabilir; hepsi önceden hazırlanmış gelir. Sistem AP'ye özel hiçbir şey bilmez/gerektirmez: AP sol porta (müşteri köprüsü) takılır, müşteriler IP'yi cihazdan alır.
- AP hazırlık listesi: çalışma modu Access Point · **DHCP sunucusu kapalı** (Smart IP / otomatik DHCP gibi "gerekirse kendin dağıt" modları da kapalı; yönetim IP'si sabit, ör. 10.50.0.2/24, ağ geçidi 10.50.0.1 — DHCP havuzu .20–.249 dışında) · ağ adı önceden belirlenmiş · şifresiz · **istemci yalıtımı (AP isolation) açık** · **yönetim parolası fabrika ayarında bırakılmamalı** (ağ şifresiz; müşteri AP'nin kendi sayfasına ulaşabilir).
- İlk denenen: TP-Link TL-WA901ND (70:4f:57:dd:7a:e8), 100 Mb/s; `GoztepeBilgisayar_Misafir`, şifresiz, DHCP kapalı, AP isolation açık, sabit 10.50.0.2. Telefon AP üzerinden 10.50.0.x aldı, portal açıldı. AP yönetimine bilgisayardan: `ssh -L 8091:10.50.0.2:80 wificorrect` → http://127.0.0.1:8091 (TP-Link girişten sonra kendi IP'sine yönlendirir; aynı yolu tünel adresiyle açmak gerekir).
- Portal başlığı işletme adını kurulum ekranından alır (`main.site_name`): "<ad>'ye hoş geldiniz", ek ünlü uyumuyla kesme işaretinden sonra (Bocafe'ye, Starbucks'a, Hilton Otel'e). Bu cihazdaki eski sistemden kalma "Bocafe" adı varsayılana ("İşletme") çevrildi; kurulumda girilen ad geçer.

## Panel hareketleri — kafe sahibinin izlenmesi (2026-10-03, kullanıcı isteği)
- İstek: kafe sahibinin hesabının da kaydı tutulsun; kafe sahibinin müşterileri sürekli izlemesi istenmiyor.
- Kişisel veri gösteren her sayfa açılışı denetime yazılır, kim + rol + IP ile (`kullanici=mudur rol=sahip ip=…`): `PANEL_OTURUMLAR` (bağlı cihazlar), `PANEL_KULLANICILAR` / `PANEL_KULLANICI_ARA` / `PANEL_KULLANICI`, `PANEL_KAYIT_GUN`, `PANEL_KAYIT_GORUNTULE` (her sayfa), `PANEL_KAYIT_INDIR`, `PANEL_TALEP_ARA`, `PANEL_TALEP_PAKET`; ayrıca `PANEL_GIRIS`, `PANEL_CIKIS`. Denetim günlük mühürlenir; panelden silinemez.
- Bu sayfalarda uyarı: "her görüntüleme, arama ve indirme kimin yaptığıyla birlikte kaydedilir ve hizmet sağlayıcı tarafından denetlenir".
- **Panel hareketleri** sayfası (yalnızca admin): hesap başına son 7 gün / dönem kişisel veriye bakma sayısı, son giriş; hesap ve "yalnızca kişisel veri" süzgeci, son 1000 hareket.

## Geliştirme notu: cihaza erişim iki kez koptu (2026-10-03) — sebep benim test yöntemimdi
- Belirti: SSH ve ping'e (ARP) cevap yok; iki kez fişten kapatıp açmak gerekti. İlk teşhis (derlemede bellek tükenmesi / ısınma) **yanlıştı**: ikinci olayda önceki açılışın günlüğü cihazın sağlam çalıştığını (testler bitti, cron çalıştı) ve düzgün kapandığını gösterdi; sıcaklık 46–48 °C.
- Gerçek sebep: sahte istemci testlerinde temizlik için `ip netns exec … pkill dhcpcd` çalıştırmıştım. netns süreçleri ayırmaz → cihazın **kendi WAN dhcpcd'si** de öldü (günlük: 16:32:15 `dhcpcd[749]: received SIGTERM`); WAN IP'si kira süresi dolunca düştü. `dhcpcd -1` kira alınca zaten kendiliğinden çıkar, `pkill` gereksizdi.
- **İkinci, kalıcı hata (aynı gün bulundu):** ağ geçişi (`ag::switch`: port değişikliği, fabrika dönüşü, 8c geçişi) `ifup <wan>`'ı geçici bir systemd işinden çalıştırıyordu → dhcpcd o işin cgroup'unda doğuyor, iş bitince systemd onu da öldürüyordu; IP kira sonunda (modem kirası 1 saat) düşüyordu. İlk kopma 8c geçişinden ~1 saat sonraydı. Düzeltme: WAN artık `systemctl restart ifup@<wan>.service` ile kalkar (dhcpcd her zaman o birimde yaşar). Cihazda doğrulandı: `ctl ag-uygula` geçici işten çalıştı, iş bitti, yeni dhcpcd `ifup@enp3s0.service` içinde yaşıyor.
- **WAN DHCP bekçisi** (kaydedici, 30 sn'de bir): internet portunda kablo takılıyken dhcpcd 1 dk'dan uzun yoksa `ifup@<wan>.service` yeniden başlatılır (en çok 5 dk'da bir), denetime `WAN_DHCP_YENIDEN`. Kablo yoksa ya da dhcpcd var ama IP alamıyorsa dokunmaz. Cihazda denendi: dhcpcd bilerek durduruldu, 85 sn sonra geri geldi, IP ve internet sağlam.
- Önlem: sahte istemci yalnızca `scripts/sahte-istemci.sh` ile (dhcpcd'ye sinyal göndermez, netns'i temizler). Derleme/test bellek sınırı (`scripts/test.sh`, `gelistir.sh`, MemoryMax=1200M) zararsız olduğu için kaldı; testler optimizasyonsuz ~2,5 dk.

## Giriş sayfası: üç metin ve açık tasarım (2026-10-03, kullanıcı isteği; örnek Starbucks portalı)
- Üç metin: **Açık Rıza Metni** (onay kutusu, varsayılan isteğe bağlı — KVKK'da hizmet rızaya bağlanamaz; panelden zorunlu yapılabilir), **İnternet Kullanım Sözleşmesi** (onay kutusu, zorunlu), **Aydınlatma Metni** (yalnızca açılır metin). Eski "KVKK okudum" kutusu ve `/kvkk` sayfası kaldırıldı.
- Metinler `<details>` ile formun içinde açılır (JS yok; ayrı sayfaya gidip dönünce telefonların giriş penceresinde yazılanlar silinmesin).
- Metinler boş başlar; panel → **Portal metinleri** (iki rol) — `ayarlar.toml` `[portal]` (`aydinlatma`, `acik_riza`, `sozlesme`, `acik_riza_zorunlu`). Düz metin, boş satır paragraf; kaçışlanır. Kaydedince portal yeniden başlar; denetim `PANEL_PORTAL_METIN` (değişen metin ve uzunluğu).
- Oturum kaydına onaylar yazılır: `OTURUM_BASLA` ek `… sozlesme=1 riza=0|1`.
- Tasarım: açık zemin, üstte gökyüzü bandı, üstüne binen beyaz kart, yeşil düğme, mavi metin bağlantıları.
- **Sözleşme metni** (2026-10-03, kullanıcı metni): "Son Kullanıcı Lisans Sözleşmesi" ürünle gelir (`src/sablon/sozlesme.txt`, `[portal] sozlesme` boşsa değil, anahtar yoksa varsayılan); panelden değiştirilebilir. Metindeki **İŞLETMECİ** giriş sayfasında kafe adıyla değişir; kesme işaretinden sonraki ek ünlü uyumuna göre yeniden kurulur (Bocafe’nin, Starbucks’un, Kahvecim’den), büyük harfli başlıkta ad da büyük (BOCAFE’NİN). İyelik ekli adlarda (Kahve Dünyası’ya) genel kural uygulanır. Köşeli parantezli bağlantılar düz `www.wificorrect.com` yazıldı (müşteri giriş yapmadan internete çıkamaz). Tamamı büyük harfli satırlar kalın gösterilir.
- **KVKK Aydınlatma Metni** (2026-10-03, kullanıcı metni; veri sorumlusu Göztepe Bilgisayar): ürünle gelir (`src/sablon/aydinlatma.txt`), giriş sayfasındaki başlık "KVKK Aydınlatma Metni". Metinlerde `* ` / `• ` / `- ` ile başlayan satırlar madde listesi olarak gösterilir.

## Her işletme için (2026-10-03, kullanıcı kararı)
- Ürün yalnızca kafelere değil otel ve internet veren her işletmeye: arayüzde "kafe" → "işletme" (kurulum: **İşletme adı**, rol: **İşletme sahibi**, Ayarlar: İşletme adı).
- Kurulumda ve Ayarlar'da **İşletme unvanı** (`main.unvan`, 2-200 karakter), notu: "Vergi levhasındaki unvan yazılmalıdır".
- **Açık rıza**: metin (kullanıcı metni, `src/sablon/acik_riza.txt`; "okudum e bu" yazım hatası "okudum ve bu" yapıldı) Starbucks örneğindeki gibi onay kutusunun yanında yazar; işaretlemek **her zaman zorunlu** (eski isteğe bağlı/zorunlu seçeneği kaldırıldı — hukuki değerlendirme kullanıcıda). Sözleşmenin adı her yerde açık rıza metnindeki gibi "İnternet Kullanıcı Sözleşmesi" (metnin kendi başlığı "Son Kullanıcı Lisans Sözleşmesi" kullanıcı metni olarak kaldı). Metindeki `[vergi levhası unvanı]` → `main.unvan`; unvan girilmemişse yer tutucu görünür. Panelden düzenlenebilir.

## Admin ayarları ve fabrika ayarları (2026-10-03, kullanıcı isteği)
- "API ayarları" → **Admin ayarları** (`/admin-ayarlari`, yalnızca admin): SMS deneme modu, sağlayıcı, NetGSM, Twilio, **SMS sınırları**, **kayıt saklama süresi**, **uzak yedek** (sunucu adı dahil), sağlayıcı durumu ve fabrika ayarları. İşletme sahibinin Ayarlar sayfasında yalnızca işletme bilgileri (ad, unvan, oturum süresi, telefon başına cihaz).
- İşletme sahibi sağlayıcı ve sunucu adlarını hiçbir yerde görmez: Özet'te yalnızca "Müşterilere doğrulama SMS'i şu an gönderilmiyor; hizmet sağlayıcınıza başvurun" (ayrıntısız), Sistem'de uzak yedek satırı yok.
- **Fabrika ayarları** (admin parolası + onay kutusu): `wificorrect ctl fabrika` ayrı systemd işinde — açık oturumlar kapanır (`neden=fabrika`), admin dışındaki hesaplar silinir (kurulum ekranı gelir), ayarlar ürün varsayılanı (işletme adı/unvanı, SMS bilgileri, yedek, metinler, yasaklı listeler; sistem yolları ve müşteri ağı adresleri korunur), yasaklı listeler uygulanır, portlar varsayılan (Ethernet 1 alır, Ethernet 2 verir, Wi-Fi kapalı; bekleyen port değişikliği silinir), servisler yeniden başlar. Denetim: `PANEL_FABRIKA`, `PANEL_FABRIKA_RED`, `FABRIKA_IPTAL`, `FABRIKA_AYARI`. Cihazda henüz çalıştırılmadı.
- **5651 kayıtları ve fabrika dönüşü** (kullanıcı kararı, seçenek 3): uzak yedek kapalıysa işlem başlamaz. Açıksa: kayıt yazan servisler durur, oturumlar kapanır, bugün dahil bütün günler mühürlenir ve sunucuya gönderilir (zincir.txt dahil — yedek artık zincir gönderilemezse de hata verir); bir gün bile gönderilemezse **iptal** (hiçbir şey silinmez, ayarlara dokunulmaz, servisler geri açılır). Hepsi gönderildiyse yerel kayıtlar (günler, zincir.txt, index.csv) silinir; işletme sahibi kayıtlarını sunucudaki panelden görür. Yeni işletmenin kaydı `FABRIKA_AYARI` satırıyla başlar.
- **Şimdi yedekle ve durum** (2026-10-03): Admin ayarları'nda "Şimdi yedekle" (`systemd-run … ctl yedekle`) ve Durum'da "Son yedek" (denetimdeki son `YEDEK_SONUC`: zaman + sonuç ya da HATA). Yedek artık gönderilecek gün olmasa da zincir.txt'yi gönderir → her çalıştırmada sunucu bağlantısı denenir; `muhur::backup` sonucu `Result`. Fabrika dönüşü sürüyorsa ya da iptal olduysa Admin ayarları'nın en üstünde mesaj (`PANEL_FABRIKA` / `FABRIKA_IPTAL` + sebep).

## Uzak erişim — WireGuard, kendi sunucumuz (2026-10-03, kullanıcı kararı: seçenek A)
- Cihaz: `wireguard-tools` kuruldu (kernel modülü Debian'da var). Gizli anahtar cihazda üretilir (`/etc/wificorrect/wg.key`, 600), cihazdan çıkmaz; açık anahtar Admin ayarları → Durum'da. Ayar `[uzak]` (`enabled`, `sunucu` adres:port, `sunucu_anahtar`, `adres` 10.99.0.x) yalnızca admin. Kaydedince `systemd-run … ctl uzak-uygula` → `/etc/wireguard/wfc.conf` + `wg-quick@wfc` (açılışta kendiliğinden). Tünel yalnızca 10.99.0.0/24'ü taşır (AllowedIPs), PersistentKeepalive 25 (NAT arkası). Denetim `UZAK_ERISIM`.
- Güvenlik duvarı (`guvenlik.nft`): `iifname "wfc" ip saddr 10.99.0.0/24` → yalnızca tcp 22, 8443 ve ping; forward yok (müşteri/modem ağına geçiş yok).
- Bekçi (kaydedici): uzak erişim açıkken arayüz yoksa ya da 10 dk'dır el sıkışma yoksa `wg-quick@wfc` yeniden başlatılır (en çok 10 dk'da bir; sunucu adı açılışta çözülemediyse ya da IP değiştiyse), denetim `UZAK_YENIDEN`.
- Sunucu: `scripts/sunucu/wg-sunucu.sh` (`kur <genel-adres>`, `yonetici-ekle <ad>`, `cihaz-ekle <ad> <anahtar>`, `liste`). Adresler: sunucu .1, yöneticiler .2–.9, cihazlar .10–.254. Sunucu yönlendirmesi: yalnızca yönetici → cihaz (22, 8443, ping), cihaz ↔ cihaz ve tünelden internete yok. UDP 51820 açık olmalı.
- Uçtan uca bağlantı sunucu bilgisi gelince denenecek.
