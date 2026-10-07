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
# HMP: komutu gönder, yanıtı (sonraki "(qemu)" istemine kadar) mon.log'a ekle
mon() { python3 -c 'import socket,sys,time
s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); s.settimeout(15); s.sendall(sys.argv[2].encode()+b"\n")
b=b""; t=time.time()
while b.count(b"(qemu)") < 2 and time.time()-t < 15:
    try: b+=s.recv(4096) or b"(qemu)(qemu)"
    except socket.timeout: break
sys.stdout.write(b.decode(errors="replace"))' "$d/mon" "$1" >>"$d/mon.log" 2>&1 || true; }
ekran() { mon "screendump $d/ekran.ppm"; }
# P6 PPM → PNG (yalnızca stdlib; çalıştırıcıda ImageMagick olmayabilir)
png() { python3 -c 'import sys,zlib,struct
b=open(sys.argv[1],"rb").read(); m,wh,mx,r=b.split(b"\n",3); w,h=map(int,wh.split())
c=lambda t,x: struct.pack(">I",len(x))+t+x+struct.pack(">I",zlib.crc32(t+x))
raw=b"".join(b"\0"+r[y*w*3:(y+1)*w*3] for y in range(h))
open(sys.argv[2],"wb").write(b"\x89PNG\r\n\x1a\n"+c(b"IHDR",struct.pack(">IIBBBBB",w,h,8,2,0,0,0))+c(b"IDAT",zlib.compress(raw))+c(b"IEND",b""))' "$1" "$2" || cp "$1" iso-test-gunluk/; }
hata() {
  ekran; kill "$q" 2>/dev/null || true
  for f in "$d"/kurulum.log "$d"/acilis.log; do [ -e "$f" ] && { echo "--- $(basename "$f") (son 60 satır)"; tail -60 "$f" | tr -d '\033'; }; done
  mkdir -p iso-test-gunluk && cp "$d"/*.log iso-test-gunluk/ 2>/dev/null || true
  [ -e "$d/ekran.ppm" ] && png "$d/ekran.ppm" iso-test-gunluk/ekran.png
  echo "ISO DUMAN TESTI BASARISIZ: $1"; exit 1
}
# 1) kurulum. Menü (GRUB, OVMF seri konsola da yazar) kendiliğinden başlamamalı: 20 sn seri çıktı değişmemeli, sonra Enter.
# Kurucu monitörde (tty) çalışır, seri konsolda görünmez → ilerleme/hata için 30 sn'de bir ekran görüntüsü.
qemu-system-x86_64 "$@" -no-reboot -cdrom "$iso" -serial file:"$d/kurulum.log" &
q=$!
n=0
until grep -aq "WifiCorrect kur" "$d/kurulum.log" 2>/dev/null; do
  sleep 2; n=$((n + 2)); [ $n -lt 180 ] || hata "kurulum menüsü seri konsolda görünmedi"
done
sleep 3; once=$(wc -c <"$d/kurulum.log"); sleep 20
[ "$(wc -c <"$d/kurulum.log")" = "$once" ] || hata "menü kendiliğinden başladı (seri çıktı değişti)"
mon "sendkey ret"
# Kurucu bitince makineyi kapatır (debian-installer/exit/poweroff).
# Ekran 10 dk hiç değişmediyse kurucu bir soruda/hatada takılmıştır → 40 dk beklemeden dur.
n=0 ayni=0 once=
while kill -0 "$q" 2>/dev/null; do
  sleep 30; n=$((n + 30)); ekran
  simdi=$(cksum <"$d/ekran.ppm" 2>/dev/null || true)
  if [ -n "$simdi" ] && [ "$simdi" = "$once" ]; then ayni=$((ayni + 1)); else ayni=0; fi
  once=$simdi
  [ $ayni -lt 20 ] || hata "kurucu ekranı 10 dk değişmedi (takıldı; son ekran: iso-test-gunluk/ekran.png)"
  [ $n -lt 2400 ] || hata "kurulum 40 dk içinde bitmedi (son ekran: iso-test-gunluk/ekran.png)"
done
# 2) diskten açılış: systemd durum satırı (açıklama konsol genişliğine kırpılır → birim adıyla aranır) + getty'nin
#    yazdığı yönetim adresi (seri konsol ttyS0).
rm -f "$d/mon"
qemu-system-x86_64 "$@" -serial file:"$d/acilis.log" &
q=$!
n=0
until grep -aq "Started.*wificorrect-panel.service" "$d/acilis.log" && grep -aq "WifiCorrect yonetim adresi: https://[0-9]" "$d/acilis.log"; do
  sleep 5; n=$((n + 5)); [ $n -lt 300 ] || { grep -a "FAILED" "$d/acilis.log" | tr -d '\033' || true; hata "açılışta panel/yönetim adresi görünmedi"; }
done
grep -a "WifiCorrect yonetim adresi" "$d/acilis.log" | tail -1 | tr -d '\033'
kill "$q" 2>/dev/null || true
mkdir -p iso-test-gunluk && cp "$d"/kurulum.log "$d"/acilis.log iso-test-gunluk/
rm -rf "$d"
echo "ISO DUMAN TESTI GECTI"
