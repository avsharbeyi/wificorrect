#!/usr/bin/env bash
# Geliştirme döngüsü: kaynağı cihaza gönder, cihazda derle, /usr/local/bin'e kur.
# Yalnızca geliştirme dönemi içindir; ürün paketi (.deb) GitHub Actions'ta derlenecek, kalıpta derleyici olmayacak.
# Kullanım: scripts/gelistir.sh [ssh-hedefi]   (varsayılan: wificorrect)
set -euo pipefail
host=${1:-wificorrect}
cd "$(dirname "$0")/.."
files=(Cargo.toml .cargo src tests)
[ -f Cargo.lock ] && files+=(Cargo.lock)
tar -cf - "${files[@]}" | ssh "$host" 'set -e; mkdir -p /root/rza; rm -rf /root/rza/src; tar -xf - -C /root/rza
cd /root/rza
# Bellek sınırlı (cihaz 2 GB, takas yok): aşılırsa yalnızca derleme durur, sistem kilitlenmez (2026-10-03)
if ! systemd-run --scope -q -p MemoryMax=1200M nice -n 10 cargo build --release -q -j 2; then echo "DERLEME BAŞARISIZ" >&2; exit 1; fi
install -m 755 target/release/wificorrect /usr/local/bin/wificorrect
echo "kuruldu: $(wificorrect surum), $(stat -c %s /usr/local/bin/wificorrect) bayt"'
# Cargo.lock cihazda üretilir; repoda tutulsun diye geri al
ssh "$host" 'cat /root/rza/Cargo.lock' > Cargo.lock
