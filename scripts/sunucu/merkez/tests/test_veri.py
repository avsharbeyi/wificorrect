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


def test_parola_acik_ve_lisans():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        v = yeni(tmp)  # saat: 2026-09-21
        n, p = v.musteri_ekle()
        m = v.musteri(n)
        assert m["parola_acik"] == p and m["lisans_bitis"] == "2027-09-21" and m["askida"] == 0
        v.parola_koy(n, "yeni-parola-1")
        assert v.musteri(n)["parola_acik"] == "yeni-parola-1"
        p2 = v.parola_sifirla(n)
        assert v.musteri(n)["parola_acik"] == p2
        assert v.lisans_durumu(v.musteri(n)) == ("aktif", "2027-09-21")
        assert v.lisans_uzat(n) == "2028-09-20"  # bitişin üstüne 1 yıl
        v.lisans_ayarla(n, "2026-09-01")
        assert v.lisans_durumu(v.musteri(n)) == ("bitti", "2026-09-01")
        assert v.lisans_uzat(n) == "2027-09-21"  # süresi geçmişse bugünden 1 yıl
        v.askiya_al(n, True)
        assert v.lisans_durumu(v.musteri(n))[0] == "askida"
        v.askiya_al(n, False)
        assert v.lisans_durumu(v.musteri(n))[0] == "aktif"
        try:
            v.lisans_ayarla(n, "bozuk")
            assert False
        except ValueError:
            pass


def test_eski_veritabani_gocu():
    import sqlite3
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        yol = os.path.join(tmp, "m.db")
        db = sqlite3.connect(yol)
        db.execute("CREATE TABLE musteri (numara INTEGER PRIMARY KEY, tuz TEXT NOT NULL, ozet TEXT NOT NULL, yineleme INTEGER NOT NULL, "
                   "not_ TEXT NOT NULL DEFAULT '', uyelik TEXT NOT NULL DEFAULT 'aktif', bitis TEXT NOT NULL DEFAULT '', "
                   "olusturma TEXT NOT NULL, son_giris TEXT NOT NULL DEFAULT '')")
        db.execute("INSERT INTO musteri (numara, tuz, ozet, yineleme, olusturma) VALUES (4511643, 't', 'o', 120000, '2026-10-04T17:00:00+03:00')")
        db.commit()
        db.close()
        v = V.Veri(yol, saat=lambda: 1_790_000_000.0)
        m = v.musteri(4511643)
        assert (m["lisans_bitis"], m["parola_acik"], m["askida"]) == ("2027-10-04", "", 0)


def test_silinen_veri_sayfalarda_kalmaz():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        v = yeni(tmp)
        assert v.db.execute("PRAGMA secure_delete").fetchone()[0] == 1  # eski açık parolalar dosyada kalmasın


if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
