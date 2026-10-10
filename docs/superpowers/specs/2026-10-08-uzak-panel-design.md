# Cihaz paneline yalnızca panel.wificorrect.com'dan giriş + admin ayarları parolayla — tasarım (2026-10-09)

Önceki sürüm (2026-10-08: ayrı admin kullanıcısı, merkezden admin geçişi, dükkân içi giriş) kullanıcı kararıyla iptal.

## 1. Kullanıcı kararları (2026-10-09)
- Herkes **kendi müşteri numarası ve parolasıyla** girer; ayrı `admin` kullanıcısı yok.
- Giriş **yalnızca panel.wificorrect.com'dan**; dükkân içinden cihaz paneline giriş yok.
- **Admin ayarları** menüsü herkeste görünür; girmek için **admin parolası** gerekir. Admin parolası merkezden belirlenir
  (Yönetim → Admin parolası; 2026-10-08'de yapıldı), bütün cihazlarda aynı, kendiliğinden değişmez.
- **Panel hareketleri** de admin parolasının arkasında.
- Hizmet sağlayıcı bir cihaza o müşterinin numarası + parolasıyla girer (yönetimde görünür); ayrı geçiş düğmesi yok.
- Müşteri parolasını yönetici yazar, müşteri isterse değiştirir (2026-10-08'de yapıldı).

## 2. Kullanıcının gördüğü
1. Telefonda/bilgisayarda **panel.wificorrect.com** → müşteri numarası + parola.
2. Girişten sonra doğrudan **cihazın paneli** açılır (Özet, Cihazlar, Kayıtlar, Ayarlar, Admin ayarları); adres
   `cihaz.wificorrect.com`, geçerli sertifika, ikinci giriş yok.
3. **Admin ayarları** → admin parolası sorulur → doğruysa o oturumda 30 dk açık (Admin ayarları + Panel hareketleri).
4. **Çıkış** her iki oturumu kapatır, panel.wificorrect.com giriş sayfasına döner.
5. Cihaz bağlı değilse ya da ulaşılamıyorsa (kapalı, internetsiz): merkezin bugünkü sayfası (yedek arşiv, parola
   değiştirme) ve "Cihazınıza şu an ulaşılamıyor" uyarısı.
6. Merkezin yedek arşivi cihaz panelinden de açılır: Kayıtlar sayfasında "Merkezdeki yedek arşiv" bağlantısı
   (`panel.wificorrect.com/arsiv`).

## 3. Mimari
```
tarayıcı ─▶ Caddy  panel.wificorrect.com ─▶ wc-merkez (giriş, arşiv, /cihaza-git)
                    cihaz.wificorrect.com
                     1) istemcinin X-WFC-* başlıklarını siler
                     2) forward_auth → wc-merkez /cihaz-yetki: çerez → müşteri → bağlı cihazın tünel adresi
                          200 + X-WFC-Cihaz: 10.99.0.11, X-WFC-Kullanici: 1537344
                          302 → panel.wificorrect.com/giris (çerez yok/süresi doldu, lisans kapalı, cihaz bağlı değil)
                     3) reverse_proxy https://{X-WFC-Cihaz}:8443 (WireGuard; cihaz sertifikası doğrulanmaz)
```
- **Merkez (Python):**
  - panel.wificorrect.com girişi başarılıysa ve müşterinin bağlı, tünel adresi olan cihazı varsa → tek kullanımlık
    (60 sn) belirteçle `cihaz.wificorrect.com/_giris?t=…`'ye yönlendirir; yoksa bugünkü merkez sayfası.
  - `cihaz.wificorrect.com` ana bilgisayarı: `/cihaz-yetki` (Caddy sorar), `/_giris?t=` (belirteç → çerez `wcc`,
    HttpOnly, Secure, SameSite=Lax → `302 /`), `/_cikis` (iki oturumu da kapatır → panel giriş sayfası).
  - Cihaz oturumu bellek içi: müşteri + oluşturma/son kullanım; boşta 30 dk, en çok 12 saat; müşteri parolası değişince,
    üyelik/lisans kapanınca, cihaz serbest bırakılınca düşer.
  - Mevcut merkez sayfaları `panel.wificorrect.com/arsiv` altında (bağlantılar buna göre).
- **Cihaz (Rust):**
  - **Kimlik:** istek tünel arayüzünden ve merkezin tünel adresinden (10.99.0.1) gelir ve `X-WFC-Kullanici` cihazın bağlı
    olduğu müşteri numarasıysa → işletme sahibi oturumu (cihazın normal oturum/CSRF düzeni). Aynı kaynaktan
    `X-Forwarded-For` gerçek istemci IP'si (denetim, kilit). Başka her istek: başlık yok sayılır.
  - **Yerel giriş yok:** bağlı cihazda giriş sayfası yalnızca "panel.wificorrect.com'dan girin" der. Bağlı olmayan
    cihaz dükkân içinden yalnızca **eşleştirme** ekranı açar (numara + parola → bugünkü merkeze bağlanma akışı); başka
    sayfa yok.
  - **Admin kilidi:** `/admin-ayarlari` (ve altı) ile `/panel-hareketleri` için oturumda geçerli admin kilidi yoksa
    parola formu; parola cihazdaki admin özetiyle (merkezden gelen) doğrulanır; doğruysa oturuma 30 dk kilit açık
    yazılır; hatalı denemeler giriş kilidiyle aynı sınır; denetim `PANEL_ADMIN_ACILDI` / `PANEL_ADMIN_HATALI`.
    Admin özeti hiç gelmemişse (merkezde admin parolası yok) Admin ayarları kapalı: "Admin parolası belirlenmemiş".
  - Rol kavramı sadeleşir: tek kullanıcı türü (işletme sahibi) + admin kilidi. Bugün yalnızca admin'e açık her şey
    (Admin ayarları, Panel hareketleri, Özet'teki SMS ayrıntıları) admin kilidine bağlanır.
  - **Çıkış:** merkez üzerinden açılan oturumda `/_cikis`'e yönlenir.
  - **Güvenlik duvarı:** 8443 yalnızca tünelden (10.99.0.0/24); modem ağından 8443 yalnızca cihaz bağlı değilken
    (eşleştirme için). SSH değişmez (modem ağı + tünel; servis için).
- **Güvenlik:** Caddy istemci başlıklarını siler; cihaz başlığa yalnızca merkezin tünel adresinden güvenir (WireGuard eş
  anahtarıyla doğrulanmış kaynak). Merkez ele geçirilirse cihazlara erişilir — SSH ve yedek için bugün de durum aynı.

## 4. Kurulum
- DNS `cihaz.wificorrect.com` → merkez (2026-10-08 eklendi). Caddyfile'a site bloğu (forward_auth + reverse_proxy).
- Merkez güncellemesi (sudo, kullanıcı); cihaz yeni sürüm.

## 5. Test
- Merkez: giriş → belirteç → `wcc` çerezi; belirteç tek kullanımlık ve 60 sn; `/cihaz-yetki` (çerezsiz, süresi dolmuş,
  lisans kapalı, serbest, parola değişti → giriş yönlendirmesi; geçerli → başlıklar); `/_cikis`; cihazı olmayan müşteri
  merkez sayfasında; arşiv `/arsiv` altında.
- Cihaz: başlık yalnızca 10.99.0.1'den; numara uyuşmazsa red; X-Forwarded-For yalnızca merkezden; bağlı cihazda yerel
  giriş yok, bağlı olmayan cihazda yalnızca eşleştirme; admin kilidi (parola yok/yanlış/doğru, 30 dk, Panel
  hareketleri de kilitli, admin özeti yoksa kapalı); çıkış yönlendirmesi; güvenlik duvarı kuralı.
- Uçtan uca: telefondan panel.wificorrect.com → cihaz paneli → Admin ayarları (parola) → çıkış.

## 6. Kapsam dışı
- Merkezden SSH geçişi; merkez oturumlarının kalıcı saklanması.
