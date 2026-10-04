"""Müşteri kayıtlarının görünümü (eski kafe paneli ekranları): son 30 gün sayıları, gün → kişi, arama.
Bir müşterinin birden çok arşivi olabilir (kendi numarası + bağlı eski arşivler); aynı gün iki arşivde varsa ilki geçerli.
Yalnızca root'un ürettiği özetleri okur (istatistik/<arşiv>.json, detay/<arşiv>/)."""
import datetime
import json
import os
import urllib.parse

import common
import detay
from detay import e

ISTATISTIK = "/var/lib/wificorrect/istatistik"
DETAY = "/var/lib/wificorrect/detay"


def _json(yol):
    try:
        with open(yol, encoding="utf-8") as f:
            return json.load(f)
    except (FileNotFoundError, NotADirectoryError, ValueError):
        return None


def son_30_gun(simdi):
    dun = datetime.datetime.fromtimestamp(simdi, common.TZ).date() - datetime.timedelta(days=1)
    return [dun - datetime.timedelta(days=i) for i in range(30)]


class Kayitlar:
    def __init__(self, istatistik=ISTATISTIK, detay_dizini=DETAY):
        self.istatistik, self.detay = istatistik, detay_dizini

    def sayilar(self, arsivler):
        out, guncel = {}, None
        for ad in arsivler:
            yol = os.path.join(self.istatistik, ad + ".json")
            v = _json(yol)
            if isinstance(v, dict):
                for g, n in v.items():
                    out.setdefault(g, n)
                guncel = max(guncel or 0, os.path.getmtime(yol))
        return out, guncel

    def gunler(self, arsivler):
        out = {}
        for ad in arsivler:
            try:
                adlar = os.listdir(os.path.join(self.detay, ad))
            except (FileNotFoundError, NotADirectoryError):
                continue
            for a in sorted(adlar):
                if a.endswith(".json") and detay.gecerli_gun(a[:-5]):
                    out.setdefault(a[:-5], ad)
        return out

    def ozet_html(self, baslik, arsivler, simdi, onek):
        sayilar, guncel = self.sayilar(arsivler)
        if not sayilar:
            tablo = '<p class="alt">Henüz veri yok. Cihazdan ilk gün geldiğinde burada görünecek.</p>'
        else:
            gunler = son_30_gun(simdi)
            enbuyuk = max([detay._int(sayilar.get(g.isoformat(), 0)) for g in gunler] + [1])
            satirlar = []
            for g in gunler:
                n = sayilar.get(g.isoformat())
                tarih = f"<td>{g:%d.%m}</td><td>{detay.KISA_GUN[g.weekday()]}</td>"
                if n is None:
                    satirlar.append(f'<tr>{tarih}<td class="cubuk"></td><td class="n bos">—</td></tr>')
                else:
                    n = detay._int(n)
                    satirlar.append(f'<tr>{tarih}<td class="cubuk"><span style="width:{n * 100 // enbuyuk}%"></span></td>'
                                    f'<td class="n">{n}</td></tr>')
            tablo = "<table>" + "".join(satirlar) + "</table>"
        son = (f'<p class="alt">Son güncelleme: {datetime.datetime.fromtimestamp(guncel, common.TZ):%d.%m.%Y %H:%M}</p>'
               if guncel else "")
        return (f'<div class="kart"><h1>{e(baslik)}</h1>'
                '<p class="alt">Son 30 gün · her gün internete çıkan farklı kişi sayısı</p></div>'
                f'<div class="kart">{tablo}{son}</div>'
                f'<div class="kart"><h2>Kişi ara</h2>{detay.arama_formu("", onek)}</div>'
                f'{detay.gunler_html(list(self.gunler(arsivler)), sayilar, onek)}')

    def gun_html(self, arsivler, gun, onek):
        if not detay.gecerli_gun(gun):
            return None
        ad = self.gunler(arsivler).get(gun)
        ozet = _json(os.path.join(self.detay, ad, gun + ".json")) if ad else None
        return detay.gun_icerik(gun, ozet, onek) if isinstance(ozet, dict) else None

    def ara_html(self, arsivler, sorgu, onek):
        q = (urllib.parse.parse_qs(sorgu).get("q") or [""])[0].strip()[:50]
        dizin = {}
        for ad in arsivler:
            d = _json(os.path.join(self.detay, ad, "kisiler.json"))
            for tel, k in (d.items() if isinstance(d, dict) else ()):
                if not isinstance(k, dict):
                    continue
                if tel in dizin:
                    dizin[tel]["gunler"] = sorted(set(dizin[tel]["gunler"]) | set(k.get("gunler") or []), reverse=True)
                else:
                    dizin[tel] = dict(k, gunler=list(k.get("gunler") or []))
        sonuclar = detay.ara_sonuclari(dizin, q) if len(q) >= 2 else None
        return detay.ara_icerik(q, sonuclar, onek)
