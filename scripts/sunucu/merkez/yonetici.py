#!/usr/bin/env python3
"""Kullanım: wc-yonetici parola <kullanıcı-adı>
Yönetim merkezi (yonetim.wificorrect.com) yönetici hesabını açar ya da parolasını değiştirir (tek hesap).
Root çalıştırılır; dosyaya dokunmadan önce wcpanel'e iner (klasör onun: root olarak yazmak sembolik bağ izleyebilirdi)."""
import getpass
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))
import common  # noqa: E402
import guvenlik  # noqa: E402

YONETICI = "/var/lib/wificorrect/merkez/yonetici.json"
KUL_RE = re.compile(r"[a-z0-9._-]{3,32}")
EN_AZ = 12


def yaz(yol, kullanici, pw):
    if not KUL_RE.fullmatch(kullanici or ""):
        raise ValueError("Kullanıcı adı 3-32 karakter: küçük harf, rakam, . _ -")
    if not isinstance(pw, str) or len(pw) < EN_AZ:
        raise ValueError(f"Parola en az {EN_AZ} karakter olmalı.")
    tuz, oz, y = guvenlik.yeni_kayit(pw)
    common.save_json(yol, {"kullanici": kullanici, "tuz": tuz, "ozet": oz, "yineleme": y})
    os.chmod(yol, 0o600)


def oku(yol):
    try:
        with open(yol, encoding="utf-8") as f:
            v = json.load(f)
    except (FileNotFoundError, ValueError):
        return None
    return v if isinstance(v, dict) and {"kullanici", "tuz", "ozet", "yineleme"} <= set(v) else None


def sor():
    s1 = getpass.getpass("Parola: ")
    s2 = getpass.getpass("Parola (tekrar): ")
    return s1 if s1 == s2 else None


def birak(sahip):
    if hasattr(os, "geteuid") and os.geteuid() == 0:
        import pwd
        p = pwd.getpwnam(sahip)
        os.setgroups([])
        os.setgid(p.pw_gid)
        os.setuid(p.pw_uid)


def main(argv, yol=YONETICI, sor=sor, sahip="wcpanel", birak=birak):
    if len(argv) != 2 or argv[0] != "parola" or not KUL_RE.fullmatch(argv[1]):
        print(__doc__.splitlines()[0], file=sys.stderr)
        return 2
    if sahip:
        birak(sahip)
    pw = sor()
    if pw is None:
        print("Parolalar aynı değil.", file=sys.stderr)
        return 1
    try:
        yaz(yol, argv[1], pw)
    except ValueError as h:
        print(h, file=sys.stderr)
        return 1
    print("tamam")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
