#!/usr/bin/env python3
"""wc-istatistik: kafe paneli sayıları (spec §3).
Her kafenin eksiksiz gelmiş her günü için o gün internete çıkan farklı telefon sayısını
/var/lib/wificorrect/istatistik/<kafe>.json'a yazar. Root çalışır; panel yalnızca bu dosyaları okur."""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))
import common  # noqa: E402

ARSIV = "/srv/hotspot-arsiv"
CIKTI = "/var/lib/wificorrect/istatistik"
HESAPLAR = "/etc/wificorrect/hesaplar.json"
DETAY = "/var/lib/wificorrect/detay"
PANEL_GRUBU = "wcpanel"  # özetler root:wcpanel 640; testlerde None (sahiplik değişmez)
GUN_RE = re.compile(r"\d{4}-\d{2}-\d{2}")


def tamam_mi(gun_dizini):
    """MANIFEST var ve listelediği her dosya yerinde mi (rsync yarıdayken hayır)."""
    try:
        with open(os.path.join(gun_dizini, "MANIFEST.sha256"), encoding="utf-8") as f:
            satirlar = [s.split(None, 1) for s in f if s.strip() and not s.startswith("#")]
    except FileNotFoundError:
        return False
    return all(len(p) == 2 and os.path.exists(os.path.join(gun_dizini, p[1].strip())) for p in satirlar)


def normal_dosya(yol):
    """Yol yoksa None; varsa ve normal dosya değilse (cihazdan gelmiş sembolik bağ/FIFO) ValueError:
    başka kafeyi okutmasın, root görevini takmasın."""
    if not os.path.lexists(yol):
        return None
    if os.path.islink(yol) or not os.path.isfile(yol):
        raise ValueError(f"{os.path.basename(yol)} normal dosya değil")
    return yol


def satirlar(yol):
    yol = normal_dosya(yol)
    return common.read_rows(yol) if yol else iter(())


def farkli_kisi(gun_dizini):
    """Günün trafik.csv.gz'sindeki farklı, boş olmayan telefon sayısı (trafik dosyası yoksa 0)."""
    return len({r["telefon"] for r in satirlar(os.path.join(gun_dizini, "trafik.csv.gz")) if r.get("telefon")})


def guncelle(kafe_dizini, json_yolu):
    """Henüz hesaplanmamış tamam günleri ekler. Bozuk gün atlanır ve sonraki çalıştırmada yeniden denenir."""
    try:
        with open(json_yolu, encoding="utf-8") as f:
            sayilar = json.load(f)
        if not isinstance(sayilar, dict):
            sayilar = {}
    except (FileNotFoundError, ValueError):
        sayilar = {}
    g = os.path.join(kafe_dizini, "gunluk")
    gunler = sorted(d for d in os.listdir(g) if GUN_RE.fullmatch(d)) if os.path.isdir(g) else []
    hatalar = []
    for gun in gunler:
        d = os.path.join(g, gun)
        if gun in sayilar or os.path.islink(d) or not tamam_mi(d):
            continue
        try:
            sayilar[gun] = farkli_kisi(d)
        except Exception as e:  # bozuk/kesik gz (zlib.error, EOFError, OSError): yalnızca o gün düşer
            hatalar.append(f"{gun}: {e!r}")
    os.makedirs(os.path.dirname(json_yolu) or ".", exist_ok=True)
    common.save_json(json_yolu, dict(sorted(sayilar.items())))
    os.chmod(json_yolu, 0o644)
    return hatalar


def main():
    import panel_auth  # eski panel; Görev 4 bu main'i değiştirir
    kod = 0
    for h in panel_auth.Accounts(HESAPLAR).list():
        kafe = h["kullanici"]
        if not panel_auth.USER_RE.fullmatch(kafe):  # hesap dosyası wcpanel'in: root yolunu ondan kurmadan önce doğrula
            print(f"{kafe!r}: geçersiz hesap adı, atlandı", file=sys.stderr)
            kod = 1
            continue
        import ozet  # istatistik'i içe aktarır; döngüsel içe aktarma olmasın diye burada
        kafe_dizini, hatalar = os.path.join(ARSIV, kafe), []
        try:
            hatalar += guncelle(kafe_dizini, os.path.join(CIKTI, kafe + ".json"))
            hatalar += ozet.detay_guncelle(kafe_dizini, os.path.join(DETAY, kafe), PANEL_GRUBU)
        except Exception as e:  # bir kafenin bozuk klasörü diğerlerini durdurmasın
            hatalar.append(repr(e))
        for hata in hatalar:
            print(f"{kafe}: {hata}", file=sys.stderr)
            kod = 1
    return kod


if __name__ == "__main__":
    sys.exit(main())
