# Kurulum ISO'su — tasarım (2026-10-05)

## 1. Amaç
Boş bir cihaz (J1900 sınıfı, 2 Ethernet, tek disk) USB'den açılıp **tek onayla** WifiCorrect cihazı olur. Teknisyen
kurulum sırasında hiçbir değer girmez. Kalıba cihaza ya da işletmeye özel hiçbir şey girmez (KURULUM_GUNLUGU "Hedef"
kuralları). Cihaz, işletme sahibi panelde müşteri numarasıyla giriş yapana kadar misafirlere internet vermez.

Başarı ölçütü: aynı ISO iki farklı cihaza kurulur, ikisi de monitörsüz açılır, ikisi farklı müşteri numaralarıyla
bağlanır; admin parolaları ve anahtarları farklıdır; NetGSM bilgileri girilmeden SMS çalışır.

## 2. Kullanıcı kararları (2026-10-05)
- **admin parolası merkezden:** her cihazın parolası ayrıdır, cihaz bağlanınca merkezden gelir. ISO'da parola yok.
- **NetGSM bilgileri merkezden:** yönetim ekranında bir kez girilir, bütün bağlı cihazlara gider.

## 3. Kurulumda ne olur (teknisyenin gördüğü)
1. Ethernet 1 modeme takılı (kurulum Debian paketlerini internetten indirir), monitör + klavye takılı.
2. USB'den açılır. Menü: **"WifiCorrect kur — DİSKTEKİ HER ŞEY SİLİNİR"**. Menü zaman aşımıyla kendiliğinden
   başlamaz (yanlış makinede kazara silme olmasın); Enter ile başlar.
3. Soru sorulmaz: dil tr, saat Europe/Istanbul, klavye trq, ana bilgisayar adı `wificorrect`, disk = USB olmayan ilk
   disk, bölümler EFI + kök (ext4) + takas. Root parolası yok (kilitli), SSH yalnızca anahtarla.
4. Kurulum sonunda WifiCorrect paketi kurulur, cihaz kapanır ("USB'yi çıkarın" yazar). Süre hedefi: 15 dk.
5. İlk açılış monitörsüz olabilir. Konsolda (bağlıysa) yönetim adresi yazar: `https://<ip>:8443`.

## 4. İlk açılış ve sahiplendirme
- Ana makine SSH anahtarları ve makine kimliği kurulumda üretilir (her kurulum farklı). WireGuard ve yedek anahtarı,
  panel sertifikası program tarafından ilk ihtiyaçta cihazda üretilir (bugünkü davranış).
- Bağlı değilken: portal kapalı, misafir internete çıkamaz (bugünkü `hizmet_acik`). Panel girişi müşteri numarasıyla.
- İlk girişte bağlanma (bugünkü akış) + yeni: merkez yanıtında **admin parola özeti** ve **SMS ayarları** gelir.
- admin, cihaz bağlanmadan önce panele giremez (parola yok). Acil durumda konsoldan `wificorrect ctl admin-parola`
  bugünkü gibi çalışır; merkez bir sonraki eşitlemede kendi değerini yazar.

## 5. admin parolası merkezden
- **Merkez:** `cihaz` kaydına `admin_parola` (açık, yönetimde görünür — müşteri parolasıyla aynı karar) eklenir.
  Cihaz bağlanınca rastgele üretilir (16 karakter, karışıklık yaratan harfler yok). Yönetimdeki müşteri sayfasında
  görünür ve **"admin parolasını yenile"** düğmesi vardır.
- **API:** `/api/giris` ve `/api/eslesme` yanıtına `admin: {tuz, ozet, yineleme}` eklenir. Açık parola cihaza
  **gönderilmez**; cihaz yalnızca özeti saklar (PBKDF2-SHA256, mevcut hesap biçimi).
- **Cihaz:** özet farklıysa `hesaplar.json`'daki `admin` girdisini değiştirir, admin oturumlarını kapatır, denetime
  `ADMIN_PAROLA_MERKEZ` yazar. Yenileme bir sonraki eşitlemede (06:00) uygulanır; yönetimde "cihaza ulaşma zamanı:
  sonraki eşitleme" notu.
- Cihaz serbest bırakılınca (zorla ayır / üyelik bitti) admin girdisi silinmez; yeni bağlanmada yenisi gelir.

## 6. SMS (NetGSM) ayarları merkezden
- **Merkez:** yönetimde yeni **"SMS ayarları"** sayfası (yalnızca yönetici): deneme modu, NetGSM kullanıcı kodu,
  şifre (yalnızca yazılır, geri gösterilmez), mesaj başlığı, uygulama anahtarı. Tek ayar takımı, bütün cihazlar.
  Kaydederken bugünkü cihaz kuralı: deneme modu kapalıysa üç zorunlu alan dolu olmalı.
- **API:** `/api/giris` ve `/api/eslesme` yanıtına `sms: {mock, provider:"netgsm", netgsm:{usercode, password,
  msgheader, appkey}}` eklenir (yalnızca kimliği doğrulanmış cihaza, HTTPS). Yönetimde ayar hiç kaydedilmemişse
  alan gönderilmez → cihaz kendi ayarını korur.
- **Cihaz:** gelen değerler `ayarlar.toml`'a yazılır; değiştiyse portal yeniden başlatılır, denetime
  `SMS_AYARI_MERKEZ` (şifre yazılmaz). Bağlı cihazda Admin ayarları'ndaki NetGSM alanları ve deneme modu salt
  okunur, "merkezden yönetiliyor" notuyla. Twilio cihazda kalır (bu işin dışında).
- Değişiklik cihaza bir sonraki eşitlemede ulaşır (lisans açıkken 06:00). Yönetimde not edilir.

## 7. ISO'nun üretimi
- **Yaklaşım:** Debian 13 resmi netinst ISO'su + otomatik cevap dosyası (preseed) + WifiCorrect `.deb` paketi,
  `xorriso` ile yeniden paketlenir. Seçilmeyenler: tamamen çevrimdışı disk kalıbı (debootstrap) — her güvenlik
  güncellemesinde kalıbı yenilemek gerekir, kurulumda internet zaten var; live-build — fazla hareketli parça.
- **`.deb` paketi** (`scripts/paket.sh`, `dpkg-deb`, ek araç yok): `/usr/local/bin/wificorrect` + `deploy/debian/`
  altındaki her dosya. Bağımlılıklar: `dnsmasq hostapd iw wireless-regdb conntrack rsync curl nftables
  wireguard-tools unattended-upgrades sudo bridge-utils openssh-server`. `postinst`: dizinler (`/srv/5651`,
  `/srv/hotspot/state`, 700), birimlerin etkinleştirilmesi, `update-grub`. Var olan `ayarlar.toml`, `ag.toml`,
  `hesaplar.json`, `merkez.json` ve anahtarlar **ezilmez** (paket güncellemesi de aynı yolu kullanır).
- **Panelin ürettiği dosyalar pakete girmez** (`interfaces.d/wificorrect`, `arayuzler.nft`, `issue.d/wificorrect.issue`,
  `yasak-siteler.conf`): güncelleme port/Wi-Fi/yasaklı site ayarlarını ezmesin. İlk kurulumda `wificorrect ctl ag-ilk` üretir:
  internet alan port = kurulumun internete çıktığı arayüz (varsayılan rota), yoksa J1900 varsayılanı, yoksa Ethernet 1.
  Böylece Ethernet adları farklı bir makinede de roller doğru olur.
- **Açılış ayarları repoya:** `deploy/debian/etc/default/grub.d/wificorrect.cfg` → `GRUB_TERMINAL=console`,
  `intel_idle.max_cstate=1` (2026-10-05'te cihazda elle yapıldı; monitörsüz açılış ve J1900 donması), ayrıca
  `console=tty0 console=ttyS0` (seri konsol: QEMU duman testi ve servis; monitörde giriş ekranı yine görünür).
- **Elle yapılmış diğer ayarlar:** plan aşamasında canlı cihaz (`1537344`) ile `deploy/debian/` karşılaştırılır,
  eksik her ayar repoya alınır. Kural değişmez: kalıp yalnızca repodan üretilir.
- **SSH erişimi:** hizmet sağlayıcının **açık** SSH anahtarı ISO derlenirken GitHub değişkeninden (`WFC_SSH_PUB`)
  `root/.ssh/authorized_keys`'e yazılır. Açık anahtar gizli değildir ama repoda tutulmaz (repo genel ürün).
- **GitHub Actions:** `release.yml` her `v*` etiketinde `.deb` ve `wificorrect-kurulum-<sürüm>.iso` üretir, sürüme
  ekler, SHA256SUMS'a yazar. Netinst ISO'su indirilirken Debian'ın imzalı SHA256SUMS'ı ile doğrulanır.

## 8. Test
- **Birim:** cihaz (Rust) — merkezden gelen admin özeti ve SMS ayarının uygulanması, salt okunur alanlar; merkez
  (Python) — yeni API alanları, parola yenileme, SMS ayarları sayfası ve doğrulaması.
- **Paket:** CI'da `.deb` temiz bir Debian 13 kapsayıcısına kurulur (bağımlılıklar çözülüyor, dosyalar yerinde,
  ikinci kurulumda ayar dosyaları ezilmiyor).
- **ISO duman testi (CI):** QEMU'da (KVM varsa) boş 8 GB diske kurulum, diskten açılış, seri konsoldan
  `wificorrect surum` ve servislerin `active` olduğu görülür. KVM yoksa adım atlanır ve bu raporda yazılır.
- **Gerçek donanım (kabul):** canlı cihaz silinmez; kurulum **boş bir yedek cihazda** ya da herhangi bir PC'de
  denenir. Monitörsüz ikinci açılış, müşteri numarasıyla bağlanma, admin parolasının yönetimde görünüp panelde
  çalışması, deneme modu kapalıyken sizin onayınızla tek gerçek SMS.

## 9. Kapsam dışı
- Kurulu cihazların uzaktan paket güncellemesi (apt deposu). Bu ISO'nun `.deb`'i sonraki işin temelidir.
- Twilio bilgilerinin merkezden yönetimi, işletme başına farklı SMS başlığı.
- Wi-Fi kartı sürücüleri (firmware) — bugünkü cihaz da Wi-Fi'siz çalışıyor.

## 10. Riskler
- **Yanlış diski silme:** USB olmayan ilk disk seçilir; menü kendiliğinden başlamaz.
- **Kurulumda internet yok:** netinst paketleri indiremez; kurulum ekranı hata verir. Teknisyen notu: Ethernet 1
  modeme takılı olmalı. (Ethernet 2'ye takılırsa kurucu yine internet bulabilir; ilk açılışta roller `ag.toml`
  varsayılanıyla düzelir.)
- **NetGSM şifresi merkezde açık saklanır:** cihaza göndermek için gerekli. Veritabanı yalnızca `wcpanel`
  kullanıcısının (600); yedeklerde de aynı koruma.
