# BOCAFE Hotspot — 5651 Uyumlu, NetGSM OTP (SMS) Doğrulamalı Captive Portal

> **Master Engineering Dokümanı** — Claude Code (Opus 5.5) ile uygulanmak üzere yazılmıştır.
> Hedef cihaz: Intel Celeron J1900 / 4 GB RAM / 32 GB SSD / 2× Ethernet / OpenWrt (2025-02-27 derlemesi)
> Sahip: Göztepe Bilgisayar · Müşteri: Bocafe · SMS sağlayıcı: NetGSM (OTP paketi, "goztepebilgisayar" hesabı)

> ⚖️ **Hukuki not:** Bu doküman teknik bir tasarımdır, hukuki görüş değildir. 5651 sayılı Kanun, İnternet Toplu
> Kullanım Sağlayıcıları Hakkında Yönetmelik ve 6698 sayılı KVKK ile ilgili saklama süresi, zaman damgası
> sağlayıcısı ve aydınlatma metni gibi konular canlıya almadan önce bir hukukçuya doğrulatılmalıdır.
> Dokümanda **⚠ DOĞRULA** ile işaretli her madde, uygulamadan önce cihazda veya resmi kaynaktan kontrol edilmelidir.

---

## İçindekiler

0. [Claude Code için çalışma kuralları](#0-claude-code-için-çalışma-kuralları)
1. [Proje özeti, hedefler, kapsam dışı](#1-proje-özeti-hedefler-kapsam-dışı)
2. [Varsayımlar ve açık sorular](#2-varsayımlar-ve-açık-sorular)
3. [Yasal çerçeve (5651 / Yönetmelik / KVKK)](#3-yasal-çerçeve)
4. [Donanım ve ağ topolojisi](#4-donanım-ve-ağ-topolojisi)
5. [Mimari](#5-mimari)
6. [Teknoloji seçimleri ve gerekçeleri](#6-teknoloji-seçimleri-ve-gerekçeleri)
7. [Dizin yapısı (repo ve cihaz)](#7-dizin-yapısı)
8. [Konfigürasyon: /etc/config/hotspot](#8-konfigürasyon-etcconfighotspot)
9. [Ağ yapılandırması (network / dhcp / firewall)](#9-ağ-yapılandırması)
10. [nftables tasarımı](#10-nftables-tasarımı)
11. [Captive portal uygulaması](#11-captive-portal-uygulaması)
12. [NetGSM OTP entegrasyonu](#12-netgsm-otp-entegrasyonu)
13. [Oturum yönetimi](#13-oturum-yönetimi)
14. [Loglama ve CSV şeması](#14-loglama-ve-csv-şeması)
15. [Log bütünlüğü, saklama, yedekleme](#15-log-bütünlüğü-saklama-yedekleme)
16. [Yönetim aracı: hotspotctl](#16-yönetim-aracı-hotspotctl)
17. [Güvenlik sertleştirme](#17-güvenlik-sertleştirme)
18. [Performans ve kapasite](#18-performans-ve-kapasite)
19. [Test stratejisi](#19-test-stratejisi)
20. [Uygulama fazları (Faz 0 → Faz 8)](#20-uygulama-fazları)
21. [Operasyon runbook](#21-operasyon-runbook)
22. [Riskler ve bilinen sınırlamalar](#22-riskler-ve-bilinen-sınırlamalar)
23. [Ekler](#23-ekler)

---

## 0. Claude Code için çalışma kuralları

Bu bölüm, bu dokümanı okuyan ajanın (Claude Code) **uyması zorunlu** kurallardır.

### 0.1 Genel
- Bu doküman **tek doğru kaynaktır** (single source of truth). Bir tasarım kararı değişirse önce bu dokümanı güncelle, sonra kodu.
- Fazları (bkz. §20) **sırayla** uygula. Bir fazın kabul kriterleri sağlanmadan sonrakine geçme.
- Her fazın sonunda: ne yapıldı, neler doğrulandı, neler doğrulanamadı — kısa rapor ver.
- **⚠ DOĞRULA** etiketli maddeleri varsayma; cihazda komutla ya da resmi dokümandan kontrol et, sonucu bu dokümana not düş (§23.6 "Doğrulama günlüğü").
- Kod tanımlayıcıları İngilizce; kullanıcıya görünen metinler, CSV başlıkları, CLI çıktıları Türkçe (CSV başlıkları ASCII Türkçe: `zaman`, `olay`, `telefon`…).

### 0.2 Çalışma ortamı
- Geliştirme makinesi: Windows 11 (bu klasör: `C:\Users\HUAWEI\Desktop\openwrt`). Cihaza **SSH** ile erişilir.
- Repo kök dizini bu klasördür. İlk iş `git init` (kullanıcı onayıyla). Sırlar (NetGSM şifresi vb.) **asla** repoya girmez; `files/usr/lib/hotspot/hotspot.example` şablon olarak tutulur (`/etc/config` altına konmaz, çünkü UCI oradaki her dosyayı paket sayar).
- `.gitattributes` ile tüm dosyalar **LF** satır sonuyla tutulur; CRLF'li bir kabuk betiği cihazda çalışmaz. `deploy.sh` CRLF bulursa durur.
- Dağıtım `scripts/deploy.sh` (Git Bash) ile yapılır: `files/` altındaki dosyalar (yalnızca dosyalar, dizin girdisi yok, sahip `0:0`) `tar | ssh` ile cihazdaki `/` altına açılır. Tek dosya kopyalamak gerekirse `scp -O` kullan (dropbear SFTP desteklemez; `-O` eski SCP protokolünü zorlar).

### 0.3 Kullanıcı onayı olmadan YAPILMAYACAKLAR
Aşağıdakiler geri dönüşü zor veya maliyetli işlemlerdir. Her biri için önce kullanıcıya ne yapılacağını ve riskini söyle, **açık "evet" bekle**:
1. Disk bölümlendirme, `mkfs`, `resize2fs`, bölüm tablosu değişikliği.
2. `sysupgrade`, firmware yeniden yükleme, `firstboot` / fabrika ayarına dönüş.
3. **Gerçek SMS gönderimi** (para harcar). Geliştirme boyunca `netgsm_mock=1` kullanılır; gerçek SMS testi yalnızca kullanıcının verdiği numaraya ve kullanıcı onayıyla.
4. Ağ/firewall değişiklikleri — yalnızca **ölü adam anahtarı** (§0.4) kurulduktan sonra.
5. Log dizinlerinden (`/srv/5651`) herhangi bir dosyayı silmek (yasal delil niteliğinde).
6. Cihazda paket kaldırmak.

### 0.4 Kilitlenmeme (lock-out) protokolü — ZORUNLU
Ağ veya firewall değiştirmeden önce:
```sh
# 1) Yedek al
mkdir -p /root/yedek && cp /etc/config/network /etc/config/firewall /etc/config/dhcp /root/yedek/
# 2) Ölü adam anahtarı: 5 dk içinde iptal edilmezse eski ayarları geri yükler
( sleep 300; cp /root/yedek/network /root/yedek/firewall /root/yedek/dhcp /etc/config/; \
  /etc/init.d/network restart; /etc/init.d/firewall restart; /etc/init.d/dnsmasq restart ) >/dev/null 2>&1 &
echo $! > /tmp/geri_al.pid
# 3) Değişikliği uygula, YENİ bir SSH oturumu açarak erişimi doğrula
# 4) Erişim sağlamsa iptal et:
kill "$(cat /tmp/geri_al.pid)"
```
Ağ değişikliğinden sonra mevcut SSH oturumuna güvenme; her zaman **yeni** bağlantıyla doğrula.

### 0.5 Kod kuralları
- Python: **yalnızca standart kütüphane** (pip yok). Hedef Python sürümü cihazdaki paket sürümüdür (⚠ DOĞRULA `python3 --version`, ≥3.10 beklenir).
- Kabuk betikleri: POSIX `sh` (busybox ash). Bash'e özgü sözdizimi yok.
- `subprocess` çağrıları **her zaman liste argümanla**, `shell=True` asla.
- Tüm HTML çıktısı `html.escape` ile kaçışlanır. Tüm CSV hücreleri §14.6'daki kurala göre temizlenir.
- Dosya yazımları: durum dosyaları için "geçici dosyaya yaz → `os.replace`" (atomik). Log dosyaları yalnızca **append**.
- Her mantıksal modül için **bir** çalıştırılabilir test dosyası (`tests/test_*.py`, düz `assert`, framework yok). Windows'ta `python tests/test_x.py` ile çalışmalı (cihaz bağımlılığı olmayan saf fonksiyonlar).
- Karmaşıklıktan kaçın: tek sınıf/tek fonksiyon yetiyorsa soyutlama ekleme. Bilinçli basitleştirmeleri `# ponytail:` yorumuyla sınırını belirterek işaretle.

---

## 1. Proje özeti, hedefler, kapsam dışı

### 1.1 Özet
Bocafe'deki misafir Wi-Fi ağı (`bocafe` SSID) bir OpenWrt cihazının arkasına alınacak. Ağa bağlanan her müşteri, internete çıkmadan önce bir **captive portal** ile karşılaşacak; **ad, soyad, cep telefonu** girecek, KVKK aydınlatma metnini onaylayacak, telefonuna **NetGSM OTP** ile gelen 6 haneli kodu girecek. Doğrulama başarılı olursa cihaz, istemcinin **MAC + IP** çiftini yetkilendirecek ve internet erişimi açılacak.

Sistem her doğrulanan kişi için telefon numarası adıyla bir CSV dosyası (`5334553132.csv`) oluşturacak; dosyada ad, soyad, MAC adresi, oturum başlangıç/bitişleri ve kişinin **internet trafik kayıtları** (bağlantı meta verisi + DNS sorguları) yer alacak. Ayrıca 5651 kapsamındaki **iç IP dağıtım logları** günlük dosyalar halinde tutulacak, günlük olarak hash zinciri ve zaman damgası ile mühürlenecek.

### 1.2 Hedefler (ölçülebilir)
| # | Hedef | Kabul ölçütü |
|---|---|---|
| H1 | Doğrulanmamış istemci internete çıkamaz | HTTP → portala yönlenir; HTTPS ve diğer tüm portlar anında reddedilir |
| H2 | Captive portal otomatik açılır | Android, iOS, Windows, macOS'ta "Ağda oturum açın" bildirimi çıkar |
| H3 | SMS OTP ile doğrulama | Kod ≤ 30 sn içinde gelir, 180 sn geçerlidir, 5 hatalı denemede geçersizleşir |
| H4 | Kullanıcı kaydı | Doğrulamada `/srv/5651/kullanicilar/<telefon>.csv` oluşur/güncellenir |
| H5 | Trafik loglama | Doğrulanmış istemcinin her bağlantısı ve DNS sorgusu ≤ 5 sn içinde kullanıcı CSV'sine ve günlük dosyaya yazılır |
| H6 | 5651 iç IP dağıtım logu | Her DHCP atama/yenileme/bırakma olayı zaman, IP, MAC ile kaydedilir |
| H7 | Bütünlük | Her gün 00:15'te önceki günün dosyaları sıkıştırılır, SHA-256 manifest + hash zinciri + (varsa) RFC 3161 zaman damgası üretilir |
| H8 | Dayanıklılık | Yeniden başlatma / firewall reload sonrası aktif oturumlar korunur |
| H9 | Resmi talep sorgusu | "Şu tarih-saatte şu iç IP / şu NAT portu kimdeydi?" sorusu tek komutla cevaplanır |
| H10 | Maliyet koruması | Günlük SMS tavanı ve numara/MAC başına hız sınırı uygulanır |

### 1.3 Kapsam dışı (bilinçli olarak yapılmayacak)
- HTTPS içeriği / tam URL loglama (teknik olarak mümkün değil, yasal olarak da gerekmez; içerik dinleme hukuken sorunludur).
- Web tabanlı yönetim paneli (ilk sürümde CLI yeterli; ihtiyaç doğarsa sonraki sürüm).
- Kupon/voucher ile giriş, sosyal medya ile giriş.
- IPv6 (hotspot ağında kapatılır; loglama yalnızca IPv4).
- Bant genişliği kısıtlama (QoS) — ihtiyaç halinde SQM ile sonraki sürüm.
- Birden fazla AP / VLAN yönetimi.

---

## 2. Varsayımlar ve açık sorular

### 2.1 Varsayımlar (aksi söylenmedikçe geçerli)
| # | Varsayım | Neden önemli |
|---|---|---|
| V1 | "Otomatik kullanıcı" = sistemde bir **kayıt** (CSV + oturum). Linux kullanıcı hesabı değildir. | Mimari |
| V2 | Kafe, internet kullanımı için **ücret almıyor** → "ticari amaç dışında toplu kullanım sağlayıcı". | Lisans/filtre yükümlülükleri (§3) |
| V3 | Modemin LAN'ı bir özel ağ (ör. `192.168.1.0/24`); OpenWrt WAN'dan DHCP ile IP alıyor (çift NAT). | NAT port logu, yönetim erişimi |
| V4 | AP bridge/erişim noktası modunda, DHCP kapalı, SSID `bocafe`. | Tüm istemciler OpenWrt DHCP'sinden IP alır |
| V5 | Günlük tahmini kullanıcı: 20–150; eşzamanlı ≤ 60. | Kapasite (§18) |
| V6 | Oturum süresi **30 gün** (2026-09-26 kullanıcı kararı; ayda bir yeniden SMS). Süre müşteriye gösterilmez. | §13 |
| V7 | Saklama süresi varsayılanı **730 gün (2 yıl)**. | §3, §15 |
| V8 | Yönetim erişimi modem tarafından (WAN bölgesi, yalnızca modem LAN alt ağından). | §9.4 |

### 2.2 Açık sorular — kullanıcıya Faz 0'da sorulacak
| # | Soru | Varsayılan (cevap gelmezse) |
|---|---|---|
| S1 | NetGSM **usercode** (abone no / kullanıcı adı), **API alt kullanıcı şifresi**, **mesaj başlığı (msgheader)** tam olarak nedir? Başlık NetGSM panelinde tanımlı ve onaylı mı? (Alfanümerik başlıklar en fazla **11 karakter**dir; "goztepebilgisayar" 17 karakter olduğu için başlık muhtemelen farklıdır, ör. `GOZTEPEBLGS`.) | Faz 3 mock ile ilerler |
| S2 | NetGSM hesabında **OTP SMS paketi aktif** mi? API erişiminde **IP kısıtlaması** var mı? Varsa hangi IP'ler? | — |
| S3 | İnternet hattının **sabit IP**'si var mı? (NetGSM IP kısıtlaması ve 5651 dış IP eşlemesi için) | Dinamik kabul edilir |
| S4 | Modem **bridge moduna** alınabilir mi? (OpenWrt doğrudan genel IP alırsa NAT port logları dış dünyayla birebir eşleşir.) | Hayır (çift NAT) |
| S5 | SSID **açık** mı olacak, **WPA2 şifreli** mi? (Açık ağda portal trafiği havada şifresizdir.) | Açık ağ + AP'de istemci izolasyonu |
| S6 | Oturum süresi kaç saat? | 12 saat |
| S7 | Saklama süresi (hukukçu görüşüyle)? | 730 gün |
| S8 | Zaman damgası sağlayıcısı: lisanslı bir ESHS (ör. TÜBİTAK Kamu SM, e-Tuğra) mı, yoksa şimdilik yalnızca hash zinciri mi? | Hash zinciri + yapılandırılabilir RFC 3161 TSA |
| S9 | Yedek hedefi: USB disk mi, uzak sunucu/NAS (SSH) mı? | USB disk |
| S10 | AP marka/model? İstemci izolasyonu (client isolation) destekliyor mu? | Destekliyor kabul edilir |
| S11 | Personel/kasa cihazları portalsız mı geçsin? MAC listesi? | Boş liste |
| S12 | Portal görünümü: logo, renkler, kafe adı yazımı ("Bocafe" / "BOCAFE")? | Sade, "Bocafe" |
| S13 | Aynı telefon numarası kaç cihaza aynı anda izinli? | 3 cihaz |

---

## 3. Yasal çerçeve

> Aşağıdaki bilgiler teknik tasarıma girdi olması için özetlenmiştir. **⚠ DOĞRULA (hukukçu)**.

### 3.1 5651 sayılı Kanun ve Toplu Kullanım Sağlayıcı Yönetmeliği
- Kafe, restoran, otel gibi yerlerde müşterilere internet sunan işletmeler **"toplu kullanım sağlayıcı"**dır.
- Ücret alınmıyorsa **"ticari amaç dışında"** toplu kullanım sağlayıcıdır; internet kafeler gibi izin belgesi / BTK onaylı filtre yazılımı yükümlülüğü genellikle ticari olanlar içindir (V2). ⚠ DOĞRULA.
- Temel teknik yükümlülük: **iç IP dağıtım loglarının** (hangi iç IP, hangi tarih-saat aralığında, hangi cihaza (MAC) verildi) elektronik ortamda kaydedilmesi, **doğruluğunun, bütünlüğünün ve gizliliğinin** sağlanması ve yönetmelikte öngörülen süre boyunca saklanması. Yönetmelikteki asgari süre ile güncel uygulama farklı yorumlanabildiğinden sistem **730 gün** varsayılanıyla tasarlanır (yapılandırılabilir). ⚠ DOĞRULA.
- Bütünlüğün ispatı için sektörde yaygın uygulama: log dosyalarının **hash**lenmesi ve **zaman damgası** ile mühürlenmesi. Bu sistem her iki mekanizmayı da sağlar (§15).
- SMS ile kimlik doğrulama kanunen zorunlu değildir; ancak IP/MAC ↔ gerçek kişi eşlemesini sağlayarak işletmeyi korur.

### 3.2 Neyi loglayıp neyi loglamıyoruz
| Loglanan | Loglanmayan |
|---|---|
| DHCP: zaman, iç IP, MAC, cihaz adı, olay (atama/yenileme/bırakma) | Sayfa içerikleri, form verileri, mesajlar |
| Oturum: telefon, ad, soyad, MAC, IP, başlangıç, bitiş | HTTPS URL yolu (ör. `/profil/123`) |
| Bağlantı meta verisi: protokol, iç IP:port, hedef IP:port, **NAT dış IP:port**, başlangıç, bitiş, bayt | Şifreler, çerezler |
| DNS sorguları: sorgulanan alan adı, sorgu tipi | TLS SNI (ilk sürümde yok) |
| Denetim: OTP istekleri/başarı/başarısızlık (kodun kendisi hariç) | OTP kodunun kendisi |

### 3.3 KVKK (6698)
- Ad, soyad, telefon, MAC, trafik meta verisi **kişisel veridir**.
- İşleme dayanağı: **hukuki yükümlülük** (5651) ve **sözleşmenin ifası** (internet hizmeti). Portalda **aydınlatma metni** gösterilir ve "okudum" onay kutusu zorunludur. Pazarlama amaçlı kullanım **yoktur**; varsa ayrı açık rıza gerekir (kapsam dışı).
- Veri minimizasyonu: yalnızca gerekli alanlar. Saklama süresi dolan veriler **silinir** (§15.5).
- Erişim: log dizini yalnızca `root` (izin `700/600`). Yedekler şifreli olmalı (§15.6).
- Aydınlatma metni taslağı: §23.5 (hukukçuya onaylatılacak).

---

## 4. Donanım ve ağ topolojisi

### 4.1 Donanım
| Bileşen | Değer | Not |
|---|---|---|
| CPU | Intel Celeron J1900 (4 çekirdek, x86_64) | 100+ Mbps NAT + Python servisleri için fazlasıyla yeterli |
| RAM | 4 GB | `/tmp` tmpfs; conntrack tablosu rahat |
| Disk | 32 GB SSD | Kök FS küçük (OpenWrt imajı ~120 MB); kalan alan `/srv` bölümüne |
| NIC | 2× Ethernet | Biri WAN (modem), biri HOTSPOT (AP). Hangisinin hangisi olduğu Faz 0'da tespit edilir |
| RTC | Var | NTP yine de zorunlu (log zamanları) |

### 4.2 Topoloji
```
          İnternet
              │
      ┌───────┴────────┐
      │     MODEM      │  LAN: 192.168.1.0/24 (V3, ⚠ DOĞRULA)
      │  (NAT, DHCP)   │  Yönetici PC de bu ağda olabilir
      └───────┬────────┘
              │ ethX  (WAN, DHCP istemcisi → ör. 192.168.1.10)
      ┌───────┴──────────────────────────────────────────────┐
      │  OpenWrt  (J1900)                                     │
      │   • nftables: table inet fw4  (OpenWrt firewall)     │
      │   • nftables: table inet hotspot (bizim; portal kapısı)│
      │   • dnsmasq: DHCP + DNS (sorgu logu)                  │
      │   • hotspot-portal  (Python, 10.50.0.1:8080)          │
      │   • hotspot-logger  (Python, conntrack + DNS + oturum)│
      │   • /srv  (SSD bölüm 3, ext4) → 5651 logları          │
      └───────┬──────────────────────────────────────────────┘
              │ ethY  (HOTSPOT, 10.50.0.1/24, DHCP sunucusu)
      ┌───────┴────────┐
      │  ACCESS POINT  │  SSID: bocafe · DHCP KAPALI · istemci izolasyonu AÇIK
      │  yönetim IP:   │  10.50.0.2 (statik)
      └───────┬────────┘
         📱 💻 📱  Müşteriler: 10.50.0.20 – 10.50.0.249
```

### 4.3 Adresleme
| Ağ | Değer | Gerekçe |
|---|---|---|
| Hotspot alt ağı | `10.50.0.0/24` | Modem LAN'ı (`192.168.x`) ve OpenWrt varsayılanı (`192.168.1.1`) ile çakışmasın |
| Router hotspot IP | `10.50.0.1` | Ağ geçidi, DNS, portal |
| AP yönetim IP | `10.50.0.2` (statik, AP üzerinde ayarlanır) | |
| DHCP havuzu | `10.50.0.20 – 10.50.0.249` (230 adres) | |
| DHCP kira süresi | `2h` | IP değişimi seyrek, loglar sık yenilenir |
| Portal | `http://10.50.0.1:8080/` | |

---

## 5. Mimari

### 5.1 Bileşenler
| Bileşen | Tür | Sorumluluk |
|---|---|---|
| `table inet hotspot` (nftables) | Çekirdek kuralı | Yetkisiz istemcinin HTTP'sini portala yönlendir, diğer her şeyi reddet; DNS'i zorla router'a; yetkili `MAC . IP` setinde olanları geçir |
| dnsmasq | OpenWrt yerleşik | DHCP + DNS; sorgu logu (`logqueries`) syslog'a |
| `/etc/hotplug.d/dhcp/90-hotspot` | sh | Her DHCP olayını günlük `dhcp.csv`'ye yazar (5651 iç IP dağıtım logu) |
| `hotspot-portal` | Python, procd servisi | Portal HTTP sunucusu, form doğrulama, OTP üretme/gönderme/doğrulama, oturum açma, nft setine ekleme |
| `hotspot-logger` | Python, procd servisi | `conntrack -E` ve `logread -f` akışlarını okuyup IP→oturum eşlemesiyle CSV'lere yazar; süresi dolan oturumları kapatır |
| `hotspotctl` | Python CLI | Durum, oturumlar, at (kick), arama (resmi talep), gün kapatma (mühürleme), temizlik, test SMS |
| cron | OpenWrt yerleşik | 00:15 gün kapatma, 02:00 yedek + yerel temizlik |
| `/srv/5651` | ext4 bölüm | Tüm yasal loglar, kullanıcı CSV'leri, manifestler |
| `/srv/hotspot/state` | ext4 bölüm | `sessions.json` (aktif oturumlar), `sms_sayac.json` |

### 5.2 Kimlik doğrulama akışı (sequence)
```
İstemci               AP        OpenWrt nft          Portal (8080)            NetGSM        Logger
  │ Wi-Fi bağlan       │              │                    │                     │             │
  │── DHCP DISCOVER ──▶│─────────────▶│ dnsmasq: 10.50.0.23 ata ─▶ hotplug → dhcp.csv          │
  │── HTTP GET captive.apple.com ────▶│ yetkisiz → redirect :8080                │             │
  │◀──────────────── 302 Location: http://10.50.0.1:8080/ ─│                     │             │
  │── GET / (form) ──────────────────────────────────────▶│                     │             │
  │── POST /kod-gonder (ad, soyad, tel, kvkk) ───────────▶│ doğrula, hız sınırı │             │
  │                                                        │── OTP XML ─────────▶│             │
  │                                                        │◀── code=0, jobID ───│             │
  │◀──────────────────────────── kod giriş sayfası ────────│  denetim.csv: OTP_GONDERILDI      │
  │  📩 SMS: "Bocafe WiFi kodunuz: 482915"                 │                     │             │
  │── POST /dogrula (kod) ───────────────────────────────▶│ karşılaştır          │             │
  │                                   │◀── nft add element auth {MAC . IP timeout 12h}        │
  │                                                        │ sessions.json yaz, <tel>.csv: KAYIT/OTURUM_BASLA
  │◀──────────────────────────── "Bağlandınız" ────────────│                     │             │
  │── HTTPS google.com ──────────────▶│ yetkili → geçir → fw4 → wan (masq)      │             │
  │                                   │ conntrack NEW/DESTROY ───────────────────────────────▶│ <tel>.csv + trafik.csv
  │── DNS sorgusu ───────────────────▶│ dnsmasq log ─────────────────────────────────────────▶│ <tel>.csv + dns.csv
  │           ... 12 saat sonra ...                                                           │ süre doldu → OTURUM_BITIS
```

### 5.3 Veri akışı özeti
```
dnsmasq DHCP ──(hotplug)──────────────▶ /srv/5651/gunluk/YYYY-MM-DD/dhcp.csv
portal       ──────────────────────────▶ /srv/5651/gunluk/YYYY-MM-DD/oturum.csv, denetim.csv
             ──────────────────────────▶ /srv/5651/kullanicilar/<tel>.csv   (KAYIT, OTURUM_*)
             ──────────────────────────▶ /srv/hotspot/state/sessions.json
conntrack -E ──▶ logger ──(IP→oturum)──▶ /srv/5651/gunluk/YYYY-MM-DD/trafik.csv
                                    └──▶ /srv/5651/kullanicilar/<tel>.csv   (BAGLANTI_*)
logread -f   ──▶ logger ──(IP→oturum)──▶ /srv/5651/gunluk/YYYY-MM-DD/dns.csv
                                    └──▶ /srv/5651/kullanicilar/<tel>.csv   (DNS)
cron 00:15   ──▶ hotspotctl gun-kapat ─▶ *.csv.gz + MANIFEST.sha256 (+ .tsq/.tsr)
```

**Önemli ilke:** Yasal delil niteliğindeki **birincil kaynak günlük dosyalardır** (`gunluk/`), çünkü bunlar gün sonunda kapanır ve mühürlenir. Kullanıcı başına CSV (`kullanicilar/<tel>.csv`) aynı verinin **kişi bazlı görünümüdür**; sürekli büyüdüğü için tek başına mühürlenemez, ancak her gün sonunda o anki SHA-256 değeri manifeste yazılır (§15.2).

---

## 6. Teknoloji seçimleri ve gerekçeleri

| Karar | Seçim | Reddedilen alternatifler ve neden |
|---|---|---|
| Captive portal motoru | **Kendi nftables tablomuz + küçük Python portal** | **openNDS**: güçlü ama OTP gibi çok adımlı akış için ThemeSpec/FAS kabuk betikleriyle uğraşmak gerekir, paket bakım durumu snapshot'larda belirsiz. **CoovaChilli**: RADIUS gerektirir, fazla ağır, bakımı zayıf. **nodogsplash**: eski, iptables tabanlı. Bizim ihtiyacımız ~10 nft kuralı + bir set; kendi çözümümüz daha az hareketli parça demek. |
| Firewall entegrasyonu | fw4'ten **bağımsız** `table inet hotspot` | fw4 include'larına (chain-pre) gömmek mümkün, ama `fw4 reload` her seferinde fw4 tablosunu yeniden oluşturur ve **setler boşalır** → tüm müşteriler düşer. Ayrı tablo fw4 reload'dan etkilenmez. |
| Portal/Logger dili | **Python 3 (yalnızca stdlib)** | **ucode**: OpenWrt'nin yerlisi ama HTTP istemcisi, CSV, güvenli rastgele sayı için dış araç çağırmak gerekir. **sh/CGI**: POST ayrıştırma, URL decode, CSV kaçışlama hataya çok açık. J1900 + 32 GB SSD'de Python'un ~30 MB'lık yeri sorun değil; `http.server`, `urllib`, `csv`, `secrets`, `hmac`, `json` hepsi stdlib. |
| Web sunucusu | `http.server.ThreadingHTTPServer` | uhttpd ikinci instance + CGI mümkün, ama tek Python süreci OTP durumunu bellekte tutabilir, daha basit. |
| Konfigürasyon | **UCI** (`/etc/config/hotspot`) | JSON dosyası daha basit olurdu ama UCI OpenWrt standardıdır, `sysupgrade -b` yedeğine otomatik girer, `uci set` ile düzenlenir. |
| Trafik kaynağı | **`conntrack -E`** (conntrack-tools) | ulogd2 daha sağlam ama ek yapılandırma ve paket; nft `log` kuralı her paketi loglar (çok fazla). conntrack akış başına 2 olay verir ve NAT eşlemesini içerir. |
| DNS kaynağı | dnsmasq `logqueries` → syslog → `logread -f` | dnsmasq'ı dosyaya loglatmak ujail/izin sorunları çıkarabilir; `logread -f` jail dışında ve rotasyon gerektirmez. |
| DHCP logu | `/etc/hotplug.d/dhcp/` | OpenWrt dnsmasq'ı DHCP olaylarını zaten hotplug'a yayınlar; ek dnsmasq ayarı gerekmez. ⚠ DOĞRULA: `cat /usr/lib/dnsmasq/dhcp-script.sh` |
| Oturum anahtarı | `ether_addr . ipv4_addr` | Yalnızca MAC: sahte MAC ile kolay taklit. MAC+IP çifti taklidi zorlaştırır (imkânsız kılmaz, bkz. §22). |
| Oturum süresi | nft set elemanında `timeout` | Süre dolumu çekirdek tarafından garanti edilir; logger yalnızca kaydı kapatır. |
| Log formatı | CSV, `;` ayraçlı, UTF-8 **BOM**'lu | Türkçe Excel `;` ayraçlı ve BOM'lu dosyayı doğrudan doğru açar. |
| Mühürleme | SHA-256 hash zinciri + opsiyonel RFC 3161 (`openssl ts`) | Hash zinciri ücretsiz ve her zaman çalışır; TSA eklemesi yapılandırmayla. |

---

## 7. Dizin yapısı

### 7.1 Repo (bu klasör)
```
openwrt/
├── MASTER_ENGINEERING.md          # bu doküman
├── CLAUDE.md                       # (opsiyonel) "@MASTER_ENGINEERING.md" içeren tek satır
├── .gitignore                      # *.local, __pycache__/, docs/kesif.txt
├── .gitattributes                  # * text eol=lf
├── files/                          # cihazdaki "/" ile birebir eşleşir
│   ├── etc/
│   │   ├── init.d/hotspot          # procd: nft tablosu + portal + logger
│   │   ├── hotplug.d/dhcp/90-hotspot
│   │   └── sysctl.d/90-hotspot.conf
│   └── usr/
│       ├── lib/hotspot/
│       │   ├── hotspot.example     # UCI şablonu (sırlar boş); deploy yoksa /etc/config/hotspot'a kopyalar
│       │   ├── common.py           # config, yollar, zaman, CSV yazıcı, durum, MAC bulma, doğrulayıcılar, nft
│       │   ├── netgsm.py           # OTP gönderimi (+ mock)
│       │   ├── portal.py           # HTTP sunucusu ve akış
│       │   ├── logger.py           # conntrack + dns + oturum süresi
│       │   ├── ctl.py              # hotspotctl alt komutları
│       │   ├── hotspot.nft         # nftables tablosu
│       │   └── templates/          # _sayfa.html (yerleşim), giris.html, kod.html, basarili.html, hata.html, kvkk.html
│       └── sbin/hotspotctl         # exec python3 /usr/lib/hotspot/ctl.py "$@"
├── scripts/
│   ├── deploy.sh                   # scp -O ile files/ → cihaz, servis yeniden başlatma
│   └── kesif.sh                    # Faz 0 keşif komutlarını çalıştırıp çıktıyı docs/kesif.txt'ye yazar
├── tests/
│   ├── fixtures/                   # cihazdan alınmış GERÇEK conntrack/logread/leases örnekleri
│   ├── test_common.py              # telefon/ad doğrulama, CSV temizleme, MAC bulma
│   ├── test_logger.py              # conntrack ve dnsmasq satır ayrıştırıcıları
│   ├── test_portal.py              # hız sınırı, OTP yaşam döngüsü (saat enjekte edilerek)
│   └── test_netgsm.py              # XML oluşturma / yanıt ayrıştırma
└── docs/
    ├── kesif.txt                   # Faz 0 çıktısı
    └── kvkk-aydinlatma.md          # hukukçu onaylı metin
```

### 7.2 Cihaz
```
/etc/config/hotspot                 (600) UCI yapılandırması, NetGSM sırları
/etc/init.d/hotspot                 procd servisi
/etc/hotplug.d/dhcp/90-hotspot      DHCP olay logu
/etc/sysctl.d/90-hotspot.conf       conntrack acct/timestamp
/usr/lib/hotspot/…                  uygulama
/usr/sbin/hotspotctl
/srv/                               SSD bölüm 3 (ext4, noatime), 700
├── 5651/
│   ├── gunluk/2026-09-25/          açık gün (O GÜNÜN TÜM TRAFİĞİ): dhcp.csv, oturum.csv, trafik.csv, dns.csv, denetim.csv
│   │   └── kullanicilar/5334553132.csv   kişinin o günkü dosyası (ilk satır KAYIT: ad, soyad, MAC)
│   ├── gunluk/2026-09-24/          kapalı gün: *.csv.gz (+ kullanicilar/*.csv.gz), MANIFEST.sha256, (.tsq/.tsr), .yedeklendi
│   ├── kullanicilar/index.csv      telefon;ad;soyad;ilk_kayit;son_oturum (tüm kullanıcıların listesi, küçük)
│   └── zincir.txt                  gün;manifest_sha256;onceki_zincir_sha256
│   (Günler cihazda retention_days (730 gün = 2 yıl) boyunca durur; her gün ayrıca her gece sunucuya gönderilir, §15.5)
└── hotspot/state/
    ├── sessions.json
    └── sms_sayac.json
```

---

## 8. Konfigürasyon: /etc/config/hotspot

```uci
config hotspot 'main'
	option enabled '1'
	option iface 'eth1'                 # hotspot NIC (Faz 0'da kesinleşir)
	option router_ip '10.50.0.1'
	option subnet '10.50.0.0/24'
	option portal_port '8080'
	option session_minutes '43200'      # 30 gün (müşteriye gösterilmez)
	option max_devices_per_phone '3'
	option retention_days '730'
	option log_root '/srv/5651'
	option state_root '/srv/hotspot/state'
	option csv_delimiter ';'
	option site_name 'Bocafe'

config netgsm 'netgsm'
	option mock '1'                     # 1: SMS gönderme, kodu logread'e yaz (geliştirme)
	option url 'https://api.netgsm.com.tr/sms/send/otp'   # ⚠ DOĞRULA
	option usercode ''
	option password ''
	option msgheader ''                 # NetGSM panelindeki onaylı başlık, birebir
	option appkey ''                    # opsiyonel
	option message 'WiFi dogrulama kodunuz: {kod}. Kod 3 dakika gecerlidir.'
	option timeout_sec '10'

config limits 'limits'
	option otp_ttl_sec '180'
	option otp_max_attempts '5'
	option resend_cooldown_sec '60'
	option sms_per_phone_15min '3'
	option sms_per_phone_day '10'
	option sms_per_mac_hour '5'
	option sms_global_day '300'         # maliyet tavanı
	option verify_fail_lock_min '15'

config tsa 'tsa'
	option enabled '0'
	option url ''                       # RFC 3161 uç noktası
	option username ''
	option password ''
	option ca_file '/etc/hotspot/tsa-ca.pem'

config backup 'backup'
	option enabled '0'
	option target ''                    # uzak sunucu, ör. yedek@sunucu:/arsiv/bocafe (sunucu tarafı kullanıcıda)
	option ssh 'ssh -i /root/.ssh/yedek_anahtar'

# Portalsız geçen cihazlar (AP, personel) — yine de loglanır
config allow
	option mac 'aa:bb:cc:dd:ee:ff'
	option name 'AP yonetim'
```

- Dosya izni **600**. `deploy.sh` bu dosyayı **ezmez**; yalnızca yoksa `/usr/lib/hotspot/hotspot.example`'dan kopyalar.
- `common.load_config()`: `uci -q show hotspot` çıktısını ayrıştırır (`hotspot.main.iface='eth1'` biçimi). Anonim `allow` bölümleri liste olarak döner. Tip dönüşümleri (int) tek yerde.
- Eksik zorunlu değer (ör. `mock=0` iken boş `usercode`) → servis başlarken net Türkçe hata ile çıkar.

---

## 9. Ağ yapılandırması

> Bu bölümdeki her değişiklik §0.4 kilitlenmeme protokolüyle uygulanır. Arayüz adları (`eth0`/`eth1`) Faz 0 keşfine göre değiştirilir.

### 9.1 /etc/config/network
```uci
config interface 'wan'
	option device 'eth0'
	option proto 'dhcp'

config interface 'hotspot'
	option device 'eth1'
	option proto 'static'
	option ipaddr '10.50.0.1'
	option netmask '255.255.255.0'

config device
	option name 'eth1'
	option ipv6 '0'                 # hotspot tarafında IPv6 kapalı
```
- Mevcut `lan` arayüzü (varsayılan `br-lan`) kaldırılır ya da hotspot'a dönüştürülür. Yönetim erişimi önce WAN tarafında sağlanmalı (§9.4), sonra `lan` kaldırılmalı.
- `wan6` arayüzü isteğe bağlı; hotspot'ta IPv6 kapalı olduğu için etkisi yok.

### 9.2 /etc/config/dhcp
```uci
config dnsmasq
	# … mevcut seçenekler korunur, logqueries AYARLANMAZ (aşağıdaki dnsmasq.conf satırı yeterli) …
	# Hotspot istemcilerinin router DNS'ini atlamasını zorlamak nft'de yapılır (§10)

config dhcp 'hotspot'
	option interface 'hotspot'
	option start '20'
	option limit '230'
	option leasetime '2h'
	option dhcpv4 'server'
	option dhcpv6 'disabled'
	option ra 'disabled'
	list dhcp_option '6,10.50.0.1'  # DNS = router
	# RFC 8910 captive portal URI (opsiyonel; bazı cihazlar yalnızca https URI'yi dikkate alır) ⚠ DOĞRULA
	# list dhcp_option '114,http://10.50.0.1:8080/'

config dhcp 'lan'                   # varsa kaldırılır
```
- `/etc/config/system` içinde logd tampon boyutu büyütülür: `option log_size '2048'` (KB). DNS sorgu logları logd tamponundan akar; logger `logread -f` ile gerçek zamanlı okur.
- DNS sorgu logu: `/etc/dnsmasq.conf`'a `log-queries=extra` satırı eklenir (seri no + istemci IP/port içeren biçim). ⚠ DOĞRULA: satır biçimi Faz 2'de `logread -e dnsmasq` ile alınıp `tests/fixtures/dnsmasq.txt`'ye kaydedilir; ayrıştırıcı hem `extra` hem düz biçimi kabul eder.

### 9.3 /etc/config/firewall
```uci
config zone
	option name 'hotspot'
	list network 'hotspot'
	option input 'REJECT'
	option output 'ACCEPT'
	option forward 'REJECT'

# hotspot → wan için "config forwarding" YOKTUR (fail-closed).
# "table inet hotspot" yetkili paketlere 0x5651 işareti koyar; fw4 yalnızca işaretli paketleri wan'a geçirir.
# Bizim tablo yüklenmemişse (servis çöktü, yapılandırma hatası) hiçbir paket işaretlenmez → kimse internete çıkamaz.
config rule
	option name 'HS-Yetkili-Internet'
	option src 'hotspot'
	option dest 'wan'
	option proto 'all'
	option mark '0x5651'            # ⚠ DOĞRULA: fw4 print | grep 5651
	option target 'ACCEPT'

config rule
	option name 'HS-DHCP'
	option src 'hotspot'
	option proto 'udp'
	option dest_port '67'
	option family 'ipv4'
	option target 'ACCEPT'

config rule
	option name 'HS-DNS'
	option src 'hotspot'
	option proto 'tcp udp'
	option dest_port '53'
	option target 'ACCEPT'

config rule
	option name 'HS-Portal'
	option src 'hotspot'
	option proto 'tcp'
	option dest_port '8080'
	option target 'ACCEPT'

config rule
	option name 'HS-Ping'
	option src 'hotspot'
	option proto 'icmp'
	option icmp_type 'echo-request'
	option family 'ipv4'
	option target 'ACCEPT'

# Yönetim erişimi: yalnızca modem LAN alt ağından (V3, ⚠ DOĞRULA alt ağı)
config rule
	option name 'Yonetim-SSH-LuCI'
	option src 'wan'
	option src_ip '192.168.1.0/24'
	option proto 'tcp'
	option dest_port '22 443'
	option target 'ACCEPT'
```
- Yazılımsal/donanımsal **flow offloading kapalı** olmalı (`option flow_offloading '0'`): offload edilen akışlarda bayt sayaçları ve conntrack olayları güvenilmez olabilir.
- wan bölgesinde `masq '1'` (varsayılan) kalır.

### 9.4 Yönetim erişimi
- SSH (dropbear) ve LuCI (uhttpd, HTTPS) **yalnızca** WAN bölgesinden ve yalnızca modem LAN alt ağından erişilebilir.
- Hotspot bölgesinden 22/80/443 **kapalıdır** (input REJECT; yalnızca 53/67/8080/ICMP açık). Portal, 80 → 8080 yönlendirmesiyle çalışır; LuCI'ye hotspot'tan erişilemez.
- SSH: anahtar tabanlı giriş; parola girişi kapatılır (`dropbear.@dropbear[0].PasswordAuth='off'`, `RootPasswordAuth='off'`) — **ancak anahtarla girişin çalıştığı yeni bir oturumla doğrulandıktan sonra**.
- Modem bridge moduna alınırsa (S4) yönetim erişim stratejisi yeniden ele alınmalı (WAN genel IP olur; SSH'ı internete açmak yerine hotspot'ta ayrı yönetim MAC'i veya VPN).

### 9.5 Zaman
```sh
uci set system.@system[0].zonename='Europe/Istanbul'
uci set system.@system[0].timezone='<+03>-3'
uci delete system.ntp.server
uci add_list system.ntp.server='0.tr.pool.ntp.org'
uci add_list system.ntp.server='1.tr.pool.ntp.org'
uci add_list system.ntp.server='time.google.com'
uci commit system && /etc/init.d/system reload && /etc/init.d/sysntpd restart
```
Tüm log zamanları ISO 8601, `+03:00` ofsetli yazılır: `2026-09-24T14:30:05+03:00`.

### 9.5a Cihazın kendi Wi-Fi yayını (2026-09-26)
- Müşteri tarafı köprüdür: `br-hotspot` = `eth0` (kablolu Zyxel AP) + `phy0-ap0` (cihazın USB Wi-Fi'si). `hotspot.main.iface='br-hotspot'`; nft kuralları, DHCP ve portal ikisinde de aynı çalışır.
- Kart: **Atheros AR9287 mini PCIe** (AR5B97, 2x2, 300 Mbps, `kmod-ath9k`, `radio1`, arayüz `phy1-ap0`) — iki anten kablosu takılı olmalı. Yedek: Atheros AR9271 USB (`kmod-ath9k-htc`, `radio0`, kapalı). Ortak: `wpad-basic-mbedtls`, `iw`. SSID `Bocafe Misafir`, WPA2-PSK (anahtar yalnızca cihazdaki `/etc/config/wireless`'ta, repoda yok), kanal 13 / HT20, ülke TR, `isolate '1'`.
- İstemci izolasyonu köprü seviyesinde de sağlanır: `table bridge hotspot` köprü portları arası iletimi düşürür (Zyxel müşterisi ↔ cihaz Wi-Fi müşterisi birbirini göremez).
- Denenip çalışmayan kartlar: Realtek RTL8723AE (23.05'te sürücü paketi yok), Broadcom BCM43142 (açık kaynak sürücü yok).
- **Kablosuz takılma bekçisi** (2026-09-26): ath9k + iPhone'da istemci bağlı kalıp kartın aldığı çerçeveler mac80211'de düşürülebiliyor ("bağlı ama internet yok", yeniden bağlanınca düzelir; ölçüm: 3 sn'de karta ~170 çerçeve, köprüye 0). `table bridge hotspot` içindeki `wlan_rx` dinamik seti köprüye ulaşan paketleri MAC başına sayar; logger her 30 sn'de `iw station dump` ile karşılaştırır, karta ≥100 çerçeve gelip köprüye 0 ulaşan istemciyi `ubus call hostapd.<arayüz> del_client` ile düşürür (telefon birkaç saniyede geri bağlanır), denetime `WIFI_TAKILMA` yazar. Boştaki telefon 30 sn'de ~6 çerçeve gönderir → yanlış alarm yok.
- **Şifre modu değişiminde tam yeniden başlatma şart** (2026-09-29): WPA2 → şifresiz geçiş `wifi reload` ile (hostapd "Reload config for bss") yapılırsa eski CCMP grup anahtarı kartta kalır (`/sys/kernel/debug/ieee80211/phy*/keys/` dolu); mac80211 şifresiz gelen tüm veri çerçevelerini düşürür → telefon bağlanır ama DHCP alamaz, portal açılmaz. Çözüm: `wifi down; wifi up` (panel port uygulaması ve geri alma betiği bunu kullanır). Kontrol: açık ağda `keys/` boş olmalı.
- Kurulum tuzağı: `wpad` sonradan kurulursa ubusd yeni ACL'yi (`/usr/share/acl.d/wpad_acl.json`) okumaz → yalıtılmış hostapd ubus'a kaydolamaz → `HOSTAPD_START_FAILED`. Çözüm: `kill -HUP $(pidof ubusd); /etc/init.d/wpad restart; /etc/init.d/network restart` (netifd de kablosuz betiğini yalnızca açılışta yükler) ya da yeniden başlatma.

### 9.5b Huawei HG658 V2 AP (2026-09-26, Superonline firmware HG658V2C163B021)
- Bağlantı: Huawei LAN portu → OpenWrt `eth0` (br-hotspot). WAN/DSL portu kullanılmaz. Kabul testi: telefon `huawei_ap`'den 10.50.0.x aldı, portal + OTP ile internete çıktı, trafik/DNS logları yazıldı.
- Yayın: **SSID2 = `huawei_ap`, şifresiz, AP yalıtımı açık.** Ana SSID kapalı (adı `SUPERONLINE_WiFi_1048`, eski WPA şifresiyle). Superonline firmware'i ana SSID'de "Yok" güvenlik modunu reddeder ("Yok modu güvenli değildir"); ek SSID'lerde bu kısıt yok.
- DHCP sunucusu, RA ve IPv6 DHCP kapalı. Ana LAN IP 192.192.1.1 (doğrudan kabloyla yönetim), ikincil LAN IP 10.50.0.3/24 — ancak firmware yönetimi ikincil adreste sunmuyor (ping/HTTP cevapsız).
- LAN MAC 64:6d:6c:a3:20:24 `hotspot` config'inde `allow` listesinde ("Huawei AP").
- Arayüz Ember.js: alan değerleri doğrudan DOM'a yazılırsa model güncellenmez, kayıt eski değeri gönderir — değişiklikler `change`/`input` olaylarıyla ya da gerçek tıklamayla yapılmalı.

### 9.5c Port görevleri paneli (2026-09-29, kullanıcı isteği)
- Panel → **Portlar** (yalnızca hizmet sağlayıcı). Ethernet 1 (sağ, `eth1`), Ethernet 2 (sol, `eth0`) ve Wi-Fi kartı (`radio1`) için görev: **internet alır** / **internet verir** / kapalı. Wi-Fi "internet alır" = istemci modu (`wireless.wwan`, `mode sta`, `network wan`; ağ adı + şifre ya da şifresiz), bu sırada müşteri yayını kapanır. Varsayılan = mevcut kurulum (eth1 alır, eth0 verir, Wi-Fi `freewifi` yayını).
- Kural: tam olarak bir "internet alır", en az bir "internet verir". Durum UCI'den okunur (`ports.py`), ayrı kopya yok. WAN hangi Ethernet'e geçerse geçsin `network.wan.macaddr` eski WAN MAC'i ile yazılır → modemdeki 192.168.1.110 rezervasyonu korunur.
- Güvenlik ağı: uygula → `/etc/config/{network,wireless}` yedeği `/tmp/port-degisim/<id>/`'ye, `port-geri-al.sh <klasör> 180` arka planda; panelden **3 dakika içinde Onayla** basılmazsa eski ayar geri yüklenir (`network restart` + `wifi reload`). "Hemen geri al" düğmesi aynı betiği 0 sn ile çalıştırır. Onay beklerken yeni değişiklik kabul edilmez. Denetim: `PANEL_PORT`, `PANEL_PORT_ONAY`, `PANEL_PORT_GERI_AL`.
- Sağ/sol etiketi cihazla ters çıkarsa `hotspot.ports.swap` panelden değiştirilir. Canlı kablo durumu `/sys/class/net/<eth>/carrier|speed`.

### 9.6 Access Point ayarları (AP'nin kendi arayüzünden, manuel)
- Mod: Access Point / Bridge. **DHCP sunucusu kapalı** (zaten kapalı).
- SSID: `bocafe`. Güvenlik: S5'e göre açık veya WPA2-PSK. AP destekliyorsa **OWE (Enhanced Open)** açık ağda havayı şifreler.
- **İstemci izolasyonu (client isolation / AP isolation) AÇIK** — müşteriler birbirini göremez.
- Yönetim IP: `10.50.0.2/24`, ağ geçidi `10.50.0.1`, DNS `10.50.0.1`.
- AP'nin MAC'i `hotspot` config'inde `allow` listesine eklenir (NTP/güncelleme için internete çıkabilsin).

---

## 10. nftables tasarımı

### 10.1 Tablo: /usr/lib/hotspot/hotspot.nft
```nft
#!/usr/sbin/nft -f
# fw4'ten bağımsız tablo. fw4 reload bu tabloya ve setlerine dokunmaz.
# Değişkenler init betiği tarafından sed ile doldurulmaz; iface/ip değişirse bu dosya güncellenir. ⚠ Faz 0 sonrası kesinleşir.

define HS_IF = "eth1"
define HS_IP = 10.50.0.1
define PORTAL_PORT = 8080

table inet hotspot
delete table inet hotspot

table inet hotspot {
	# Doğrulanmış istemciler. Eleman: MAC . IP, süre dolunca çekirdek siler.
	set auth {
		type ether_addr . ipv4_addr
		flags timeout
	}

	# Portalsız geçen cihazlar (AP, personel). Loglama yine yapılır.
	set allow_mac {
		type ether_addr
	}

	chain pre_nat {
		type nat hook prerouting priority dstnat - 5; policy accept;

		# 1) Tüm DNS'i router'a zorla (yetkili/yetkisiz herkes) → DNS loglaması eksiksiz olsun
		iifname $HS_IF meta l4proto { tcp, udp } th dport 53 redirect to :53

		# 2) Yetkili / izinli olanlara dokunma
		iifname $HS_IF ether saddr @allow_mac return
		iifname $HS_IF ether saddr . ip saddr @auth return

		# 3) Yetkisizlerin router dışına giden HTTP'sini portala çevir
		iifname $HS_IF ip daddr != $HS_IP tcp dport 80 redirect to :$PORTAL_PORT
	}

	chain gate {  # "fwd" nft anahtar kelimesi, zincir adı olamaz
		type filter hook forward priority filter - 5; policy accept;

		# Hotspot'tan IPv6 yok
		iifname $HS_IF meta nfproto ipv6 drop

		# Hotspot istemcileri özel ağlara (modem arayüzü, yönetim ağı vb.) asla gidemez
		iifname $HS_IF ip daddr { 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 100.64.0.0/10, 169.254.0.0/16 } reject with icmpx type admin-prohibited

		# DNS-over-TLS engeli (DNS loglamasını atlatmasın). DoH (443) engellenemez → §22
		iifname $HS_IF tcp dport 853 reject with tcp reset

		# Yetkili / izinli → işaretle, fw4'e devam (fw4 yalnızca 0x5651 işaretlileri wan'a geçirir + masquerade)
		iifname $HS_IF ether saddr @allow_mac meta mark set 0x5651 accept
		iifname $HS_IF ether saddr . ip saddr @auth meta mark set 0x5651 accept

		# Diğer her şey: hızlı başarısızlık (HTTPS zaman aşımına düşmesin)
		iifname $HS_IF meta l4proto tcp reject with tcp reset
		iifname $HS_IF reject with icmpx type admin-prohibited
	}
}
```

### 10.2 Notlar ve doğrulamalar
- **Neden 443 yönlendirilmiyor?** HTTPS'i portala yönlendirmek sertifika hatası üretir. İşletim sistemleri captive portal tespitini **HTTP** sondalarıyla yapar (Android: `connectivitycheck.gstatic.com/generate_204`, iOS/macOS: `captive.apple.com/hotspot-detect.html`, Windows: `www.msftconnecttest.com/connecttest.txt`). HTTP'nin yönlendirilmesi ve HTTPS'in anında reddedilmesi, portal açılır penceresinin çıkması için yeterlidir.
- Çoklu tablo davranışı: bir paket, aynı kancadaki **tüm** tabloların base chain'lerinden `accept` almalıdır. Bizim tablodaki `accept`, fw4'teki `forward` zincirini atlatmaz. Bu yüzden bizim tablo yetkili paketlere `0x5651` işareti koyar ve fw4'teki `HS-Yetkili-Internet` kuralı yalnızca işaretli paketleri wan'a geçirir (§9.3). Bizim `reject`, fw4'ten önce (öncelik `filter - 5`) devreye girer.
- **Fail-closed:** Tablo yoksa işaret de yoktur → fw4 hotspot'tan wan'a hiçbir şeyi geçirmez. `/etc/init.d/hotspot` yapılandırma eksikse (ör. boş `iface`) tabloyu yüklemeden hata verir; sonuç yine "kimse çıkamaz"dır, "herkes çıkar" değil.
- ⚠ DOĞRULA: `ether saddr` eşleşmesinin inet ailesinde `forward` kancasında çalıştığı (Ethernet'ten gelen paketlerde MAC başlığı mevcut). Test: `nft add rule inet hotspot gate iifname eth1 ether saddr <mac> counter` ile sayaç artışını gör, ya da `nft monitor trace`.
- ⚠ DOĞRULA: `priority dstnat - 5` ve `filter - 5` sözdiziminin cihazdaki nft sürümünde kabulü (`nft --version`).
- Eleman ekleme / silme (Python `common.nft_*` bu komutları liste argümanla çağırır):
  ```sh
  nft add element inet hotspot auth '{ aa:bb:cc:dd:ee:ff . 10.50.0.23 timeout 720m }'
  nft delete element inet hotspot auth '{ aa:bb:cc:dd:ee:ff . 10.50.0.23 }'
  nft list set inet hotspot auth          # kalan süreler "expires" alanında
  conntrack -D -s 10.50.0.23              # atılan istemcinin açık bağlantılarını kes
  ```
- `allow_mac` seti init betiğinde UCI `allow` bölümlerinden doldurulur.

### 10.4 DNS kelime filtresi (2026-10-01, kullanıcı isteği)
- Alan adında **`bet`** veya **`porn`** geçen siteler müşteri ağında açılmaz. dnsmasq 2.90 yalnızca alan adı soneki eşleştirir (`*bet*` desenleri hiçbir şeyi engellemedi, cihazda test edildi), bu yüzden filtre DNS sorgusunun içinde kelime arar: `iptables-nft` + `xt_string` (paketler: `iptables-nft`, `iptables-mod-filter`, `kmod-ipt-filter`).
- `dns-filtre.sh` (init betiği her başlatmada çalıştırır): `table ip filter` → `INPUT -i <hotspot iface> -p udp|tcp --dport 53 -j HS_DNS`; zincirde önce `hotspot.filter.allow` kelimeleri `RETURN` (istisna), sonra `hotspot.filter.word` kelimeleri `REJECT` (UDP: ICMP port-unreachable → tarayıcı anında "site bulunamadı" der; TCP: reset). Eşleşme büyük/küçük harfe duyarsız (`--icase`). DNS'in tamamı zaten router'a yönlendirildiği için (§10.1) başka DNS sunucusu yazmak filtreyi atlatmaz.
- Varsayılan istisnalar: `alphabet`, `between`, `better`, `diabet`, `tibet` ("bet" içeren meşru adlar; ör. `ads.betweendigital.com` gerçek kayıtlarda vardı). Liste `uci add_list hotspot.filter.word=…` / `hotspot.filter.allow=…` + `/etc/init.d/hotspot restart` ile değişir.
- Test (loopback'te aynı zincir): `pornhub.com`, `WWW.PoRnHuB.CoM`, `bets10.com`, `m.bet365.com`, `superbet.com.tr` engellendi; `nesine.com`, `google.com`, `alphabet.com`, `betterhelp.com`, `diabetes.org`, `youtube.com` açık. `fw4 reload` ve `firewall restart` kurala dokunmaz.
- Sınırlar: DoH (443 üzerinden şifreli DNS, ör. Android "Özel DNS" ya da tarayıcıda elle açılmış güvenli DNS) bu filtreyi atlatır (§17.3). Engellenen sorgu dnsmasq'a ulaşmadığı için `dns.csv`'de görünmez; sayaç: `iptables-nft -L HS_DNS -v -n`.

### 10.3 sysctl: /etc/sysctl.d/90-hotspot.conf
```
net.netfilter.nf_conntrack_acct=1          # DESTROY olaylarında bayt/paket
net.netfilter.nf_conntrack_timestamp=1     # başlangıç/bitiş zamanı
net.netfilter.nf_conntrack_events=1
net.netfilter.nf_conntrack_max=65536
```

---

## 11. Captive portal uygulaması

### 11.1 Süreç
- `portal.py`, procd altında tek süreç. `ThreadingHTTPServer(("10.50.0.1", 8080))`, soket zaman aşımı 10 sn, istek gövdesi en fazla 4 KB.
- İki tür istek gelir:
  1. **Yönlendirilmiş istekler** (istemci başka bir siteye HTTP ile gitmek istedi, nft 8080'e çevirdi). `Host` başlığı `10.50.0.1:8080` değildir → `302 Location: http://10.50.0.1:8080/?dst=<orijinal-url-kodlanmış>` döner. Gövde boş, `Cache-Control: no-store`.
  2. **Portalın kendi istekleri** (`Host: 10.50.0.1:8080`) → aşağıdaki uç noktalar.
- Yanıtlara her zaman: `Cache-Control: no-store`, `X-Frame-Options: DENY`, `Content-Security-Policy: default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:`. `Server` başlığı sade (`hotspot`).
- **Harici kaynak yok, JavaScript yok**: CSS satır içi, logo (S12 gelirse) `_sayfa.html` içinde `data:` URI. Yetkisiz istemci CDN'e erişemez.
- Form `action`'ları mutlak portal adresidir (`http://10.50.0.1:8080/...`); böylece POST istekleri NAT'lanmaz ve oturum açılırken yapılan conntrack temizliğinden etkilenmez.

### 11.2 Uç noktalar
| Yöntem | Yol | İşlev |
|---|---|---|
| GET | `/` | İstemci zaten yetkiliyse "Bağlısınız" sayfası; değilse giriş formu |
| POST | `/kod-gonder` | Form doğrulama → hız sınırı → OTP üret → NetGSM → kod giriş sayfası |
| POST | `/dogrula` | Kod kontrolü → oturum aç → başarılı sayfası |
| POST | `/tekrar-gonder` | Bekleme süresi dolmuşsa aynı bilgilerle yeni kod |
| GET | `/kvkk` | Aydınlatma metni |
| GET | diğer | 302 → `/` |

### 11.3 İstemci kimliği
- IP: `self.client_address[0]` (nft `redirect` kaynak IP'yi korur).
- MAC: önce `/tmp/dhcp.leases` (sütunlar: `bitis_epoch mac ip hostname clientid`), bulunamazsa `/proc/net/arp` (bayrak `0x2` olan satırlar). İkisinde de yoksa → hata sayfası "Cihazınız tanınamadı, Wi-Fi'yi kapatıp açın".
- MAC her zaman küçük harf, `:` ayraçlı normalize edilir.
- IP `subnet` dışındaysa istek reddedilir.

### 11.4 Form doğrulama (`common.py`, test edilir)
| Alan | Kural |
|---|---|
| `ad`, `soyad` | Baş/son boşluk kırp, çoklu boşluğu teke indir. 2–40 karakter. Yalnızca harf (Türkçe dahil: `a-zA-ZçÇğĞıİöÖşŞüÜ` + genel Unicode harfleri `str.isalpha`), boşluk, `-`, `'`, `.`. Rakam yok. |
| `telefon` | Rakam dışı karakterleri at. Başta `90` varsa ve 12 haneyse → `90`'ı at. Başta `0` varsa ve 11 haneyse → `0`'ı at. Sonuç `^5\d{9}$` olmalı (Türkiye GSM). Örn: `+90 (533) 455 31 32`, `0533 455 3132`, `5334553132` → `5334553132`. |
| `kvkk` | İşaretli olmalı. |
| Tümü | Form gövdesi `application/x-www-form-urlencoded`, UTF-8. Bilinmeyen alanlar yok sayılır. |

Hata durumunda form, girilen değerler korunarak (escape'lenmiş) ve alan bazlı Türkçe hata mesajıyla yeniden gösterilir.

### 11.5 OTP yaşam döngüsü (bellek içi)
```python
# pending[mac] = {
#   "phone": "5334553132", "ad": "Ayşe", "soyad": "Yılmaz", "ip": "10.50.0.23",
#   "code": "482915", "expires": <monotonic>, "attempts": 0, "sent_at": <monotonic>
# }
```
- Kod: `f"{secrets.randbelow(10**6):06d}"`.
- Karşılaştırma: `hmac.compare_digest(girilen, beklenen)`.
- Geçerlilik: `otp_ttl_sec` (180). Deneme: `otp_max_attempts` (5) → aşılınca pending silinir, "Yeni kod isteyin".
- `/dogrula` isteğindeki MAC, pending kaydının MAC'i ile eşleşmeli; IP değiştiyse yeni IP kullanılır.
- Süreç yeniden başlarsa pending kaybolur → kullanıcı yeni kod ister (kabul edilebilir).
- Zaman için `time.monotonic()` kullanılır (NTP sıçramalarından etkilenmez); fonksiyonlar test için `now` parametresi alır.

### 11.6 Hız sınırları ve maliyet koruması
| Sınır | Varsayılan | Kapsam | Aşılınca |
|---|---|---|---|
| Tekrar gönderim bekleme | 60 sn | MAC | "X sn sonra tekrar deneyin" |
| Telefon başına | 3 SMS / 15 dk, 10 SMS / gün | Telefon | Hata sayfası |
| MAC başına | 5 SMS / saat | MAC | Hata sayfası |
| Global | 300 SMS / gün | Sistem | "Şu an SMS gönderilemiyor, personele başvurun" + denetim kaydı `SMS_TAVAN` |
| Hatalı doğrulama kilidi | 10 hata → 15 dk | MAC | Hata sayfası |
| Aynı telefona cihaz sayısı | 3 aktif oturum | Telefon | En eski oturum kapatılır (OTURUM_BITIS, neden=`cihaz_limiti`) |

- Kayan pencere sayaçları bellekte (`collections.deque` zaman damgaları). Günlük global sayaç `/srv/hotspot/state/sms_sayac.json`'da (`{"tarih":"2026-09-24","adet":57}`) → süreç yeniden başlasa da tavan korunur.
- Tüm SMS isteği sonuçları `denetim.csv`'ye yazılır.

### 11.7 Başarılı doğrulamada yapılacaklar (sıra önemli)
1. Aynı MAC için aktif oturum varsa (farklı telefon/IP) → onu kapat (nft sil, `OTURUM_BITIS` neden=`yeniden_giris`).
2. Telefonun aktif oturum sayısı `max_devices_per_phone`'u aşıyorsa → en eskiyi kapat.
3. `nft add element … timeout <session_minutes>m` — **başarısız olursa** kullanıcıya hata göster, oturum yazma (tutarsızlık olmasın).
4. `sessions.json`'a ekle (kilitli, atomik): `{mac: {phone, ad, soyad, ip, start_iso, expires_epoch, session_id}}`. `session_id` = `uuid4().hex[:12]`.
5. `kullanicilar/<tel>.csv` yoksa oluştur (BOM + başlık + `KAYIT` satırı); `index.csv`'yi güncelle.
6. `OTURUM_BASLA` satırını hem `<tel>.csv`'ye hem `gunluk/<gün>/oturum.csv`'ye yaz.
7. Denetim: `OTP_BASARILI`.
8. Başarılı sayfası: "Bocafe Wi-Fi'ye bağlandınız." (oturum süresi gösterilmez). Sayfa **1 sn sonra kendiliğinden** `dst` adresine (yalnızca `http(s)://` ise — açık yönlendirme koruması), yoksa `http://www.google.com/`'a yönlenir (`<meta http-equiv="refresh">`, JS yok). Android/iOS giriş penceresi her sayfa yüklemesinde interneti yeniden yoklar ve erişimi görünce kendini kapatır; kullanıcının bir yere dokunması gerekmez. Bilgisayar tarayıcısında sekme kapatılamaz, istenen sayfa açılır.
- Portal üst görseli: gerçekçi bulutlu gökyüzü, SVG fractal-noise ile üretilip ~20 KB JPEG olarak `_sayfa.html` içine `data:` URI ile gömülüdür (harici kaynak yok).

### 11.8 Sayfalar (mobil öncelikli, Türkçe, tek sütun, satır içi CSS)
1. **Giriş**: Logo · "Bocafe Wi-Fi'ye hoş geldiniz" · Ad · Soyad · Cep telefonu (`inputmode="numeric"`, `autocomplete="tel"`, placeholder `5XX XXX XX XX`) · ☐ "[KVKK Aydınlatma Metni](/kvkk)'ni okudum" · [Kod Gönder]
2. **Kod girişi**: "5XX XXX XX 32 numarasına kod gönderdik" (maskeli) · 6 haneli kod (`inputmode="numeric"`, `autocomplete="one-time-code"`, `pattern="\d{6}"`) · [Doğrula] · [Kodu tekrar gönder] (bekleme süresi dolmadan basılırsa sunucu "X sn bekleyin" der) · "Numarayı değiştir"
   - Aynı numara için bekleme süresi içinde "Kod Gönder"e ikinci kez basılırsa (çift tıklama) yeni SMS **gönderilmez**, kod ekranı "Kod az önce gönderildi" notuyla tekrar gösterilir.
3. **Başarılı**: onay + oturum bitiş saati.
4. **Hata**: anlaşılır mesaj + "Başa dön".
5. **KVKK**: §23.5 metni.
- Şablonlar `string.Template` ile (`$ad` gibi yer tutucular; değerler önceden `html.escape`). Şablon motoru yok.
- Erişilebilirlik: her input'un `<label>`'ı, yeterli kontrast, en az 44 px dokunma alanı, `lang="tr"`.

---

## 12. NetGSM OTP entegrasyonu

> ⚠ DOĞRULA (Faz 0): NetGSM'in **güncel** OTP API dokümantasyonu (https://www.netgsm.com.tr/dokuman/). Aşağıdaki XML arayüzü yaygın kullanılan sürümdür; NetGSM daha yeni bir REST/JSON uç noktası sunuyorsa ve hesap onu destekliyorsa onu tercih et ve bu bölümü güncelle.

### 12.1 İstek (XML, POST)
```
POST https://api.netgsm.com.tr/sms/send/otp
Content-Type: application/xml; charset=UTF-8
```
```xml
<?xml version="1.0" encoding="UTF-8"?>
<mainbody>
  <header>
    <usercode>USERCODE</usercode>
    <password>PASSWORD</password>
    <msgheader>BASLIK</msgheader>
    <appkey>APPKEY_OPSIYONEL</appkey>
  </header>
  <body>
    <msg><![CDATA[WiFi dogrulama kodunuz: 482915. Kod 3 dakika gecerlidir.]]></msg>
    <no>5334553132</no>
  </body>
</mainbody>
```
- XML **stdlib `xml.etree.ElementTree`** ile üretilir (string birleştirme yok; kullanıcı girdisi zaten yalnızca rakam).
- Mesaj **ASCII** tutulur (OTP servisinde Türkçe karakter / uzunluk kısıtı olabilir ⚠ DOĞRULA). Tek SMS boyunu (160) aşmaz.
- HTTPS doğrulaması açık (`ssl.create_default_context()`); cihazda `ca-bundle` / `ca-certificates` kurulu olmalı.
- Zaman aşımı `timeout_sec` (10). Yeniden deneme **yok** (çift SMS ve çift maliyet riski); hata kullanıcıya gösterilir, kullanıcı tekrar isteyebilir.

### 12.2 Yanıt
Başarılı yanıt örneği (⚠ DOĞRULA):
```xml
<?xml version="1.0"?><xml><main><code>0</code><jobID>123456789</jobID></main></xml>
```
`netgsm.send_otp(phone, code) -> (ok: bool, netgsm_code: str, job_id: str|None)`. Yanıt XML değilse veya `code` bulunamazsa `ok=False, netgsm_code="PARSE"`.

### 12.3 Hata kodları → kullanıcı / denetim mesajı (⚠ DOĞRULA listeyi resmi dokümandan)
| Kod | Olası anlamı | Kullanıcıya | Yöneticiye (denetim + logread) |
|---|---|---|---|
| 0 | Başarılı | — | — |
| 20 | Mesaj metni/uzunluk hatası | "SMS gönderilemedi, personele başvurun" | Mesaj şablonunu kontrol et |
| 30 | Geçersiz kullanıcı/şifre, API yetkisi yok veya **IP kısıtlaması** | aynı | Kimlik bilgileri / IP izin listesi |
| 40, 41 | Gönderici adı (msgheader) hatalı | aynı | `msgheader` birebir doğru mu |
| 50 | Gönderilen numara hatalı | "Bu numaraya SMS gönderilemiyor, numarayı kontrol edin" | — |
| 60 | Hesapta OTP SMS paketi tanımlı değil | aynı | Paket yenile |
| 70 | Parametre hatası | aynı | XML'i kontrol et |
| 80 | Sorgulama sınır aşımı (dakikada 100 gönderim) | "Çok fazla deneme, biraz sonra tekrar deneyin" | NetGSM tarafı hız sınırı |
| 100 | Sistem hatası | "SMS servisi geçici olarak yanıt vermiyor" | Sonra tekrar |
| ağ/timeout | — | "SMS servisine ulaşılamadı" | WAN / DNS / TLS kontrol |

### 12.4 Mock modu
`netgsm.mock=1` iken HTTP isteği yapılmaz; `logger -t hotspot "MOCK OTP 5334553132 -> 482915"` yazılır ve `(True, "0", "MOCK")` döner. Geliştirme ve kabul testlerinin çoğu mock ile yapılır.

### 12.5 Hesap tarafı kontrol listesi (kullanıcı yapar)
- [ ] NetGSM panelinde **API için alt kullanıcı** oluştur (ana hesap şifresini kullanma), yalnızca SMS/OTP yetkisi ver.
- [ ] OTP paketi aktif ve bakiye yeterli.
- [ ] Mesaj başlığı onaylı; tam yazımı not et.
- [ ] API IP kısıtlaması: sabit IP varsa ekle. Dinamik IP'de kısıtlama sorun çıkarır → ya kapat (daha az güvenli) ya da sabit IP al.
- [ ] Test için kendi numaranı hazırla.

### 12.6 Twilio Verify (alternatif sağlayıcı, 2026-09-28)
- `config twilio 'twilio'` (`enabled`, `mock`, `account_sid` AC…, `verify_sid` VA…, `auth_token` ya da `api_key_sid` SK… + `api_key_secret`, `timeout_sec`). `enabled=1` ise NetGSM yerine Twilio kullanılır; kimlik bilgileri yalnızca panelden (hizmet sağlayıcı) girilir, repoda yok.
- Gönderim: `POST https://verify.twilio.com/v2/Services/<VA>/Verifications` (`To=+90…`, `Channel=sms`, `Locale=tr`); kodu Twilio üretir. Doğrulama: `POST …/VerificationCheck` (`To`, `Code`) → `status=approved`. Portal gerçek modda kodu yerelde değil Twilio'da doğrular; ağ hatasında deneme hakkı düşmez. Hız sınırları ve günlük tavan (§11.6) aynen geçerli. Mock modunda NetGSM mock'u gibi yerel kod + `logread`.

---

## 13. Oturum yönetimi

### 13.1 Durum dosyası: /srv/hotspot/state/sessions.json
```json
{
  "aa:bb:cc:dd:ee:ff": {
    "phone": "5334553132", "ad": "Ayşe", "soyad": "Yılmaz",
    "ip": "10.50.0.23", "session_id": "3f9a1c0b7d2e",
    "start": "2026-09-24T14:30:05+03:00", "expires_epoch": 1790336405
  }
}
```
- Yazan: portal (ekleme), logger (süre dolumu), hotspotctl (at). Okuyan: logger (IP→oturum eşleme), hotspotctl.
- Eşzamanlılık: `fcntl.flock` ile `/srv/hotspot/state/.lock`; yazma = geçici dosya + `os.replace`.
- Logger, dosyanın `mtime`'ı değişince yeniden yükler (her olayda diske gitmez) ve bellekte `ip → oturum` sözlüğü tutar.

### 13.2 Oturum sonu nedenleri
| Neden | Tetikleyen | Nasıl |
|---|---|---|
| `sure_doldu` | Logger (30 sn'de bir kontrol) | nft elemanını çekirdek zaten sildi; logger `expires_epoch < now` olanları `OTURUM_BITIS` ile kapatır (satır zamanı = `expires_epoch`), sessions.json'dan çıkarır |
| `yeniden_giris` | Portal | Aynı MAC yeni doğrulama yaptı (conntrack temizlenmez; kullanıcı zaten yeniden giriyor) |
| `cihaz_limiti` | Portal | Telefon başına cihaz sınırı |
| `yonetici` | `hotspotctl at` | nft sil + `conntrack -D -s IP` |
| `ip_degisti` | Logger | Yalnızca yedek yol: IP değişen oturum yeni IP'ye taşınamazsa (nft hatası) kapatılır. |

> **IP değişimi (30 günlük oturum için zorunlu):** DHCP kirası 2 saat; cihaz ertesi gün farklı IP alabilir. Oturum kapatılmaz, **yeni IP'ye taşınır** (`common.move_session`): portal cihaz sayfaya düştüğü anda, logger 30 sn'lik kontrolünde. nft'de `MAC . eskiIP` silinir, `MAC . yeniIP` kalan süreyle eklenir, `OTURUM_IP_DEGISTI` (ek=`eski_ip=…`) yazılır.
> **Anında taşıma (2026-09-27, kullanıcı isteği):** DHCP hotplug betiği her IP atamasında (`add`/`update`) arka planda `hotspotctl dhcp-olay <mac> <ip>` çağırır (~380 ms): oturumu süren cihaz hangi boş IP'yi aldıysa oturum oraya taşınır (eski IP'de ısrar yok, SMS yok); bu IP'yi tutan başka oturum IP'siz beklemeye alınır. Böylece telefonun bağlantı kontrolünden önce yetki hazırdır, portal açılmaz. Portal ve logger (30 sn) yolu yedek olarak kalır.
> **IP başka cihaza verildiyse:** Logger, DHCP kira tablosunda oturumun IP'si başka MAC'e aitse (telefon ayrılmış) trafiği o oturuma yazmaz (sahibi kira tablosundaki MAC olur) ve 30 sn içinde oturumu IP'siz beklemeye alır.
> **Yanlış atfı önleme:** Bir oturum bir IP'yi aldığında (yeni giriş veya taşıma) aynı IP'yi tutan başka oturum varsa o cihaz artık o IP'de değildir: nft kaydı silinir, oturum **IP'siz beklemeye** alınır (`ip=""`, `OTURUM_IP_DEGISTI` ek=`eski_ip=…`, ic_ip boş). Cihaz dönünce yeni IP'sine taşınır. Böylece aynı IP'de iki oturum olmaz, trafik yanlış kişiye yazılmaz.

### 13.3 Açılış ve yeniden yükleme
`/etc/init.d/hotspot start`:
1. `sysctl -p /etc/sysctl.d/90-hotspot.conf`
2. `nft -f /usr/lib/hotspot/hotspot.nft`
3. `allow_mac` setini UCI'den doldur.
4. `hotspotctl yukle`: `sessions.json`'daki süresi dolmamış oturumları kalan süreyle (`timeout <kalan>s`) nft'ye geri ekle; süresi dolanları `OTURUM_BITIS neden=sure_doldu` ile kapat.
5. procd instance'ları: `portal` ve `logger` (`respawn 3600 5 0`).

`START=20` (firewall=19'dan hemen sonra; nft tablosu arayüz beklemeden yüklenir). Portal `10.50.0.1` adresi henüz yoksa bağlanamaz ve procd onu 5 sn arayla yeniden başlatır. `/srv` bağlı değilse tablo yüklenir ama portal/logger başlatılmaz (fail-closed). `reload` → config değişikliklerini uygular (nft tablosunu yeniden yükler **ve** oturumları geri ekler).

fw4 reload bizim tabloya dokunmaz; yine de Faz 4 testinde `fw4 reload` sonrası oturumların sürdüğü doğrulanır.

---

## 14. Loglama ve CSV şeması

### 14.1 Tek şema (tüm CSV'ler aynı sütunlara sahip)
Ayraç `;`, kodlama UTF-8 **BOM ile** (yalnızca dosya oluşturulurken), satır sonu `\r\n` (Excel uyumu; `csv` modülü varsayılanı), ilk satır başlık.

| # | Sütun | Açıklama | Örnek |
|---|---|---|---|
| 1 | `zaman` | ISO 8601 `+03:00` | `2026-09-24T14:30:05+03:00` |
| 2 | `olay` | Olay türü (§14.2) | `BAGLANTI_BITIS` |
| 3 | `telefon` | 10 hane; eşleşmezse boş | `5334553132` |
| 4 | `ad` | | `Ayşe` |
| 5 | `soyad` | | `Yılmaz` |
| 6 | `mac` | küçük harf | `aa:bb:cc:dd:ee:ff` |
| 7 | `ic_ip` | | `10.50.0.23` |
| 8 | `protokol` | `tcp`/`udp`/`icmp`/DNS tipi (`A`,`AAAA`…) | `tcp` |
| 9 | `ic_port` | | `51234` |
| 10 | `hedef_ip` | | `142.250.187.110` |
| 11 | `hedef_port` | | `443` |
| 12 | `nat_ip` | Router WAN IP (conntrack reply tuple) | `192.168.1.10` |
| 13 | `nat_port` | NAT sonrası kaynak port | `40123` |
| 14 | `alan_adi` | DNS sorgusu veya DHCP hostname | `www.google.com` |
| 15 | `gonderilen_bayt` | orig yönü | `18344` |
| 16 | `alinan_bayt` | reply yönü | `912331` |
| 17 | `sure_sn` | bağlantı/oturum süresi | `183` |
| 18 | `oturum_id` | | `3f9a1c0b7d2e` |
| 19 | `ek` | serbest (neden, NetGSM kodu, conntrack id…) | `neden=sure_doldu` |

Ad/soyad yalnızca `KAYIT`, `OTURUM_BASLA`, `OTURUM_BITIS` satırlarında doldurulur; trafik/DNS satırlarında boş bırakılır (dosyada kimlik zaten `KAYIT`'ta; boyut tasarrufu). **Günlük** `trafik.csv`/`dns.csv` dosyalarında ise `telefon` her satırda yazılır (resmi sorguda tek dosyadan cevap verilebilsin).

### 14.2 Olay türleri
| Olay | Kaynak | Yazıldığı dosyalar |
|---|---|---|
| `KAYIT` | Portal | `<tel>.csv` (yalnızca dosya oluşturulurken) |
| `OTURUM_BASLA` / `OTURUM_BITIS` / `OTURUM_IP_DEGISTI` | Portal / Logger / ctl | `<tel>.csv`, `gunluk/oturum.csv` |
| `BAGLANTI_BASLA` / `BAGLANTI_BITIS` | Logger (conntrack NEW / DESTROY) | `<tel>.csv` (oturum eşleşirse), `gunluk/trafik.csv` (her zaman) |
| `DNS` | Logger (dnsmasq) | `<tel>.csv` (eşleşirse), `gunluk/dns.csv` (her zaman) |
| `DHCP_ATAMA` / `DHCP_YENILEME` / `DHCP_BIRAKMA` | hotplug | `gunluk/dhcp.csv` |
| `OTP_ISTEK`, `OTP_GONDERILDI`, `OTP_HATA`, `OTP_BASARILI`, `OTP_HATALI_KOD`, `SMS_TAVAN`, `LOG_BOSLUK`, `SERVIS_BASLADI` | Portal / Logger | `gunluk/denetim.csv` |

### 14.3 Örnek: /srv/5651/kullanicilar/5334553132.csv
```csv
zaman;olay;telefon;ad;soyad;mac;ic_ip;protokol;ic_port;hedef_ip;hedef_port;nat_ip;nat_port;alan_adi;gonderilen_bayt;alinan_bayt;sure_sn;oturum_id;ek
2026-09-24T14:30:05+03:00;KAYIT;5334553132;Ayşe;Yılmaz;aa:bb:cc:dd:ee:ff;10.50.0.23;;;;;;;;;;;3f9a1c0b7d2e;
2026-09-24T14:30:05+03:00;OTURUM_BASLA;5334553132;Ayşe;Yılmaz;aa:bb:cc:dd:ee:ff;10.50.0.23;;;;;;;;;;;3f9a1c0b7d2e;bitis=2026-09-25T02:30:05+03:00
2026-09-24T14:30:07+03:00;DNS;5334553132;;;aa:bb:cc:dd:ee:ff;10.50.0.23;A;;;;;;www.google.com;;;;3f9a1c0b7d2e;
2026-09-24T14:30:07+03:00;BAGLANTI_BASLA;5334553132;;;aa:bb:cc:dd:ee:ff;10.50.0.23;tcp;51234;142.250.187.110;443;192.168.1.10;40123;;;;;3f9a1c0b7d2e;ct=3301452
2026-09-24T14:33:10+03:00;BAGLANTI_BITIS;5334553132;;;aa:bb:cc:dd:ee:ff;10.50.0.23;tcp;51234;142.250.187.110;443;192.168.1.10;40123;;18344;912331;183;3f9a1c0b7d2e;ct=3301452
2026-09-25T02:30:31+03:00;OTURUM_BITIS;5334553132;Ayşe;Yılmaz;aa:bb:cc:dd:ee:ff;10.50.0.23;;;;;;;;;;43226;3f9a1c0b7d2e;neden=sure_doldu
```

### 14.4 conntrack okuyucu (logger.py)
Komut (⚠ DOĞRULA seçenekleri cihazdaki conntrack-tools sürümünde):
```sh
conntrack -E -e NEW,DESTROY -o timestamp,extended,id -b 33554432 -s 10.50.0.0/24
```
- Satır örnekleri (**Faz 0'da cihazdan gerçekleri alınıp `tests/fixtures/conntrack.txt`'ye konur**; ayrıştırıcı bunlara karşı test edilir):
  ```
  [1790253007.123456]	    [NEW] ipv4     2 tcp      6 120 SYN_SENT src=10.50.0.23 dst=142.250.187.110 sport=51234 dport=443 [UNREPLIED] src=142.250.187.110 dst=192.168.1.10 sport=443 dport=40123 id=3301452
  [1790253190.654321]	[DESTROY] ipv4     2 tcp      6 src=10.50.0.23 dst=142.250.187.110 sport=51234 dport=443 packets=41 bytes=18344 src=142.250.187.110 dst=192.168.1.10 sport=443 dport=40123 packets=702 bytes=912331 [ASSURED] delta-time=183 id=3301452
  ```
- Ayrıştırma: ilk `src=/dst=/sport=/dport=` grubu **orig**, ikinci grup **reply** tuple'ıdır. `nat_ip = reply.dst`, `nat_port = reply.dport`. `packets=/bytes=` sırasıyla orig ve reply. Zaman = köşeli parantezdeki epoch.
- Filtre (Python'da): `orig.src` hotspot alt ağında **ve** `orig.dst != router_ip` (router'a giden DNS/portal trafiği trafik logu değildir; DNS ayrıca loglanır). ICMP'de port yoktur → boş.
- Eşleme: `orig.src` → `ip_to_session` (sessions.json) → telefon/oturum. Eşleşmezse: `allow_mac` cihazıysa `ek=izinli:<ad>`, değilse `telefon` boş (yetkisiz istemci forward edemez; bu durum yalnızca yarış anlarında olur).
- MAC: oturumdan; oturum yoksa leases'tan.
- **Çekirdek süzmesi (2026-09-27):** komut `-s <subnet>` ile çalışır (conntrack-tools BPF filtresi) → yalnızca müşteri ağından başlayan akışların olayları logger'a gelir; router'ın kendi trafiği ve dükkân ağındaki olay fırtınaları çekirdekte elenir. Tampon `-b 33554432` (çekirdek 64 MB yapar), okuma borusu `F_SETPIPE_SZ` ile 1 MB. Neden: 26.09'daki `LOG_BOSLUK` kayıtları müşteri trafiğinin düşük olduğu (≤110 olay/sn) ama müşteri dışı olay patlamalarının olduğu saniyelere denk geldi; logger ~3.600 olay/sn işler. Test: 2,2 sn'de 30.000 müşteri dışı akış → eski komut 30.046 olay, yeni komut 20 olay; canlı logger kayıp yazmadı.
- `ENOBUFS` / "No buffer space available" veya süreç çıkışı → `denetim.csv`'ye `LOG_BOSLUK` (başlangıç/bitiş zamanıyla) yazılır ve conntrack yeniden başlatılır. Bu satır, olası kayıp aralığını belgeler.
- Bağlantı başına 2 satır yazılır. ⚠ `udp` DNS dışı küçük akışlar (NTP, QUIC) çok satır üretir; kapasite §18.

### 14.5 DNS okuyucu (logger.py)
```sh
logread -f -e dnsmasq
```
- `log-queries=extra` biçiminde satır örneği (⚠ DOĞRULA, fixture'a kaydet):
  ```
  Thu Sep 24 14:30:07 2026 daemon.info dnsmasq[2345]: 1734 10.50.0.23/53012 query[A] www.google.com from 10.50.0.23
  ```
- Yalnızca `query[` içeren satırlar alınır; `forwarded`, `reply`, `cached` satırları atlanır. Zaman = logger'ın satırı okuduğu an (`datetime.now()`), çünkü syslog zamanında saniye altı ve yıl/ofset belirsizliği var.
- Alt ağ dışı istemciler (router'ın kendisi, 127.0.0.1) atlanır.

### 14.6 CSV yazıcı kuralları (`common.CsvSink`)
- Hücre temizleme: `\r`, `\n`, `;`, `"` → boşluk / kaldır; ayrıca `=`, `+`, `-`, `@`, `\t` ile başlayan hücrelere başına `'` eklenir (**CSV/formül enjeksiyonu** koruması — ad alanına `=HYPERLINK(...)` yazılmasın). Test edilir.
- Yazma: tek fonksiyon `common.append_rows(path, rows)` — dosyayı `O_APPEND` ile aç, `fcntl.flock` al, satırları `csv.writer(delimiter=';')` ile yaz, kapat. Logger satırları bellekte **1 saniye** biriktirir ve her dosyaya saniyede en fazla bir kez yazar (açık dosya önbelleği yok). ext4 günlüğü veriyi ~5 sn içinde diske işler.
- Satırın gideceği günlük dosya, yazma anına göre değil **satırın `zaman` alanına** göre seçilir (23:59:59 olayı gece yarısından sonra yazılsa da o günün dosyasına gider).
- Yeni dosya: BOM (`\ufeff`) + başlık satırı, izin `600`.
- Dizinler: `700`.
- Aynı dosyaya birden çok süreç (portal, logger, hotplug, hotspotctl) yazabilir; hepsi aynı `flock` ile sıralanır. BOM ve başlık yalnızca kilit alındıktan sonra dosya boşsa yazılır. `hotspotctl temizle` kullanıcı dosyasını **aynı kilit altında yerinde** (`seek(0)` + `truncate`) yeniden yazar; dosyayı başka bir inode ile değiştirmez, böylece bekleyen yazıcıların satırları kaybolmaz.

### 14.7 DHCP hotplug: /etc/hotplug.d/dhcp/90-hotspot
```sh
#!/bin/sh
# dnsmasq → /usr/lib/dnsmasq/dhcp-script.sh → hotplug "dhcp" olayı
# Ortam değişkenleri (⚠ DOĞRULA): ACTION=add|old|del, MACADDR, IPADDR, HOSTNAME
case "$ACTION" in
	add) olay=DHCP_ATAMA ;;
	old) olay=DHCP_YENILEME ;;
	del) olay=DHCP_BIRAKMA ;;
	*) exit 0 ;;
esac
gun=$(date +%F)
dir=/srv/5651/gunluk/$gun
f=$dir/dhcp.csv
mkdir -p "$dir" && chmod 700 "$dir"
host=$(printf '%s' "$HOSTNAME" | tr -cd 'A-Za-z0-9._-' | cut -c1-63)
zaman=$(date +%Y-%m-%dT%H:%M:%S%z | sed 's/\(..\)$/:\1/')   # %z gerçek ofseti yazar → zaman her durumda doğru
(
	flock 9
	[ -s "$f" ] || { printf '\357\273\277'; echo 'zaman;olay;telefon;ad;soyad;mac;ic_ip;protokol;ic_port;hedef_ip;hedef_port;nat_ip;nat_port;alan_adi;gonderilen_bayt;alinan_bayt;sure_sn;oturum_id;ek'; } >> "$f"
	printf '%s;%s;;;;%s;%s;;;;;;;%s;;;;;\r\n' "$zaman" "$olay" "$MACADDR" "$IPADDR" "$host" >> "$f"
) 9>>"$dir/.dhcp.lock"
chmod 600 "$f"
```
- Busybox `flock` mevcut olmalı (⚠ DOĞRULA; yoksa `util-linux-flock` / `flock` paketi).
- Portal/Logger aynı olayı dinlemez; `ip_degisti` tespiti logger'da `/tmp/dhcp.leases` değişikliği izlenerek yapılır.

---

## 15. Log bütünlüğü, saklama, yedekleme

### 15.1 Gün kapatma: `hotspotctl gun-kapat [TARIH]` (cron 00:15)
Varsayılan TARIH = dün.
1. `gunluk/TARIH/` altındaki `*.csv` dosyaları için: logger'ın o günün dosyalarını kapattığını doğrula (gün değişiminde kapatır; ek güvenlik: dosya son 5 dk'da değişmemiş olmalı).
2. Her `*.csv` → `gzip -9` → `*.csv.gz` (orijinal silinir yalnızca gz başarıyla yazılıp `gzip -t` testi geçtiyse).
3. `MANIFEST.sha256` oluştur:
   ```
   # gun=2026-09-23 olusturma=2026-09-24T00:15:01+03:00 onceki_zincir=<dünkü zincir hash'i>
   <sha256>  dhcp.csv.gz
   <sha256>  dns.csv.gz
   <sha256>  denetim.csv.gz
   <sha256>  oturum.csv.gz
   <sha256>  trafik.csv.gz
   <sha256>  ../../kullanicilar/5334553132.csv   (anlık görüntü hash'i)
   …
   ```
4. **Hash zinciri**: `zincir_hash = sha256(onceki_zincir_hash + sha256(MANIFEST.sha256))` → `/srv/5651/zincir.txt`'ye `TARIH;manifest_sha256;zincir_hash` satırı eklenir. Geçmişteki herhangi bir dosya değiştirilirse zincir kırılır.
5. **Zaman damgası** (`tsa.enabled=1` ise):
   ```sh
   openssl ts -query -data MANIFEST.sha256 -sha256 -cert -out MANIFEST.tsq
   # Python urllib ile POST, Content-Type: application/timestamp-query, (varsa) Basic Auth → MANIFEST.tsr
   openssl ts -verify -in MANIFEST.tsr -queryfile MANIFEST.tsq -CAfile /etc/hotspot/tsa-ca.pem
   ```
   Doğrulama başarısızsa denetime `TSA_HATA` yazılır ve ertesi çalıştırmada tekrar denenir (`gun-kapat` idempotent: zaten `.tsr` varsa atlar).
   > Lisanslı Türk ESHS'lerinin (ör. TÜBİTAK Kamu SM) zaman damgası servisleri standart RFC 3161'e ek olarak kendine özgü kimlik doğrulama isteyebilir. Sağlayıcı seçilince (S8) istemci bu bölüme göre uyarlanır. Geliştirmede ücretsiz bir RFC 3161 servisi (ör. freetsa.org) kullanılabilir; bunun yasal geçerliliği tartışmalıdır.
6. Dizin ve dosyalar salt okunur yapılır (`chmod 400`, dizin `500`).

### 15.2 Doğrulama: `hotspotctl dogrula [TARIH|--hepsi]`
- Manifestteki her `.gz` hash'ini yeniden hesaplar, zinciri baştan yürütür, `.tsr` varsa `openssl ts -verify` çalıştırır. Çıktı: `2026-09-23 ✔ hash ✔ zincir ✔ zaman damgası` veya hangi adımın kırıldığı.

### 15.3 Resmi talep akışı
Talep genellikle "**Tarih-saat** + **dış IP** (+ port)" veya "**iç IP**" ile gelir:
- Çift NAT (S4=hayır) durumunda dış IP modemin genel IP'sidir; modemin port eşlemesi bilinmez → eşleştirme **zaman + hedef IP/port** üzerinden `trafik.csv`'de yapılır.
- Modem bridge ise `nat_ip`/`nat_port` doğrudan genel IP:port'tur → birebir eşleşme.
- `hotspotctl ara …` komutları (§16) ilgili satırları ve kişiyi bulur; `hotspotctl disa-aktar --tarih … --telefon …` ilgili günlerin `.gz` + manifest + `.tsr` dosyalarını tek bir `tar` paketine koyar (orijinaller değiştirilmeden).

### 15.4 Disk ve boyut tahmini
- Aktif kullanıcı başına günde ~1.000–5.000 bağlantı + ~1.000–3.000 DNS sorgusu; satır ~180 bayt.
- 60 kullanıcı/gün × ~6.000 satır × 180 B ≈ **65 MB/gün ham** (günlük) + aynı kadar kullanıcı CSV'leri.
- gzip ~%85–90 sıkıştırır → günlük dosyalar ≈ 8 MB/gün → 730 gün ≈ **6 GB**.
- Kullanıcı CSV'leri sıkıştırılmaz → en büyük kalem. `hotspotctl temizle` bunları da saklama süresine göre kırpar. Gerekirse kullanıcı CSV'leri **aylık** parçalanabilir (`5334553132.csv` aktif ay, eskiler `5334553132_2026-08.csv.gz`) — **ilk sürümde yok**, disk %60'ı aşarsa eklenir.
- `/srv` %80 dolulukta denetime `DISK_UYARI` + `logger -p daemon.warn`; %95'te **yasal loglar silinmez**, bunun yerine trafik satırlarının kullanıcı CSV kopyası durdurulur (günlük dosyalar yazılmaya devam eder).

> **Güncel karar (2026-09-29, kullanıcı; 25.09 kararını değiştirir):** Her gün her gece **uzak sunucuya da** gönderilir, ama cihaz kayıtları kendisi de **2 yıl (retention_days=730)** tutar; 2 günde silme iptal. 32 GB disk günde 20 kişi × 2 saat için ~1 MB/gün (sıkıştırılmış) ile bunu rahat taşır. Sunucu tarafı (kurulum, saklama süresi, arayüz) kullanıcının sorumluluğundadır; cihaz yalnızca gönderir. Bu karar §15.4'teki disk hesabını ve "aylık parçalama" önerisini geçersiz kılar.

### 15.5 Gece akışı (cron)
| Saat | Komut | İş |
|---|---|---|
| 00:15 | `hotspotctl gun-kapat` | Dünkü klasörü mühürle: `kullanicilar/` dahil tüm `*.csv` → `.csv.gz`, `MANIFEST.sha256` (kişi dosyaları da manifestte), hash zinciri, (varsa) zaman damgası. Son 5 dk'da yazılmış dosya varsa ertelenir (`GUN_KAPAT_ERTELENDI`) ve ertesi gece kapanır. Mühürlenmiş güne ait geç gelen `OTURUM_BITIS` o güne değil bugüne yazılır (`ek=… bitis=<gerçek bitiş>`) |
| 02:00 | `hotspotctl yedekle; hotspotctl temizle` | Mühürlenmiş ama henüz gönderilmemiş her günü sunucuya gönder, sonra saklama süresini (2 yıl) aşmış yedeklenmiş günleri sil |

### 15.6 Yedekleme: `hotspotctl yedekle` (02:00, `backup.enabled=1` ise)
- Mühürlenmiş (`MANIFEST.sha256` var) ve `.yedeklendi` işareti olmayan **her gün** için: `rsync -a --timeout=120 --ignore-existing <gün_klasörü> <target>/gunluk/`. Hedef uzaksa (`user@host:/yol`) `-e "<backup.ssh>"` (varsayılan `ssh -i /root/.ssh/yedek_anahtar`). rsync'e 1 saat süre tanınır (diğer komutlardaki 15 sn sınırı yedeğe uygulanmaz).
- rsync başarılıysa gün klasörüne `.yedeklendi` dosyası (gönderim zamanıyla) yazılır, denetime `YEDEK gun=…`. Başarısızsa `YEDEK_HATA`, işlem durur; ertesi gece kaldığı yerden devam eder (kaçan günler birikmez, hepsi gönderilir).
- Ardından `zincir.txt` ve `kullanicilar/index.csv` hedefin köküne gönderilir; sunucudaki önceki sürümleri **ezilmez**, `eski/zincir.txt.<tarih>` olarak saklanır (`--backup --backup-dir=eski`). Sunucuda hiçbir dosya silinmez veya üzerine yazılmaz.
- **Depolama koruması:** `/srv` bağlı değilse `gun-kapat`, `yedekle`, `temizle` ve DHCP hotplug betiği hiçbir şey yazmaz/göndermez (kök dosya sisteminde sahte bir gün ve tek satırlık yeni bir zincir oluşup sunucuya gitmesin).
- `backup.enabled=0` veya `target` boşsa hiçbir şey gönderilmez **ve hiçbir şey silinmez**.

### 15.7 Yerel temizlik: `hotspotctl temizle` (02:00, yedekten hemen sonra)
- `main.retention_days` (varsayılan **730** = 2 yıl; panelde "Kayıtlar cihazda kaç gün saklansın"): Kural: `gün < bugün − retention_days` **ve** `.yedeklendi` varsa sil. (2026-09-29'a kadar ayrı `backup.keep_days` = 2 gündü; kaldırıldı, deploy.sh eski seçeneği siler.)
- **Yedeklenmemiş gün asla silinmez**; süresi geçmişse denetime `YEDEKSIZ_GUN` yazılır ve `hotspotctl durum` uyarır.
- `zincir.txt` hiç silinmez (zincirin sürekliliği). `index.csv`'de son oturumu `retention_days`'ten eski kişiler çıkarılır (KVKK).
- Silinen her gün denetime `SAKLAMA_SILME` olarak yazılır.
- Resmi talep aramaları (`hotspotctl ara`) yalnızca cihazdaki günlerde çalışır; daha eski günler sunucudaki arşivden aranır.

### 15.8 Yedek güvenliği
- Yedekler kişisel veri içerir → hedef disk şifreli olmalı (kullanıcı sorumluluğunda).
- **Kritik uyarı (x86 sysupgrade):** x86'da `sysupgrade`, imajı diske yazarken **bölüm tablosunu imajınkiyle değiştirebilir** ve `/srv` bölümü (sda3) tablodan silinebilir. Veri fiziksel olarak durur ama bölüm girdisi kaybolur. Bu yüzden:
  1. Faz 1'de `/srv` bölümünün **başlangıç sektörünü ve boyutunu** `docs/kesif.txt`'ye kaydet (bölüm girdisi aynı değerlerle yeniden oluşturulursa veri geri gelir).
  2. Sysupgrade'den önce tam yedek al.
  3. Mümkünse sysupgrade yerine aynı sürümde kalıp paket güncellemesi yap.

---

## 16. Yönetim aracı: hotspotctl

`/usr/sbin/hotspotctl` → `exec python3 /usr/lib/hotspot/ctl.py "$@"`. Tüm çıktılar Türkçe, tablo biçiminde; `--csv` bayrağıyla makine okunur.

| Komut | İşlev |
|---|---|
| `hotspotctl durum` | Servisler (portal/logger çalışıyor mu), nft tablo var mı, aktif oturum sayısı, bugünkü SMS sayısı/tavan, son NetGSM hatası, `/srv` doluluğu, son `gun-kapat` sonucu, NTP senkron mu |
| `hotspotctl oturumlar` | Aktif oturumlar: telefon, ad soyad, MAC, IP, başlangıç, kalan süre |
| `hotspotctl at <mac\|telefon>` | Oturum(ları) kapat, nft'den sil, conntrack temizle, `OTURUM_BITIS neden=yonetici` |
| `hotspotctl ara --ic-ip 10.50.0.23 --zaman "2026-09-24 14:30"` | O anda o IP'de kimin oturumu vardı + DHCP kaydı |
| `hotspotctl ara --nat-port 40123 --zaman "2026-09-24 14:31" [--tolerans 120]` | O portu kullanan bağlantı(lar) ve kişi |
| `hotspotctl ara --hedef-ip 142.250.187.110 --zaman "…" --tolerans 300` | O hedefe o aralıkta bağlananlar |
| `hotspotctl ara --telefon 5334553132` | Kullanıcının oturum özetleri |
| `hotspotctl ara --mac aa:bb:…` | MAC geçmişi (DHCP + oturumlar) |
| `hotspotctl yukle` | sessions.json → nft (init tarafından çağrılır) |
| `hotspotctl gun-kapat [TARIH]` | §15.1 |
| `hotspotctl dogrula [--son N]` | §15.2 (zincir her zaman baştan doğrulanır; dosya hash'leri son N gün için, varsayılan hepsi) |
| `hotspotctl temizle [--kuru]` | §15.7 — yalnızca yedeklenmiş ve `retention_days`'ten eski günleri siler (`--kuru`: yalnızca listele) |
| `hotspotctl yedekle` | §15.6 — mühürlenmiş, gönderilmemiş günleri sunucuya gönderir |
| `hotspotctl disa-aktar --baslangic … --bitis … [--telefon …] --cikti /tmp/talep.tar` | Resmi talep paketi |
| `hotspotctl sms-test <telefon>` | Gerçek SMS gönderir (onay sorar: "Bu işlem ücretlidir, devam? [e/H]") |

`ara` komutları hem açık günün `.csv` dosyalarında hem kapalı günlerin `.csv.gz` dosyalarında (`gzip.open`) arar; zaman aralığındaki günlerle sınırlıdır.

---

## 17. Güvenlik sertleştirme

### 17.1 Ağ
- [ ] Hotspot → router: yalnızca 53, 67, 8080, ICMP echo. (Test: hotspot'tan `nc -zv 10.50.0.1 22` başarısız.)
- [ ] Hotspot → özel ağlar (modem arayüzü dahil) engelli. (Test: `curl http://192.168.1.1` başarısız.)
- [ ] Hotspot → IPv6 yok; RA/DHCPv6 kapalı.
- [ ] AP istemci izolasyonu açık.
- [ ] Yönetim (22/443) yalnızca WAN'dan ve yalnızca modem LAN alt ağından.
- [ ] SSH yalnızca anahtar; root parola girişi kapalı; güçlü root parolası (LuCI için) yine de ayarlı.
- [ ] LuCI yalnızca HTTPS (`uhttpd.main.redirect_https='1'`).

### 17.2 Uygulama
- [ ] Portal root olarak çalışır (nft için CAP_NET_ADMIN gerekir). Saldırı yüzeyi: yalnızca 8080'deki HTTP ayrıştırıcı. Önlemler: gövde ≤ 4 KB, başlık sayısı sınırı (stdlib), soket zaman aşımı 10 sn, eşzamanlı bağlantı sınırı (ThreadingHTTPServer + semafor 32), IP başına istek sınırı (ör. 30 istek/dk). `# ponytail:` root yerine ayrı kullanıcı + yalnızca nft için yetkili yardımcı mümkün; ilk sürümde gereksiz.
- [ ] `subprocess` yalnızca liste argüman; nft'ye giden MAC/IP değerleri önce regex ile doğrulanır (`^[0-9a-f]{2}(:[0-9a-f]{2}){5}$`, `ipaddress.IPv4Address` + alt ağ kontrolü).
- [ ] HTML escape her yerde; `dst` parametresi yalnızca `http(s)://` ise ve 2048 karakterden kısaysa link olarak gösterilir.
- [ ] CSV enjeksiyonu koruması (§14.6).
- [ ] OTP: `secrets` modülü, sabit zamanlı karşılaştırma, TTL, deneme sınırı, kodun kendisi hiçbir yere loglanmaz (mock modu hariç, o da yalnızca syslog'a).
- [ ] Sırlar: `/etc/config/hotspot` 600; logda veya hata mesajında NetGSM şifresi asla görünmez (istek gövdesi loglanmaz).
- [ ] `/srv` 700, log dosyaları 600, kapalı günler 400.

### 17.3 Bilinen zayıflıklar (kabul edilen)
- **MAC+IP taklidi**: aynı ağdaki bir saldırgan yetkili bir cihazın MAC'ini ve IP'sini klonlarsa onun oturumunu kullanabilir. AP istemci izolasyonu ve kısa oturum süresi riski azaltır. Tam çözüm (802.1X/WPA-Enterprise) kapsam dışı.
- **Açık ağda şifresiz HTTP portal**: form verisi ve OTP havada okunabilir. Azaltma: WPA2-PSK (şifre kafede yazılı) veya OWE; ileride kendi alan adı + Let's Encrypt (DNS-01) ile HTTPS portal.
- **DoH**: 443 üzerinden şifreli DNS engellenemez → bazı DNS sorguları loglanmaz. Bağlantı meta verisi (hedef IP) yine loglanır; yasal yükümlülük DNS logu değildir.
- **DNS tünelleme** yetkisiz istemci için teorik olarak mümkün (router DNS'i dışarı çözer). Etkisi düşük; gerekirse yetkisiz istemcilere DNS hız sınırı.

---

## 18. Performans ve kapasite

| Kaynak | Beklenen yük | Sınır/önlem |
|---|---|---|
| CPU | NAT 100–300 Mbps; Python logger ~50–300 olay/sn | J1900 rahat. Logger tek iş parçacıklı okuma + toplu yazma |
| RAM | conntrack 65.536 giriş (~20 MB), Python süreçleri ~40 MB | 4 GB fazlasıyla yeterli |
| Disk yazma | ~130 MB/gün | SSD ömrü açısından önemsiz; `noatime` |
| conntrack olay kuyruğu | Patlamalarda ENOBUFS | `-b 8388608`; olursa `LOG_BOSLUK` |
| logd tamponu | DNS logu | `log_size 2048` |
| NetGSM | 1 SMS/doğrulama | Global günlük tavan |

Ölçüm (Faz 7): 10 dk boyunca 3–5 cihazla video + gezinme; `top`, `hotspotctl durum`, `wc -l` ile olay/sn ve CPU; ENOBUFS olmamalı.

---

## 19. Test stratejisi

### 19.1 Birim testleri (Windows'ta çalışır, `python tests/test_*.py`)
Her dosya düz `assert` + dosya sonunda `if __name__ == "__main__":` ile tüm test fonksiyonlarını çağırır.

| Dosya | Kapsam (en az) |
|---|---|
| `test_common.py` | telefon normalize (10 geçerli + 10 geçersiz örnek: `+90 533…`, `0533…`, `533…`, `212…` sabit hat ✗, 9 hane ✗, harf ✗); ad doğrulama (`Ayşe`, `İsmail Hakkı`, `O'Neil` ✓; `A`, `=cmd`, `Ali1` ✗); CSV hücre temizleme (`=HYPERLINK(...)` → `'=HYPERLINK(...)`, `;` ve `\n` kaldırılır); leases/arp ayrıştırma (fixture) |
| `test_logger.py` | conntrack NEW/DESTROY satırı → dict (orig/reply, nat_ip/port, bytes, id), ICMP satırı, IPv6 satırı atlanır, router hedefli satır atlanır; dnsmasq `query[A]` satırı → (ip, tip, alan adı), `forwarded`/`reply` atlanır — **cihazdan alınmış gerçek fixture'larla** |
| `test_portal.py` | OTP: doğru kod ✓, yanlış kod sayacı, 5. hatada iptal, TTL sonrası ✗ (enjekte `now`); hız sınırları (60 sn bekleme, 3/15 dk, global tavan); `dst` açık yönlendirme koruması |
| `test_netgsm.py` | XML üretimi (ElementTree ile geri ayrıştırıp alanları kontrol), yanıt ayrıştırma (`code=0`, `code=30`, bozuk XML → PARSE) |

### 19.2 Cihaz üzerinde entegrasyon kontrolleri (Claude Code SSH ile çalıştırır)
```sh
nft list table inet hotspot                    # tablo yüklü
nft list set inet hotspot auth                 # oturumlar
/etc/init.d/hotspot status; logread -e hotspot | tail -50
hotspotctl durum
tail -f /srv/5651/gunluk/$(date +%F)/trafik.csv
fw4 reload && nft list set inet hotspot auth   # oturumlar hâlâ orada
```

### 19.3 Kabul testleri (gerçek cihazlarla — kullanıcıyla birlikte)
| # | Senaryo | Beklenen |
|---|---|---|
| K1 | Android telefon `bocafe`'ye bağlanır | "Ağda oturum açın" bildirimi, portal açılır |
| K2 | iPhone bağlanır | Captive portal penceresi (CNA) otomatik açılır |
| K3 | Windows dizüstü bağlanır | Tarayıcı portal sayfasını açar |
| K4 | Yetkisiz: `https://google.com` | Anında bağlantı hatası (zaman aşımı değil) |
| K5 | Yetkisiz: `http://example.com` | Portala yönlenir |
| K6 | Yanlış formatta telefon | Alan hatası, değerler korunur |
| K7 | Doğru bilgiler + KVKK onayı (mock) | Kod ekranı, `logread`'de MOCK OTP |
| K8 | Yanlış kod ×5 | 5.'de "Yeni kod isteyin" |
| K9 | 3 dk bekle, kodu gir | "Kodun süresi doldu" |
| K10 | Doğru kod | "Bağlandınız"; CNA kapanır / internet çalışır |
| K11 | `/srv/5651/kullanicilar/<tel>.csv` | KAYIT + OTURUM_BASLA satırları, doğru ad/soyad/MAC |
| K12 | Birkaç site gez | ≤ 5 sn içinde DNS ve BAGLANTI satırları hem kullanıcı CSV'sinde hem günlük dosyalarda |
| K13 | Excel ile `<tel>.csv` aç | Türkçe karakterler ve sütunlar doğru |
| K14 | Router'ı yeniden başlat | Oturum sürüyor, kullanıcı tekrar SMS istemeden internette |
| K15 | `fw4 reload` / `service firewall restart` | Oturum sürüyor |
| K16 | `session_minutes=2` ile test | 2 dk sonra portal geri gelir, OTURUM_BITIS neden=sure_doldu |
| K17 | `hotspotctl at <mac>` | Anında internet kesilir, portal gelir |
| K18 | Aynı telefonla 4. cihaz | En eski oturum kapanır |
| K19 | Hotspot'tan `ssh 10.50.0.1`, `http://10.50.0.1` (LuCI), `http://192.168.1.1` | Hepsi başarısız |
| K20 | Hotspot'tan `nslookup google.com 8.8.8.8` | Yanıt gelir ama dnsmasq logunda görünür (yönlendirme çalışıyor) |
| K21 | `hotspotctl gun-kapat <bugün-test>` + `dogrula` | ✔ hash ✔ zincir (✔ TSA varsa) |
| K22 | Bir `.gz` dosyasında 1 bayt değiştir, `dogrula` | ✘ ile hangi dosya olduğunu gösterir |
| K23 | `hotspotctl ara --ic-ip … --zaman …` | Doğru kişi |
| K24 | WAN kablosunu çek, SMS iste | "SMS servisine ulaşılamadı", servis çökmez |
| K25 | Gerçek SMS (**kullanıcı onayıyla, kendi numarası**) | ≤ 30 sn'de SMS gelir, başlık doğru |
| K26 | Global SMS tavanını 2 yap, 3. istek | "Şu an SMS gönderilemiyor", denetimde SMS_TAVAN |

---

## 20. Uygulama fazları

Her faz: **Görevler → Kabul kriterleri → Kullanıcıya rapor.** Bir faz bitmeden sonrakine geçilmez.

### Faz 0 — Keşif ve doğrulama (cihazda değişiklik YOK)
**Görevler**
1. Kullanıcıdan cihaz SSH erişim bilgisi (IP, kullanıcı; anahtar yoksa önce parolayla) iste. Kullanıcı parolayı kendi terminalinde girer; ajan parolayı hiçbir dosyaya yazmaz. Anahtar kurulumunu öner (`ssh-keygen` + `/etc/dropbear/authorized_keys`).
2. `scripts/kesif.sh` ile topla ve `docs/kesif.txt`'ye yaz:
   ```sh
   cat /etc/openwrt_release; uname -a; ubus call system board
   command -v apk opkg                         # paket yöneticisi: apk (yeni) mi opkg mi
   ip -br link; ip -br addr; ip route
   uci show network; uci show firewall; uci show dhcp; uci show system
   cat /proc/partitions; mount; df -h
   (command -v fdisk && fdisk -l) || (command -v parted && parted -l)
   free -m; nproc; date; cat /etc/TZ
   nft --version; command -v conntrack python3 flock openssl curl
   cat /usr/lib/dnsmasq/dhcp-script.sh; ls /etc/hotplug.d/
   logread | tail -100
   ```
3. **Sürüm kararı:** `DISTRIB_RELEASE` "SNAPSHOT" ise → kullanıcıyı uyar: snapshot'larda sonradan `kmod-*` paketleri çekirdekle uyuşmayabilir. Öneri: gerekli paketler imaja dahil edilmiş **güncel kararlı sürüm, x86/64, ext4-combined(-efi)** imajıyla yeniden kurulum (firmware-selector.openwrt.org). Karar kullanıcıya aittir.
4. Hangi NIC'in modem, hangisinin AP'ye bağlı olduğunu tespit et (`ip -br link` + kablo durumu `LOWER_UP`, gerekirse kullanıcıdan bir kabloyu çıkarıp takmasını iste).
5. Yönetici PC'nin cihaza hangi taraftan eriştiğini tespit et (lock-out planı için kritik).
6. NetGSM güncel OTP API dokümanını (internetten) kontrol et; §12'yi güncelle.
7. §2.2 açık sorularını kullanıcıya sor, cevapları §23.6'ya işle.
8. Mevcut yapılandırmanın tam yedeği: `sysupgrade -b /tmp/yedek-ilk.tar.gz` → bilgisayara indir (`scp -O`).

**Kabul:** `docs/kesif.txt` var; NIC rolleri, paket yöneticisi, disk düzeni, yönetim erişim yolu, sürüm kararı ve açık sorular netleşti; ilk yedek bilgisayarda.

### Faz 1 — Temel sistem ve disk
**Görevler**
1. (Onayla) Kalan SSD alanında yeni bölüm (`/dev/sdX3`), `mkfs.ext4 -L hs5651`, `/etc/config/fstab`'a `target '/srv'`, `options 'noatime'`, `enabled '1'` (`block detect` + düzenleme), `block mount`. Başlangıç sektörü + boyutu `docs/kesif.txt`'ye.
   - Kök bölüm Python için yetersizse (boş < 150 MB): OpenWrt wiki "Expanding root partition and filesystem (x86)" prosedürü **veya** yeniden kurulumda büyük rootfs. Kararı kullanıcıyla ver.
2. Paketler (adları ⚠ DOĞRULA; apk için `apk add`, opkg için `opkg install`):
   ```
   python3 (veya python3-light + python3-urllib + python3-openssl + python3-email + python3-codecs + python3-logging)
   ca-bundle  ca-certificates
   conntrack  kmod-nf-conntrack-netlink
   iptables-nft  iptables-mod-filter  kmod-ipt-filter   (DNS kelime filtresi, §10.4)
   kmod-nft-bridge   (ZORUNLU: hotspot.nft'deki köprü izolasyon tablosu; yoksa tablo yüklenmez → kimse çıkamaz)
   openssl-util
   block-mount  e2fsprogs  fdisk (veya parted)
   flock (busybox'ta yoksa)
   rsync (yedek için, opsiyonel)
   curl (hata ayıklama için)
   ```
3. Saat dilimi + NTP (§9.5). `date` doğru.
4. `logd` tampon boyutu (§9.2), cron etkin (`/etc/init.d/cron enable && start`).
5. `/srv/5651`, `/srv/hotspot/state` dizinleri (700).

**Kabul:** `/srv` yeniden başlatma sonrası bağlı; `python3 -c "import ssl, urllib.request, http.server, csv, secrets"` hatasız; `conntrack -E` çalışıyor (Ctrl+C); saat NTP'li.

### Faz 2 — Ağ ve firewall (ölü adam anahtarıyla)
**Görevler**
1. §0.4 protokolünü kur.
2. Önce **yönetim erişimini WAN tarafına aç** (§9.3 son kural) ve yeni oturumla doğrula.
3. `hotspot` arayüzü, DHCP, firewall bölgesi, kurallar (§9.1–9.3). Eski `lan` kaldır. Flow offloading kapat.
4. Henüz nft `hotspot` tablosu yok → bu aşamada hotspot istemcileri **serbestçe** internete çıkar (fw4 forwarding var). Bu, ağ katmanının doğru olduğunu ayrıca test etmeyi sağlar.
5. AP ayarları (kullanıcı yapar, §9.6).

**Kabul:** Telefon `bocafe`'den `10.50.0.x` alır, internete çıkar; hotspot'tan router'ın 22/80/443'üne ve `192.168.1.1`'e erişilemez; yönetim WAN'dan çalışır; ölü adam anahtarı iptal edildi.

### Faz 3 — nft kapısı + portal (mock SMS)
**Görevler**
1. `hotspot.nft`, `/etc/init.d/hotspot` (şimdilik yalnızca nft + portal), `common.py`, `netgsm.py` (mock), `portal.py`, şablonlar, `hotspot.example`.
2. Birim testleri: `test_common.py`, `test_portal.py`, `test_netgsm.py` Windows'ta geçer.
3. `scripts/deploy.sh` yaz ve dağıt.

**Kabul:** K1–K10, K19 (mock ile). Yetkisiz istemci HTTPS'te anında hata alır.

### Faz 4 — Oturum yönetimi ve dayanıklılık
**Görevler**
1. `sessions.json`, flock, `hotspotctl yukle`, init'te geri yükleme, `allow_mac`, cihaz limiti, `yeniden_giris`.
2. `hotspotctl oturumlar`, `at`.

**Kabul:** K14–K18.

### Faz 5 — Loglama
**Görevler**
1. sysctl dosyası; DHCP hotplug betiği.
2. Faz 0'da alınan gerçek conntrack ve dnsmasq satırlarından fixture'lar; `test_logger.py`.
3. `logger.py`: conntrack okuyucu, DNS okuyucu, oturum süresi dolumu, `ip_degisti`, CsvSink, gün değişimi, `LOG_BOSLUK`, disk doluluk kontrolü.
4. Portal'ın KAYIT/OTURUM yazımları; `index.csv`.

**Kabul:** K11–K13, K16 (OTURUM_BITIS), `dhcp.csv` DHCP olaylarını içeriyor; 10 dk yük testinde ENOBUFS yok.

### Faz 6 — Bütünlük, saklama, yedek
**Görevler**
1. `gun-kapat`, `dogrula`, `temizle`, `yedekle`, cron satırları:
   ```
   15 0 * * * /usr/sbin/hotspotctl gun-kapat
   30 3 * * * /usr/sbin/hotspotctl temizle
   0 4 * * *  /usr/sbin/hotspotctl yedekle
   ```
2. TSA (S8 kararına göre).

**Kabul:** K21, K22; `temizle --kuru` doğru listeyi verir; yedek hedefinde dosyalar var.

### Faz 7 — Yönetim aracı, sertleştirme, performans
**Görevler**
1. `hotspotctl durum`, `ara` (tüm varyantlar), `disa-aktar`, `sms-test`.
2. §17 kontrol listesinin tamamı; SSH parola girişini kapat (anahtar doğrulandıktan sonra).
3. §18 performans ölçümü.

**Kabul:** K20, K23, K24, K26; §17.1 ve §17.2 kutularının hepsi işaretli.

### Faz 8 — Canlıya alma
**Görevler**
1. KVKK metni (hukukçu onaylı) `templates/kvkk.html`'e.
2. NetGSM gerçek bilgileri `/etc/config/hotspot`'a (kullanıcı kendisi `uci set` ile girer ya da ajan kullanıcının verdiği değeri yazar; değer repoya girmez). `mock=0`.
3. **Kullanıcı onayıyla** K25 (gerçek SMS).
4. Tüm kabul testlerinin son turu (K1–K26).
5. `sysupgrade -b` ile son yapılandırma yedeği + `files/` ile repo'nun cihazla aynı olduğunun doğrulanması (`deploy.sh --fark`).
6. Personel için 1 sayfalık kullanım notu (§21).

**Kabul:** Tüm K testleri geçti; kullanıcı onayladı.

---

## 21. Operasyon runbook

### 21.1 Günlük (otomatik)
- 00:15 gün kapatma → sabah `hotspotctl durum` "son gun-kapat: ✔" göstermeli.

### 21.2 Haftalık (yönetici, 2 dk)
```sh
hotspotctl durum
hotspotctl dogrula --son 7
df -h /srv
```

### 21.3 Sık sorunlar
| Belirti | Kontrol | Çözüm |
|---|---|---|
| Portal açılmıyor | `nft list table inet hotspot`, `/etc/init.d/hotspot status` | `/etc/init.d/hotspot restart` |
| SMS gelmiyor | `hotspotctl durum` (son NetGSM hatası), `logread -e hotspot` | §12.3 tablosu; NetGSM paneli bakiye/IP izni |
| Herkes düştü | `nft list set inet hotspot auth` boş mu? | `hotspotctl yukle` |
| Müşteri "bağlandım ama internet yok" | `hotspotctl oturumlar` o MAC var mı, IP aynı mı | `hotspotctl at <mac>` → yeniden giriş |
| Disk doluyor | `du -sh /srv/5651/*` | Saklama süresi / kullanıcı CSV parçalama (§15.4) |
| Saat yanlış | `date`, `ntpd -q -p tr.pool.ntp.org` | NTP erişimi; **saat yanlışken üretilen loglar denetime not edilmeli** |

### 21.4 Resmi talep geldiğinde
1. Talepteki tarih-saat, IP, port bilgilerini not al.
2. `hotspotctl ara …` ile kişiyi bul.
3. `hotspotctl disa-aktar …` ile ilgili günlerin mühürlü dosyalarını paketle.
4. `hotspotctl dogrula <günler>` çıktısını pakete ekle.
5. Paketi yalnızca resmi makama, hukukçu yönlendirmesiyle teslim et.

### 21.5 Personel notu (tek sayfa)
- Müşteri "SMS gelmedi": 1 dk bekleyip "Kodu tekrar gönder". Hâlâ yoksa numarayı kontrol et. Sürekli sorun → yöneticiye haber.
- Müşteri "Sayfa açılmıyor": Wi-Fi'yi kapat-aç veya tarayıcıya `http://10.50.0.1:8080` yaz.
- Router'ın fişini **çekme**; gerekirse yöneticiye haber ver.

---

## 22. Riskler ve bilinen sınırlamalar

| Risk | Olasılık | Etki | Azaltma |
|---|---|---|---|
| Snapshot sürümde paket/kmod uyumsuzluğu | Yüksek (snapshot ise) | Yüksek | Faz 0 sürüm kararı; kararlı sürüme geçiş |
| x86 sysupgrade `/srv` bölüm girdisini siler | Orta | Yüksek | §15.6 uyarısı, sektör kaydı, yedek |
| SSD arızası → 2 yıllık log kaybı | Düşük | Çok yüksek | Günlük yedek (§15.6) |
| NetGSM IP kısıtlaması + dinamik IP | Orta | Yüksek (kimse bağlanamaz) | S3; sabit IP veya kısıtlamasız alt kullanıcı |
| NetGSM kesintisi | Düşük | Yüksek | Hata mesajı; ileride personel onaylı acil giriş (kapsam dışı) |
| Rastgele MAC (iOS/Android "özel adres") | Yüksek | Düşük | Cihazlar ağ başına sabit rastgele MAC kullanır; yine de loglarda kişi telefonla eşleşir. "Rotating" modda IP değişimi → yeniden giriş |
| Çift NAT → dış port eşleşmesi yok | Yüksek (S4=hayır) | Orta | Modem bridge; yoksa zaman + hedef ile eşleme |
| DoH ile DNS logu atlanır | Orta | Düşük | Bağlantı logu yine var |
| MAC+IP taklidi | Düşük | Orta | İstemci izolasyonu, kısa oturum |
| Hukuki yorum farkı (süre, zaman damgası) | Orta | Orta | Yapılandırılabilir; hukukçu onayı (Faz 8) |
| conntrack olay kaybı (ENOBUFS) | Düşük | Orta | Büyük tampon, `LOG_BOSLUK` kaydı |
| Kötü niyetli SMS bombalama (maliyet) | Orta | Orta | §11.6 hız sınırları + global tavan |

---

## 23. Ekler

### 23.1 Paket yöneticisi karşılıkları
| İşlem | opkg (≤ 24.10) | apk (yeni sürümler / snapshot) |
|---|---|---|
| Liste güncelle | `opkg update` | `apk update` |
| Kur | `opkg install PAKET` | `apk add PAKET` |
| Ara | `opkg find '*python3*'` | `apk search python3` |
| Kurulu mu | `opkg list-installed \| grep X` | `apk info -e X` |

### 23.2 procd init iskeleti: /etc/init.d/hotspot
```sh
#!/bin/sh /etc/rc.common
# Taslak — güncel hali uygulama planındadır (docs/superpowers/plans/)
START=20
USE_PROCD=1

start_service() {
	[ "$(uci -q get hotspot.main.enabled)" = "1" ] || return 0
	sysctl -q -p /etc/sysctl.d/90-hotspot.conf
	nft -f /usr/lib/hotspot/hotspot.nft || return 1
	/usr/sbin/hotspotctl yukle   # allow_mac + oturumları geri yükle

	procd_open_instance portal
	procd_set_param command /usr/bin/python3 /usr/lib/hotspot/portal.py
	procd_set_param respawn 3600 5 0
	procd_set_param stdout 1
	procd_set_param stderr 1
	procd_close_instance

	procd_open_instance logger
	procd_set_param command /usr/bin/python3 /usr/lib/hotspot/logger.py
	procd_set_param respawn 3600 5 0
	procd_set_param stdout 1
	procd_set_param stderr 1
	procd_close_instance
}

stop_service() {
	# Tablo bilerek SİLİNMEZ: servis durunca yetkisiz trafik de kapalı kalır (fail-closed).
	# Tamamen devre dışı bırakmak için: nft delete table inet hotspot
	:
}

service_triggers() {
	procd_add_reload_trigger hotspot
}
```
> Fail-closed tercihi: portal çökerse müşteriler **internete çıkamaz** (yetkisiz); mevcut yetkili oturumlar nft'de kaldığı için onlar etkilenmez. Loglama yükümlülüğü açısından doğru olan budur.

### 23.3 deploy.sh davranışı (Git Bash)
```
Kullanım: scripts/deploy.sh [--fark|--kuru] root@192.168.1.10
- CRLF içeren dosya varsa durur
- files/ altındaki dosyaları (dizin girdisi yok, sahip 0:0) tar | ssh ile cihazda / altına açar
- /etc/config/hotspot yoksa /usr/lib/hotspot/hotspot.example'dan oluşturur (varsa dokunmaz), izin 600
- izinler: init.d, hotplug, sbin → 755
- cron satırlarını (yoksa) ekler; /etc/init.d/hotspot enable && restart
- --fark: cihazdaki dosyaların sha256'sını repo ile karşılaştırır, farkları listeler
- --kuru: yalnızca kopyalanacak dosyaları listeler
```

### 23.4 Faydalı komutlar
```sh
nft list ruleset | less
nft monitor trace                                   # kural izleme (önce: nft add rule inet hotspot pre_nat meta nftrace set 1)
conntrack -L -s 10.50.0.23                          # bir istemcinin bağlantıları
cat /tmp/dhcp.leases
logread -f -e hotspot
zcat /srv/5651/gunluk/2026-09-23/trafik.csv.gz | grep ';40123;'
hotspotctl dogrula --son 1                          # manifest + zincir + zaman damgası kontrolü
```

### 23.5 KVKK aydınlatma metni taslağı (HUKUKÇUYA ONAYLATILACAK)
> **Bocafe Misafir İnternet Hizmeti — Kişisel Verilerin İşlenmesine İlişkin Aydınlatma Metni**
>
> **Veri sorumlusu:** [İşletme unvanı, adres, iletişim]
> **İşlenen veriler:** Ad, soyad, cep telefonu numarası, cihazınızın donanım (MAC) adresi, size atanan iç IP adresi, bağlantı zamanları ve internet erişim kayıtları (bağlantı kurulan adresler, zaman ve veri miktarı; içerik kaydedilmez).
> **Amaç ve hukuki sebep:** 5651 sayılı Kanun ve ilgili yönetmelik uyarınca toplu kullanım sağlayıcı olarak yükümlülüklerimizin yerine getirilmesi (KVKK m.5/2-ç) ve talep ettiğiniz internet hizmetinin sunulması (KVKK m.5/2-c). Telefon numaranız yalnızca doğrulama kodu göndermek için kullanılır; pazarlama amacıyla kullanılmaz.
> **Aktarım:** Doğrulama SMS'i için telefon numaranız SMS hizmet sağlayıcımıza (NetGSM) iletilir. Kayıtlar yalnızca yetkili adli/idari makamların talebi halinde bu makamlarla paylaşılır.
> **Saklama süresi:** Kayıtlar mevzuatta öngörülen süre ([730 gün]) boyunca saklanır, süre sonunda silinir.
> **Haklarınız:** KVKK m.11 kapsamındaki haklarınız için [e-posta / adres] üzerinden başvurabilirsiniz.

### 23.6 Doğrulama günlüğü (Claude Code doldurur)
| Tarih | Madde | Sonuç / Kaynak |
|---|---|---|
| 2026-09-24 | OpenWrt sürümü, paket yöneticisi | ✔ OpenWrt 23.05.5 (r24106) x86/64, UEFI (AMI), opkg. Python 3.11.7, conntrack 1.4.8, OpenSSL 3.0.16. RAM gerçekte ~1,9 GB. `timeout` komutu yok, `flock` var |
| 2026-09-24 | Disk düzeni | OpenWrt başta 8 GB SanDisk USB'deydi, 32 GB LiteOn SSD'de Windows vardı. Kullanıcı onayıyla Windows silindi, sistem SSD'ye klonlandı: sda1 EFI (ESP türü), sda2 kök 2 GB, sda3 bios-boot, sda4 `/srv` 27,8 GB ext4 (etiket `hs5651`, fstab'da label ile). Eski USB yedek olarak saklanıyor (aynı PARTUUID → SSD ile birlikte takılmamalı) |
| 2026-09-24 | NIC rolleri (WAN=?, HOTSPOT=?) | ✔ WAN = `eth1` (modemden DHCP, 192.168.1.105), HOTSPOT = `eth0` (eski br-lan 192.168.10.1) |
| 2026-09-27 | Router WAN IP / dükkân modemi | Dükkân modemi Aidata WR854GVR (192.168.1.1, DHCP havuzu .100–.200, müşterilere DNS 8.8.8.8/1.1.1.1/8.8.4.4). Modem 26.09 ~21:36'da yeniden başladı → router WAN IP'si .105'ten **.100**'e değişti ve istemcilerde ~78 sn internet kesildi (bizim sistemden değil). Router MAC 00:0e:c4:ce:a0:9b modemde **MAC tabanlı atama ile 192.168.1.110'a sabitlendi** (kullanıcı isteği, 27.09). Panel: https://192.168.1.110:8443, SSH takma adı `bocafe-wan` → .110 |
| 2026-09-24 | Modem LAN alt ağı | ✔ 192.168.1.0/24, ağ geçidi 192.168.1.1; yönetici PC Wi-Fi'de 192.168.1.101 |
| 2026-09-24 | dnsmasq hotplug değişken adları | ✔ `MACADDR`, `IPADDR`, `HOSTNAME`; **`ACTION` = `add` / `update` / `remove`** (§14.7'deki `old`/`del` DEĞİL). dnsmasq, `dhcp-script`'i yalnızca başlarken `/etc/hotplug.d/dhcp` doluysa bağlar → betik kurulduktan sonra `dnsmasq restart` şart (deploy.sh yapıyor) |
| 2026-09-24 | conntrack çıktı biçimi (fixture alındı mı) | ✔ `tests/fixtures/conntrack.txt` (558 satır). `[epoch]\t[NEW] ipv4 2 tcp 6 120 SYN_SENT src=… [UNREPLIED] src=… id=…`; DESTROY'da `packets=/bytes=` var, **`delta-time` YOK**: çekirdekte `nf_conntrack_timestamp` desteği yok (sysctl anahtarı bilinmiyor). Süre logger'da NEW→DESTROY eşleştirmesiyle hesaplanır |
| 2026-09-24 | dnsmasq sorgu log biçimi (fixture alındı mı) | ✔ `tests/fixtures/dnsmasq.txt`: `… daemon.info dnsmasq[1]: 824 10.50.0.155/57805 query[A] s.youtube.com from 10.50.0.155` (`log-queries=extra`). Not: Android "Özel DNS" 1.1.1.1:443 DoH kullanıyor → o sorgular DNS logunda yok, bağlantı logunda var |
| 2026-09-25 | `ether saddr` forward kancasında çalışıyor mu | ✔ `gate` zincirinde `ether saddr . ip saddr @auth` sayacı yetkili telefonla 4.860 paket saydı; yetkisiz TCP reset, DoT 853 engeli, DNS ve HTTP yönlendirmeleri sayaçlarla doğrulandı. nft 1.0.8 `dstnat - 5` / `filter - 5` önceliklerini kabul ediyor. Zincir adı `fwd` OLAMAZ (nft anahtar kelimesi) → `gate` |
| 2026-09-24 | NetGSM OTP uç noktası, istek/yanıt biçimi, hata kodları | ✔ `https://api.netgsm.com.tr/sms/send/otp`, XML `mainbody/header(usercode,password,msgheader)/body(msg,no)`, yanıt `main/code` + `main/jobID`; kodlar 20,30,40,41,50,60,70,80,100. OTP: tek SMS, Türkçe karakter yok, ≤3 dk teslim. Kaynak: github.com/netgsm1/otp (resmi NetGSM paketi), netgsm.com.tr/sms/otp-sms |
| | NetGSM msgheader, OTP paketi, IP kısıtlaması | |
| | Python sürümü ve gerekli modüller | |
| 2026-09-24 | `/srv` bölümü başlangıç sektörü / boyutu | ✔ /dev/sda4 start=4229120 size=58302464 (512 B sektör), GPT, tür Linux FS. Kök /dev/sda2 start=33280 size=4194304 |
| | Açık soruların (S1–S13) cevapları | |

### 23.7 Claude Code'a önerilen ilk mesaj
```
MASTER_ENGINEERING.md dosyasını baştan sona oku. §0'daki kurallara uy.
Faz 0'ı başlat: önce bana cihaza SSH ile nasıl bağlanacağını sor, sonra keşif
komutlarını çalıştır, docs/kesif.txt'yi oluştur ve §2.2'deki açık soruları sor.
Cihazda hiçbir şeyi değiştirme.
```
