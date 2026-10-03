#!/bin/sh
# WifiCorrect uzak erişim sunucusu (WireGuard). Debian/Ubuntu, root olarak.
#   wg-sunucu.sh kur <genel-adres>          sunucuyu kurar (genel-adres: alan adı ya da IP, cihazlar buna bağlanır)
#   wg-sunucu.sh yonetici-ekle <ad>         yönetici bilgisayarı için bağlantı dosyası üretir (10.99.0.2–9)
#   wg-sunucu.sh cihaz-ekle <ad> <anahtar>  cihazı tanıtır (10.99.0.10–254); cihazın Admin ayarları'na girilecekleri yazar
#   wg-sunucu.sh liste                      tanıtılmış cihazlar/yöneticiler ve son el sıkışma
# Kural: cihazlar birbirine ulaşamaz; yalnızca yöneticiler cihazlara (panel 8443, SSH 22) ulaşır. İnternet trafiği taşınmaz.
set -eu
CONF=/etc/wireguard/wg0.conf
DIR=/etc/wireguard/wificorrect
PORT=51820
NET=10.99.0

kullanim() { sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }
ad_kontrol() { printf '%s' "$1" | grep -Eq '^[A-Za-z0-9._-]{1,40}$' || { echo "ad yalnızca harf, rakam, . _ - (1-40)" >&2; exit 2; }; }
bos_adres() { # $1=ilk $2=son
	i=$1
	while [ "$i" -le "$2" ]; do
		grep -q "AllowedIPs = $NET.$i/32" "$CONF" || { echo "$NET.$i"; return; }
		i=$((i + 1))
	done
	echo "adres kalmadı" >&2; exit 1
}
# Dosyadaki eşleri çalışan tünele uygular (bağlı olanlar kopmaz)
uygula() { wg-quick strip wg0 > "$DIR/.strip"; wg syncconf wg0 "$DIR/.strip"; rm -f "$DIR/.strip"; }

case "${1:-}" in
kur)
	[ $# -eq 2 ] || kullanim
	[ -f "$CONF" ] && { echo "$CONF zaten var" >&2; exit 1; }
	apt-get install -y --no-install-recommends wireguard-tools nftables >/dev/null
	umask 077
	mkdir -p "$DIR"
	wg genkey > "$DIR/sunucu.key"
	wg pubkey < "$DIR/sunucu.key" > "$DIR/sunucu.pub"
	echo "$2:$PORT" > "$DIR/uc-nokta"
	cat > "$CONF" <<EOF
# WifiCorrect uzak erişim sunucusu (wg-sunucu.sh). Eşler aşağıya eklenir.
[Interface]
Address = $NET.1/24
ListenPort = $PORT
PrivateKey = $(cat "$DIR/sunucu.key")
PostUp = sysctl -qw net.ipv4.ip_forward=1; nft -f $DIR/yonlendirme.nft
PostDown = nft delete table inet wfc_vpn 2>/dev/null || true
EOF
	cat > "$DIR/yonlendirme.nft" <<EOF
table inet wfc_vpn
delete table inet wfc_vpn
table inet wfc_vpn {
	chain forward {
		type filter hook forward priority filter - 5; policy accept;
		# yönetici (.2–.9) → cihaz: yalnızca panel ve SSH; cevaplar geçer; cihaz ↔ cihaz ve tünelden dışarı yok
		ct state established,related accept
		iifname "wg0" oifname "wg0" ip saddr $NET.2-$NET.9 tcp dport { 22, 8443 } accept
		iifname "wg0" oifname "wg0" ip saddr $NET.2-$NET.9 icmp type echo-request accept
		iifname "wg0" drop
	}
}
EOF
	systemctl enable --now wg-quick@wg0
	echo "Kuruldu. Güvenlik duvarında UDP $PORT açık olmalı."
	echo "Sunucu: $2:$PORT   Açık anahtar: $(cat "$DIR/sunucu.pub")"
	;;
yonetici-ekle)
	[ $# -eq 2 ] || kullanim
	ad_kontrol "$2"
	ip=$(bos_adres 2 9)
	umask 077
	key=$(wg genkey); pub=$(printf '%s' "$key" | wg pubkey)
	printf '\n# yonetici %s\n[Peer]\nPublicKey = %s\nAllowedIPs = %s/32\n' "$2" "$pub" "$ip" >> "$CONF"
	uygula
	out="$DIR/yonetici-$2.conf"
	cat > "$out" <<EOF
# WifiCorrect yönetici bağlantısı: $2 ($ip). Windows/telefon WireGuard uygulamasına "dosyadan içe aktar".
[Interface]
PrivateKey = $key
Address = $ip/32

[Peer]
PublicKey = $(cat "$DIR/sunucu.pub")
Endpoint = $(cat "$DIR/uc-nokta")
AllowedIPs = $NET.0/24
PersistentKeepalive = 25
EOF
	echo "Yönetici eklendi: $2 = $ip"
	echo "Bağlantı dosyası (gizli anahtar içerir, güvenli taşıyın): $out"
	;;
cihaz-ekle)
	[ $# -eq 3 ] || kullanim
	ad_kontrol "$2"
	printf '%s' "$3" | grep -Eq '^[A-Za-z0-9+/]{43}=$' || { echo "anahtar geçersiz (44 karakter, = ile biter)" >&2; exit 2; }
	grep -q "PublicKey = $3" "$CONF" && { echo "bu anahtar zaten ekli" >&2; exit 1; }
	ip=$(bos_adres 10 254)
	printf '\n# cihaz %s\n[Peer]\nPublicKey = %s\nAllowedIPs = %s/32\n' "$2" "$3" "$ip" >> "$CONF"
	uygula
	echo "Cihaz eklendi: $2 = $ip"
	echo "Cihazın panelinde Admin ayarları → Uzak erişim:"
	echo "  Sunucu:                 $(cat "$DIR/uc-nokta")"
	echo "  Sunucunun açık anahtarı: $(cat "$DIR/sunucu.pub")"
	echo "  Bu cihazın tünel adresi: $ip"
	echo "Bağlandıktan sonra panel: https://$ip:8443   SSH: ssh root@$ip"
	;;
liste)
	now=$(date +%s)
	awk '/^# (cihaz|yonetici) /{tur=$2; ad=$3} /^PublicKey/{pub=$3} /^AllowedIPs/{print tur, ad, $3, pub}' "$CONF" |
	while read -r tur ad ip pub; do
		t=$(wg show wg0 latest-handshakes | awk -v p="$pub" '$1==p{print $2}')
		if [ -z "$t" ] || [ "$t" = 0 ]; then d="hiç bağlanmadı"; else d="$(( (now - t) / 60 )) dk önce"; fi
		printf '%-9s %-28s %-16s %s\n' "$tur" "$ad" "$ip" "$d"
	done
	;;
*) kullanim ;;
esac
