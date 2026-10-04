import gzip
import json
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))
import istatistik as I  # noqa: E402

BASLIK = ("zaman;olay;telefon;ad;soyad;mac;ic_ip;protokol;ic_port;hedef_ip;hedef_port;nat_ip;nat_port;"
          "alan_adi;gonderilen_bayt;alinan_bayt;sure_sn;oturum_id;ek")


def gun_yaz(kok, gun, telefonlar, manifest=("trafik.csv.gz",), bozuk=False):
    d = os.path.join(kok, "gunluk", gun)
    os.makedirs(d)
    yol = os.path.join(d, "trafik.csv.gz")
    if bozuk:
        with open(yol, "wb") as f:
            f.write(b"\x1f\x8b\x08\x00bozuk-veri")
    else:
        with gzip.open(yol, "wt", encoding="utf-8", newline="") as f:
            f.write("﻿" + BASLIK + "\n")
            for t in telefonlar:
                f.write(";".join([f"{gun}T10:00:00+03:00", "BAGLANTI_BASLA", t] + [""] * 16) + "\n")
    if manifest is not None:
        with open(os.path.join(d, "MANIFEST.sha256"), "w", encoding="utf-8") as f:
            f.write(f"# gun={gun} onceki_zincir=0\n")
            for p in manifest:
                f.write(f"abc123  {p}\n")
    return d


def oku(yol):
    with open(yol, encoding="utf-8") as f:
        return json.load(f)


def test_farkli_kisi_bos_telefon_sayilmaz():
    with tempfile.TemporaryDirectory() as tmp:
        d = gun_yaz(tmp, "2026-09-27", ["5551112233", "5551112233", "5559998877", ""])
        assert I.farkli_kisi(d) == 2


def test_trafiksiz_tamam_gun_sifirdir():
    with tempfile.TemporaryDirectory() as tmp:
        d = gun_yaz(tmp, "2026-09-27", [], manifest=())
        os.remove(os.path.join(d, "trafik.csv.gz"))
        assert I.tamam_mi(d)
        assert I.farkli_kisi(d) == 0


def test_guncelle_artimli():
    with tempfile.TemporaryDirectory() as tmp:
        kafe, js = os.path.join(tmp, "bocafe"), os.path.join(tmp, "ist", "bocafe.json")
        gun_yaz(kafe, "2026-09-26", ["5551112233"])
        assert I.guncelle(kafe, js) == []
        assert oku(js) == {"2026-09-26": 1}
        with open(js, "w", encoding="utf-8") as f:
            json.dump({"2026-09-26": 99}, f)  # hesaplanmış gün yeniden hesaplanmaz
        gun_yaz(kafe, "2026-09-27", ["5551112233", "5559998877"])
        assert I.guncelle(kafe, js) == []
        assert oku(js) == {"2026-09-26": 99, "2026-09-27": 2}


def test_yarim_gun_atlanir():
    with tempfile.TemporaryDirectory() as tmp:
        kafe, js = os.path.join(tmp, "bocafe"), os.path.join(tmp, "bocafe.json")
        d = gun_yaz(kafe, "2026-09-27", ["5551112233"], manifest=("trafik.csv.gz", "kullanicilar/5551112233.csv.gz"))
        gun_yaz(kafe, "2026-09-28", ["5551112233"], manifest=None)  # MANIFEST henüz gelmemiş
        assert not I.tamam_mi(d)
        assert I.guncelle(kafe, js) == []
        assert oku(js) == {}
        os.makedirs(os.path.join(d, "kullanicilar"))
        open(os.path.join(d, "kullanicilar", "5551112233.csv.gz"), "wb").close()
        I.guncelle(kafe, js)
        assert oku(js) == {"2026-09-27": 1}


def test_bozuk_gz_gunu_atlar():
    with tempfile.TemporaryDirectory() as tmp:
        kafe, js = os.path.join(tmp, "bocafe"), os.path.join(tmp, "bocafe.json")
        gun_yaz(kafe, "2026-09-26", ["5551112233"], bozuk=True)
        gun_yaz(kafe, "2026-09-27", ["5551112233"])
        hatalar = I.guncelle(kafe, js)
        assert len(hatalar) == 1 and hatalar[0].startswith("2026-09-26")
        assert oku(js) == {"2026-09-27": 1}


def test_bozuk_json_yeniden_hesaplanir():
    with tempfile.TemporaryDirectory() as tmp:
        kafe, js = os.path.join(tmp, "bocafe"), os.path.join(tmp, "bocafe.json")
        gun_yaz(kafe, "2026-09-27", ["5551112233"])
        with open(js, "w", encoding="utf-8") as f:
            f.write('{"2026-09-27": ')
        assert I.guncelle(kafe, js) == []
        assert oku(js) == {"2026-09-27": 1}


def test_gun_olmayan_klasorler_yok_sayilir():
    with tempfile.TemporaryDirectory() as tmp:
        kafe, js = os.path.join(tmp, "bocafe"), os.path.join(tmp, "bocafe.json")
        gun_yaz(kafe, "2026-09-27", ["5551112233"])
        os.makedirs(os.path.join(kafe, "gunluk", "eski"))
        assert I.guncelle(kafe, js) == []
        assert oku(js) == {"2026-09-27": 1}
        assert I.guncelle(os.path.join(tmp, "olmayan"), os.path.join(tmp, "olmayan.json")) == []
        assert oku(os.path.join(tmp, "olmayan.json")) == {}


def _bag(hedef, bag):
    try:
        os.symlink(hedef, bag)
        return True
    except (OSError, NotImplementedError):
        print("atlandi (sembolik bağ desteklenmiyor):", bag)
        return False


def test_sembolik_bag_gun_ve_dosya_atlanir():
    with tempfile.TemporaryDirectory() as tmp:
        kafe, js = os.path.join(tmp, "bocafe"), os.path.join(tmp, "bocafe.json")
        baska = gun_yaz(os.path.join(tmp, "baska"), "2026-09-26", ["5551112233", "5559998877"])
        gun_yaz(kafe, "2026-09-27", ["5551112233"])
        if not _bag(baska, os.path.join(kafe, "gunluk", "2026-09-26")):
            return
        d = gun_yaz(kafe, "2026-09-28", [], manifest=())
        os.remove(os.path.join(d, "trafik.csv.gz"))
        _bag(os.path.join(baska, "trafik.csv.gz"), os.path.join(d, "trafik.csv.gz"))
        hatalar = I.guncelle(kafe, js)
        assert oku(js) == {"2026-09-27": 1}
        assert len(hatalar) == 1 and hatalar[0].startswith("2026-09-28")




if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
