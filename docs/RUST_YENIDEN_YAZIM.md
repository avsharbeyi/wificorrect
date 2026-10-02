# BOCAFE Hotspot — Rust ile yeniden yazım rehberi

Bu dosya üç bölümden oluşur:

- **A.** Bugünkü sistemin (Python, OpenWrt 23.05.5, 2026-10) yaptığı her iş, sırayla. Dilden bağımsız davranış tarifidir; yeni sistem bunların **hepsini** aynı şekilde yapmalıdır. A14 yeni sistemin çok kafeli olma şartıdır.
- **B.** Yeni bir OpenWrt cihazında Rust ile yeniden yazım için hazır istem.
- **C.** Aynı işi Debian üzerinde Rust ile yapmak için hazır istem ve OpenWrt'den farkları.

Yeni repoya bu dosyayla birlikte `MASTER_ENGINEERING.md` dosyasını da kopyalayın. O doküman ayrıntılı tasarımdır (CSV şeması, nft kuralları, yasal çerçeve, kabul testleri K1–K26). Bu dosya ise neyin gerçekten yapıldığını ve sahada öğrenilen tuzakları özetler.

---

## A. Sistemin yaptığı işler (teker teker)

### A1. Donanım ve temel kurulum
1. **Cihaz:**
   - Intel Celeron J1900, 4 çekirdek, x86_64. RAM gerçekte ~1,9 GB görünüyor.
   - 32 GB LiteOn SSD, UEFI.
   - Gigabit Ethernet: `eth1` = sağdaki port = "Ethernet 1", `eth0` = soldaki port = "Ethernet 2". İkisi de Realtek r8169.
2. **Disk bölümleri:**
   - sda1 EFI.
   - sda2 kök, 2 GB.
   - sda3 bios-boot.
   - sda4 `/srv`: 27,8 GB, ext4, etiket `hs5651`, `noatime`, fstab'da etiketle bağlanır. Başlangıç sektörü 4229120, boyut 58302464 sektör.
   - Bütün yasal kayıtlar `/srv` altındadır. `/srv` bağlı değilse hiçbir kayıt yazılmaz, mühürlenmez, gönderilmez. Bu "sahte gün" oluşmasını engeller.
3. **Wi-Fi kartı:**
   - Atheros AR9287 mini PCIe, sürücü `ath9k`, iki anten takılı.
   - Yedek kart: Atheros AR9271 USB, sürücü `ath9k_htc`, normalde kapalı.
   - Denenip çalışmayan kartlar: Realtek RTL8723AE ve Broadcom BCM43142 (sürücü yok).
4. **Saat:**
   - Saat dilimi Europe/Istanbul.
   - NTP: `0.tr.pool.ntp.org`, `1.tr.pool.ntp.org`, `time.google.com`.
   - Bütün kayıt zamanları ISO 8601 biçiminde, `+03:00` ekiyle yazılır.
5. **Yönetim erişimi:**
   - Cihaza **yalnızca modem tarafından** (WAN bölgesi, 192.168.1.0/24) erişilir. SSH 22, panel 8443.
   - Müşteri ağından cihaza yalnızca DNS 53, DHCP 67, portal 8080 ve ping açıktır.

### A2. Ağ topolojisi
6. **WAN (internet alan taraf):**
   - `eth1` → dükkân modemi (Aidata, 192.168.1.1), DHCP istemcisi.
   - Modemde cihazın MAC'i (00:0e:c4:ce:a0:9b) **192.168.1.110**'a sabitlenmiştir.
   - Çift NAT vardır: modem + cihaz.
7. **Müşteri tarafı:**
   - `br-hotspot` köprüsü = `eth0` (kablolu erişim noktası için) + cihazın kendi Wi-Fi yayını.
   - Köprü adresi 10.50.0.1/24.
   - DHCP havuzu 10.50.0.20–249, kira süresi 2 saat, DNS = 10.50.0.1. Müşteri tarafında IPv6 kapalı.
8. **Wi-Fi yayını:**
   - SSID `freewifi`, **şifresiz**.
   - Kanal 13, HT20, ülke TR.
   - `isolate=1`: müşteriler birbirini göremez.
9. **Köprü düzeyinde yalıtım:** köprü portları arası iletim düşürülür. Kablolu erişim noktasındaki müşteri ile Wi-Fi müşterisi birbirini göremez.
10. **Port görevleri (panelden değiştirilebilir, A10'a bakın):**
    - Her iki Ethernet ve Wi-Fi kartı için görev seçilir: internet alır / internet verir / kapalı.
    - Wi-Fi "internet alır" görevinde başka bir Wi-Fi ağına istemci olarak bağlanır.
    - Kural: tam olarak bir "internet alır", en az bir "internet verir".
    - Internet hangi porttan gelirse gelsin WAN MAC'i aynı kalır, böylece modemdeki IP sabitlemesi bozulmaz.

### A3. Güvenlik duvarı ve giriş kapısı (fail-closed)
11. **Kapı OpenWrt'nin kendi güvenlik duvarından (fw4) bağımsızdır.** Kendi tablomuz (`table inet hotspot`) vardır. fw4 yeniden yüklense de setler ve oturumlar silinmez.
12. **Yetkili oturum seti:** eleman `MAC . IP`, her elemanın kendi süresi (timeout) vardır. Süre dolunca çekirdek elemanı kendisi siler.
13. **İzinli MAC listesi** (erişim noktası, personel): portal sormadan geçerler ama trafikleri yine kaydedilir.
14. **Yasaklı MAC listesi:** bu cihazların paketleri en başta düşürülür.
15. **DNS yönlendirmesi:** müşterinin bütün DNS sorguları (tcp/udp 53, hangi sunucuya giderse gitsin) cihaza yönlendirilir. Böylece DNS kaydı eksiksiz olur.
16. **Portala yönlendirme:** giriş yapmamış müşterinin HTTP (80) isteği portala (8080) yönlendirilir. Bu, telefonların "Ağda oturum açın" bildirimini tetikler.
17. **Hızlı ret:** giriş yapmamış müşterinin HTTPS ve diğer bütün trafiği anında reddedilir (TCP reset ya da ICMP admin-prohibited). Zaman aşımına düşülmez.
18. **Kapalı adresler:** müşteri tarafından özel ağlara (10/8, 172.16/12, 192.168/16, 100.64/10, 169.254/16) erişim ve IPv6 kapalıdır. DNS-over-TLS (853) engellidir.
19. **İnternete çıkış izni:** yetkili ve izinli cihazların paketleri `0x5651` işaretiyle işaretlenir. Ana güvenlik duvarı müşteri tarafından internete **yalnızca işaretli** paketleri geçirir. Bizim tablo yüklenmemişse hiçbir paket işaretlenmez ve **kimse internete çıkamaz**. Bu, sistemin bilerek seçilmiş "kapalıyken güvenli" davranışıdır.
20. **Bağlantı kaydının doğruluğu:** flow offloading kapalıdır; offload açıkken bağlantı kayıtları ve bayt sayaçları güvenilmez olur.

### A4. Giriş portalı (müşterinin gördüğü)
21. **Sunucu:** HTTP, 10.50.0.1:8080. JavaScript yok, dış kaynak yok, CSS sayfanın içinde. Üstteki bulutlu gökyüzü görseli JPEG olarak sayfaya gömülüdür.
22. **Başka sitelere giden istekler:** portalın kendi adresine gelmeyen istekler `302 → http://10.50.0.1:8080/?dst=<asıl adres>` ile cevaplanır.
23. **Sayfalar ve uç noktalar:**
    - `GET /`: giriş formu. Cihaz zaten yetkiliyse "Bağlısınız" sayfası.
    - `POST /kod-gonder`, `POST /dogrula`, `POST /tekrar-gonder`.
    - `GET /kvkk`: KVKK aydınlatma metni.
    - Diğer bütün yollar `/`'a yönlendirilir.
24. **Form:**
    - Ad ve soyad: 2–40 karakter; harf, boşluk, `-`, `'`, `.` kabul edilir.
    - Cep telefonu: TR GSM numarası, `5XXXXXXXXX` biçimine normalize edilir. `+90`, `0` ve boşluklu yazımlar da kabul edilir.
    - KVKK onay kutusu zorunlu.
    - Hata olursa girilen değerler korunur.
25. **İstemci kimliği:**
    - IP isteğin geldiği adrestir.
    - MAC önce DHCP kira tablosundan, bulunamazsa ARP tablosundan okunur.
    - MAC tanınamazsa "Wi-Fi'yi kapatıp açın" uyarısı gösterilir.
26. **Kod:**
    - 6 hane, güvenli rastgele üretilir, 180 sn geçerlidir.
    - En fazla 5 deneme hakkı vardır. Karşılaştırma sabit sürede yapılır.
    - Kodun kendisi hiçbir kayda yazılmaz; yalnızca deneme modunda sistem günlüğüne yazılır.
27. **Hız sınırları:**

    | Sınır | Değer |
    |---|---|
    | Aynı cihazdan yeniden kod isteme | 60 sn bekleme |
    | Numara başına | 3 SMS / 15 dk, 10 SMS / gün |
    | Cihaz (MAC) başına | 5 SMS / saat |
    | Hatalı kod | 10 hata → 15 dk kilit |
    | Sistem geneli günlük tavan | 300 SMS; sayaç diskte tutulur, yeniden başlatmada sıfırlanmaz |

    Aynı numaraya bekleme süresi içinde ikinci kez kod istenirse yeni SMS gönderilmez.
28. **Doğru kod girildiğinde, sırasıyla:**
    1. Aynı MAC'in eski oturumu kapatılır (`yeniden_giris`).
    2. Telefon başına cihaz sınırı (3) aşılıyorsa en eski oturum kapatılır (`cihaz_limiti`).
    3. Aynı IP'yi tutan başka bir oturum varsa o oturum "IP'siz beklemeye" alınır.
    4. Güvenlik duvarı setine `MAC . IP` 30 günlük süreyle eklenir. Bu adım başarısız olursa oturum yazılmaz.
    5. `sessions.json` dosyasına kayıt atomik olarak yazılır.
    6. Kişinin dosyası oluşturulur, kişi listesi (index) güncellenir.
    7. `OTURUM_BASLA` kaydı yazılır.
    8. Denetim kaydına `OTP_BASARILI` yazılır.
29. **Başarı sayfası:**
    - Metin: "Wi-Fi'ye bağlandınız". Oturum süresi gösterilmez.
    - 1 sn sonra kendiliğinden (`meta refresh`) `dst` adresine geçer; `dst` yalnızca `http(s)` ise kullanılır, yoksa google.com'a gider. Telefonların giriş penceresi kendiliğinden kapanır.
30. **Oturum süresi:** 30 gün (43.200 dk). Müşteriye gösterilmez, ayda bir yeniden SMS ister.
31. **HTTP güvenlik başlıkları:**
    - Her cevapta `Cache-Control: no-store`, `X-Frame-Options: DENY` ve sıkı bir CSP bulunur.
    - İstek gövdesi en fazla 4 KB, soket zaman aşımı 10 sn.
    - Bütün HTML çıktısı escape edilir.

### A5. SMS (OTP) sağlayıcıları
32. **NetGSM** (varsayılan):
    - Uç nokta: `POST https://api.netgsm.com.tr/sms/send/otp`, XML gövde (`mainbody/header{usercode,password,msgheader}/body{msg,no}`).
    - Cevap: `main/code` (0 = başarılı) ve `main/jobID`.
    - Hata kodları (20, 30, 40, 41, 50, 60, 70, 80, 100) kullanıcıya Türkçe mesaja çevrilir.
    - Yeniden deneme **yok**; çift SMS ve çift ücret riski var.
    - Mesaj: `WiFi dogrulama kodunuz: {kod}. Kod 3 dakika gecerlidir.` OTP servisi Türkçe karakter almadığı için metin ASCII'dir.
    - Hesap: abone no 8503027084, başlık `gztp.blgsyr`. Şifre yalnızca cihazdadır.
    - Kimlik bilgilerini SMS harcamadan denemek için bakiye sorgusu kullanılır: `POST https://api.netgsm.com.tr/balance/list/xml`, `stip=1` ya da `2`.
33. **Twilio Verify** (alternatif, `twilio.enabled=1` iken):
    - Gönderim: `POST https://verify.twilio.com/v2/Services/<VA>/Verifications`, parametreler `To=+90…`, `Channel=sms`, `Locale=tr`.
    - Doğrulama: `POST …/VerificationCheck`, cevapta `status=approved`. Kodu Twilio üretir ve doğrular.
    - Ağ hatasında müşterinin deneme hakkı düşmez.
34. **Deneme (mock) modu:** sağlayıcıya istek gitmez, kod sistem günlüğüne `MOCK OTP <tel> -> <kod>` olarak yazılır.
35. **Sırlar** (şifre, token): yalnızca cihazdaki 600 izinli ayar dosyasında durur. Repoya, kayıtlara ve panel çıktısına asla girmez; panelde yalnızca yazılabilir alan olarak bulunur.

### A6. Oturum yönetimi
36. **Durum dosyası:**
    - `sessions.json`, alanlar: `{mac: {phone, ad, soyad, ip, session_id, start, expires_epoch}}`.
    - Yazma: dosya kilidi + geçici dosya + atomik değiştirme.
37. **DHCP anında taşıma:**
    - Kayıtlı bir cihaz DHCP'den yeni IP aldığı anda oturumu o IP'ye taşınır. Eski IP'de ısrar edilmez, SMS istenmez.
    - Eski elemanı silip yenisini kalan süreyle ekler ve `OTURUM_IP_DEGISTI` yazar.
    - Bugünkü süre ~380 ms. Hedef, telefonun bağlantı kontrolünden önce bitmesi.
38. **Yedek taşıma yolları:** portal (cihaz sayfaya düşünce) ve kaydedici (30 sn'de bir) aynı taşımayı yapar.
39. **Aynı IP'de iki oturum olmaz:** yeni gelen alır, eskisi "IP'siz beklemeye" (`ip=""`) düşer, cihaz dönünce yeni IP'sine taşınır.
40. **IP başkasında ise:** kira tablosunda IP başka bir MAC'e geçmişse trafik eski oturuma yazılmaz.
41. **Süresi dolan oturum:** kaydedici 30 sn'de bir kontrol eder ve `OTURUM_BITIS neden=sure_doldu` yazar. Kaydın zamanı süre bitiş anıdır.
42. **Yeniden başlatmada:** süresi dolmamış oturumlar kalan süreleriyle güvenlik duvarına geri eklenir. Güvenlik duvarı yeniden yüklense de oturumlar kalır.
43. **Yönetici "at":** güvenlik duvarından siler, açık bağlantıları keser (`conntrack -D -s IP`) ve `OTURUM_BITIS neden=yonetici` yazar.

### A7. 5651 kayıtları (CSV)
44. **Biçim:** tek şema, 19 sütun, `;` ayraçlı, UTF-8 BOM'lu, satır sonu `\r\n`.
    - Sütunlar: `zaman;olay;telefon;ad;soyad;mac;ic_ip;protokol;ic_port;hedef_ip;hedef_port;nat_ip;nat_port;alan_adi;gonderilen_bayt;alinan_bayt;sure_sn;oturum_id;ek`.
    - CSV/formül enjeksiyonu koruması: `=`, `+`, `-`, `@`, tab ile başlayan hücrenin başına `'` eklenir; `;`, `"`, CR ve LF temizlenir.
45. **Günlük dosyalar** (`/srv/5651/gunluk/YYYY-MM-DD/`): `dhcp.csv`, `oturum.csv`, `trafik.csv`, `dns.csv`, `denetim.csv`, ayrıca kişinin o güne ait dosyası `kullanicilar/<tel>.csv`.
    - Satır, yazıldığı ana göre değil **kendi zaman alanına göre** doğru güne yazılır.
    - Birden çok süreç aynı dosyaya yazar; hepsi aynı dosya kilidiyle sıralanır, dosyaya yalnızca eklenir.
46. **DHCP kaydı (5651 iç IP dağıtım kaydı):**
    - Her atama, yenileme ve bırakmada zaman, olay, MAC, IP ve cihaz adı yazılır.
    - dnsmasq'ın DHCP betiği kancasıyla çalışır. `ACTION` değeri `add`, `update` ya da `remove` olur; `old` ya da `del` **değil**.
47. **Trafik kaydı:**
    - Kaynak: `conntrack -E -e NEW,DESTROY -o timestamp,extended,id -b 33554432 -s 10.50.0.0/24`. Çekirdek tarafındaki süzme, müşteri dışı olay fırtınasını keser.
    - Her bağlantı için `BAGLANTI_BASLA` ve `BAGLANTI_BITIS` yazılır: iç IP/port, hedef IP/port, NAT IP/port (reply tuple'ından), bayt sayısı.
    - Bağlantı süresi NEW ile DESTROY eşleştirilerek hesaplanır, çünkü bu çekirdekte `delta-time` yok.
    - Router'ın kendi trafiği atlanır.
    - Okuma borusu 1 MB.
48. **DNS kaydı:**
    - dnsmasq `log-queries=extra` → sistem günlüğü → `logread -f`.
    - Yalnızca `query[TIP] alan from IP` satırları alınır, istemci IP'siyle oturuma eşlenir.
49. **Kayıt boşluğu:** conntrack ENOBUFS verirse ya da süreç ölürse denetime `LOG_BOSLUK` yazılır (kuyruk ve döngü gecikmesi bilgisiyle) ve conntrack yeniden başlatılır.
    - Açık iş: kök nedeni henüz bilinmiyor. Kablo değişimi gibi olaylarda ~60 sn arayla tekrarlandı.
50. **Kişi listesi** (`kullanicilar/index.csv`): telefon, ad, soyad, ilk kayıt, son oturum.
51. **Denetim olayları:**
    - Portal: `OTP_ISTEK`, `OTP_GONDERILDI`, `OTP_HATA`, `OTP_BASARILI`, `OTP_HATALI_KOD`, `SMS_TAVAN`.
    - Sistem: `LOG_BOSLUK`, `SERVIS_BASLADI`, `WIFI_TAKILMA`, `GUN_KAPAT_ERTELENDI`.
    - Yedek ve saklama: `YEDEK`, `YEDEK_HATA`, `SAKLAMA_SILME`, `YEDEKSIZ_GUN`.
    - Panel: `PANEL_*` (giriş, ayar, port vb.).

### A8. Mühürleme, saklama, yedek
52. **00:15 `gun-kapat`:**
    - Bugünden önceki **mühürlenmemiş bütün günler** kapatılır. Cihaz o saatte kapalıysa kaçan günler de sonra kapanır.
    - Son 5 dk'da yazılmış dosya varsa kapatma ertelenir.
    - Kapatma adımları:
      1. Her CSV gzip'lenir.
      2. `MANIFEST.sha256` dosyası oluşturulur.
      3. Hash zinciri güncellenir: `zincir.txt` → `gün;manifest_sha;zincir_sha`, zincir_sha = sha256(önceki + manifest).
      4. Zaman damgası (RFC 3161) şimdilik kapalı.
      5. Dosyalar salt okunur yapılır.
53. **`dogrula`:** manifest hash'leri ve zincirin tamamı baştan doğrulanır.
54. **02:00 `yedekle`:**
    - Mühürlenmiş ama gönderilmemiş her gün `rsync -a --timeout=120 --ignore-existing` ile `kafe-bocafe@192.168.1.109:` adresine gönderilir.
    - SSH anahtarı `/root/.ssh/yedek_anahtar`. Sunucuda salt-yazma hesap ve rrsync kökü var.
    - Başarılı günlere `.yedeklendi` işareti konur.
    - `zincir.txt` ve `index.csv` sunucudaki eski sürümü ezmeden `eski/` altına yedeklenerek gönderilir.
55. **02:00 `temizle`:**
    - Yalnızca **2 yılı (730 gün) aşmış ve yedeklenmiş** günler silinir. Yedeklenmemiş gün asla silinmez, yerine `YEDEKSIZ_GUN` yazılır.
    - Kişi listesinden 2 yıldır gelmeyen kişiler çıkarılır.
56. **Kapasite:** ölçüm, günde 20 kişi × 2 saatte ~10 MB ham, ~1 MB sıkıştırılmış kayıt gösterdi. 27 GB onlarca yıl yeter.

### A9. Wi-Fi bakımı
57. **Wi-Fi takılma bekçisi:**
    - ath9k + iPhone'da, bağlı istemcinin çerçeveleri kartta düşebiliyor.
    - Köprüye ulaşan paketler MAC başına bir sayaçla sayılır (dinamik nft seti `wlan_rx`).
    - 30 sn'de bir `iw station dump` ile karşılaştırılır. Karta ≥100 çerçeve gelip köprüye 0 ulaşan istemci `ubus call hostapd.<arayüz> del_client` ile düşürülür ve `WIFI_TAKILMA` yazılır.
    - Not: dinamik setteki sayaç `update` ile artmayabilir. Yeni sistemde sayacı farklı ölçün, örneğin arayüz istatistiklerinden.
58. **Şifre modu değişimi:**
    - WPA2'den şifresize geçişte `wifi reload` **kullanılmaz**, tam `wifi down; wifi up` yapılır.
    - Neden: reload eski CCMP anahtarını kartta bırakıyor ve şifresiz ağda bütün veri çerçeveleri düşüyor. Telefon bağlanır ama IP alamaz.
    - Kontrol: `/sys/kernel/debug/ieee80211/phy*/keys/` boş olmalı.
59. **wpad sonradan kurulursa:** önce `kill -HUP ubusd`, sonra wpad'i yeniden başlat, sonra ağı yeniden başlat. Yoksa `HOSTAPD_START_FAILED` hatası alınır.

### A10. Yönetim paneli (https://192.168.1.110:8443)
60. **Sunucu ve erişim:**
    - HTTPS, öz-imzalı sertifika. Yalnızca 192.168.1.0/24'ten erişilebilir.
    - İlk açılışta hesap oluşturma sayfası (`/kurulum`) çıkar.
    - Şifreler PBKDF2 ile saklanır. Bilinmeyen kullanıcı için de sahte PBKDF2 hesaplanır, böylece süre farkından kullanıcı adı anlaşılmaz.
    - Giriş kilidi IP ve kullanıcı başına uygulanır: 15 dk içinde 5 hata.
    - Oturum çerezi kullanılır, her POST'ta CSRF kontrolü yapılır.
61. **İki rol var** (yeni sistemde değişti, 2026-10-02 kullanıcı kararı):
    - **Kafe sahibi:** **SMS sağlayıcısının API kimlik bilgileri hariç bütün ayarlara** erişir. İstisnalar: NetGSM kullanıcı kodu, şifre, mesaj başlığı, appkey ve Twilio'nun bütün SID, token ve anahtarları ve **SMS deneme modu** (gerçek SMS harcamasını başlatır; 2026-10-02 kullanıcı kararı). Bu alanları göremez ve değiştiremez; panelde yalnızca "tanımlı / tanımsız" durumunu görür.
    - Kafe sahibi kişisel veriyi ve kayıtları (Loglar, Kullanıcılar, Resmi talep) **göremez**; bunlar KVKK gereği hizmet sağlayıcıda kalır.
    - **Hizmet sağlayıcı** (Göztepe Bilgisayar): her şeyi görür, API kimlik bilgilerini yalnızca o girer.
    - Bugünkü sistemde "işletme yöneticisi" yalnızca bir kısım ayarı görebiliyordu; yeni sistemde bu genişliyor.
62. **Sayfalar:**

    | Sayfa | Görebilen | İçerik |
    |---|---|---|
    | Özet | ikisi | bağlı cihaz, bugün farklı kullanıcı, bugün SMS / tavan, log diski |
    | Bağlı cihazlar | ikisi | at düğmesi; kafe sahibine maskeli gösterilir |
    | Yasaklı cihazlar | ikisi | ekle / kaldır |
    | İzinli cihazlar | ikisi | ekle / kaldır |
    | Ayarlar | ikisi (API kimlik alanları yalnızca hizmet sağlayıcıda) | aşağıda |
    | Portlar | ikisi | A2 #10; aşağıda |
    | Wi-Fi | ikisi | SSID, şifre / şifresiz, kanal |
    | Sistem | ikisi | aşağıda |
    | Şifremi değiştir | ikisi | — |
    | Loglar | hizmet sağlayıcı | gün listesi, dosya görüntüleme ve indirme, günü indirme |
    | Kullanıcılar | hizmet sağlayıcı | arama + kişi detayı |
    | Resmi talep | hizmet sağlayıcı | iç IP, NAT portu, hedef IP, telefon, MAC + zaman ile arama ve talep paketi indirme |
    | Hesaplar | hizmet sağlayıcı | ekle / sil / şifre sıfırla |

    - **Ayarlar:** kafe adı, oturum süresi, telefon başına cihaz, SMS sınırları, uzak yedek, saklama günü, zaman damgası, kelime filtresi listesi. NetGSM ve Twilio kimlik alanları ile SMS deneme modu yalnızca hizmet sağlayıcıda.
    - **Portlar:** canlı kablo durumu. Uygula dendiğinde ayar yedeklenir ve **3 dk içinde "Onayla" denmezse eski ayar kendiliğinden geri gelir**. "Hemen geri al" düğmesi ve sağ/sol etiket değiştirme var.
    - **Sistem:** SMS sağlayıcı durumu, şimdi yedekle, bütünlüğü doğrula, günü kapat, servisleri yeniden başlat, yedek SSH açık anahtarı.
63. **Ayar kaydı:** her ayar değişikliği denetime eski ve yeni değerle yazılır (sır alanlar `***`). SMS ayarı değişirse portal yeniden başlatılır.
64. **Doğrulama kuralları:**
    - Yedek hedefi `kullanici@sunucu:` biçiminde klasörsüz de olabilir. `-` ile başlayan değer reddedilir.
    - Saklama süresi 30–3650 gün.
    - Oturum süresi 30–64800 dk.

### A11. Kelime filtresi
65. **Engelleme:** müşteri ağında alan adında **`bet`** veya **`porn`** geçen siteler açılmaz.
    - DNS sorgusunun içinde büyük/küçük harfe duyarsız kelime aranır ve sorgu reddedilir.
    - Bugün `iptables-nft` + `xt_string` ile yapılıyor; dnsmasq alan adının ortasındaki kelimeyi yakalayamıyor.
66. **İstisnalar:** `alphabet`, `between`, `better`, `diabet`, `tibet`.
67. **Ayar:** liste UCI'den (`hotspot.filter.word` / `allow`) okunur, açılışta uygulanır, güvenlik duvarı yeniden yüklense de kalır.

### A12. Komut satırı aracı (`hotspotctl`)
68. **Komutlar:** `durum`, `oturumlar`, `at`, `ara`, `yukle`, `gun-kapat`, `dogrula`, `temizle`, `yedekle`, `disa-aktar`, `sms-test`, `dhcp-olay`, `hesap-sifirla`.
    - `ara` arama türleri: `--ic-ip`, `--nat-port`, `--hedef-ip`, `--telefon`, `--mac`.
    - `sms-test` gerçek SMS için `[e/H]` onayı ister.
69. **Cron:**
    - `15 0 * * * hotspotctl gun-kapat`
    - `0 2 * * * hotspotctl yedekle; hotspotctl temizle`

### A13. Çalışma kuralları (yeni sistemde de geçerli)
70. Sır asla repoya girmez. Gerçek SMS yalnızca kullanıcının onayı ve verdiği numarayla gönderilir. `/srv/5651` altından dosya silinmez.
71. Ağ ve güvenlik duvarı değişikliği yalnızca **ölü adam anahtarıyla** yapılır: 5 dk içinde iptal edilmezse eski ayar geri yüklenir. Değişiklikten sonra erişim **yeni** SSH oturumuyla doğrulanır.
72. Kullanıcıya görünen bütün metinler Türkçe, kod tanımlayıcıları İngilizce. CSV başlıkları ASCII Türkçe.

### A14. Çok kafeli ürün (2026-10-02 kullanıcı kararı; yeni sistemin temel şartı)
Sistem tek bir kafeye (Bocafe) özel değildir, birçok farklı kafeye kurulacaktır. Bu dokümandaki Bocafe'ye özel değerler (`Bocafe`, `freewifi`, 192.168.1.110, `kafe-bocafe@192.168.1.109:`, MAC adresleri, NetGSM hesabı) yalnızca **örnektir**.
73. **Kodda kafeye özel sabit yok.** Kafe adı, cihaz adı (hostname), SSID, ağ adresleri, yedek hedefi ve hesap adları kurulumda belirlenir ve ayar dosyasında tutulur.
74. **Kurulum sihirbazı:**
    - Sıfırdan kurulan cihaz ilk açılışta panelde (yalnızca modem tarafından erişilebilen `/kurulum`) sihirbazı gösterir.
    - Sorulanlar: kafe adı, **kafe sahibinin kullanıcı adı ve parolası**, Wi-Fi adı ve şifresi (ya da şifresiz), port görevleri.
    - Hesaplar kurulumda oluşturulur; varsayılan kullanıcı adı veya parola yoktur. Kullanıcı adı `admin`, `root`, `bocafe` gibi tahmin edilebilir bir değer olmak zorunda değildir.
    - Parola en az 10 karakter olmalıdır.
    - Sihirbaz bir kez çalışır; sonra kapanır ve tekrar açılmaz.
75. **Hizmet sağlayıcı hesabı:** kurulumda ayrıca hizmet sağlayıcı hesabı oluşturulur (Göztepe teknisyeni). SMS API kimlik bilgilerini, kayıtlara erişimi ve yedek sunucusu hesabını yalnızca bu hesap yönetir.
76. **İşletim sistemi hesabı:** cihazın Linux kullanıcı adı ve parolası da kurulumda belirlenir, `bocafe` gibi sabit bir ad kullanılmaz. SSH yalnızca anahtarla açılır, parola ile giriş kapalıdır.
77. **Kafe başına yedek hesabı:** her kafe arşiv sunucusunda kendi salt-yazma hesabına (`kafe-<kısa-ad>`) yedek gönderir. Kurulum sihirbazı cihazın yedek açık anahtarını gösterir; hizmet sağlayıcı bunu sunucuya ekler (`sunucu/hotspot-kafe-ekle`).
78. **SMS hesabı:** NetGSM veya Twilio hesabı kafeler arasında ortak (hizmet sağlayıcının hesabı) ya da kafeye özel olabilir. Kimlik bilgileri her durumda yalnızca hizmet sağlayıcı tarafından girilir.

---

## B. İstem: Yeni OpenWrt cihazında Rust ile

Aşağıdaki metni yeni repoda Claude Code'a verin. Repoya önce bu dosyayı (`docs/RUST_YENIDEN_YAZIM.md`) ve `MASTER_ENGINEERING.md` dosyasını koyun.

```
docs/RUST_YENIDEN_YAZIM.md (A bölümü) ve MASTER_ENGINEERING.md dosyalarını baştan sona oku.
Görev: A bölümündeki 78 maddenin TAMAMINI, aynı donanımda (Celeron J1900, 32 GB SSD,
2× Realtek r8169, Atheros AR9287 ath9k) yeni kurulmuş OpenWrt 23.05.x (x86/64, ext4-combined-efi)
üzerinde Rust ile yeniden yaz. Davranış, CSV şeması, panel sayfaları, olay adları ve dosya
yolları birebir aynı kalsın (eski kayıtlar ve arşiv sunucusu uyumlu olsun).

Mimari kararlar:
- Tek Rust çalışma alanı (cargo workspace), musl ile statik derlenen TEK ikili dosya:
  `hotspot portal | logger | panel | ctl ...` alt komutları. Python ve pip yok.
- Çapraz derleme Windows/WSL'de: target x86_64-unknown-linux-musl, `opt-level="z"`,
  `lto=true`, `strip=true`, `panic="abort"`. Hedef: ikili dosya < 6 MB, üç servis toplam < 15 MB RAM.
- Asenkron çalışma zamanı için tokio (current_thread yeterli). HTTP için hyper ya da axum.
  Panel TLS'i için rustls (OpenSSL'e bağımlılık yok). XML için quick-xml,
  CSV için csv crate, şifre özeti için pbkdf2+sha2, gzip için flate2.
- nftables'a komut satırıyla değil netlink ile bağlan (rustables/nftnl).
  Komut kullanılırsa argümanlar her zaman liste olsun, kabuk asla kullanılmasın.
  conntrack olaylarını `conntrack -E` yerine doğrudan netlink'ten oku
  (NFNLGRP_CONNTRACK_NEW/DESTROY, alıcı tampon büyük, ENOBUFS → LOG_BOSLUK).
  Kök nedeni bilinmeyen LOG_BOSLUK sorununu (A7 #49) bu fırsatla ölç ve raporla.
- DNS kaydı için logread yerine: dnsmasq `log-facility` ile UNIX soketine ya da FIFO'ya
  yazsın, Rust dinlesin. Olmazsa `logread -f -e dnsmasq`.
- Kelime filtresi (A11) için xt_string yerine şunu değerlendir: Rust içinde küçük bir
  DNS ön-süzgeci (127.0.0.1:5353 → dnsmasq). Ama müşteri IP'si dnsmasq kaydında
  kaybolmasın. Kaybolacaksa iptables-nft + xt_string yöntemini aynen koru.
- Ayarlar yine UCI'de (/etc/config/hotspot); `uci -q show hotspot` çıktısını ayrıştır
  ya da libuci kullan. Servisler procd ile (respawn), cron satırları aynı.
- DHCP olayı: /etc/hotplug.d/dhcp/90-hotspot betiği `hotspot ctl dhcp-olay <mac> <ip>`
  çağırsın. Hedef: işlem 20 ms'nin altında bitsin.
- Her modül için birim testleri (cargo test), cihazdan alınmış gerçek conntrack ve
  dnsmasq satırları fixture olarak. MASTER_ENGINEERING.md §19.3'teki K1–K26 kabul
  testleri cihazda tek tek doğrulansın.

A14 ZORUNLU: sistem birçok kafeye kurulacak. Kafe adı, SSID, adresler, yedek hesabı ve
kullanıcı adları/parolaları kodda sabit olmasın; ilk açılıştaki kurulum sihirbazında sorulsun.
Kafe sahibi SMS API kimlik bilgileri ve SMS deneme modu hariç bütün ayarları yönetir; bunlar ve kayıtlar
yalnızca hizmet sağlayıcı hesabında. Bu istemdeki Bocafe değerleri (SSID, IP, hesap adları) yalnızca örnektir.

Kurallar: A13 maddeleri zorunlu. Fazları MASTER_ENGINEERING.md §20 sırasıyla uygula,
her fazın sonunda Türkçe kısa rapor ver. Gerçek SMS, disk bölümlendirme, sysupgrade
ve paket kaldırma için önce onay iste. Ağ değişikliğinden önce ölü adam anahtarı kur.
Eski Python sistemini, yeni sistem K1–K26'yı geçene kadar silme.
Sahada öğrenilen tuzakları (A9 #58–59, A7 #46–47, A3 #11, A3 #19) baştan uygula.
```

---

## C. İstem: Debian üzerinde Rust ile

### C1. OpenWrt'nin hazır verdiği, Debian'da sizin kurmanız gerekenler

| İş | OpenWrt'de | Debian 12/13'te |
|---|---|---|
| Ağ arayüzleri, köprü | netifd + UCI | systemd-networkd (`.netdev` + `.network`) ya da ifupdown + bridge-utils. NetworkManager kaldırılmalı |
| IP yönlendirme | açık | `net.ipv4.ip_forward=1` (sysctl) |
| Güvenlik duvarı | fw4 (hazır bölgeler) | **Kural setinin tamamını siz yazarsınız**: `/etc/nftables.conf` (WAN girişi, NAT/masquerade, müşteri bölgesi, yönetim kuralları) + bizim tablo. fw4'ün yaptığı her şey elle yapılır |
| DHCP/DNS | dnsmasq + UCI | dnsmasq paketi, `/etc/dnsmasq.d/hotspot.conf` |
| DHCP olay kancası | `/etc/hotplug.d/dhcp` | dnsmasq `dhcp-script=/usr/local/bin/hotspot-dhcp`; argümanlar `add|old|del mac ip host` (OpenWrt'den farklı!) |
| DNS sorgu kaydı | logd + logread | dnsmasq `log-facility=/run/dnsmasq/sorgu.fifo` ya da journald (`journalctl -fu dnsmasq -o cat`) |
| Wi-Fi yayını | wpad + UCI wireless | hostapd paketi, `/etc/hostapd/hostapd.conf` (`bridge=br-hotspot`, `ap_isolate=1`, `country_code=TR`, `channel=13`); ath9k çekirdekte hazır. Yedek AR9271 için `firmware-atheros` paketi |
| Takılan istemciyi düşürme | `ubus call hostapd.* del_client` | `hostapd_cli -i wlan0 deauthenticate <mac>` |
| Servisler | procd | systemd unit (`Restart=always`, `After=network-online.target`), `/srv` için `RequiresMountsFor=/srv` |
| Zamanlanmış işler | busybox cron | systemd timer (`OnCalendar=*-*-* 00:15`, `Persistent=true`: kapalıyken kaçan iş açılışta çalışır) |
| Saat | sysntpd | systemd-timesyncd ya da chrony |
| Ayar dosyası | UCI | `/etc/bocafe/hotspot.toml` (600). Panel buraya atomik yazar |
| Paket kurulumu | opkg | `apt install nftables dnsmasq hostapd conntrack iw rsync iptables` (xt_string Debian çekirdeğinde hazır) |
| Yükseltme | sysupgrade (`/srv` bölümünü riske atar) | `apt upgrade`; bölüm tablosu korunur. Çekirdek güncellemesinden sonra yeniden başlatma gerekir. unattended-upgrades yalnızca güvenlik güncellemeleri için açılsın |
| Disk / RAM | kök ~80 MB | kök ~1,5–2 GB, boşta ~150–250 MB RAM. J1900'de sorun değil, ucuz router'larda Debian uygun değil |

**Debian'ın artısı:** standart araçlar (systemd, journald, apt) var ve güvenlik güncellemeleri kolay; x86 cihazlarda daha rahat bakım sağlar.
**Eksisi:** OpenWrt'nin hazır verdiği router parçalarını (bölgeli güvenlik duvarı, netifd, wireless ayarları, hotplug) elle kurup bakımını yapmak gerekir. Hata yüzeyi de daha büyüktür.

### C2. İstem

```
docs/RUST_YENIDEN_YAZIM.md (A ve C1 bölümleri) ve MASTER_ENGINEERING.md dosyalarını baştan sona oku.
Görev: A bölümündeki 78 maddenin TAMAMINI aynı donanımda (Celeron J1900, 32 GB SSD,
2× Realtek r8169, Atheros AR9287) Debian 12 (bookworm, amd64, minimal, masaüstü yok)
üzerinde Rust ile yeniden yaz. Davranış, CSV şeması, panel, olay adları ve /srv/5651 yolları
birebir aynı kalsın.

Kurulum (Faz 0–2):
- Debian minimal kurulumu: kök 8 GB ext4, /srv için kalan alan ext4 (etiket hs5651, noatime).
  SSH yalnızca anahtar ile. NetworkManager yok.
- eth1 = WAN (DHCP istemcisi, modemde MAC'e sabit 192.168.1.110), eth0 + wlan0 → br-hotspot
  (10.50.0.1/24). systemd-networkd ile yap. IPv6 müşteri tarafında kapalı. ip_forward=1.
- /etc/nftables.conf: OpenWrt fw4'ün verdiği her şeyi (WAN'dan giriş yasak; yalnızca
  192.168.1.0/24'ten 22 ve 8443 açık; müşteri bölgesinden yalnızca 53, 67, 8080 ve ICMP;
  masquerade; 0x5651 işaretli olmayan müşteri trafiğinin internete çıkamaması) ve bizim
  `table inet hotspot` + `table bridge hotspot` tablolarını yaz. Fail-closed kalsın:
  hotspot tablosu yoksa kimse çıkamasın.
- dnsmasq: br-hotspot üzerinde DHCP 10.50.0.20–249, 2 saat; log-queries=extra;
  dhcp-script → `hotspot ctl dhcp-olay`. Debian'da dhcp-script argümanları
  `add|old|del <mac> <ip> [host]`. Bunu OpenWrt'nin add/update/remove'una eşle.
- hostapd: SSID freewifi, şifresiz, kanal 13, HT20, TR, ap_isolate=1, bridge=br-hotspot.
  Şifre modu değişince hostapd'yi tamamen yeniden başlat; reload kullanma.
- Kelime filtresi: iptables-nft + xt_string (Debian'da hazır) ya da Rust DNS ön-süzgeci;
  müşteri IP'si DNS kaydında korunmalı.
- Servisler systemd unit (Restart=always, RequiresMountsFor=/srv), gece işleri systemd
  timer (Persistent=true), saat systemd-timesyncd (TR NTP havuzu), Europe/Istanbul.
- Ayarlar /etc/bocafe/hotspot.toml (600, atomik yazma). Panel aynı alanları düzenlesin.
- Portlar sayfası (A10 #62) için: networkd dosyalarını yedekle, yenisini yaz,
  `networkctl reload`; 3 dk onay yoksa yedeği geri koy.

Rust tarafı B bölümündeki mimariyle aynı: tek statik ikili (musl), tokio, hyper/axum,
rustls, netlink ile nftables ve conntrack, cargo test + gerçek fixture'lar.
Wi-Fi bekçisi için `ubus del_client` yerine `hostapd_cli deauthenticate`.

A14 ZORUNLU: sistem birçok kafeye kurulacak. Kafe adı, SSID, adresler, yedek hesabı ve
kullanıcı adları/parolaları kodda sabit olmasın; ilk açılıştaki kurulum sihirbazında sorulsun.
Kafe sahibi SMS API kimlik bilgileri ve SMS deneme modu hariç bütün ayarları yönetir; bunlar ve kayıtlar
yalnızca hizmet sağlayıcı hesabında. Bu istemdeki Bocafe değerleri (SSID, IP, hesap adları) yalnızca örnektir.

Kurallar: A13 zorunlu. MASTER_ENGINEERING.md §20 fazlarını sırayla uygula, her fazın
sonunda Türkçe rapor ver. Ağ ve güvenlik duvarı değişikliklerinde ölü adam anahtarı kur
(systemd-run --on-active=300 ile geri yükleme) ve yeni SSH oturumuyla doğrula.
Gerçek SMS ve disk bölümlendirme için önce onay iste. K1–K26 kabul testlerinin
hepsi cihazda geçmeden işi bitmiş sayma.
```
