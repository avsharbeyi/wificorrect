import json
import os
import re
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import yardim as Y  # noqa: E402
import kayitlar  # noqa: E402
import yonetici  # noqa: E402
import yonetim as YN  # noqa: E402

WG, SSH = "A" * 43 + "=", "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI c"
PW = "yonetici-parola-1"


def kur(tmp, durum=None):
    v, k, saat = Y.ortam(tmp)
    yol, dyol = os.path.join(tmp, "yonetici.json"), os.path.join(tmp, "durum.json")
    yonetici.yaz(yol, "serkan", PW)
    with open(dyol, "w", encoding="utf-8") as f:
        json.dump({"zaman": "", "arsivler": durum or {}}, f)
    isler = []
    app = YN.Yonetim(v, k, saat, yol, dyol, lambda islem, veri: isler.append((islem, veri)) or "0" * 16)
    c = Y.cerez(Y.giris(app, "serkan", PW))
    return app, v, saat, c, isler


def test_yonetici_dosyasi_ve_giris():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        yol = os.path.join(tmp, "y.json")
        try:
            yonetici.yaz(yol, "serkan", "kisa-parola")
            assert False
        except ValueError:
            pass
        assert yonetici.main(["parola", "serkan"], yol, sor=lambda: PW, sahip=None) == 0
        assert yonetici.oku(yol)["kullanici"] == "serkan"
        assert yonetici.main(["parola", "Kötü Ad"], yol, sor=lambda: PW, sahip=None) == 2
        app, v, _, c, _ = kur(tmp)
        assert c.startswith("wc_yonetim=")
        assert Y.giris(app, "serkan", "yanlis-parola-1")[0] == 401 and Y.giris(app, "baska", PW)[0] == 401
        n, pw = v.musteri_ekle()
        assert Y.giris(app, str(n), pw)[0] == 401  # müşteri yönetime giremez


def test_yeni_musteri_parola_bir_kez():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        app, v, _, c, _ = kur(tmp)
        f = Y.al(app, "/yeni", c)
        r = Y.gonder(app, "/yeni", c, {"csrf": Y.csrf(f), "not": "Bocafe <Göztepe>"})
        numara, pw = re.findall(r'class="sir">([^<]+)<', r[2].decode())
        assert r[0] == 200 and re.fullmatch(r"[1-9][0-9]{6}", numara) and v.parola_dogrula(int(numara), pw)
        assert pw in Y.al(app, f"/m/{numara}", c)[2].decode()  # 2026-10-04: parola yönetimde görünür (kullanıcı isteği)
        assert "Bocafe &lt;Göztepe&gt;" in Y.al(app, "/", c)[2].decode()
        assert v.hareketler(int(numara))[0]["olay"] == "MUSTERI_ACILDI"


def test_liste_durumlar_sorunlu_ustte_ve_arama():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        app, v, saat, c, _ = kur(tmp)
        n1, _ = v.musteri_ekle("Sağlam kafe")
        n2, _ = v.musteri_ekle("Gecikmiş kafe")
        with open(app.durum_yolu, "w", encoding="utf-8") as f:
            json.dump({"zaman": "", "arsivler": {str(n2): {"tur": "cihaz", "son_gun": "2026-09-01", "boyut": 2048,
                                                          "el_sikisma": 1_790_000_000 - 60}}}, f)
        cid, _ = v.cihaz_bagla(n2, WG, SSH, "Kafe <B>")
        s = Y.al(app, "/", c)[2].decode()
        assert s.index("Gecikmiş kafe") < s.index("Sağlam kafe")
        assert "GECİKTİ" in s and "çevrimiçi" in s and "Kafe &lt;B&gt;" in s and "cihaz yok" in s
        s = Y.al(app, "/?q=sa%C4%9Flam", c)[2].decode()
        assert "Sağlam kafe" in s and "Gecikmiş kafe" not in s


def test_parola_sifirla_serbest_birak_uyelik():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        app, v, _, c, isler = kur(tmp)
        n, pw = v.musteri_ekle()
        v.cihaz_bagla(n, WG, SSH)
        f = Y.al(app, f"/m/{n}", c)
        t = Y.csrf(f)
        r = Y.gonder(app, f"/m/{n}/parola-sifirla", c, {"csrf": t})
        yeni = re.search(r'class="sir">([^<]+)<', r[2].decode()).group(1)
        assert v.parola_dogrula(n, yeni) and not v.parola_dogrula(n, pw)
        assert Y.gonder(app, f"/m/{n}/serbest", c, {"csrf": t, "onay": "123"})[0] == 400  # numara yazılmadı
        assert Y.gonder(app, f"/m/{n}/serbest", c, {"csrf": t, "onay": str(n)})[0] == 303
        assert v.bagli_cihaz(n)["durum"] == "serbest_birakiliyor" and isler == []
        assert Y.gonder(app, f"/m/{n}/serbest", c, {"csrf": t, "onay": str(n), "zorla": "1"})[0] == 303
        assert v.bagli_cihaz(n) is None and isler == [("cihaz-kapat", {"numara": str(n), "wg_pub": WG})]
        Y.gonder(app, f"/m/{n}/uyelik", c, {"csrf": t, "islem": "bitir", "onay": str(n)})
        assert v.musteri(n)["uyelik"] == "bitti"
        Y.gonder(app, f"/m/{n}/uyelik", c, {"csrf": t, "islem": "ac"})
        assert v.musteri(n)["uyelik"] == "aktif"
        olaylar = [h["olay"] for h in v.hareketler(n)]
        for o in ("PAROLA_SIFIRLANDI", "SERBEST_BIRAKMA_ISTENDI", "CIHAZ_ZORLA_AYRILDI", "UYELIK_BITTI", "UYELIK_ACILDI"):
            assert o in olaylar, o
        assert Y.al(app, "/m/1234567" if n != 1234567 else "/m/7654321", c)[0] == 404
        assert Y.al(app, "/m/abc", c)[0] == 404 and Y.al(app, f"/m/{str(n)[:6]}", c)[0] == 404


def test_musteri_kayitlari_gerekce_ister():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        app, v, saat, c, _ = kur(tmp)
        n, _ = v.musteri_ekle()
        dun = kayitlar.son_30_gun(saat())[0].isoformat()
        Y.sayi_yaz(tmp, str(n), {dun: 9})
        Y.gun_yaz(tmp, str(n), "2026-09-20", [{"telefon": "5330000001", "ad": "Ayşe"}])
        assert ">9<" in Y.al(app, f"/m/{n}/kayitlar/", c)[2].decode()
        r = Y.al(app, f"/m/{n}/kayitlar/gun/2026-09-20", c)
        assert r[0] == 303 and f"donus=%2Fm%2F{n}%2Fkayitlar%2Fgun" in r[1]["Location"]
        Y.gerekce_ver(app, c)
        assert "Ayşe" in Y.al(app, f"/m/{n}/kayitlar/gun/2026-09-20", c)[2].decode()
        assert v.hareketler(n)[0]["olay"] == "GORUNTULEME" and v.hareketler(n)[0]["kim"] == "admin"


def test_eski_arsiv_baglama_ve_hesabim():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        app, v, _, c, _ = kur(tmp, {"bocafe": {"tur": "eski", "son_gun": "2026-09-20", "boyut": 10, "el_sikisma": None},
                                    "1234567": {"tur": "cihaz", "son_gun": None, "boyut": 0, "el_sikisma": None}})
        n, _ = v.musteri_ekle()
        f = Y.al(app, "/eski-arsivler", c)
        s = f[2].decode()
        assert "<td>bocafe</td>" in s and "<td>1234567</td>" not in s  # müşteri arşivi eski arşiv adayı değil
        assert Y.gonder(app, "/eski-arsivler", c, {"csrf": Y.csrf(f), "ad": "yok-boyle", "musteri": str(n)})[0] == 400
        assert Y.gonder(app, "/eski-arsivler", c, {"csrf": Y.csrf(f), "ad": "bocafe", "musteri": "1234567" if n != 1234567 else "7654321"})[0] == 400
        assert Y.gonder(app, "/eski-arsivler", c, {"csrf": Y.csrf(f), "ad": "bocafe", "musteri": str(n)})[0] == 303
        assert v.arsivler(n) == [str(n), "bocafe"]
        h = Y.al(app, "/hesabim", c)
        assert Y.gonder(app, "/hesabim", c, {"csrf": Y.csrf(h), "mevcut": PW, "yeni": "kisa", "tekrar": "kisa"})[0] == 400
        assert Y.gonder(app, "/hesabim", c, {"csrf": Y.csrf(h), "mevcut": PW, "yeni": "yeni-yonetici-12", "tekrar": "yeni-yonetici-12"})[0] == 200
        assert Y.giris(app, "serkan", "yeni-yonetici-12")[0] == 303


def test_yonetici_parolasi_degisince_eski_oturum_duser():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        app, v, _, c, _ = kur(tmp)
        c2 = Y.cerez(Y.giris(app, "serkan", PW))
        h = Y.al(app, "/hesabim", c)
        r = Y.gonder(app, "/hesabim", c, {"csrf": Y.csrf(h), "mevcut": PW, "yeni": "yeni-yonetici-12", "tekrar": "yeni-yonetici-12"})
        assert r[0] == 200 and Y.al(app, "/", Y.cerez(r))[0] == 200  # değiştiren yeni oturumla devam eder
        assert Y.al(app, "/", c2)[0] == 303 and Y.al(app, "/", c)[0] == 303


def test_uyelik_bitir_onay_ister_yeniden_acma_serbest_birakmayi_geri_alir():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        app, v, _, c, _ = kur(tmp)
        n, _ = v.musteri_ekle()
        v.cihaz_bagla(n, WG, SSH)
        t = Y.csrf(Y.al(app, f"/m/{n}", c))
        assert Y.gonder(app, f"/m/{n}/uyelik", c, {"csrf": t, "islem": "bitir"})[0] == 400
        assert v.musteri(n)["uyelik"] == "aktif" and v.bagli_cihaz(n)["durum"] == "bagli"
        Y.gonder(app, f"/m/{n}/uyelik", c, {"csrf": t, "islem": "bitir", "onay": str(n)})
        assert v.musteri(n)["uyelik"] == "bitti" and v.bagli_cihaz(n)["durum"] == "serbest_birakiliyor"
        Y.gonder(app, f"/m/{n}/uyelik", c, {"csrf": t, "islem": "ac"})
        assert v.musteri(n)["uyelik"] == "aktif" and v.bagli_cihaz(n)["durum"] == "bagli"


def test_parola_yetki_birakilmadan_once_sorulur():
    with tempfile.TemporaryDirectory() as tmp:
        sira = []
        kod = yonetici.main(["parola", "serkan"], os.path.join(tmp, "y.json"), sor=lambda: sira.append("sor") or PW,
                            sahip="wcpanel", birak=lambda s: sira.append("birak"))
        assert kod == 0 and sira == ["sor", "birak"]  # wcpanel terminale erişemez: parola root iken sorulur


def test_parola_gorunur_ve_lisans_yonetimi():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        app, v, _, c, _ = kur(tmp)
        n, pw = v.musteri_ekle("Göztepe Bilgisayar")
        s = Y.al(app, f"/m/{n}", c)[2].decode()
        assert pw in s and "2027-09-21" in s and "Lisans" in s
        t = Y.csrf(Y.al(app, f"/m/{n}", c))
        assert Y.gonder(app, f"/m/{n}/lisans", c, {"csrf": t, "islem": "uzat"})[0] == 303
        assert v.musteri(n)["lisans_bitis"] == "2028-09-20"
        Y.gonder(app, f"/m/{n}/lisans", c, {"csrf": t, "islem": "askiya"})
        assert v.musteri(n)["askida"] == 1
        Y.gonder(app, f"/m/{n}/lisans", c, {"csrf": t, "islem": "ac"})
        assert v.musteri(n)["askida"] == 0
        Y.gonder(app, f"/m/{n}/lisans", c, {"csrf": t, "islem": "tarih", "tarih": "2026-09-01"})
        assert v.musteri(n)["lisans_bitis"] == "2026-09-01"
        assert Y.gonder(app, f"/m/{n}/lisans", c, {"csrf": t, "islem": "tarih", "tarih": "bozuk"})[0] == 400
        liste = Y.al(app, "/", c)[2].decode()
        assert "Lisans" in liste and "süresi doldu" in liste
        olaylar = [h["olay"] for h in v.hareketler(n)]
        for o in ("LISANS_UZATILDI", "LISANS_ASKIYA_ALINDI", "LISANS_ACILDI", "LISANS_TARIHI"):
            assert o in olaylar, o
        v2, _ = v.musteri_ekle()
        v.db.execute("UPDATE musteri SET parola_acik = '' WHERE numara = ?", (v2,))
        assert "bilinmiyor" in Y.al(app, f"/m/{v2}", c)[2].decode()


def test_sifirlama_sayfasi_ve_lisans_tarihi_kaydi():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        app, v, _, c, _ = kur(tmp)
        n, _ = v.musteri_ekle()
        t = Y.csrf(Y.al(app, f"/m/{n}", c))
        s = Y.gonder(app, f"/m/{n}/parola-sifirla", c, {"csrf": t})[2].decode()
        assert "yalnızca şimdi" not in s and "müşteri sayfasında" in s
        Y.gonder(app, f"/m/{n}/lisans", c, {"csrf": t, "islem": "tarih", "tarih": "20270101"})
        assert v.hareketler(n)[0]["ayrinti"] == "bitis=2027-01-01"  # girilen değil, kaydedilen tarih
        assert "gününden sonra" in Y.al(app, f"/m/{n}", c)[2].decode()


if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
