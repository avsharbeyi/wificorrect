import json
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import yardim as Y  # noqa: E402
import api as A  # noqa: E402

WG, WG2 = "A" * 43 + "=", "B" * 43 + "="
SSH = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI cihaz"
TAMAM = {"durum": "tamam", "tunel_ip": "10.99.0.10", "sunucu_pub": "S" * 43 + "=", "uc_nokta": "vpn.wificorrect.com:51820"}


class Kuyruk:
    def __init__(self, sonuc=TAMAM):
        self.isler, self.sonuc = [], sonuc

    def ekle(self, islem, veri):
        self.isler.append((islem, veri))
        return f"{len(self.isler):016x}"

    def bekle(self, kimlik):
        return self.sonuc


def kur(tmp, sonuc=TAMAM):
    v, _, saat = Y.ortam(tmp)
    q = Kuyruk(sonuc)
    return A.Api(v, saat, q.ekle, q.bekle), v, q, saat


def baska(n):
    """n'den farklı, geçerli biçimde ama kayıtlı olmayan numara."""
    return 1234567 if n != 1234567 else 7654321


def post(api, yol, govde, ip="198.51.100.7"):
    d, b, g = api.istek("POST", yol, {}, json.dumps(govde) if not isinstance(govde, str) else govde, "", ip)
    return d, json.loads(g)


def giris(api, n, pw, wg=WG, ip="198.51.100.7"):
    return post(api, "/api/giris", {"numara": str(n), "parola": pw, "wg_pub": wg, "ssh_pub": SSH,
                                    "isletme_adi": "Bocafe", "unvan": "Bocafe Ltd.", "surum": "1.0"}, ip)


def test_ilk_giris_baglar_ve_kuyruga_atar():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        api, v, q, _ = kur(tmp)
        n, pw = v.musteri_ekle()
        d, j = giris(api, n, pw)
        assert d == 200 and j["tunel_ip"] == "10.99.0.10" and j["yedek_hedefi"] == f"wfc-{n}@10.99.0.1:"
        assert j["uc_nokta"] == "vpn.wificorrect.com:51820" and j["yineleme"] == 120000 and len(j["cihaz_anahtari"]) > 30
        m = v.musteri(n)
        assert (j["tuz"], j["ozet"]) == (m["tuz"], m["ozet"])
        assert q.isler == [("cihaz-ekle", {"numara": str(n), "wg_pub": WG, "ssh_pub": SSH})]
        c = v.bagli_cihaz(n)
        assert c["tunel_ip"] == "10.99.0.10" and c["isletme_adi"] == "Bocafe"
        assert [h["olay"] for h in v.hareketler(n)] == ["CIHAZ_BAGLANDI"]


def test_hatali_parola_kilit_ve_gecersiz_girdi():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        api, v, q, _ = kur(tmp)
        n, pw = v.musteri_ekle()
        assert giris(api, n, "yanlis-parola")[1]["kod"] == "hatali"
        assert giris(api, baska(n), "yanlis-parola")[1]["kod"] == "hatali"
        assert giris(api, 100001, "yanlis-parola")[1]["kod"] == "gecersiz"  # 6 hane
        for _ in range(4):
            giris(api, n, "yanlis-parola", ip="198.51.100.8")
        assert giris(api, n, pw)[0] == 429  # numara kilitli
        assert post(api, "/api/giris", "{bozuk")[0] == 400
        assert post(api, "/api/giris", {"numara": "abc", "parola": "x"})[1]["kod"] == "gecersiz"
        assert post(api, "/api/giris", {"numara": str(baska(n)), "parola": "x" * 10, "wg_pub": "kotu", "ssh_pub": SSH})[1]["kod"] == "hatali"
        assert api.istek("GET", "/api/giris", {}, "", "", "1.1.1.1")[0] == 404
        assert api.istek("POST", "/baska", {}, "{}", "", "1.1.1.1")[0] == 404
        assert api.istek("POST", "/api/giris", {}, "x" * 5000, "", "1.1.1.1")[0] == 413
        assert q.isler == []


def test_baska_cihaz_409_ve_ayni_cihaz_yeniden_dener():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        api, v, q, _ = kur(tmp, sonuc=None)  # kuyruk zaman aşımı
        n, pw = v.musteri_ekle()
        d, j = giris(api, n, pw)
        assert d == 503 and j["kod"] == "kayit"
        q.sonuc = TAMAM
        d, j1 = giris(api, n, pw)  # aynı cihaz yeniden
        assert d == 200
        d, j2 = giris(api, n, pw)
        assert d == 200 and j2["cihaz_anahtari"] != j1["cihaz_anahtari"]
        assert post(api, "/api/eslesme", {"cihaz_anahtari": j1["cihaz_anahtari"]})[1]["kod"] == "taninmadi"
        d, j = giris(api, n, pw, wg=WG2)
        assert d == 409 and j["kod"] == "baska_cihaz"


def test_uyelik_bitti_giris_yok():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        api, v, _, _ = kur(tmp)
        n, pw = v.musteri_ekle()
        v.uyelik(n, "bitti")
        assert giris(api, n, pw)[1]["kod"] == "uyelik"
        v.uyelik(n, "aktif")
        assert giris(api, n, pw)[0] == 200


def test_eslesme_parola_ve_serbest_birakma():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        api, v, q, _ = kur(tmp)
        n, pw = v.musteri_ekle()
        a = giris(api, n, pw)[1]["cihaz_anahtari"]
        d, j = post(api, "/api/eslesme", {"cihaz_anahtari": a, "isletme_adi": "Bocafe Göztepe", "unvan": "<b>", "surum": "1.1"})
        assert d == 200 and j["durum"] == "bagli" and j["uyelik"] == "aktif"
        c = v.bagli_cihaz(n)
        assert c["isletme_adi"] == "Bocafe Göztepe" and c["surum"] == "1.1" and c["son_eslesme"]
        assert post(api, "/api/parola", {"cihaz_anahtari": a, "eski": "yanlis-1234", "yeni": "yeni-parola-1"})[1]["kod"] == "hatali"
        assert post(api, "/api/parola", {"cihaz_anahtari": a, "eski": pw, "yeni": "kisa"})[0] == 400
        d, j = post(api, "/api/parola", {"cihaz_anahtari": a, "eski": pw, "yeni": "yeni-parola-1"})
        assert d == 200 and v.parola_dogrula(n, "yeni-parola-1") and j["ozet"] == v.musteri(n)["ozet"]
        v.serbest_birak(n)
        assert post(api, "/api/eslesme", {"cihaz_anahtari": a})[1]["durum"] == "serbest"
        assert post(api, "/api/eslesme", {"cihaz_anahtari": a, "temizlendi": True})[1]["durum"] == "serbest"
        assert q.isler[-1] == ("cihaz-kapat", {"numara": str(n), "wg_pub": WG})
        assert post(api, "/api/eslesme", {"cihaz_anahtari": a})[1]["kod"] == "taninmadi"
        assert v.bagli_cihaz(n) is None


def test_metinler_kirpilir_ve_hiz_siniri():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        api, v, _, _ = kur(tmp)
        n, pw = v.musteri_ekle()
        a = giris(api, n, pw)[1]["cihaz_anahtari"]
        post(api, "/api/eslesme", {"cihaz_anahtari": a, "isletme_adi": "x" * 500, "unvan": "a\x00b", "surum": 5})
        c = v.bagli_cihaz(n)
        assert c["isletme_adi"] == "" and c["unvan"] == "" and c["surum"] == ""
        kodlar = [post(api, "/api/eslesme", {"cihaz_anahtari": a}, ip="203.0.113.99")[0] for _ in range(21)]
        assert kodlar[-1] == 429 and 429 not in kodlar[:20]


def test_unicode_rakam_kilidi_atlatamaz():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        api, v, q, _ = kur(tmp)
        n, pw = v.musteri_ekle()
        for _ in range(5):
            giris(api, n, "yanlis-parola")
        assert giris(api, n, pw)[0] == 429
        d, j = post(api, "/api/giris", {"numara": "1\uff100001", "parola": pw, "wg_pub": WG, "ssh_pub": SSH})
        assert d == 400 and j["kod"] == "gecersiz" and q.isler == []


if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
