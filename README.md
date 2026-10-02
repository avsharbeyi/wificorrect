# WifiCorrect — rza

Kafeler için 5651 uyumlu, SMS (OTP) doğrulamalı misafir internet sistemi. Debian 13 üzerinde, uygulama katmanı Rust ile yazılır.

- Ne yapması gerektiği (davranış tarifi + istemler): [docs/RUST_YENIDEN_YAZIM.md](docs/RUST_YENIDEN_YAZIM.md)
- Ayrıntılı tasarım (eski OpenWrt/Python sistemi için yazıldı; yasal çerçeve, CSV şeması, kabul testleri K1–K26): [docs/MASTER_ENGINEERING.md](docs/MASTER_ENGINEERING.md)
- Cihazda yapılan kurulum adımları ve doğrulamaları: [docs/KURULUM_GUNLUGU.md](docs/KURULUM_GUNLUGU.md)

## Klasörler
- `deploy/debian/` — cihazdaki `/` ile birebir eşleşen sistem dosyaları (güvenlik duvarı, köprü, dnsmasq, güncellemeler, saat).
- Rust uygulaması (portal, kaydedici, panel, ctl) Adım 4'te eklenecek.

## Kurallar
- Sır (SMS API şifresi, anahtar, parola) asla repoya girmez.
- Ağ / güvenlik duvarı değişikliği yalnızca ölü adam anahtarıyla (5 dk içinde onaylanmazsa geri alınır) ve yeni SSH oturumuyla doğrulanarak yapılır.
- Kullanıcıya görünen metinler Türkçe, kod tanımlayıcıları İngilizce.
