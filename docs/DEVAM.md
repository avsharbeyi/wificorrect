# Devir notu — kaldığın yerden devam (2026-10-10)

Bu dosya, projeye yeni bir oturumda/hesapta devam eden ajan içindir. Önce bunu, sonra `CLAUDE.md`'deki belgeleri oku.

## Kullanıcıyla çalışma kuralları (zorunlu)
- **Her zaman Türkçe** konuş. Kullanıcı kısa ve doğrudan yanıt ister; "devam edeyim mi" diye sık sorma, uygun varsayılanla ilerle.
- Cihaz komutlarını **sen çalıştırırsın**: `ssh wificorrect` (root, anahtarla; cihaz 192.168.1.110, modem ağı).
  Sunucu: `ssh -i ~/.ssh/gbserver gbserver@192.168.1.109` (sudo parolası **yalnızca kullanıcıda**; sudo gereken işleri
  kullanıcıya terminalde çalıştırtırsın — cmd için: `"C:\Program Files\Git\bin\bash.exe" <betik>`).
- **Asla** parola, API anahtarı, belirteç girme/kullanma — kullanıcı sohbete yazsa bile. `openwrt/serverp` dosyasını okuma.
- Gerçek SMS yalnızca kullanıcı açıkça onaylarsa. `/srv/5651` dosyalarını silme. Ağ/güvenlik duvarı değişikliği yalnızca
  ölü adam anahtarıyla (yedek + `systemd-run --on-active=300` geri alma + YENİ bağlantıyla doğrula + iptal).
- Sırlar repoya girmez (repo **herkese açık**: github.com/avsharbeyi/wificorrect).
- Cihaz testleri **cihazda** koşar: `bash scripts/test.sh wificorrect [süzgeç]` (~5 dk; aynı anda iki tane çalıştırma).
  Cihaza kurulum: `bash scripts/gelistir.sh` + `ssh wificorrect 'systemctl restart wificorrect-panel wificorrect-portal wificorrect-kaydedici'`.
  Merkez testleri: `sh scripts/sunucu/merkez/tests/hepsi.sh python`.
- Merkez kurulumu: `C:\Users\HUAWEI\Desktop\wificorrect\rza\.superpowers\sunucu-guncelle.sh` (`git archive uzak-panel` ile;
  önce veritabanı yedeği, sonra `kur.sh` — Caddyfile doğrulanmadan kurulmaz). Kullanıcı çalıştırır (sudo).

## Ürün özeti
WifiCorrect: işletmelere yıllık abonelikle satılan 5651 uyumlu, SMS (NetGSM) doğrulamalı misafir internet cihazı.
Cihaz: Debian 13 + Rust ikilisi `wificorrect` (`src/`). Merkez: Python stdlib (`scripts/sunucu/merkez/`), Caddy 2.6.2 arkasında,
`yonetim.wificorrect.com` (hizmet sağlayıcı), `panel.wificorrect.com` (müşteri), `api.wificorrect.com` (cihazlar),
`cihaz.wificorrect.com` (cihaz paneline geçit). Cihaz ↔ merkez WireGuard (10.99.0.0/24; merkez .1, cihaz 1537344 = .11).
Canlı müşteri: **Göztepe Bilgisayar = 1537344**, cihaz 192.168.1.110.

## Dallar ve durum
`main` ← (birleştirilmedi) `duzeltme/portal-sms-conntrack` (PR #2) ← `kurulum-iso` ← `arayuz-menu` ← **`uzak-panel` (güncel, itildi)**.
Hepsi `uzak-panel`'in içinde. Canlı cihaz ve merkez `uzak-panel` HEAD'i (e011902 ve öncesi) çalıştırıyor.

Bitenler (canlıda):
- ISO (Debian netinst + preseed + .deb), GitHub'da QEMU duman testi (`iso.yml`), `release.yml` yalnızca `v*` etiketinde.
- Panel menüsü: Özet, Cihazlar, Kayıtlar, Ayarlar, Admin ayarları (bölümler tek sayfada alt alta); "Bağlı" yeşil rozeti;
  Kayıtlar → Site / IP arama.
- Parolalar: müşteri parolasını yönetici yazar; **tek admin parolası** (Yönetim → Admin parolası, yalnızca özeti saklanır,
  bütün cihazlara eşitlemede gider).
- **Uzak panel** (spec `docs/superpowers/specs/2026-10-08-uzak-panel-design.md`, plan `docs/superpowers/plans/2026-10-09-uzak-panel.md`):
  giriş yalnızca panel.wificorrect.com'dan (numara + parola) → cihaz paneli `cihaz.wificorrect.com` (Caddy forward_auth +
  WireGuard). Cihaz kimlik başlığına yalnızca tünel ayaktayken ve merkez adresinden güvenir; bağlıyken yerel erişim boş 404;
  bağlı olmayan cihaz yalnızca eşleştirme ekranı. Admin ayarları + Panel hareketleri admin parolasıyla 30 dk açılır.
  Dışarıdan sahte başlık/çerez/belirteç denemeleri reddedildi (2026-10-10).

## Sıradaki işler (öncelik sırasıyla)
1. **Kullanıcının telefondan denemesi** (2026-10-10'da istendi, sonucu gelmedi): panel.wificorrect.com → 1537344 → cihaz
   paneli açılmalı → Admin ayarları admin parolası sorar → Çıkış panel girişine döner. Sorun varsa sistematik hata ayıkla.
2. **GitHub testleri**: e011902'nin `test`/`iso` çalışmaları `paket` adımında 125 (Docker, büyük olasılıkla geçici) ile düştü,
   yeniden başlatıldı: `gh run list -R avsharbeyi/wificorrect --branch uzak-panel`. Yeşil değilse sebebi bul.
3. **main'e birleştirme**: `uzak-panel` → `main` için PR aç (kullanıcı onayıyla), eski PR #2'yi kapat. Sonra **v0.3.0**:
   kullanıcı GitHub değişkeni `WFC_SSH_PUB`'a açık SSH anahtarını girmeli (yoksa sürüm derlemesi durur), etiket onayı.
4. Önerilen sonraki özellikler (kullanıcı listeden seçecek): donanım bekçisi (watchdog — J1900 donuyor), merkezden uzaktan
   güncelleme (apt deposu), merkezde sağlık izleme + uyarı, online ödeme (`veri.lisans_uzat`), personel kodu ile giriş,
   aylık rapor, hazır engel listeleri, hız sınırı, imzalı resmi talep raporu. Zaman damgası: kontör alınınca (merkezde).
   İki aşamalı giriş istenmedi.

## Açık küçük notlar
- AP (TP-Link TL-WA901ND, 10.50.0.2) menzili zayıf; kullanıcı sonra ilgilenecek (iletim gücü High, kanal 1/6/11, 20 MHz).
- Monitörsüz açılış ve J1900 donması için GRUB ayarı `deploy/debian/etc/default/grub.d/wificorrect.cfg` (cihazda uygulandı).
- Ertelenmiş küçük bulgular ve verilen bütün kararlar: `.superpowers/sdd/*/progress.md` (git'e girmez, yalnızca bu makinede).
