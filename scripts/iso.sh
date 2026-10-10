#!/bin/sh
# Debian 13 netinst (imzası doğrulanır) + preseed + .deb → kurulum ISO'su. Kullanım: scripts/iso.sh <deb> <sürüm> <çıktı>
# Ortam: WFC_SSH_PUB (opsiyonel) → kurulan sistemde /root/.ssh/authorized_keys.
set -eu
deb=$(realpath "$1") surum=$2 cikti=$(realpath -m "$3")
cd "$(dirname "$0")/.."
u=https://cdimage.debian.org/debian-cd/current/amd64/iso-cd
d=$(mktemp -d)
curl -fsSL "$u/SHA256SUMS" -o "$d/SHA256SUMS"
curl -fsSL "$u/SHA256SUMS.sign" -o "$d/SHA256SUMS.sign"
gpgv --keyring /usr/share/keyrings/debian-role-keys.gpg "$d/SHA256SUMS.sign" "$d/SHA256SUMS"
ad=$(grep -o 'debian-13[^ ]*-amd64-netinst\.iso' "$d/SHA256SUMS" | head -1)
curl -fsSL "$u/$ad" -o "$d/$ad"
(cd "$d" && grep " $ad\$" SHA256SUMS | sha256sum -c -)
mkdir -p "$d/ek/wificorrect" "$cikti"
cp "$deb" iso/son.sh iso/disk.sh "$d/ek/wificorrect/"
if [ -n "${WFC_SSH_PUB:-}" ]; then printf '%s\n' "$WFC_SSH_PUB" > "$d/ek/wificorrect/authorized_keys"; fi
rm -f "$cikti/wificorrect-kurulum-$surum.iso"
xorriso -indev "$d/$ad" -outdev "$cikti/wificorrect-kurulum-$surum.iso" \
  -map iso/preseed.cfg /preseed.cfg -map "$d/ek/wificorrect" /wificorrect \
  -map iso/grub.cfg /boot/grub/grub.cfg -map iso/isolinux.cfg /isolinux/isolinux.cfg \
  -boot_image any replay
rm -rf "$d"
