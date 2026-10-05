#!/bin/sh
# Kurucunun son adımı: WifiCorrect paketi (bağımlılıklar internetten) + hizmet sağlayıcının açık SSH anahtarı.
set -e
cp /cdrom/wificorrect/wificorrect_*_amd64.deb /target/tmp/
if [ -s /cdrom/wificorrect/authorized_keys ]; then
  mkdir -p /target/root/.ssh && chmod 700 /target/root/.ssh
  cp /cdrom/wificorrect/authorized_keys /target/root/.ssh/authorized_keys && chmod 600 /target/root/.ssh/authorized_keys
fi
in-target sh -c 'DEBIAN_FRONTEND=noninteractive apt-get install -y -q /tmp/wificorrect_*_amd64.deb && rm -f /tmp/wificorrect_*_amd64.deb'
