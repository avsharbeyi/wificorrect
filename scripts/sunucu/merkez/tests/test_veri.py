import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))
import veri as V  # noqa: E402

WG1, WG2 = "A" * 43 + "=", "B" * 43 + "="
SSH = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI test@cihaz"


def yeni(tmp):
    return V.Veri(os.path.join(tmp, "m.db"), saat=lambda: 1_790_000_000.0)


def test_musteri_numara_sirasi_ve_parola():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        v = yeni(tmp)
        n1, p1 = v.musteri_ekle("Bocafe, Göztepe")
        numaralar = [n1] + [v.musteri_ekle()[0] for _ in range(30)]
        assert all(1_000_000 <= n <= 9_999_999 for n in numaralar) and len(set(numaralar)) == 31 and len(p1) == 12
        assert sorted(numaralar) != numaralar or numaralar != list(range(n1, n1 + 31))  # sıralı değil, rastgele
        assert v.parola_dogrula(n1, p1)["not_"] == "Bocafe, Göztepe"
        assert v.parola_dogrula(n1, "yanlis") is None and v.parola_dogrula(1, p1) is None
        assert v.parola_dogrula(str(n1), p1) is None  # numara int olmalı
        try:
            v.parola_koy(n1, "kisa")
            assert False
        except ValueError:
            pass
        v.parola_koy(n1, "yeni-parola-1")
        assert v.parola_dogrula(n1, "yeni-parola-1")
        p = v.parola_sifirla(n1)
        assert v.parola_dogrula(n1, p) and not v.parola_dogrula(n1, "yeni-parola-1")


def test_cihaz_bagla_tek_cihaz_ve_yeniden_deneme():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        v = yeni(tmp)
        n, _ = v.musteri_ekle()
        cid, a1 = v.cihaz_bagla(n, WG1, SSH, "Bocafe", "Bocafe Ltd.", "1.0")
        assert v.cihaz_anahtarla(a1)["id"] == cid
        cid2, a2 = v.cihaz_bagla(n, WG1, SSH)  # aynı cihaz yeniden denedi
        assert cid2 == cid and v.cihaz_anahtarla(a1) is None and v.cihaz_anahtarla(a2)["id"] == cid
        try:
            v.cihaz_bagla(n, WG2, SSH)  # başka cihaz
            assert False
        except V.BaskaCihaz:
            pass
        m2, _ = v.musteri_ekle()
        try:
            v.cihaz_bagla(m2, WG1, SSH)  # bu cihaz başka müşteriye bağlı
            assert False
        except V.BaskaCihaz:
            pass


def test_serbest_birakma_akisi():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        v = yeni(tmp)
        n, _ = v.musteri_ekle()
        cid, a = v.cihaz_bagla(n, WG1, SSH)
        assert v.serbest_birak(n)["id"] == cid
        assert v.bagli_cihaz(n)["durum"] == "serbest_birakiliyor" and v.cihaz_anahtarla(a) is not None
        try:
            v.cihaz_bagla(n, WG2, SSH)  # eski cihaz temizlenmeden yenisi bağlanmaz
            assert False
        except V.BaskaCihaz:
            pass
        v.temizlendi(cid)
        assert v.bagli_cihaz(n) is None and v.cihaz_anahtarla(a) is None
        cid3, _ = v.cihaz_bagla(n, WG2, SSH)
        assert v.serbest_birak(n, zorla=True)["id"] == cid3 and v.bagli_cihaz(n) is None
        assert [c["durum"] for c in v.cihaz_gecmisi(n)] == ["serbest", "serbest"]
        assert v.serbest_birak(n) is None


def test_uyelik_hareket_eski_arsiv():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        v = yeni(tmp)
        n, _ = v.musteri_ekle()
        v.uyelik(n, "bitti")
        assert v.musteri(n)["uyelik"] == "bitti" and v.musteri(n)["bitis"]
        v.uyelik(n, "aktif")
        assert v.musteri(n)["bitis"] == ""
        v.hareket("admin", "1.2.3.4", "MUSTERI_ACILDI", n, "x" * 1000)
        v.hareket("admin", "1.2.3.4", "GIRIS")
        assert [h["olay"] for h in v.hareketler()] == ["GIRIS", "MUSTERI_ACILDI"]
        assert len(v.hareketler(n)) == 1 and len(v.hareketler(n)[0]["ayrinti"]) == 500
        v.eski_arsiv_bagla("bocafe", n)
        v.eski_arsiv_bagla("bocafe-test", None)
        assert v.eski_arsivler() == {"bocafe": n, "bocafe-test": None}
        assert v.arsivler(n) == [str(n), "bocafe"]


def test_kalici():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        v = yeni(tmp)
        n, p = v.musteri_ekle()
        v.db.close()
        assert yeni(tmp).parola_dogrula(n, p)


if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
