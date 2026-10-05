"""Kafe paneli detay ekranlarının HTML parçaları ve biçimlendirme (detay spec §3). Sayfa çerçevesi panel.py'de.
Özetlerdeki cihazdan gelen tüm metin (alan adı, MAC, ek, ad) güvenilmezdir: hepsi e() ile kaçışlı basılır."""
import datetime
import html
import re

GUN_RE = re.compile(r"\d{4}-\d{2}-\d{2}")
SAAT_RE = re.compile(r"\d\d:\d\d")
KISA_GUN = ("Pzt", "Sal", "Çar", "Per", "Cum", "Cmt", "Paz")
GUN_ADI = ("Pazartesi", "Salı", "Çarşamba", "Perşembe", "Cuma", "Cumartesi", "Pazar")
AYLAR = ("Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim", "Kasım", "Aralık")
OLAY = {"OTURUM_BASLA": "Giriş", "OTURUM_BITIS": "Çıkış", "OTURUM_IP_DEGISTI": "IP değişti"}
ARAMA_SINIRI, GUN_BAGLANTI_SINIRI = 50, 30


def e(s):
    return html.escape(str(s), quote=True)


def _int(n):
    try:
        return int(n)
    except (TypeError, ValueError):
        return 0


def gecerli_gun(s):
    if not isinstance(s, str) or not GUN_RE.fullmatch(s):
        return False
    try:
        datetime.date.fromisoformat(s)
    except ValueError:
        return False
    return True


def boyut(n):
    n = float(_int(n))
    for birim in ("B", "KB", "MB", "GB"):
        if n < 1024 or birim == "GB":
            return f"{n:.0f} B" if birim == "B" else f"{n:.1f} {birim}".replace(".", ",")
        n /= 1024


def saat(z):
    """ISO zamandan HH:MM; cihazdan gelen alan biçimsizse boş (çıktı hiçbir zaman işaretleme taşımaz)."""
    s = z[11:16] if isinstance(z, str) else ""
    return s if SAAT_RE.fullmatch(s) else ""


def sure(sn):
    sn = _int(sn)
    return f"{sn // 3600}:{sn % 3600 // 60:02d}:{sn % 60:02d}" if sn >= 3600 else f"{sn // 60}:{sn % 60:02d}"


def tarih(g):
    return f"{datetime.date.fromisoformat(g):%d.%m.%Y}" if gecerli_gun(g) else str(g)


def tr_kucuk(s):
    return str(s).replace("İ", "i").replace("I", "ı").lower()


def _ad(k):
    return f'{k.get("ad") or ""} {k.get("soyad") or ""}'.strip() or "(adı yok)"


def _tablo(basliklar, satirlar, bos="Kayıt yok."):
    if not satirlar:
        return f'<p class="alt">{e(bos)}</p>'
    bas = "".join(f"<th>{e(b)}</th>" for b in basliklar)
    govde = "".join("<tr>" + "".join(f"<td>{e(h)}</td>" for h in s) + "</tr>" for s in satirlar)
    return f'<div class="kaydir"><table class="detay"><tr>{bas}</tr>{govde}</table></div>'


def arama_formu(q="", onek=""):
    return (f'<form class="ara" method="get" action="{onek}/ara">'
            f'<input name="q" value="{e(q)}" maxlength="50" placeholder="Telefon ya da ad soyad" aria-label="Kişi ara">'
            '<button>Ara</button></form>')


def gunler_html(gunler, sayilar, onek=""):
    """Özeti olan günler aya göre açılır listelerde; en yeni ay açık. Geçersiz gün adları gösterilmez."""
    aylar = {}
    for g in sorted((g for g in gunler if gecerli_gun(g)), reverse=True):
        aylar.setdefault(g[:7], []).append(g)
    if not aylar:
        return ""
    parcalar = []
    for i, liste in enumerate(aylar.values()):
        ilk = datetime.date.fromisoformat(liste[0])
        maddeler = []
        for g in liste:
            d = datetime.date.fromisoformat(g)
            n = f' <span class="alt">· {_int(sayilar[g])} kişi</span>' if g in sayilar else ""
            maddeler.append(f'<li><a href="{onek}/gun/{g}">{d:%d.%m} {KISA_GUN[d.weekday()]}</a>{n}</li>')
        parcalar.append(f'<details{" open" if i == 0 else ""}><summary>{AYLAR[ilk.month - 1]} {ilk.year} '
                        f'<span class="alt">({len(liste)} gün)</span></summary><ul class="gunler">{"".join(maddeler)}</ul></details>')
    return '<div class="kart"><h2>Günler</h2>' + "".join(parcalar) + "</div>"


def kisi_html(k):
    tel = str(k.get("telefon") or "")
    siteler = [s for s in k.get("siteler") or [] if isinstance(s, dict)]
    baglantilar = [b for b in k.get("baglantilar") or [] if isinstance(b, dict)]
    diger, toplam_bag = _int(k.get("diger_site_sayisi")), _int(k.get("baglanti_sayisi"))
    oturum_t = _tablo(("Saat", "Olay", "İç IP", "Not"),
                      [(saat(o.get("zaman")), OLAY.get(o.get("olay"), o.get("olay") or ""), o.get("ic_ip") or "", o.get("ek") or "")
                       for o in k.get("oturumlar") or [] if isinstance(o, dict)],
                      "Bu gün oturum olayı yok (oturum daha önce başlamış).")
    site_t = _tablo(("Site", "Kaç kez", "İlk", "Son"),
                    [(s.get("alan") or "", _int(s.get("sayi")), saat(s.get("ilk")), saat(s.get("son"))) for s in siteler])
    if diger:
        site_t += f'<p class="alt">ve {diger} site daha</p>'
    bag_t = _tablo(("Saat", "Protokol", "Hedef", "Gönderilen", "İndirilen", "Süre"),
                   [(saat(b.get("zaman")), b.get("protokol") or "",
                     f'{b.get("hedef_ip") or ""}:{b.get("hedef_port") or ""}'.rstrip(":"),
                     boyut(b.get("gonderilen")), boyut(b.get("alinan")), sure(b.get("sure_sn"))) for b in baglantilar])
    toplam = _int(k.get("gonderilen")) + _int(k.get("alinan"))
    return (f'<details class="kisi" id="k{e(tel)}"><summary><b>{e(_ad(k))}</b> · {e(tel)} · '
            f'{saat(k.get("ilk"))}–{saat(k.get("son"))} · {boyut(toplam)}</summary><div>'
            f'<p class="alt">Cihaz: {e(", ".join(map(str, k.get("mac") or [])) or "—")} · {toplam_bag} bağlantı · '
            f'gönderilen {boyut(k.get("gonderilen"))}, indirilen {boyut(k.get("alinan"))}</p>'
            f'<h3>Oturum</h3>{oturum_t}'
            f'<details><summary>Girdiği siteler ({len(siteler) + diger})</summary><div>{site_t}</div></details>'
            f'<details><summary>Bağlantılar (en çok veri kullanan {len(baglantilar)} / toplam {toplam_bag})</summary>'
            f'<div>{bag_t}</div></details></div></details>')


def gun_icerik(gun, ozet, onek=""):
    d = datetime.date.fromisoformat(gun)
    kisiler = [k for k in ozet.get("kisiler") or [] if isinstance(k, dict)]
    liste = "".join(kisi_html(k) for k in kisiler) or '<p class="alt">Bu gün kayıt yok.</p>'
    return (f'<p><a href="{onek}/">← Özet</a></p><div class="kart"><h1>{d:%d.%m.%Y} {GUN_ADI[d.weekday()]}</h1>'
            f'<p class="alt">{len(kisiler)} kişi · ayrıntı için kişiye dokunun</p></div>{liste}')


def telefon_sorgusu(q):
    """Harf yoksa rakamlar alınır; baştaki 90 (ardından 5 gelirse) ya da 0 atılır: 0533…, +90 533… → 533…"""
    if any(ch.isalpha() for ch in q):
        return None
    d = re.sub(r"\D", "", q)
    if d.startswith("90") and d[2:3] == "5":
        d = d[2:]
    elif d.startswith("0"):
        d = d[1:]
    return d or None


def ara_sonuclari(dizin, q):
    """Telefon biçimindeyse telefonda, değilse ad soyadda (Türkçe büyük/küçük harf duyarsız) arar;
    en son geleni önce, 50 kişi."""
    rakam = telefon_sorgusu(q)

    def eslesir(tel, k):
        if rakam:
            return rakam in tel
        return tr_kucuk(q) in tr_kucuk(f'{k.get("ad") or ""} {k.get("soyad") or ""}')

    bulunan = [(tel, k) for tel, k in dizin.items() if isinstance(k, dict) and eslesir(str(tel), k)]
    bulunan.sort(key=lambda tk: max(tk[1].get("gunler") or [""]), reverse=True)
    return bulunan[:ARAMA_SINIRI]


def ara_icerik(q, sonuclar, onek=""):
    if sonuclar is None:
        govde = '<p class="alt">En az 2 karakter yazın.</p>'
    elif not sonuclar:
        govde = '<p class="alt">Sonuç yok.</p>'
    else:
        parcalar = []
        for tel, k in sonuclar:
            gunler = [g for g in k.get("gunler") or [] if gecerli_gun(g)]
            linkler = ", ".join(f'<a href="{onek}/gun/{g}#k{e(tel)}">{tarih(g)}</a>' for g in gunler[:GUN_BAGLANTI_SINIRI])
            fazla = len(gunler) - GUN_BAGLANTI_SINIRI
            if fazla > 0:
                linkler += f' <span class="alt">ve {fazla} gün daha</span>'
            parcalar.append(f'<div class="kart"><b>{e(_ad(k))}</b> · {e(tel)}<p>{linkler}</p></div>')
        govde = "".join(parcalar)
    return f'<p><a href="{onek}/">← Özet</a></p><div class="kart"><h1>Kişi ara</h1>{arama_formu(q, onek)}</div>{govde}'
