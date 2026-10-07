#!/bin/sh
# Kurucunun son adımı: WifiCorrect paketi (bağımlılıklar internetten) + hizmet sağlayıcının açık SSH anahtarı.
set -e
# Tanı: kurucu günlüğü (apt/dpkg çıktısı dahil) seri porta; QEMU duman testi kurulum.log'da görür. Arka planda, tüm
# tanımlayıcılar kapalı: seri port yoksa açılamaz, taşıyıcı beklerse yalnızca bu süreç bekler — kurulum etkilenmez.
( tail -n 100 -f /var/log/syslog </dev/null >/dev/ttyS0 2>/dev/null 3>&- 4>&- 5>&- 6>&- 7>&- 8>&- 9>&- & )
cp /cdrom/wificorrect/wificorrect_*_amd64.deb /target/tmp/
if [ -s /cdrom/wificorrect/authorized_keys ]; then
  mkdir -p /target/root/.ssh && chmod 700 /target/root/.ssh
  cp /cdrom/wificorrect/authorized_keys /target/root/.ssh/authorized_keys && chmod 600 /target/root/.ssh/authorized_keys
fi
# </dev/null: bir paket soru sorarsa (dpkg conffile vb.) sonsuza dek beklemek yerine hata verir
in-target sh -c 'DEBIAN_FRONTEND=noninteractive apt-get install -y -q /tmp/wificorrect_*_amd64.deb </dev/null && rm -f /tmp/wificorrect_*_amd64.deb'
# Bitti mesajı (bekletmez: 10 sn görünür, sonra kurucu kendi son adımlarını yapıp cihazı kapatır)
cat >/tmp/wificorrect-bitti.templates <<'T'
Template: wificorrect/bitti
Type: text
Description: Kurulum bitti. Cihaz birazdan kapanacak; kapandıktan sonra USB belleği çıkarın.
T
logger -t wificorrect "KURULUM BITTI"
if debconf-loadtemplate wificorrect /tmp/wificorrect-bitti.templates; then
  . /usr/share/debconf/confmodule
  db_progress INFO wificorrect/bitti || true
  sleep 10
fi
