import json
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))
import kuyruk as K  # noqa: E402

WG, WG2 = "A" * 43 + "=", "B" * 43 + "="
SSH = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI test"


class Sunucu:
    """wificorrect-sunucu taklidi: kayit.csv'yi gerçek komut gibi değiştirir."""

    def __init__(self, tmp):
        self.kayit = os.path.join(tmp, "kayit.csv")
        self.wg = os.path.join(tmp, "wg")
        os.makedirs(self.wg)
        for ad, icerik in (("sunucu.pub", "S" * 43 + "=\n"), ("uc-nokta", "vpn.wificorrect.com:51820\n")):
            with open(os.path.join(self.wg, ad), "w", encoding="utf-8") as f:
                f.write(icerik)
        open(self.kayit, "w").close()
        self.komutlar = []

    def __call__(self, komut):
        self.komutlar.append(komut[1:])
        with open(self.kayit, encoding="utf-8") as f:
            satirlar = f.read().splitlines()
        if komut[1] == "cihaz-ekle":
            satirlar.append(f"cihaz;{komut[2]};10.99.0.{10 + len(satirlar)};{komut[3]};wfc-{komut[2]}")
        elif komut[1] == "cihaz-kapat":
            satirlar = [s.replace("cihaz;", "bitti;", 1) + ";2026-10-04" if s.startswith(f"cihaz;{komut[2]};") else s for s in satirlar]
        with open(self.kayit, "w", encoding="utf-8") as f:
            f.write("\n".join(satirlar) + "\n")
        return 0, "", ""


def test_ekle_ve_bekle():
    with tempfile.TemporaryDirectory() as tmp:
        kq, sd = os.path.join(tmp, "kuyruk"), os.path.join(tmp, "sonuc")
        os.makedirs(kq)
        os.makedirs(sd)
        kimlik = K.ekle("cihaz-ekle", {"numara": "100001"}, kq)
        with open(os.path.join(kq, kimlik + ".json"), encoding="utf-8") as f:
            assert json.load(f) == {"numara": "100001", "islem": "cihaz-ekle"}
        assert [a for a in os.listdir(kq) if a.endswith(".tmp")] == []
        uykular = []
        assert K.bekle(kimlik, sure=1.0, aralik=0.5, sonuc_dizini=sd, uyku=uykular.append) is None and len(uykular) == 3
        with open(os.path.join(sd, kimlik + ".json"), "w", encoding="utf-8") as f:
            json.dump({"durum": "tamam"}, f)
        assert K.bekle(kimlik, sonuc_dizini=sd, uyku=uykular.append) == {"durum": "tamam"}


def test_cihaz_ekle_yeniden_ve_degisen_cihaz():
    with tempfile.TemporaryDirectory() as tmp:
        s = Sunucu(tmp)
        r = K.isle({"islem": "cihaz-ekle", "numara": "100001", "wg_pub": WG, "ssh_pub": SSH}, s, s.kayit, s.wg)
        assert r == {"durum": "tamam", "tunel_ip": "10.99.0.10", "sunucu_pub": "S" * 43 + "=", "uc_nokta": "vpn.wificorrect.com:51820"}
        r2 = K.isle({"islem": "cihaz-ekle", "numara": "100001", "wg_pub": WG, "ssh_pub": SSH}, s, s.kayit, s.wg)
        assert r2["tunel_ip"] == "10.99.0.10" and len(s.komutlar) == 1  # aynı cihaz: komut çalışmaz
        K.isle({"islem": "cihaz-ekle", "numara": "100001", "wg_pub": WG2, "ssh_pub": SSH}, s, s.kayit, s.wg)
        assert [k[0] for k in s.komutlar] == ["cihaz-ekle", "cihaz-kapat", "cihaz-ekle"]
        assert K.isle({"islem": "cihaz-kapat", "numara": "100001"}, s, s.kayit, s.wg) == {"durum": "tamam"}
        assert K.isle({"islem": "cihaz-kapat", "numara": "100001"}, s, s.kayit, s.wg) == {"durum": "tamam"}  # zaten kapalı
        assert s.komutlar[-1][0] == "cihaz-kapat" and len(s.komutlar) == 4


def test_gecersiz_girdiler_komut_calistirmaz():
    with tempfile.TemporaryDirectory() as tmp:
        s = Sunucu(tmp)
        for is_ in ({"islem": "cihaz-ekle", "numara": "1; rm -rf /", "wg_pub": WG, "ssh_pub": SSH},
                    {"islem": "cihaz-ekle", "numara": 100001, "wg_pub": WG, "ssh_pub": SSH},
                    {"islem": "cihaz-ekle", "numara": "100001", "wg_pub": WG + "\n[Peer]", "ssh_pub": SSH},
                    {"islem": "cihaz-ekle", "numara": "100001", "wg_pub": WG, "ssh_pub": SSH + "\ncommand=x"},
                    {"islem": "kabuk", "numara": "100001"},
                    {"islem": "cihaz-kapat", "numara": "1\uff100001"}):
            assert K.isle(is_, s, s.kayit, s.wg)["durum"] == "hata"
        assert s.komutlar == []


def test_main_isler_siler_ve_cop_dosyada_donmez():
    with tempfile.TemporaryDirectory() as tmp:
        s = Sunucu(tmp)
        kq, sd = os.path.join(tmp, "kuyruk"), os.path.join(tmp, "sonuc")
        os.makedirs(kq)
        os.makedirs(sd)
        iyi = K.ekle("cihaz-ekle", {"numara": "100001", "wg_pub": WG, "ssh_pub": SSH}, kq)
        with open(os.path.join(kq, "0123456789abcdef.json"), "w") as f:
            f.write("{bozuk")
        with open(os.path.join(kq, "baska-ad.json"), "w") as f:  # .path birimi bunu da görür: silinmeli
            f.write("{}")
        with open(os.path.join(kq, "fedcba9876543210.json"), "w") as f:
            f.write("x" * 5000)
        assert K.main(kq, sd, s, s.kayit, s.wg, grup=None) == 0
        assert os.listdir(kq) == []
        assert K.sonuc(iyi, sd)["tunel_ip"] == "10.99.0.10"
        assert K.sonuc("0123456789abcdef", sd)["durum"] == "hata" and K.sonuc("fedcba9876543210", sd)["durum"] == "hata"
        os.utime(os.path.join(sd, iyi + ".json"), (1, 1))  # 1 günden eski sonuç temizlenir
        K.main(kq, sd, s, s.kayit, s.wg, grup=None)
        assert K.sonuc(iyi, sd) is None


def test_sembolik_bag_ve_fifo_izlenmez():
    if not hasattr(os, "O_NOFOLLOW"):
        return  # Windows: sunucuda koşar
    with tempfile.TemporaryDirectory() as tmp:
        hedef = os.path.join(tmp, "gizli")
        with open(hedef, "w") as f:
            json.dump({"islem": "cihaz-kapat", "numara": "100001"}, f)
        os.symlink(hedef, os.path.join(tmp, "a.json"))
        assert K.is_oku(os.path.join(tmp, "a.json")) is None
        os.mkfifo(os.path.join(tmp, "b.json"))
        assert K.is_oku(os.path.join(tmp, "b.json")) is None


def test_eski_kapatma_isi_yeni_cihazi_kapatmaz():
    with tempfile.TemporaryDirectory() as tmp:
        s = Sunucu(tmp)
        K.isle({"islem": "cihaz-ekle", "numara": "100001", "wg_pub": WG2, "ssh_pub": SSH}, s, s.kayit, s.wg)
        assert K.isle({"islem": "cihaz-kapat", "numara": "100001", "wg_pub": WG}, s, s.kayit, s.wg) == {"durum": "tamam"}
        assert [k[0] for k in s.komutlar] == ["cihaz-ekle"]
        K.isle({"islem": "cihaz-kapat", "numara": "100001", "wg_pub": WG2}, s, s.kayit, s.wg)
        assert [k[0] for k in s.komutlar] == ["cihaz-ekle", "cihaz-kapat"]
        assert K.isle({"islem": "cihaz-kapat", "numara": "100001", "wg_pub": "kotu"}, s, s.kayit, s.wg)["durum"] == "hata"


def test_isler_yazilis_sirasiyla_islenir():
    with tempfile.TemporaryDirectory() as tmp:
        s = Sunucu(tmp)
        K.isle({"islem": "cihaz-ekle", "numara": "100001", "wg_pub": WG, "ssh_pub": SSH}, s, s.kayit, s.wg)
        kq, sd = os.path.join(tmp, "kuyruk"), os.path.join(tmp, "sonuc")
        os.makedirs(kq)
        os.makedirs(sd)
        for ad, is_, zaman in (("ffffffffffffffff", {"islem": "cihaz-kapat", "numara": "100001", "wg_pub": WG}, 1000),
                               ("0000000000000000", {"islem": "cihaz-ekle", "numara": "100001", "wg_pub": WG, "ssh_pub": SSH}, 2000)):
            with open(os.path.join(kq, ad + ".json"), "w", encoding="utf-8") as f:
                json.dump(is_, f)
            os.utime(os.path.join(kq, ad + ".json"), (zaman, zaman))
        K.main(kq, sd, s, s.kayit, s.wg, grup=None)
        assert K.kayit_satiri("100001", s.kayit) is not None  # önce kapat, sonra ekle: cihaz açık kalır


if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
