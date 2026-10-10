#!/bin/sh
# ISO'yu QEMU'da (UEFI, KVM) dener. Makinede önce sıralanan bir USB bellek (sda, 64 MB, bilinen içerik) hep takılı:
#  0) Yalnız USB bellekle: kurucu "KURULACAK DISK YOK" deyip durmalı (partman başlamamalı), USB değişmemeli.
#  1) USB + boş 8 GB virtio disk: menü kendiliğinden başlamamalı; kurulum /dev/vda'ya; "Kurulum bitti" mesajı; kapanır.
#  2) Diskten açılış: panel servisi + IP'li yönetim adresi (seri konsol); WFC_TEST_SSH_KEY (özel anahtar dosyası)
#     verildiyse o anahtarla root SSH. Her aşamadan sonra USB belleğin sha256'sı değişmemiş olmalı.
# KVM yoksa atlar (rapora yazılır). Seri günlükler + ekran görüntüleri ./iso-test-gunluk/ altına (her durumda).
# Kullanım: scripts/iso-test.sh dist/wificorrect-kurulum-X.iso
set -eu
iso=$1 d=$(mktemp -d) g=$PWD/iso-test-gunluk
[ -w /dev/kvm ] || { echo "ISO DUMAN TESTI ATLANDI: KVM yok"; exit 0; }
mkdir -p "$g"
qemu-img create -f qcow2 "$d/disk.qcow2" 8G >/dev/null
yes WIFICORRECT-USB-TEST | head -c 67108864 >"$d/usb.img"
usb_sha=$(sha256sum <"$d/usb.img")
# vm <seri-günlük> [ek argümanlar]: arka planda başlatır, $q = süreç
vm() {
  log=$1; shift
  rm -f "$d/mon"
  qemu-system-x86_64 -machine q35 -enable-kvm -m 2048 -smp 2 -display none \
    -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd \
    -drive if=pflash,format=raw,file="$d/vars.fd" \
    -device qemu-xhci,id=xhci -drive if=none,id=usb,format=raw,file="$d/usb.img" -device usb-storage,bus=xhci.0,drive=usb \
    -netdev user,id=n0,hostfwd=tcp:127.0.0.1:2222-:22 -device virtio-net-pci,netdev=n0 \
    -netdev user,id=n1,restrict=on,hostfwd=tcp:127.0.0.1:2223-:22 -device virtio-net-pci,netdev=n1 \
    -monitor unix:"$d/mon",server,nowait -serial file:"$d/$log" "$@" &
  q=$!
}
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
open(sys.argv[2],"wb").write(b"\x89PNG\r\n\x1a\n"+c(b"IHDR",struct.pack(">IIBBBBB",w,h,8,2,0,0,0))+c(b"IDAT",zlib.compress(raw))+c(b"IEND",b""))' "$1" "$2" || cp "$1" "$g/"; }
gunluk() { cp "$d"/*.log "$g/" 2>/dev/null || true; }
hata() {
  ekran; kill "$q" 2>/dev/null || true
  for f in "$d"/kurulum0.log "$d"/kurulum.log "$d"/acilis.log; do [ -e "$f" ] && { echo "--- $(basename "$f") (son 60 satır)"; tail -60 "$f" | tr -d '\033'; }; done
  gunluk; [ -e "$d/ekran.ppm" ] && png "$d/ekran.ppm" "$g/ekran.png"
  echo "ISO DUMAN TESTI BASARISIZ: $1"; exit 1
}
usb_ayni() { [ "$(sha256sum <"$d/usb.img")" = "$usb_sha" ] || hata "USB bellek değişti ($1)"; }
# Menü (GRUB, OVMF seri konsola da yazar) kendiliğinden başlamamalı: 20 sn seri çıktı değişmemeli, sonra Enter.
menu() {
  n=0
  until grep -aq "WifiCorrect kur" "$d/$1" 2>/dev/null; do
    sleep 2; n=$((n + 2)); [ $n -lt 180 ] || hata "kurulum menüsü seri konsolda görünmedi"
  done
  sleep 3; once=$(wc -c <"$d/$1"); sleep 20
  [ "$(wc -c <"$d/$1")" = "$once" ] || hata "menü kendiliğinden başladı (seri çıktı değişti)"
  mon "sendkey ret"
}

# 0) USB'den başka disk yok → kurucu disk.sh hatasında durur. Kurucu monitörde (tty) çalışır; disk.sh seriye yazar.
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$d/vars.fd"
vm kurulum0.log -cdrom "$iso"
menu kurulum0.log
n=0
until grep -aq "WIFICORRECT: KURULACAK DISK YOK" "$d/kurulum0.log"; do
  sleep 5; n=$((n + 5)); usb_ayni "yalnız USB varken"
  grep -aq "WIFICORRECT: KURULUM DISKI" "$d/kurulum0.log" && hata "yalnız USB varken disk seçildi"
  kill -0 "$q" 2>/dev/null || hata "yalnız USB varken kurucu kapandı"
  [ $n -lt 900 ] || hata "yalnız USB varken 'disk yok' hatası 15 dk içinde gelmedi"
done
sleep 60   # durduğunu gör: partman başlamış olsaydı bu sürede USB'ye yazardı
ekran; png "$d/ekran.ppm" "$g/ekran-disk-yok.png"
kill -0 "$q" 2>/dev/null || hata "'disk yok'tan sonra kurucu kapandı"
kill "$q"; wait "$q" 2>/dev/null || true
usb_ayni "yalnız USB varken"
echo "disk yok: kurucu durdu, USB değişmedi"

# 1) kurulum (USB + virtio). Kurucu bitince makineyi kapatır (debian-installer/exit/poweroff).
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$d/vars.fd"
vm kurulum.log -drive file="$d/disk.qcow2",if=virtio -no-reboot -cdrom "$iso"
menu kurulum.log
# Ekran 10 dk hiç değişmediyse kurucu bir soruda/hatada takılmıştır → 40 dk beklemeden dur.
n=0 ayni=0 once= bitti=
while kill -0 "$q" 2>/dev/null; do
  sleep 2; n=$((n + 2))
  if [ -z "$bitti" ] && grep -aq "KURULUM BITTI" "$d/kurulum.log"; then
    bitti=1; sleep 3; ekran; png "$d/ekran.ppm" "$g/ekran-bitti.png"
  fi
  [ $((n % 30)) = 0 ] || continue
  ekran
  simdi=$(cksum <"$d/ekran.ppm" 2>/dev/null || true)
  if [ -n "$simdi" ] && [ "$simdi" = "$once" ]; then ayni=$((ayni + 1)); else ayni=0; fi
  once=$simdi
  [ $ayni -lt 20 ] || hata "kurucu ekranı 10 dk değişmedi (takıldı; son ekran: iso-test-gunluk/ekran.png)"
  [ $n -lt 2400 ] || hata "kurulum 40 dk içinde bitmedi (son ekran: iso-test-gunluk/ekran.png)"
done
grep -a "WIFICORRECT: KURULUM DISKI" "$d/kurulum.log" | tr -d '\r'
grep -aq "WIFICORRECT: KURULUM DISKI: /dev/vda" "$d/kurulum.log" || hata "kurulum /dev/vda'ya gitmedi"
[ -n "$bitti" ] || hata "'Kurulum bitti' mesajı görünmedi"
usb_ayni "kurulumda"

# 2) diskten açılış: systemd durum satırı (açıklama konsol genişliğine kırpılır → birim adıyla aranır) + getty'nin
#    yazdığı yönetim adresi (seri konsol ttyS0).
vm acilis.log -drive file="$d/disk.qcow2",if=virtio
n=0
until grep -aq "Started.*wificorrect-panel.service" "$d/acilis.log" && grep -aq "WifiCorrect panel: https://panel.wificorrect.com (eslestirme: https://[0-9]" "$d/acilis.log"; do
  sleep 5; n=$((n + 5)); [ $n -lt 300 ] || { grep -a "FAILED" "$d/acilis.log" | tr -d '\033' || true; hata "açılışta panel/yönetim adresi görünmedi"; }
done
grep -a "WifiCorrect panel: https://panel.wificorrect.com" "$d/acilis.log" | tail -1 | tr -d '\033'
if [ -n "${WFC_TEST_SSH_KEY:-}" ]; then
  n=0
  until for p in 2222 2223; do
      ssh -q -i "$WFC_TEST_SSH_KEY" -p $p -o BatchMode=yes -o ConnectTimeout=5 -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null root@127.0.0.1 'test -s /root/.ssh/authorized_keys && echo "SSH ANAHTARI CALISIYOR (port '$p')"' && break
    done | grep -a "SSH ANAHTARI CALISIYOR"; do
    sleep 5; n=$((n + 5)); [ $n -lt 120 ] || hata "SSH anahtarıyla root girişi olmadı"
  done
fi
kill "$q" 2>/dev/null || true; wait "$q" 2>/dev/null || true
usb_ayni "açılışta"
gunluk; rm -rf "$d"
echo "ISO DUMAN TESTI GECTI"
