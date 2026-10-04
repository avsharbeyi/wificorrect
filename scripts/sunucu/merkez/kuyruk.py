#!/usr/bin/env python3
"""Root işleri kuyruğu (spec §2). Panel (wcpanel) iş dosyası yazar ve sonucu bekler; wc-kuyruk (root, systemd .path ile
anında) işi doğrulayıp wificorrect-sunucu ile yapar, sonucu yalnızca root'un yazabildiği klasöre koyar.
Panelden gelen hiçbir metin kabuğa verilmez: alanlar düzenli ifadeyle doğrulanır, komutlar liste argümanla çalışır."""
import json
import os
import re
import secrets
import shutil
import stat
import subprocess
import sys
import time

KUYRUK = "/var/lib/wificorrect/kuyruk"
SONUC = "/var/lib/wificorrect/kuyruk-sonuc"
SUNUCU = "/usr/local/sbin/wificorrect-sunucu"
KAYIT = "/etc/wireguard/wificorrect/kayit.csv"
WG_DIR = "/etc/wireguard/wificorrect"
NUMARA_RE = re.compile(r"[1-9][0-9]{5}")  # yalnızca ASCII rakam
WG_RE = re.compile(r"[A-Za-z0-9+/]{43}=")
SSH_RE = re.compile(r"ssh-ed25519 [A-Za-z0-9+/]+=*( [A-Za-z0-9@._-]+)?")
IS_RE = re.compile(r"[0-9a-f]{16}\.json")


# --- panel tarafı (wcpanel) ---
def ekle(islem, veri, kuyruk=KUYRUK):
    kimlik = secrets.token_hex(8)
    gecici = os.path.join(kuyruk, f".{kimlik}.tmp")  # .path birimi yalnızca *.json görür
    with open(gecici, "w", encoding="utf-8") as f:
        json.dump(dict(veri, islem=islem), f)
    os.replace(gecici, os.path.join(kuyruk, kimlik + ".json"))
    return kimlik


def sonuc(kimlik, sonuc_dizini=SONUC):
    try:
        with open(os.path.join(sonuc_dizini, kimlik + ".json"), encoding="utf-8") as f:
            v = json.load(f)
    except (FileNotFoundError, ValueError):
        return None
    return v if isinstance(v, dict) else None


def bekle(kimlik, sure=15.0, aralik=0.5, sonuc_dizini=SONUC, uyku=time.sleep):
    for _ in range(int(sure / aralik) + 1):
        s = sonuc(kimlik, sonuc_dizini)
        if s is not None:
            return s
        uyku(aralik)
    return None


# --- root tarafı (wc-kuyruk) ---
def is_oku(yol):
    """İş dosyası güvenilmez (wcpanel'in klasöründe): sembolik bağ izlenmez, FIFO'da beklenmez, en çok 4 KB."""
    try:
        fd = os.open(yol, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0))
    except OSError:
        return None
    try:
        st = os.fstat(fd)
        if not stat.S_ISREG(st.st_mode) or st.st_size > 4096:
            return None
        ham = os.read(fd, 4097)
    finally:
        os.close(fd)
    try:
        v = json.loads(ham.decode("utf-8"))
    except ValueError:
        return None
    return v if isinstance(v, dict) else None


def kayit_satiri(ad, kayit=KAYIT):
    """kayit.csv'deki aktif cihaz satırı: [tur, ad, ip, wg, yedek-kullanıcısı] ya da None."""
    try:
        with open(kayit, encoding="utf-8") as f:
            for s in f.read().splitlines():
                p = s.split(";")
                if len(p) >= 5 and p[0] == "cihaz" and p[1] == ad:
                    return p
    except FileNotFoundError:
        pass
    return None


def _oku(yol):
    with open(yol, encoding="utf-8") as f:
        return f.read().strip()


def _hata(metin):
    return {"durum": "hata", "hata": (metin or "bilinmeyen hata").strip()[:300]}


def isle(is_, calistir, kayit=KAYIT, wg_dir=WG_DIR):
    numara = is_.get("numara")
    if not isinstance(numara, str) or not NUMARA_RE.fullmatch(numara):
        return _hata("gecersiz numara")
    if is_.get("islem") == "cihaz-kapat":
        wg = is_.get("wg_pub")
        if wg is not None and not (isinstance(wg, str) and WG_RE.fullmatch(wg)):
            return _hata("gecersiz anahtar")
        s = kayit_satiri(numara, kayit)
        if s and (wg is None or s[3] == wg):  # eski bir kapatma işi, numaraya sonradan bağlanan cihazı kapatmasın
            kod, _, hata = calistir([SUNUCU, "cihaz-kapat", numara])
            if kod != 0:
                return _hata(hata)
        return {"durum": "tamam"}
    if is_.get("islem") != "cihaz-ekle":
        return _hata("bilinmeyen islem")
    wg, ssh = is_.get("wg_pub"), is_.get("ssh_pub")
    if not (isinstance(wg, str) and WG_RE.fullmatch(wg) and isinstance(ssh, str) and SSH_RE.fullmatch(ssh)):
        return _hata("gecersiz anahtar")
    s = kayit_satiri(numara, kayit)
    if s and s[3] != wg:  # müşterinin eski cihazı hâlâ tünelde: önce kapat
        kod, _, hata = calistir([SUNUCU, "cihaz-kapat", numara])
        if kod != 0:
            return _hata(hata)
        s = None
    if s is None:
        kod, _, hata = calistir([SUNUCU, "cihaz-ekle", numara, wg, ssh])
        s = kayit_satiri(numara, kayit)
        if kod != 0 or s is None:
            return _hata(hata or "cihaz eklenemedi")
    return {"durum": "tamam", "tunel_ip": s[2], "sunucu_pub": _oku(os.path.join(wg_dir, "sunucu.pub")),
            "uc_nokta": _oku(os.path.join(wg_dir, "uc-nokta"))}


def _calistir(komut):
    try:
        r = subprocess.run(komut, capture_output=True, text=True, timeout=120)
        return r.returncode, r.stdout, r.stderr
    except (OSError, subprocess.SubprocessError) as e:
        return 1, "", repr(e)


def main(kuyruk=KUYRUK, sonuc_dizini=SONUC, calistir=_calistir, kayit=KAYIT, wg_dir=WG_DIR, grup="wcpanel", simdi=None):
    simdi = time.time() if simdi is None else simdi
    def yazilis(ad):
        try:
            return os.lstat(os.path.join(kuyruk, ad)).st_mtime, ad
        except OSError:
            return 0, ad

    for ad in sorted(os.listdir(kuyruk), key=yazilis):  # yazılış sırası: kapat → ekle sırası bozulmasın
        yol = os.path.join(kuyruk, ad)
        if not ad.endswith(".json"):
            continue
        if not IS_RE.fullmatch(ad):  # .path birimi her *.json'da tetiklenir: tanınmayanı sil, döngü olmasın
            os.unlink(yol)
            continue
        is_ = is_oku(yol)
        s = isle(is_, calistir, kayit, wg_dir) if is_ is not None else _hata("is okunamadi")
        hedef = os.path.join(sonuc_dizini, ad)
        with open(hedef + ".tmp", "w", encoding="utf-8") as f:
            json.dump(s, f)
        os.chmod(hedef + ".tmp", 0o640)
        if grup:
            shutil.chown(hedef + ".tmp", group=grup)
        os.replace(hedef + ".tmp", hedef)
        os.unlink(yol)
        print(f"{ad}: {(is_ or {}).get('islem', '?')} {s['durum']} {s.get('hata', '')}", file=sys.stderr)
    for ad in os.listdir(sonuc_dizini):  # ponytail: okunmuş/okunmamış ayrılmaz; 1 günden eski sonuçlar silinir
        y = os.path.join(sonuc_dizini, ad)
        if os.path.getmtime(y) < simdi - 86400:
            os.unlink(y)
    return 0


if __name__ == "__main__":
    sys.exit(main())
