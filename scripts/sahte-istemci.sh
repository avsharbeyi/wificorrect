#!/usr/bin/env bash
# Müşteri ağında sahte istemci: geçici netns + veth köprüye, DHCP'den IP alır, verilen komutu onun içinde çalıştırır, temizler.
# DİKKAT: netns süreçleri ayırmaz — `ip netns exec … pkill dhcpcd` cihazın KENDİ WAN DHCP istemcisini de öldürür
# (2026-10-03'te iki kez WAN IP'si kira sonunda düştü). dhcpcd -1 kira alınca zaten kendiliğinden çıkar.
# Kullanım: scripts/sahte-istemci.sh 'curl -s http://10.50.0.1:8080/' [ssh-hedefi]
set -euo pipefail
komut=${1:?komut gerekli}
host=${2:-wificorrect}
ssh "$host" "set -e; n=sahte\$\$; ip netns add \$n; ip link add v\$n type veth peer name i\$n; ip link set i\$n netns \$n
ip link set v\$n master br-hotspot up; ip netns exec \$n ip link set lo up; ip netns exec \$n ip link set i\$n up
trap 'ip link del v\$n 2>/dev/null; ip netns del \$n 2>/dev/null' EXIT
timeout 30 ip netns exec \$n dhcpcd -1 -4 -B -C resolv.conf -C hostname i\$n >/dev/null 2>&1 || true
ip netns exec \$n sh -c '$komut'"
