# Merkezi panel (panel.wificorrect.com) — yeni sisteme uyarlama — Tasarım

> Tarih: 2026-10-03 · Durum: kullanıcı incelemesi bekliyor
> Temel: eski kafe paneli (`openwrt-kafe-paneli/docs/superpowers/specs/2026-09-28-kafe-paneli-design.md` ve
> `…-detay-design.md`). Bu belge onları genişletir; çelişkide bu belge geçerlidir.

## 1. Amaç

- İşletme sahibi, cihaz kurulumunda oluşturduğu **kullanıcı adı ve şifreyle** `https://panel.wificorrect.com`'a girer ve
  kendi işletmesinin kayıtlarını (günlük kişi sayısı, gün → kişi → oturum/site/bağlantı, arama) görür.
- Hesap elle açılmaz: cihaz kurulumdan sonra hesabı sunucuya kendisi bildirir.
- **Üyelik bitse de** sahip aynı hesapla girer ve arşivdeki kayıtlarını (730 gün) görmeye devam eder.
- Hizmet sağlayıcı (admin) tüm işletmelerin üyelik ve etkinlik durumunu **tek sayfadan** (İşletmeler) görür.

**Kullanıcı kararları (2026-10-03):** Kullanıcı adı çakışmasında ilk gelen hesabı alır. Kişi verisi açan sayfalarda
cihazdaki gibi gerekçe zorunlu ve her görüntüleme kayıtlı. Admin girişi yalnızca VPN'den.

**Kapsam dışı:** Üyelik bittikten sonra merkezi panelden mühürlü resmi talep paketi indirme (internete açık panelin
`/srv`'ye erişmesini gerektirir; gerekirse root'un ürettiği ayrı bir paket kuyruğu olarak eklenir). Üyelik
sonlandırma ve geri açma web'den yapılmaz (yalnızca komut; panel yetkisiz süreçtir).

## 2. Mimari

```
Sahip tarayıcısı ─HTTPS─▶ Caddy (panel.wificorrect.com) ─▶ wc-panel        127.0.0.1:8081  (rol: isletme)
Admin (VPN içinde) ─HTTP─▶ ─────────────────────────────▶ wc-panel-yonetim 10.99.0.1:8082  (rol: admin)
                                         │ ikisi de aynı panel.py, kullanıcı wcpanel, /srv'ye erişim YOK
                                         ▼ yalnızca okur
     /var/lib/wificorrect/{istatistik,detay}/<isletme>/…   genel.json   /etc/wificorrect/hesaplar.json
                                         ▲ yazar (root)
     wc-genel (her 5 dk)       : hesap.json → hesaplar.json, tünel/arşiv durumu → genel.json
     wc-istatistik (02:30, 12:30): arşiv → sayılar + gün özetleri (değişmedi, yalnızca kaynak yolları)
                                         ▲ okur
     /srv/wificorrect-arsiv/<ad>/veri/   (yeni cihazlar, rrsync yalnızca-yazma)
     /srv/hotspot-arsiv/<ad>/            (OpenWrt dönemi, eski bocafe arşivi — okunmaya devam eder)
```

- Kod Python (sunucuda mevcut, test edilmiş), stdlib. Eski repodaki `sunucu/panel/*` bu repoya
  `scripts/sunucu/panel/` olarak taşınır; `common.py` ve `panel_auth.py` artık buradaki kopyalardır.
- `wificorrect-sunucu kur` paneli de kurar/günceller (Caddy, iki panel servisi, iki zamanlayıcı, `wcpanel` kullanıcısı).

## 3. Hesabın cihazdan sunucuya gelmesi

### 3.1 Cihaz tarafı (Rust)
- Cihaz `hesap.json` üretir ve yedekle birlikte hedefin köküne (`veri/hesap.json`) gönderir:
  ```json
  {"surum": 1, "kullanici": "ahmet", "tuz": "…", "ozet": "…64 hex…", "yineleme": 120000,
   "isletme_adi": "Bocafe", "unvan": "…", "zaman": "2026-10-03T21:00:00+03:00"}
  ```
  `tuz/ozet/yineleme` cihazdaki `hesaplar.json`'un sahip kaydıyla aynıdır (PBKDF2-HMAC-SHA256; şifrenin kendisi hiçbir
  yerde yok). Sahip hesabı yoksa (kurulum yapılmamış) dosya gönderilmez.
- Gönderim: her `yedekle` çalıştırmasında (gece 02:00 ve "Şimdi yedekle"), ayrıca kurulumdan, sahip şifre/kullanıcı adı
  değişiminden ve fabrika ayarına dönüşten sonra arka planda bir kez (`ctl hesap-gonder`, uzak yedek ayarlı değilse
  hiçbir şey yapmaz; başarısızsa sessizce bir sonraki yedeğe kalır). `--backup` kullanılmaz (eski sürüm birikmesin).
- Fabrika ayarına dönüşte sahip hesabı silinir; cihaz yeniden kurulana kadar yeni `hesap.json` gelmez, sunucudaki hesap
  olduğu gibi kalır. Cihaz **başka bir işletmeye** verilecekse admin eski kaydı `uyelik-bitir` ile kapatır ve cihazı yeni
  işletme adıyla yeniden tanıtır (yeni arşiv, yeni hesap); aynı kayıtta kalırsa yeni sahibin `hesap.json`'u kural 3 ile
  eski sahibin hesabını devralır ve eski kayıtları görür.

### 3.2 Sunucu tarafı: `wc-genel` (root, 5 dakikada bir)
Her kayıtlı cihaz (`kayit.csv`, tür `cihaz` ve `bitti`) için `veri/hesap.json`'u okur. **Güvenilmez girdi** sayılır:
- Kullanıcı adı `^[a-z0-9._-]{3,32}$` ve `admin` değil; `ozet` 64 hex; `tuz` 1–64 yazdırılabilir ASCII;
  `yineleme` 100.000–1.000.000 (büyük değerle girişte CPU tüketme saldırısı olmasın); metinler ≤ 200 karakter, dosya ≤ 4 KB.
  Geçersizse uygulanmaz, durum `hesap=gecersiz`.
- Panel hesabı işletmeye bağlıdır (`isletme: <ad>`). Kurallar:
  1. İşletmenin hesabı yoksa ve kullanıcı adı boşsa → hesap açılır (`kaynak: cihaz`).
  2. Kullanıcı adı **başka bir işletmenin** hesabındaysa → uygulanmaz, durum `hesap=cakisma` (ilk gelen hesabı korur).
  3. İşletmenin hesabı varsa → özet/tuz/yineleme güncellenir; kullanıcı adı değiştiyse hesap yeni ada taşınır
     (açık oturumları düşer).
  4. İçerik son uygulananla aynıysa hiçbir şey yazılmaz.
- Üyeliği bitmiş (`bitti`) işletmenin `hesap.json`'u artık uygulanmaz (cihaz zaten gönderemez); hesap kalır.
- `hesaplar.json` yazımı atomik, ardından `wcpanel:wcpanel 600` (eski `wc-hesap` kuralı).

### 3.3 Şifre değiştirme
- Üyelik aktifken şifrenin kaynağı cihazdır: merkezi panelde "Şifre" sayfası yerine "Şifrenizi işletmenizdeki cihaz
  panelinden değiştirin" notu görünür (yoksa bir sonraki eşitleme değişikliği geri alırdı).
- Üyelik bittikten sonra sahip şifresini merkezi panelden değiştirir (eski `/sifre` akışı).
- Şifresini unutan sahip için: `wificorrect-sunucu panel-sifre <kullanici>` (şifre terminalde iki kez sorulur).

### 3.4 Panelin cihaz özetini doğrulaması
`panel_auth` iki kayıt biçimini tanır: eski (`salt` hex bayt, `hash`, 200.000 tur) ve cihaz biçimi
(`tuz` metin baytları, `ozet`, kayıttaki `yineleme`). Karşılaştırma `hmac.compare_digest`; bilinmeyen kullanıcıda da
sahte özet hesaplanır (zamanlama farkı olmasın).

## 4. Veri kaynakları

- İşletme listesi: `kayit.csv`'deki cihazlar (`/srv/wificorrect-arsiv/<ad>/veri`) + `/srv/hotspot-arsiv/` altındaki
  eski klasörler. Aynı ad ikisinde de varsa ikisi ayrı işletme sayılmaz: eski arşiv, yeni arşivde olmayan günleri
  tamamlar (bocafe'nin OpenWrt dönemi günleri kaybolmaz).
- `wc-istatistik`'in özet kuralları (sayım, gün özeti, arama dizini, artımlılık, bozuk gün) **değişmez**; yalnızca
  gün klasörlerini bu iki kaynaktan toplar. CSV biçimi Rust cihazında aynıdır (doğrulanacak: gerçek bir gün klasörüyle test).
- Saklama: `hotspot-arsiv-saklama` yeni arşiv kökünü de kapsar (730 günden eski gün klasörü + `detay/<ad>/<gün>.json`).

## 5. İşletmeler sayfası (admin)

`wc-genel` her çalıştırmada `/var/lib/wificorrect/genel.json` (`root:wcpanel 640`) yazar; sayfa yalnızca bunu okur.

| Sütun | Kaynak |
|---|---|
| İşletme adı / unvan | `hesap.json` (yoksa kayıt adı) |
| Panel kullanıcısı | `hesaplar.json`; `cakisma`/`gecersiz`/`yok` kırmızı |
| Cihaz bağlantısı | `wg show wg0 latest-handshakes` → "2 dk önce" / "3 gündür yok" / "hiç" |
| Son gelen gün | arşivdeki en yeni gün klasörü; bugünden 2 günden eskiyse **GECİKTİ** |
| Son 7 gün | `istatistik/<ad>.json` günlük kişi sayıları |
| Üyelik | Aktif / Sona erdi (tarih) — `kayit.csv` |
| Arşiv boyutu | `du -sb` (ponytail: 5 dk'da bir tüm arşivi tarar; yüzlerce işletmede günlüğe çekilir) |

Sıralama: önce sorunlular (GECİKTİ, çakışma, bağlantı yok), sonra ada göre. Satıra tıklayınca o işletmenin gün/kişi/arama
sayfaları açılır (`/i/<ad>/…`); sahip sayfalarıyla aynı kod, işletme oturumdan değil URL'den gelir — **yalnızca admin
dinleyicisinde ve admin rolünde**. Sahip dinleyicisinde `/i/…` yolu yoktur (404).

## 6. Admin girişi ve dinleyiciler

- `panel.py --dinle 127.0.0.1:8081 --rol isletme` (wc-panel, Caddy arkası) ve
  `panel.py --dinle 10.99.0.1:8082 --rol admin` (wc-panel-yonetim, `After/BindsTo=wg-quick@wg0`).
- Her dinleyici yalnızca kendi rolündeki hesapları kabul eder: admin hesabı internetteki panelden giremez, sahip hesabı
  VPN dinleyicisinden giremez. Admin dinleyicisi düz HTTP'dir (trafik WireGuard içinde şifreli); çerezi `Secure`
  bayraksız ve farklı adlıdır (`wc_yonetim`). Sunucu güvenlik kuralı 8082'yi yalnızca yönetici tünel adreslerine
  (10.99.0.2–.9) açar; cihazlar (10.99.0.10+) erişemez.
- Admin hesabı: `wificorrect-sunucu panel-admin <kullanici>` (şifre terminalde sorulur, en az 12 karakter).
- Giriş kilidi, oturum süreleri, CSRF, güvenlik başlıkları eski tasarımdaki gibi (admin dinleyicisinde başlıkları
  panel.py kendisi ekler, Caddy yok).

## 7. Gerekçe ve hareket kaydı

- Kişi verisi açan sayfalar (`/gun/…`, `/ara`, admin'de `/i/<ad>/gun/…`, `/i/<ad>/ara`) gerekçe ister: cihazdaki
  `/gerekce` akışı (seçenekler + serbest metin, 30 dk geçerli, oturuma bağlı). Sayı sayfaları gerekçe istemez.
- Her görüntüleme `/var/lib/wificorrect/hareket/<isletme>.csv`'ye eklenir (`wcpanel` yazar, `600`):
  `zaman;kullanici;rol;ip;yol;gerekce`. Girişler ve başarısız girişler de yazılır.
- Admin, İşletmeler sayfasından her işletmenin "Panel hareketleri"ni görür; sahip görmez (cihazla aynı kural).

## 8. Üyelik sonlandırma: `wificorrect-sunucu uyelik-bitir <ad>`

- WireGuard eşi kaldırılır (`wg syncconf`), yedek hesabı kilitlenir (`authorized_keys` boşaltılır; arşiv **silinmez**),
  `kayit.csv` satırı `bitti;<ad>;…;<bitiş tarihi>` olur.
- Panel hesabı, arşiv ve özetler kalır; saklama süresi (730 gün) dolan günler normal akışla silinir.
- Tersine işlem (üyeliği yeniden açma) şimdilik yok: gerekirse `cihaz-ekle` aynı adla yeniden yapılır (komut `bitti`
  kaydını yeniden etkinleştirir, arşiv ve hesap korunur).
- `liste` komutu `bitti` satırlarını ve hesap durumunu (`tamam/cakisma/gecersiz/yok`) da gösterir.

## 9. Test

- Python (`scripts/sunucu/panel/test_*.py`, düz assert, Windows'ta çalışır): cihaz biçimi özet doğrulama (Rust'ın ürettiği
  bilinen bir kayıtla); `hesap.json` uygulama kuralları 1–4, çakışma, geçersiz girdi (büyük yineleme, kötü ad, büyük
  dosya), `bitti` işletme; genel.json üretimi (sahte `wg` çıktısı, sahte arşiv); iki kaynaktan gün toplama; dinleyici-rol
  ayrımı (admin internet dinleyicisinden giremez, sahip `/i/…`'ye ulaşamaz, A işletmesi B'yi göremez); gerekçe olmadan
  kişi sayfası açılmaz; hareket kaydı yazılır; cihazdan gelen `<script>` kaçışlı.
- Rust (`scripts/test.sh`): `hesap.json` içeriği (sahip kaydıyla aynı özet, şifre yok), kurulumda yoksa üretilmez.
- Sunucuda uçtan uca: cihazda "Şimdi yedekle" → ≤ 5 dk içinde sahip panel.wificorrect.com'a cihaz şifresiyle girer;
  admin VPN'den İşletmeler sayfasını görür; `uyelik-bitir` sonrası sahip hâlâ girer, cihaz yedek gönderemez.
