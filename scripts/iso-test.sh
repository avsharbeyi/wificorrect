#!/bin/sh
# ISO'yu QEMU'da (UEFI) boş diske kurar, diskten açar, seri konsolda panel servisini ve yönetim adresini görür.
# KVM yoksa atlar (rapora yazılır). Başarısızlıkta seri günlükler + ekran görüntüsü ./iso-test-gunluk/ altına.
# Kullanım: scripts/iso-test.sh dist/wificorrect-kurulum-X.iso
set -eu
iso=$1 d=$(mktemp -d)
[ -w /dev/kvm ] || { echo "ISO DUMAN TESTI ATLANDI: KVM yok"; exit 0; }
qemu-img create -f qcow2 "$d/disk.qcow2" 8G >/dev/null
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$d/vars.fd"
set -- -machine q35 -enable-kvm -m 2048 -smp 2 -display none \
  -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd \
  -drive if=pflash,format=raw,file="$d/vars.fd" -drive file="$d/disk.qcow2",if=virtio \
  -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
  -netdev user,id=n1,restrict=on -device virtio-net-pci,netdev=n1 \
  -monitor unix:"$d/mon",server,nowait
mon() { python3 -c 'import socket,sys,time
s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); s.sendall(sys.argv[2].encode()+b"\n"); time.sleep(2)' "$d/mon" "$1" || true; }
ekran() { mon "screendump -f png $d/ekran.png"; }
hata() {
  ekran; kill "$q" 2>/dev/null || true
  for f in "$d"/*.log; do [ -e "$f" ] && { echo "--- $(basename "$f") (son 60 satır)"; tail -60 "$f" | tr -d '\033'; }; done
  mkdir -p iso-test-gunluk && cp "$d"/*.log "$d"/*.png iso-test-gunluk/ 2>/dev/null || true
  echo "ISO DUMAN TESTI BASARISIZ: $1"; exit 1
}
# 1) kurulum. Menü kendiliğinden başlamamalı: menü görününce 20 sn bekle, çekirdek açılmadıysa Enter.
qemu-system-x86_64 "$@" -no-reboot -cdrom "$iso" -serial file:"$d/kurulum.log" &
q=$!
n=0
until grep -aq "WifiCorrect kur" "$d/kurulum.log" 2>/dev/null; do
  sleep 2; n=$((n + 2)); [ $n -lt 180 ] || hata "kurulum menüsü seri konsolda görünmedi"
done
sleep 20
! grep -aq "Linux version" "$d/kurulum.log" || hata "menü kendiliğinden başladı"
mon "sendkey ret"
# Kurucu bitince makineyi kapatır (debian-installer/exit/poweroff).
n=0
while kill -0 "$q" 2>/dev/null; do
  sleep 30; n=$((n + 30)); ekran
  [ $n -lt 2400 ] || hata "kurulum 40 dk içinde bitmedi (son ekran: ekran.png)"
done
grep -aq "Linux version" "$d/kurulum.log" || hata "kurucu çekirdeği hiç açılmadı"
# 2) diskten açılış: systemd durum satırı + getty'nin yazdığı yönetim adresi (seri konsol ttyS0).
rm -f "$d/mon"
qemu-system-x86_64 "$@" -serial file:"$d/acilis.log" &
q=$!
n=0
until grep -aq "Started.*WifiCorrect yonetim paneli" "$d/acilis.log" && grep -aq "WifiCorrect yonetim adresi" "$d/acilis.log"; do
  sleep 5; n=$((n + 5)); [ $n -lt 300 ] || { grep -a "FAILED" "$d/acilis.log" | tr -d '\033' || true; hata "açılışta panel/yönetim adresi görünmedi"; }
done
grep -a "WifiCorrect yonetim adresi" "$d/acilis.log" | tail -1 | tr -d '\033'
kill "$q" 2>/dev/null || true
rm -rf "$d"
echo "ISO DUMAN TESTI GECTI"
