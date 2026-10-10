#!/bin/bash
# Merkezi (yönetim merkezi + wificorrect-sunucu) şu anki daldan kurar: önce veritabanı yedeği, sonra kur.sh
# (Caddyfile doğrulanmadan kurulmaz). sudo parolasını kullanıcı terminalde girer; ajan girmez.
# Kullanım (Git Bash): scripts/sunucu/merkez-guncelle.sh [gbserver@192.168.1.109]
#   cmd'den: "C:\Program Files\Git\bin\bash.exe" scripts/sunucu/merkez-guncelle.sh
set -euo pipefail
host=${1:-gbserver@192.168.1.109}
cd "$(dirname "$0")/../.."
gecici=$(mktemp -d)
git archive HEAD scripts/sunucu | tar -x -C "$gecici"
tar --exclude=__pycache__ -cf - -C "$gecici/scripts/sunucu" wificorrect-sunucu merkez \
  | ssh -i ~/.ssh/gbserver "$host" 'rm -rf /tmp/wfc-sunucu && mkdir /tmp/wfc-sunucu && tar -C /tmp/wfc-sunucu -xf -'
rm -rf "$gecici"
ssh -i ~/.ssh/gbserver "$host" 'cat > /tmp/wfc-sunucu/son.sh' <<'EOF'
set -eu
db=/var/lib/wificorrect/merkez/merkez.db
yedek=$db.yedek-$(date +%Y%m%d-%H%M%S)
cp -p "$db" "$yedek"
echo "veritabanı yedeği: $yedek"
install -m 755 /tmp/wfc-sunucu/wificorrect-sunucu /usr/local/sbin/
bash /tmp/wfc-sunucu/merkez/kur.sh
sleep 3
systemctl is-active wc-merkez wc-kuyruk.path caddy
EOF
ssh -t -i ~/.ssh/gbserver "$host" 'sudo bash /tmp/wfc-sunucu/son.sh 2>&1 | tee /tmp/wfc-sunucu/sonuc.txt; sudo chmod 644 /tmp/wfc-sunucu/sonuc.txt'
