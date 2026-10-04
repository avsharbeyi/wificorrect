"""Yönetim merkezi (spec §5): yonetim.wificorrect.com — müşteriler, cihazlar, parola sıfırlama, serbest bırakma,
üyelik, kayıtlar (gerekçeyle), hareketler, eski arşivler, yönetici hesabı."""
import datetime
import json
import re
import time
import urllib.parse

import common
import detay
import durum as durum_modulu
import guvenlik
import kayitlar as kayitlar_modulu
import kuyruk
import web
import yonetici
from web import bilgi_tablosu, e, kart, yanit_html, yonlendir

NUMARA_RE = re.compile(r"[1-9]\d{5}")
CEVRIMICI_SN = 600
ESLESME_GECIKME_SN = 26 * 3600
GECIKME_GUN = 2


def _epoch(iso):
    try:
        return datetime.datetime.fromisoformat(iso).timestamp()
    except (TypeError, ValueError):
        return None


def once(sn):
    dk = int(sn) // 60
    return f"{dk} dk önce" if dk < 120 else (f"{dk // 60} saat önce" if dk < 2880 else f"{dk // 1440} gün önce")


class Yonetim(web.Taban):
    rol = "yonetici"
    cerez = "wc_yonetim"
    giris_basligi = "Yönetim merkezi"
    giris_aciklama = "Hizmet sağlayıcı girişi."

    def __init__(self, veri, kayitlar, saat=time.time, yonetici_yolu=yonetici.YONETICI,
                 durum_yolu=durum_modulu.DURUM, kuyruk_ekle=kuyruk.ekle):
        super().__init__(veri, kayitlar, saat)
        self.yonetici_yolu, self.durum_yolu, self.kuyruk_ekle = yonetici_yolu, durum_yolu, kuyruk_ekle

    # --- Taban ---
    def dogrula(self, kul, pw):
        y = yonetici.oku(self.yonetici_yolu)
        if not y or kul != y["kullanici"]:
            guvenlik.bos_dogrulama(pw)
            return None
        return kul if guvenlik.dogru(pw, y["tuz"], y["ozet"], y["yineleme"]) else None

    def hesap_var(self, kimlik):
        y = yonetici.oku(self.yonetici_yolu)
        return bool(y) and y["kullanici"] == kimlik

    def girdi(self, kimlik, ip):
        self.veri.hareket("admin", ip, "GIRIS", None, "yönetim")

    def kim(self, ot):
        return "admin"

    def hareket_musterisi(self, ot, yol):
        m = re.match(r"/m/(\d{6})/", yol)
        return int(m.group(1)) if m else None

    def menu(self, ot):
        return ('<a href="/">Müşteriler</a><a href="/yeni">Yeni müşteri</a><a href="/hareketler">Hareketler</a>'
                '<a href="/eski-arsivler">Eski arşivler</a><a href="/hesabim">Hesabım</a>')

    # --- yardımcılar ---
    def _durum(self):
        try:
            with open(self.durum_yolu, encoding="utf-8") as f:
                d = json.load(f)
        except (FileNotFoundError, ValueError):
            return {}
        a = d.get("arsivler") if isinstance(d, dict) else None
        return a if isinstance(a, dict) else {}

    def _sinir_gun(self):
        bugun = datetime.datetime.fromtimestamp(self.saat(), common.TZ).date()
        return (bugun - datetime.timedelta(days=GECIKME_GUN)).isoformat()

    def _ozet(self, m, c, d):
        """Bir müşterinin liste satırı bilgileri: (cihaz metni, son gün metni, sorunlu mu)."""
        simdi = self.saat()
        if c is None:
            return "cihaz yok", e(d.get("son_gun") or "—"), False
        el = d.get("el_sikisma")
        es = _epoch(c["son_eslesme"]) or _epoch(c["baglanma"]) or 0
        if c["durum"] == "serbest_birakiliyor":
            cihaz = "serbest bırakılıyor"
        elif el and simdi - el <= CEVRIMICI_SN:
            cihaz = "çevrimiçi"
        else:
            cihaz = f"son eşitleme {once(simdi - es)}" if c["son_eslesme"] else "henüz eşitlenmedi"
        sg = d.get("son_gun")
        gecikti = not sg or sg < self._sinir_gun()
        son = e(sg or "—") + (' <b class="kotu">GECİKTİ</b>' if gecikti else "")
        sorunlu = gecikti or simdi - es > ESLESME_GECIKME_SN or c["durum"] == "serbest_birakiliyor"
        return cihaz, son, sorunlu

    # --- sayfalar ---
    def sayfalar(self, ot, token, yontem, yol, sorgu, form, ip):
        if (yontem, yol) == ("GET", "/"):
            return self.liste(ot, sorgu)
        if yol == "/yeni":
            return self.yeni(ot, yontem, form, ip)
        if (yontem, yol) == ("GET", "/hareketler"):
            return yanit_html(self.sayfa("Hareketler", kart("<h1>Hareketler</h1>" + self._hareket_tablosu(self.veri.hareketler())), ot, genis=True))
        if yol == "/eski-arsivler":
            return self.eski_arsivler(ot, yontem, form, ip)
        if yol == "/hesabim":
            return self.hesabim(ot, yontem, form, ip)
        m = re.fullmatch(r"/m/(\d{6})(/.*)?", yol)
        if not m or self.veri.musteri(int(m.group(1))) is None:
            return None
        n, alt = int(m.group(1)), m.group(2) or "/"
        if (yontem, alt) == ("GET", "/"):
            return self.musteri_sayfasi(ot, n)
        if (yontem, alt) == ("POST", "/parola-sifirla"):
            pw = self.veri.parola_sifirla(n)
            self.veri.hareket("admin", ip, "PAROLA_SIFIRLANDI", n)
            return yanit_html(self.sayfa("Parola sıfırlandı", kart(
                f'<h1>Müşteri {n}: yeni parola</h1><p>Parola</p><p class="sir">{e(pw)}</p>'
                '<p class="alt">Bu parola yalnızca şimdi gösterilir. Merkezi panelde hemen, cihazda ertesi sabah '
                f'06:00 eşitlemesinden sonra geçerlidir.</p><p><a href="/m/{n}">← Müşteri sayfası</a></p>'), ot))
        if (yontem, alt) == ("POST", "/serbest"):
            return self.serbest(ot, n, form, ip)
        if (yontem, alt) == ("POST", "/uyelik"):
            if form.get("islem") == "bitir":
                self.veri.uyelik(n, "bitti")
                self.veri.serbest_birak(n)
                self.veri.hareket("admin", ip, "UYELIK_BITTI", n)
            elif form.get("islem") == "ac":
                self.veri.uyelik(n, "aktif")
                self.veri.hareket("admin", ip, "UYELIK_ACILDI", n)
            return yonlendir(f"/m/{n}")
        if (yontem, alt) == ("GET", "/hareketler"):
            return yanit_html(self.sayfa("Hareketler", kart(f"<h1>Müşteri {n} · hareketler</h1>"
                                                            + self._hareket_tablosu(self.veri.hareketler(n))), ot, genis=True))
        if yontem == "GET" and (alt == "/kayitlar" or alt.startswith("/kayitlar/")):
            c = self.veri.bagli_cihaz(n)
            baslik = (c["isletme_adi"] if c is not None and c["isletme_adi"] else f"Müşteri {n}")
            return self.kayit_sayfasi(ot, n, alt[len("/kayitlar"):] or "/", sorgu, f"/m/{n}/kayitlar", ip, baslik)
        return None

    def liste(self, ot, sorgu):
        q = detay.tr_kucuk((urllib.parse.parse_qs(sorgu).get("q") or [""])[0].strip())
        durum, cihazlar, satirlar = self._durum(), self.veri.cihazlar(), []
        gunler = [g.isoformat() for g in kayitlar_modulu.son_30_gun(self.saat())[:7]]
        for m in self.veri.musteriler():
            n, c = m["numara"], cihazlar.get(m["numara"])
            ad = c["isletme_adi"] if c is not None else ""
            if q and q not in str(n) and q not in detay.tr_kucuk(ad) and q not in detay.tr_kucuk(m["not_"]):
                continue
            cihaz, son, sorunlu = self._ozet(m, c, durum.get(str(n), {}))
            sayilar = self.kayitlar.sayilar(self.veri.arsivler(n))[0]
            yedi = " ".join(str(sayilar.get(g, "—")) for g in reversed(gunler))
            uyelik = "Aktif" if m["uyelik"] == "aktif" else f"Bitti {m['bitis'][:10]}"
            satirlar.append((not sorunlu, n,
                             f'<tr><td><a href="/m/{n}">{n}</a></td><td>{e(ad or "—")}<br><span class="alt">{e(m["not_"])}</span></td>'
                             f'<td>{e(cihaz)}</td><td>{son}</td><td>{e(yedi)}</td><td>{e(uyelik)}</td></tr>'))
        satirlar.sort()
        tablo = ('<div class="kaydir"><table class="detay"><tr><th>No</th><th>İşletme / not</th><th>Cihaz</th>'
                 '<th>Son gelen gün</th><th>Son 7 gün</th><th>Üyelik</th></tr>' + "".join(s[2] for s in satirlar)
                 + "</table></div>") if satirlar else '<p class="alt">Müşteri yok.</p>'
        ara = (f'<form class="ara" method="get" action="/"><input name="q" value="{e(q)}" placeholder="Numara, işletme adı ya da not">'
               '<button>Ara</button></form>')
        return yanit_html(self.sayfa("Müşteriler", kart(f"<h1>Müşteriler</h1>{ara}") + kart(tablo), ot, genis=True))

    def yeni(self, ot, yontem, form, ip):
        if yontem == "GET":
            return yanit_html(self.sayfa("Yeni müşteri", kart(
                '<h1>Yeni müşteri</h1><form method="post" action="/yeni">'
                f'<input type="hidden" name="csrf" value="{e(ot["csrf"])}">'
                '<label for="n">Not (isteğe bağlı, yalnızca siz görürsünüz)</label>'
                '<input id="n" name="not" maxlength="200" placeholder="ör. Ahmet Bey, Kadıköy"><button>Oluştur</button></form>'), ot))
        n, pw = self.veri.musteri_ekle(form.get("not", ""))
        self.veri.hareket("admin", ip, "MUSTERI_ACILDI", n)
        return yanit_html(self.sayfa("Müşteri açıldı", kart(
            f'<h1>Müşteri açıldı</h1><p>Müşteri numarası</p><p class="sir">{n}</p><p>Parola</p><p class="sir">{e(pw)}</p>'
            '<p class="alt">Parola yalnızca şimdi gösterilir; müşteriye iletin. Müşteri cihazın giriş ekranına bu numara ve '
            f'parolayla girer.</p><p><a href="/m/{n}">Müşteri sayfası →</a></p>'), ot))

    def musteri_sayfasi(self, ot, n):
        m, c, d = self.veri.musteri(n), self.veri.bagli_cihaz(n), self._durum().get(str(n), {})
        cs = e(ot["csrf"])
        bilgi = bilgi_tablosu([("Numara", str(n)), ("Not", e(m["not_"] or "—")), ("Oluşturma", e(m["olusturma"][:16])),
                               ("Son giriş (panel)", e(m["son_giris"][:16] or "—")),
                               ("Üyelik", "Aktif" if m["uyelik"] == "aktif" else f"Bitti {e(m['bitis'][:10])}")])
        if c is None:
            cihaz = '<p class="alt">Bağlı cihaz yok. Müşteri bir cihazın giriş ekranına numarasıyla girince bağlanır.</p>'
        else:
            metin, son, _ = self._ozet(m, c, d)
            cihaz = bilgi_tablosu([
                ("Durum", e(metin)), ("İşletme adı", e(c["isletme_adi"] or "—")), ("Unvan", e(c["unvan"] or "—")),
                ("Tünel adresi", e(c["tunel_ip"] or "—")), ("Sürüm", e(c["surum"] or "—")),
                ("Bağlanma", e(c["baglanma"][:16])), ("Son eşitleme", e(c["son_eslesme"][:16] or "—")),
                ("Son gelen gün", son), ("Arşiv", e(detay.boyut(d.get("boyut", 0)))),
                ("Cihaz paneli", f'<a href="https://{e(c["tunel_ip"])}:8443">https://{e(c["tunel_ip"])}:8443</a> (VPN gerekli)'
                 if c["tunel_ip"] else "—")])
            cihaz += (f'<form method="post" action="/m/{n}/serbest"><input type="hidden" name="csrf" value="{cs}">'
                      f'<label for="o">Cihazı serbest bırakmak için müşteri numarasını yazın ({n})</label>'
                      '<input id="o" name="onay" inputmode="numeric" autocomplete="off" required>'
                      '<label><input type="checkbox" name="zorla" value="1"> Cihaz ulaşılamaz (arızalı/kayıp): beklemeden ayır. '
                      'Cihazda kalmış gönderilmemiş günler kaybolabilir.</label>'
                      '<button class="tehlike">Cihazı serbest bırak</button></form>')
        uyelik_dugme = ("bitir", "Üyeliği bitir", "tehlike") if m["uyelik"] == "aktif" else ("ac", "Üyeliği yeniden aç", "")
        islemler = (f'<form method="post" action="/m/{n}/parola-sifirla"><input type="hidden" name="csrf" value="{cs}">'
                    '<button>Parola sıfırla</button></form>'
                    f'<form method="post" action="/m/{n}/uyelik"><input type="hidden" name="csrf" value="{cs}">'
                    f'<input type="hidden" name="islem" value="{uyelik_dugme[0]}"><button class="{uyelik_dugme[2]}">{uyelik_dugme[1]}</button></form>'
                    f'<p><a href="/m/{n}/kayitlar/">Kayıtlar</a> · <a href="/m/{n}/hareketler">Hareketler</a></p>')
        gecmis = detay._tablo(("No", "Durum", "Bağlanma", "Ayrılma", "Tünel"),
                                  [(x["id"], x["durum"], x["baglanma"][:16], x["ayrilma"][:16], x["tunel_ip"])
                                   for x in self.veri.cihaz_gecmisi(n)], "Henüz cihaz yok.")
        return yanit_html(self.sayfa(f"Müşteri {n}", kart(f"<h1>Müşteri {n}</h1>{bilgi}") + kart(f"<h2>Cihaz</h2>{cihaz}")
                                     + kart(f"<h2>İşlemler</h2>{islemler}") + kart(f"<h2>Cihaz geçmişi</h2>{gecmis}"), ot, genis=True))

    def serbest(self, ot, n, form, ip):
        if form.get("onay", "").strip() != str(n):
            return yanit_html(self.mesaj(ot, "Onaylanmadı", "Serbest bırakmak için müşteri numarasını doğru yazın."), 400)
        zorla = form.get("zorla") == "1"
        c = self.veri.serbest_birak(n, zorla)
        if c is None and not zorla:
            return yanit_html(self.mesaj(ot, "Cihaz yok", "Bu müşterinin bağlı cihazı yok."), 400)
        if zorla:
            self.kuyruk_ekle("cihaz-kapat", {"numara": str(n)})
            self.veri.hareket("admin", ip, "CIHAZ_ZORLA_AYRILDI", n, f"cihaz={c['id'] if c else '-'}")
        else:
            self.veri.hareket("admin", ip, "SERBEST_BIRAKMA_ISTENDI", n, f"cihaz={c['id']}")
        return yonlendir(f"/m/{n}")

    def _hareket_tablosu(self, hareketler):
        return detay._tablo(("Zaman", "Kim", "IP", "Olay", "Müşteri", "Ayrıntı"),
                                [(h["zaman"][:19], h["kim"], h["ip"], h["olay"], h["musteri"] or "", h["ayrinti"]) for h in hareketler])

    def eski_arsivler(self, ot, yontem, form, ip):
        durum, bagli = self._durum(), self.veri.eski_arsivler()
        adaylar = sorted(set(a for a in durum if not NUMARA_RE.fullmatch(a)) | set(bagli))
        if yontem == "POST":
            ad, mn = form.get("ad", ""), form.get("musteri", "").strip()
            if ad not in adaylar or (mn and (not NUMARA_RE.fullmatch(mn) or self.veri.musteri(int(mn)) is None)):
                return yanit_html(self.mesaj(ot, "Geçersiz", "Arşiv ya da müşteri numarası bulunamadı."), 400)
            self.veri.eski_arsiv_bagla(ad, int(mn) if mn else None)
            self.veri.hareket("admin", ip, "ESKI_ARSIV_BAGLANDI", int(mn) if mn else None, f"arsiv={ad}")
            return yonlendir("/eski-arsivler")
        cs = e(ot["csrf"])
        satirlar = "".join(
            f'<tr><td>{e(a)}</td><td>{e(durum.get(a, {}).get("tur", "—"))}</td><td>{e(durum.get(a, {}).get("son_gun") or "—")}</td>'
            f'<td><form method="post" action="/eski-arsivler" class="ara"><input type="hidden" name="csrf" value="{cs}">'
            f'<input type="hidden" name="ad" value="{e(a)}"><input name="musteri" value="{e(bagli.get(a) or "")}" '
            'placeholder="müşteri no (boş = bağsız)" inputmode="numeric"><button>Kaydet</button></form></td></tr>'
            for a in adaylar)
        tablo = ('<div class="kaydir"><table class="detay"><tr><th>Arşiv</th><th>Tür</th><th>Son gün</th><th>Bağlı müşteri</th></tr>'
                 f"{satirlar}</table></div>") if adaylar else '<p class="alt">Eski arşiv yok.</p>'
        return yanit_html(self.sayfa("Eski arşivler", kart(
            '<h1>Eski arşivler</h1><p class="alt">OpenWrt dönemi ve elle eklenmiş arşivler. Bir müşteriye bağlanırsa müşteri '
            f'kayıtlarında görünür.</p>{tablo}'), ot, genis=True))

    def hesabim(self, ot, yontem, form, ip):
        def html(hata="", tamam=""):
            m = f'<p class="hata">{e(hata)}</p>' if hata else (f'<p class="tamam">{e(tamam)}</p>' if tamam else "")
            return self.sayfa("Hesabım", kart(
                '<h1>Yönetici parolası</h1><form method="post" action="/hesabim">'
                f'<input type="hidden" name="csrf" value="{e(ot["csrf"])}">'
                '<label for="m">Mevcut parola</label><input id="m" name="mevcut" type="password" required>'
                '<label for="y">Yeni parola (en az 12)</label><input id="y" name="yeni" type="password" minlength="12" required>'
                '<label for="t">Yeni parola (tekrar)</label><input id="t" name="tekrar" type="password" minlength="12" required>'
                f'{m}<button>Değiştir</button></form>'), ot)
        if yontem == "GET":
            return yanit_html(html())
        if self.dogrula(ot["user"], form.get("mevcut", "")) is None:
            return yanit_html(html("Mevcut parola yanlış."), 400)
        if form.get("yeni", "") != form.get("tekrar", ""):
            return yanit_html(html("Yeni parolalar aynı değil."), 400)
        try:
            yonetici.yaz(self.yonetici_yolu, ot["user"], form.get("yeni", ""))
        except ValueError as h:
            return yanit_html(html(str(h)), 400)
        self.veri.hareket("admin", ip, "YONETICI_PAROLA_DEGISTI")
        return yanit_html(html(tamam="Parola değiştirildi."))
