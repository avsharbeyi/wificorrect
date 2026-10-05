# Yönetim merkezi (wificorrect.com) ve merkezden müşteri hesapları — Tasarım

> Tarih: 2026-10-04 · Durum: kullanıcı incelemesi bekliyor
> Bu belge `2026-10-03-merkezi-panel-design.md`'nin **yerine geçer** (orada hesap cihazdan merkeze gidiyordu; artık
> merkezde açılıp cihaza iniyor). Eski kafe paneli tasarımlarından (`openwrt-kafe-paneli`) gün özetleri, arama ve
> oturum/kilit kuralları aynen alınır.

## 1. Amaç ve kararlar

Hizmet sağlayıcı (admin) bütün müşterileri **tek noktadan** yönetir: müşteri hesabını açar, parolasını sıfırlar, cihazını
serbest bırakır, üyeliğini bitirir; cihazların ve yedeklerin durumunu görür. Cihazda sahiplendirme ekranı yoktur.

**Kullanıcı kararları (2026-10-04):**
- Müşteri hesabı = **kullanıcı numarası + parola**, yönetim merkezinde açılır. Cihaz önceden müşteriye tanımlanmaz.
- Cihaz her zaman **giriş ekranıyla** açılır. Müşteri numarası ve parolasıyla ilk giren müşteri cihazın sahibi olur;
  sonra o cihaza yalnızca o müşteri (ve admin) girer.
- **Bir müşteri numarası = bir cihaz.** Çok şubeli işletmeye her şube için ayrı numara açılır.
- Müşteri parolasını değiştirebilir (cihazdan ya da merkezi panelden).
- İşletme adı ve unvanı müşteri cihazdaki mevcut ayar ekranlarından girer/değiştirir.
- Yönetim merkezine **internetten** girilir; iki adımlı doğrulama şimdilik yok (giriş kilidi + fail2ban var).
- Cihaz–merkez eşitlemesi **günde bir kez, 06:00'da**; başarılıysa ertesi gün 06:00'a kadar yapılmaz.

**Başarı ölçütü:** Admin yönetim merkezinde müşteri açar, numara ve parolayı müşteriye verir. Müşteri yeni bir cihazın
giriş ekranına bunları yazar; cihaz tüneli ve yedeği kendiliğinden kurar, admin cihaza hiç dokunmaz. Ertesi gün admin
müşteriyi yönetim merkezinde "çevrimiçi, yedek güncel" görür; müşteri `panel.wificorrect.com`'a aynı numara ve
parolayla girip kayıtlarına bakar.

**Kapsam dışı:** İki adımlı doğrulama; birden çok yönetici; bir numaraya birden çok cihaz; cihazda yerel ek sahip hesabı;
müşterinin kendi kendine kaydolması; ödeme/fatura.

## 2. Genel yapı

```
yonetim.wificorrect.com ── admin: müşteriler, cihazlar, parola sıfırlama, serbest bırakma, hareketler, kayıtlar
panel.wificorrect.com   ── müşteri: kayıtları (gün/kişi/arama), parola değiştirme
api.wificorrect.com/api/ ── cihazlar: ilk giriş, günlük eşitleme, parola değişimi
          │ Caddy (HTTPS, Let's Encrypt) → tek Python programı (stdlib), kullanıcı wcpanel, /srv'ye erişimi yok
          ▼
   /var/lib/wificorrect/merkez.db (SQLite, wcpanel 600)
          │ yetki gereken işler dosya kuyruğuna yazılır (/var/lib/wificorrect/kuyruk/<id>.json)
          ▼
   wc-kuyruk (root, systemd .path ile anında): wificorrect-sunucu cihaz-ekle / cihaz-kapat
             sonuç → /var/lib/wificorrect/kuyruk-sonuc/<id>.json (yalnızca root yazar); root veritabanına dokunmaz
   wc-istatistik (root, 02:30/12:30): arşiv → gün özetleri (arşivleri wc-durum'un listesinden alır)
   wc-durum (root, 5 dk): arşiv klasörleri + kayit.csv + tünel el sıkışmaları → durum.json (son gün, boyut, el sıkışma)
```

- Program: mevcut Python paneli (`openwrt-kafe-paneli/sunucu/panel`) bu repoya `scripts/sunucu/merkez/` olarak taşınır
  ve genişletilir. Yalnızca standart kütüphane (`sqlite3` dahil). Üç alan adı tek süreçte, `Host` başlığına göre ayrılır.
- Panel süreci root olmaz. WireGuard eşi ve yedek hesabı açmak root gerektirir: panel kuyruk klasörüne iş dosyası yazar,
  `wc-kuyruk` (root) işi doğrulayıp çalıştırır, sonucu ayrı (root'un) klasöre yazar. Panel sonucu en çok 15 sn bekler.
- Cihaz API'si `api.wificorrect.com`'dadır; kök `wificorrect.com` hosting firmasındadır (185.106.208.2), ona dokunulmaz.
- `wc-kuyruk` girdiyi yeniden doğrular (numara rakam, anahtarlar biçim), panelden gelen hiçbir metni kabuğa vermez.

## 3. Kayıtlar (SQLite)

| Tablo | Alanlar |
|---|---|
| `musteri` | `numara` INTEGER PK (7 hane, rastgele 1000000–9999999; sıralı değil — müşteri sayısı/sırası anlaşılmasın; kullanıcı kararı 2026-10-04), `tuz`, `ozet`, `yineleme` (cihazla aynı PBKDF2-HMAC-SHA256 biçimi), `not_` (admin notu), `uyelik` (`aktif`/`bitti`), `bitis`, `olusturma`, `son_giris` |
| `cihaz` | `id`, `musteri` (numara), `durum` (`bagli`/`serbest_birakiliyor`/`serbest`), `tunel_ip`, `wg_pub`, `ssh_pub`, `anahtar_ozet` (cihaz anahtarının SHA-256'sı), `isletme_adi`, `unvan`, `surum` (cihazın bildirdiği), `son_eslesme`, `baglanma`, `ayrilma` |
| `hareket` | `zaman`, `kim` (`admin` / müşteri numarası / `cihaz:<id>`), `ip`, `olay`, `musteri`, `ayrinti` |
| `eski_arsiv` | `ad` (eski arşiv klasörü), `musteri` (bağlandığı numara, boş olabilir) |

- Bir müşterinin en çok bir `bagli` cihazı olur (kısmi benzersiz dizin). Eski cihaz satırları tarihçe olarak kalır.
- Yönetici hesabı veritabanında değil, `/var/lib/wificorrect/merkez/yonetici.json`'da (tek kayıt; `wificorrect-sunucu yonetici-parola` → `wc-yonetici`).
- Arşiv klasörü müşteri numarasıdır: `/srv/wificorrect-arsiv/<numara>/veri`, yedek hesabı `wfc-<numara>`. İşletme adı
  değişse ya da cihaz değişse kayıtlar aynı yerde birikir.
- Parola en az 10 karakter. Admin'in ürettiği parola 12 karakter, karışması kolay harfler (`0/O`, `1/l/I`) olmadan.

## 4. Cihaz API'si (`https://api.wificorrect.com/api/…`)

JSON gövde, en çok 4 KB, IP başına dakikada 20 istek.

| Uç | Girdi | Çıktı | Kural |
|---|---|---|---|
| `POST /api/giris` | `numara`, `parola`, `wg_pub`, `ssh_pub`, `isletme_adi`, `unvan`, `surum` | `tuz`, `ozet`, `yineleme`, `cihaz_anahtari`, `tunel_ip`, `sunucu_pub`, `uc_nokta`, `yedek_hedefi` | Parola doğru, üyelik aktif, müşterinin `bagli` cihazı yok (ya da aynı `wg_pub` ile yeniden deneme) → cihaz bağlanır, kuyruğa `cihaz-ekle`. Başka cihaz bağlıysa 409 "Bu numara başka bir cihazda kullanılıyor". Giriş kilidi (numara ve IP başına 15 dk'da 5) |
| `POST /api/eslesme` | `cihaz_anahtari`, `isletme_adi`, `unvan`, `surum` | `durum` (`bagli`/`serbest`), `tuz`, `ozet`, `yineleme`, `uyelik` | Cihazın bildirdikleri kaydedilir, `son_eslesme` güncellenir |
| `POST /api/parola` | `cihaz_anahtari`, `eski`, `yeni` | `tuz`, `ozet`, `yineleme` | Eski parola doğruysa merkezde değişir |

- Cihaz anahtarı 32 bayt rastgele, yalnızca ilk girişte bir kez döner; merkez yalnızca SHA-256'sını tutar.
- Serbest bırakılan cihazın anahtarı `serbest` döner (cihaz yerel temizliği yapsın diye) ve cihaz temizliği bildirdikten
  sonra (`/api/eslesme` `temizlendi: true`) geçersizleşir.

## 5. Yönetim merkezi ekranları (`yonetim.wificorrect.com`)

1. **Giriş** — kullanıcı adı + parola; 15 dk'da 5 hata → kilit; sunucuda fail2ban (journal'daki `GIRIS_HATALI`).
2. **Müşteriler** (ana sayfa) — numara, işletme adı (cihazdan), not, cihaz (çevrimiçi = son 10 dk'da tünel el sıkışması;
   değilse "son eşitleme X önce"; son eşitleme 26 saatten eskiyse sorunlu / "cihaz yok"), son gelen gün (**GECİKTİ**: bugünden 2 günden eski), son 7 gün kişi sayısı, üyelik.
   Sorunlular üstte. Numara/ad/not ile arama.
3. **Yeni müşteri** — not (isteğe bağlı) → numara ve üretilmiş parola **bir kez** gösterilir.
4. **Müşteri sayfası**
   - Bilgiler, cihaz (tünel adresi, sürüm, son eşitleme, son yedek), VPN açıkken cihaz paneli bağlantısı `https://<tunel_ip>:8443`.
   - **Parola sıfırla** — yeni parola bir kez gösterilir; merkezi panelde hemen, cihazda bir sonraki 06:00 eşitlemesinde geçerli.
   - **Cihazı serbest bırak** — numara yazılarak onaylanır; cihaz `serbest_birakiliyor`. Cihaz bir sonraki eşitlemede
     kayıtları gönderir, siler, `temizlendi` bildirir → `serbest`, kuyruğa `cihaz-kapat`. Numara yeni cihaza girebilir
     (eski cihaz `serbest` olduktan sonra). Cihaz arızalı/kayıpsa (hiç eşitlenmeyecekse) **"Cihaz ulaşılamaz, zorla ayır"**:
     cihaz doğrudan `serbest`, kuyruğa `cihaz-kapat`; o cihazda kalmış gönderilmemiş günler kaybolabilir (uyarı ekranda,
     işlem `hareket`'e yazılır).
   - **Üyeliği bitir** / **yeniden aç** — bitince cihaz serbest bırakılır, numara yeni cihaza giremez; müşteri merkezi
     panele girip eski kayıtlarına bakmaya devam eder.
   - **Kayıtlar** — gün/kişi/arama (gerekçe istenir, kaydedilir). **Hareketler** — o müşterinin olayları.
5. **Hareketler** — bütün sistem, en yeni üstte.
6. **Eski arşivler** — OpenWrt dönemi ve elle eklenmiş arşivler; bir müşteriye bağlanabilir (yalnızca bu ekrandan, komut yok).
7. **Hesabım** — admin parolasını değiştirir.

## 6. Müşteri paneli (`panel.wificorrect.com`)

Giriş: müşteri numarası + parola (giriş kilidi aynı). Eski paneldeki son 30 gün sayıları, gün → kişi → oturum/site/
bağlantı, arama aynen; kaynak müşterinin arşivi + ona bağlı eski arşivler. Kişi verisi açan sayfalar 30 dk geçerli
gerekçe ister; her görüntüleme `hareket`'e yazılır. Parola değiştirme buradan da yapılır (cihaz bir sonraki 06:00'da alır).
Üyeliği bitmiş müşteri de girer (yalnızca okuma).

## 7. Cihaz tarafı (Rust)

- **Kurulum ekranı kalkar** (`/kurulum`, `Hesaplar::setup`). Cihaz her zaman giriş ekranı açar; "müşteriye bağlanmadı"
  gibi bir yazı yoktur.
- **Giriş:**
  - `admin` → yerel doğrulama (değişmedi).
  - Cihaz bağlı değilse (merkez dosyası yok): numara + parola + `wg_pub` + `ssh_pub` `/api/giris`'e. Başarılıysa
    `/etc/wificorrect/merkez.json` (600): numara, tuz/özet/yineleme, cihaz anahtarı, son eşitleme zamanı; ayarlara uzak
    erişim (`vpn.wificorrect.com:51820`, sunucu anahtarı, tünel adresi) ve yedek (`wfc-<numara>@10.99.0.1:`) yazılır,
    `ctl uzak-uygula` arka planda başlar. Merkeze ulaşılamazsa "Merkeze ulaşılamadı, internet bağlantısını kontrol edin."
  - Cihaz bağlıysa: yerel özetle doğrulanır (internet gerekmez). Başka numara → "Bu cihaz başka bir müşteriye ait."
- **Günlük eşitleme** (kaydedici): her gün 06:00'dan sonraki ilk turda `/api/eslesme`. Başarılıysa ertesi gün 06:00'a
  kadar yapılmaz; başarısızsa 30 dk'da bir yeniden dener. Yeni özet gelirse `merkez.json` güncellenir. `serbest` gelirse
  mevcut fabrika ayarı akışı (`ctl fabrika`) çalışır; kayıtlar teslim edilemezse silme yapılmaz, bir sonraki denemede
  tekrar edilir. Fabrika akışı `merkez.json`'u, uzak erişim ve yedek ayarlarını da siler; bitince `temizlendi` bildirir.
- **Parola değiştir** (cihaz paneli): `/api/parola`; başarılıysa yerel özet hemen güncellenir. İnternet yoksa
  "Parola değişimi için internet gerekli."
- **Hesaplar sayfası:** yerel ek sahip hesabı açma kalkar; yalnızca müşteri ve `admin` görünür.
- **İstemci:** `curl --fail --max-time 20 https://api.wificorrect.com/api/…`, gövde standart girdiden (`--data-binary @-`);
  parola komut satırında görünmez. Sertifika doğrulanır.
- Admin ayarlarındaki uzak erişim/yedek alanları elle düzeltme için kalır.

**Sonuç (kullanıcıya bildirildi):** admin'in parola sıfırlaması ve serbest bırakması cihaza en geç ertesi sabah 06:00'da
iner; o zamana kadar cihazda eski parola geçerlidir.

## 8. Sunucu komutları (`wificorrect-sunucu`)

- `cihaz-ekle` ve yeni `cihaz-kapat` artık `wc-kuyruk` tarafından çağrılır; ad = müşteri numarası.
- `yonetici-parola` — yönetim merkezi admin parolası (terminalde iki kez sorulur, en az 12 karakter).
- `liste` — müşteri numarasıyla. `tasi-paketle` — `merkez.db`, `yonetici.json`, eski arşivler de pakete girer.

## 9. Geçiş

- Bocafe → yeni bir müşteri numarası (7 hane, rastgele); OpenWrt dönemi arşivi `bocafe` ona (Eski arşivler ekranından) bağlanır.
- Test cihazının elle eklenmiş tüneli (`bocafe-test`) cihaz planında, cihaz yeni sürüme geçerken `cihaz-kapat` ile kapanır
  (önce kapatılırsa cihazın yedeği durur); arşivi "eski arşiv" olur. Cihaz giriş ekranına döner; Bocafe'nin numarasıyla girince
  kendiliğinden yeniden kaydolur.
- Eski panel servisleri (`wc-panel`, `hesaplar.json`) yeni program kurulunca durdurulur; eski hesap dosyası silinmez,
  `hesaplar.json.eski` olarak kalır.
- Caddy: `yonetim.wificorrect.com`, `panel.wificorrect.com`, `api.wificorrect.com` → aynı program. DNS'e `yonetim` ve
  `api` A kayıtları (<dış-ip>) eklenir; kök kayda dokunulmaz.
- Dükkândaki cihazın `https://panel.wificorrect.com`'a modemin dış IP'si üzerinden ulaştığı doğrulandı (2026-10-04, NAT
  döngüsü TCP'de çalışıyor); `api` aynı IP'de olduğu için yerel DNS kaydı gerekmez.

## 10. Güvenlik

- Panel süreci `/srv`'yi göremez; kişi verisi root'un ürettiği özetlerden gelir. Root, `wcpanel`'in klasörüne yazmaz.
- Tüm POST formlarında CSRF; çerezler `HttpOnly; Secure; SameSite=Strict`; alan adı başına ayrı çerez.
- Cihazdan gelen metinler (işletme adı, unvan, sürüm) ≤ 200 karakter, yazdırılabilir; HTML'de kaçışlı.
- Parola özetleri sabit zamanlı karşılaştırılır; olmayan numarada da özet hesaplanır.
- Yönetim girişi internete açık: güçlü parola (≥ 12), kilit, fail2ban. İki adımlı doğrulama sonradan eklenebilir.

## 11. Test

- Python (`scripts/sunucu/merkez/tests/`, düz assert): müşteri açma/numara sırası; parola özeti cihazla aynı (Rust'ın
  ürettiği bilinen değer); `/api/giris` (doğru, yanlış, kilit, başka cihaz bağlı → 409, aynı cihaz yeniden deneme,
  üyelik bitti); `/api/eslesme` (özet değişimi, serbest → temizlendi → anahtar geçersiz); `/api/parola`; kuyruk girdisi
  doğrulaması; alan adı ayrımı (müşteri yönetim sayfasına, yönetici müşteri paneline giremez); A müşterisi B'yi göremez;
  gerekçe; XSS kaçışı.
- Rust (`scripts/test.sh`): giriş akışı (bağlı değil → API, bağlı → yerel, başka numara reddi, internet yok);
  eşitleme zamanlaması (06:00, başarıdan sonra ertesi güne kadar yok, hatada 30 dk); `serbest` → fabrika;
  parola değişimi; `curl` komutunda parolanın argümanda olmaması.
- Uçtan uca: yönetimde müşteri aç → cihazda giriş → tünel ve yedek kendiliğinden → ertesi sabah eşitleme →
  merkezi panelde kayıtlar → serbest bırak → cihaz temizlenip giriş ekranına döner.

## 12. Ek (2026-10-05, kullanıcı kararları): lisans ve parola görme

- **Müşteri parolası yönetimde görünür.** Sunucu müşteri parolasını (`parola_acik`) özetle birlikte açık saklar; yalnızca
  yönetim ekranındaki müşteri sayfasında gösterilir (müşteri panelinde, API'de, hareket kaydında yok). Veritabanı `wcpanel`
  600, `secure_delete` açık. Bu özellikten önce açılmış müşterilerde "bilinmiyor — sıfırlayın".
- **Yıllık lisans.** Her müşterinin `lisans_bitis` (YYYY-AA-GG) ve `askida` alanı var; yeni müşteriye 1 yıl verilir.
  Yönetim: "Lisansı 1 yıl uzat" (bitişin üstüne, geçmişse bugünden), "Askıya al / Askıdan çıkar", "Bitişi ayarla".
  API `giris`/`eslesme` yanıtları `lisans` (aktif|bitti|askida) ve `lisans_bitis` döner. Ödeme ekranı sonradan
  `veri.lisans_uzat` çağırır.
- **Cihazda uygulama.** Lisans açık = durum "aktif" ve bitiş günü (TR) geçmemiş. Kapalıysa ya da cihaz bir müşteriye bağlı
  değilse portal misafire internet vermez, açık misafir oturumları kapanır (kapalıyken her turda). Bitiş tarihini cihaz
  kendisi uygular (internetsiz de). Lisans kapalıyken merkezle 30 dk'da bir eşitlenir (ödeme/askıdan çıkarma en geç
  30 dk'da cihaza iner); açıkken her gün 06:00. Sahip ve admin panele girmeye devam eder; panelde uyarı görünür.
