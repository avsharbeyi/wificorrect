# Cihaz paneline tarayıcıdan uzaktan erişim + parolaları yönetici belirler — tasarım (2026-10-08)

## 1. Amaç
İşletme sahibi telefonundan ya da bilgisayarından **yalnızca tarayıcıyla** kendi cihazının paneline (Özet, Cihazlar,
Kayıtlar, Ayarlar) her yerden girer; VPN, port yönlendirme, sertifika uyarısı yok. Hizmet sağlayıcı (admin) da
yönetimden herhangi bir müşterinin cihaz paneline aynı yolla geçer. WireGuard yalnızca cihaz ↔ merkez arasında kalır.

Ayrıca (kullanıcı kararı 2026-10-08): müşteri parolası ve cihaz admin parolası **rastgele üretilmez**; yönetici yazar,
müşteri isterse kendi parolasını sonra değiştirir. Hiçbir parola kendiliğinden değişmez.

## 2. Kullanıcının gördüğü
1. `panel.wificorrect.com` → müşteri numarası + parola (bugünkü giriş) → menüde **Cihaz paneli** düğmesi.
2. Düğme `cihaz.wificorrect.com`'u açar: cihazın kendi paneli, ikinci giriş yok, geçerli sertifika.
3. Cihaz panelindeki **Çıkış** merkez oturumunu da kapatır, `panel.wificorrect.com` giriş sayfasına döner.
4. Yönetimde müşteri sayfasında **Cihaz paneline git** (admin rolüyle).
5. Dükkân içinden `https://<cihaz-ip>:8443` bugünkü gibi çalışır (yerel giriş).

## 3. Mimari
```
tarayıcı ──HTTPS──▶ Caddy (cihaz.wificorrect.com)
                     │ 1) istemcinin X-WFC-* başlıklarını siler
                     │ 2) forward_auth → wc-merkez /cihaz-yetki : çerez geçerli mi, hangi cihaz?
                     │      200 + X-WFC-Cihaz: 10.99.0.11, X-WFC-Kullanici: 1537344 | admin
                     │      302 → panel.wificorrect.com/giris  (çerez yok / lisans kapalı / cihaz bağlı değil)
                     └ 3) reverse_proxy https://{X-WFC-Cihaz}:8443  (WireGuard tüneli; cihaz sertifikası
                          kendinden imzalı → doğrulama yok: kimliği WireGuard doğruluyor)
```
- **Merkez (`wc-merkez`, Python):** `cihaz.wificorrect.com` ana bilgisayarı için yalnızca `/cihaz-yetki` (Caddy'nin
  sorduğu) ve özel yollar `/_giris?t=…` (tek kullanımlık geçiş belirteci → çerez) ve `/_cikis` (çerezi siler).
- **Geçiş belirteci:** `panel.wificorrect.com` → `POST /cihaz-paneli` (CSRF) → 60 sn geçerli tek kullanımlık belirteç →
  `302 https://cihaz.wificorrect.com/_giris?t=…` → merkez çerezi koyar (`wcc`, HttpOnly, Secure, SameSite=Lax,
  yalnızca cihaz.wificorrect.com) → `302 /`. Yönetimden: `POST /m/<n>/cihaz-paneli` (admin rolü, seçilen müşteri).
- **Cihaz oturumu (merkezde):** müşteri numarası + rol + oluşturma/son kullanım; boşta 30 dk, en çok 12 saat;
  müşteri parolası değişince, üyelik/lisans kapanınca, cihaz serbest bırakılınca geçersiz. Bellek içi (bugünkü
  oturumlar gibi; servis yeniden başlarsa yeniden girilir).
- **Cihaz (Rust):** istek **kaynak IP'si merkezin tünel adresi** (10.99.0.1; ayardaki uzak sunucu ağından türetilir)
  ve `X-WFC-Kullanici` varsa: değer cihazın bağlı olduğu müşteri numarasıysa işletme sahibi, `admin` ise hizmet
  sağlayıcı oturumu açılır (cihazın normal oturum/CSRF düzeni aynen). Başlık başka kaynaktan gelirse yok sayılır.
  Aynı kaynaktan `X-Forwarded-For` gerçek istemci IP'si olarak alınır (denetim kaydı, giriş kilidi). Merkez üzerinden
  açılan oturumda **Çıkış** `/_cikis`'e yönlenir.
- **Güvenlik:** Caddy istemci başlıklarını siler; cihaz başlığa yalnızca tünelden ve merkez adresinden güvenir
  (WireGuard eş anahtarıyla doğrulanmış kaynak). Merkez ele geçirilirse bütün cihazlara erişilir — bugün de SSH ve
  yedek için durum aynı (kabul edilen risk). Her müşteri yalnızca kendi cihazına gider (oturum → müşteri → bağlı cihaz).

## 4. Parolalar
- **Müşteri parolası:** yeni müşteri formunda yönetici yazar (en az 10 karakter); "Parola sıfırla" → "Parola belirle"
  formu. Rastgele üretim kalkar. Müşteri cihaz panelinden / merkez panelinden değiştirebilir (bugünkü akış).
- **Cihaz admin parolası:** yönetimde müşteri sayfasında "Admin parolası belirle" formu; boşsa merkez cihaza admin
  bilgisi **göndermez** (cihazdaki mevcut admin aynen kalır). Bağlanmada rastgele üretim kalkar. Mevcut kayıtlar
  (1537344 için üretilmiş olan) değişmez. Admin uzaktan erişimi merkez üzerinden olduğu için cihaz admin parolası
  yalnızca dükkân içinden yerel giriş için gerekir.

## 5. Kurulum
- DNS: `cihaz.wificorrect.com` A → merkez dış IP (2026-10-08 eklendi).
- Caddyfile'a site bloğu; merkez güncellemesi (sudo, kullanıcı). Sunucuda tünel adresi 10.99.0.1'den cihazların
  8443'üne erişim (cihaz güvenlik duvarı 10.99.0.0/24'e zaten açık).
- Cihaz yeni sürüm (geliştirme döneminde `scripts/gelistir.sh`; sonra sürüm/ISO).

## 6. Test
- Merkez: `/cihaz-yetki` (çerezsiz → giriş yönlendirmesi; geçerli → başlıklar; lisans kapalı / serbest / parola
  değişti → red), belirteç tek kullanımlık ve 60 sn, `/_cikis`, admin geçişi yalnızca admin oturumundan, yönetici
  parola formları (kısa parola reddi, rastgele üretim yok), admin parolası boşsa API'de `admin` alanı yok.
- Cihaz: başlık yalnızca 10.99.0.1'den kabul (başka IP'den aynı başlık → giriş sayfası), müşteri numarası
  uyuşmazsa red, `admin` → hizmet rolü, X-Forwarded-For yalnızca merkezden, Çıkış yönlendirmesi.
- Uçtan uca (kurulumdan sonra): telefondan `panel.wificorrect.com` → Cihaz paneli → Cihazlar sayfası; çıkış.

## 7. Kapsam dışı
- Merkezden SSH geçişi (yalnızca admin; sonra).
- Merkez oturumlarının kalıcı saklanması.
