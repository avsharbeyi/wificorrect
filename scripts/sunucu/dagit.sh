#!/usr/bin/env bash
# Sunucu tarafını (wificorrect-sunucu + yönetim merkezi) merkez sunucuya kurar. sudo parolasını terminalde siz girersiniz.
# Kullanım: scripts/sunucu/dagit.sh [gbserver@192.168.1.109]
set -euo pipefail
host=${1:-gbserver@192.168.1.109}
cd "$(dirname "$0")"
tar --exclude=__pycache__ -cf - wificorrect-sunucu merkez \
  | ssh -i ~/.ssh/gbserver "$host" 'rm -rf /tmp/wfc-sunucu && mkdir /tmp/wfc-sunucu && tar -C /tmp/wfc-sunucu -xf -'
ssh -t -i ~/.ssh/gbserver "$host" 'sudo install -m 755 /tmp/wfc-sunucu/wificorrect-sunucu /usr/local/sbin/ && sudo bash /tmp/wfc-sunucu/merkez/kur.sh'
