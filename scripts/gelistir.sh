#!/usr/bin/env bash
# Geliştirme döngüsü: kaynağı cihaza gönder, cihazda derle, /usr/local/bin'e kur.
# Yalnızca geliştirme dönemi içindir; ürün paketi (.deb) GitHub Actions'ta derlenecek, kalıpta derleyici olmayacak.
# Kullanım: scripts/gelistir.sh [ssh-hedefi]   (varsayılan: wificorrect)
set -euo pipefail
host=${1:-wificorrect}
cd "$(dirname "$0")/.."
files=(Cargo.toml .cargo src)
[ -f Cargo.lock ] && files+=(Cargo.lock)
tar -cf - "${files[@]}" | ssh "$host" 'set -e; mkdir -p /root/rza; rm -rf /root/rza/src; tar -xf - -C /root/rza
cd /root/rza; cargo build --release -q 2>&1 | grep -v "^$" || true
test -x target/release/wificorrect
install -m 755 target/release/wificorrect /usr/local/bin/wificorrect
echo "kuruldu: $(wificorrect surum), $(stat -c %s /usr/local/bin/wificorrect) bayt"'
# Cargo.lock cihazda üretilir; repoda tutulsun diye geri al
ssh "$host" 'cat /root/rza/Cargo.lock' > Cargo.lock
