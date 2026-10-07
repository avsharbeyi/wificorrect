#!/bin/sh
# partman/early_command: kurulacak disk = kurulum ortamı ve USB'ye bağlı diskler dışındaki ilk disk.
# Aday yoksa kurulum açık bir hatayla KALICI olarak durur: preseed_command başarısız komuttan sonra devam ettiğinden
# "exit 1" yetmez, partman kendi seçimini yapıp (ör. tek USB belleği) silerdi. Seçim seri porta da yazılır (duman testi).
seri() {
  logger -t wificorrect "$*"
  # Arka planda, tanımlayıcılar kapalı: seri port taşıyıcı beklerse yalnızca bu süreç bekler
  ( echo "WIFICORRECT: $*" </dev/null >/dev/ttyS0 2>/dev/null 3>&- 4>&- 5>&- 6>&- 7>&- 8>&- 9>&- & )
}
ortam=$(mount | awk '$3=="/cdrom"{print $1}')
ortam=${ortam##*/}
for d in $(list-devices disk); do
  n=${d#/dev/}
  # kurulum ortamı: diskin kendisi ya da bir bölümü (/sys/block/sdb/sdb1)
  if [ -n "$ortam" ] && { [ "$n" = "$ortam" ] || [ -e "/sys/block/$n/$ortam" ]; }; then continue; fi
  case $(readlink -f "/sys/block/$n") in */usb*) continue ;; esac
  debconf-set partman-auto/disk "$d"
  debconf-set grub-installer/bootdev "$d"
  seri "KURULUM DISKI: $d"
  exit 0
done
seri "KURULACAK DISK YOK"
cat >/tmp/wificorrect.templates <<'EOF'
Template: wificorrect/disk-yok
Type: error
Description: Kurulacak disk bulunamadı
 Kurulum belleği ve USB'ye takılı diskler dışında disk yok. Hiçbir disk silinmedi.
 .
 Cihazı kapatın, kendi diskini (SSD) kontrol edip yeniden deneyin.
EOF
debconf-loadtemplate wificorrect /tmp/wificorrect.templates || true
. /usr/share/debconf/confmodule
while :; do
  db_fset wificorrect/disk-yok seen false || true
  db_input critical wificorrect/disk-yok || true
  db_go || true
  sleep 1
done
