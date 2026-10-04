# Yönetim Merkezi — Sunucu Tarafı Uygulama Planı

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `yonetim.wificorrect.com` (hizmet sağlayıcı), `panel.wificorrect.com` (müşteri) ve `api.wificorrect.com`
(cihazlar) tek bir Python programıyla çalışsın; müşteri numarası + parola merkezde açılsın, cihaz ilk girişte kendini
bağlasın, tünel ve yedek hesabı root kuyruğuyla kendiliğinden açılsın.

**Architecture:** Mevcut kafe paneli (`openwrt-kafe-paneli/sunucu/panel`) `scripts/sunucu/merkez/`'e taşınır; özet
modülleri (`ozet`, `detay`, `istatistik`) korunur, üstüne SQLite veri katmanı, ortak web tabanı, üç uygulama (yönetim,
müşteri, API) ve `Host` başlığına göre yönlendiren tek süreç gelir. Root gerektiren her şey (tünel eşi, yedek hesabı,
arşiv okuma) ayrı root birimlerinde: `wc-kuyruk` (iş dosyaları), `wc-durum` (arşiv/tünel durumu), `wc-istatistik`
(gün özetleri). Panel süreci root olmaz, `/srv`'yi göremez, root da panelin veritabanına dokunmaz.

**Tech Stack:** Python 3 stdlib (`sqlite3`, `http.server`, `hashlib`), systemd (.path/.timer), Caddy, fail2ban, POSIX sh.

**Spec:** `docs/superpowers/specs/2026-10-04-yonetim-merkezi-design.md` (Görev 0'daki düzeltmelerle).
Cihaz (Rust) tarafı ayrı plandır: `docs/superpowers/plans/2026-10-04-yonetim-merkezi-cihaz.md` (bu plan bitince yazılır).

## Global Constraints

- Sunucu kodu yalnızca Python standart kütüphanesi; `subprocess` her zaman liste argümanla, `shell=True` yok.
- Parola özeti cihazla aynı: PBKDF2-HMAC-SHA256, tuz = metnin UTF-8 baytları, 120.000 tur, hex. Test vektörü:
  `ozet("parola-12345", "a1b2c3d4", 120000) == "ee38d08b3d6573ecc281263ad6259d2e7ee39f37d6b8958b3180906580a44495"`.
- Müşteri numarası 6 hane, 100001'den sırayla. Müşteri parolası ≥ 10, yönetici parolası ≥ 12 karakter.
- Üretilen parola 12 karakter, `0/O`, `1/l/I`, `8/B` yok.
- Panel süreci `wcpanel`, `/srv`'ye erişimi yok (`InaccessiblePaths=/srv`); root `wcpanel`'in dosyalarına yazmaz.
- Cihazdan / panelden gelen her dosya root için güvenilmez: sembolik bağ izlenmez, FIFO'da beklenmez, ≤ 4 KB.
- Tüm HTML çıktısı `html.escape`; tüm POST formlarında CSRF; çerezler `HttpOnly; Secure; SameSite=Strict`.
- Giriş kilidi: hesap ve IP başına 15 dk'da 5 deneme; API: IP başına dakikada 20 istek.
- Kullanıcıya görünen metin Türkçe; tanımlayıcılar Türkçe (mevcut sunucu kodunun dili).
- Testler: düz `assert`, `python scripts/sunucu/merkez/tests/test_x.py`; Windows'ta ve sunucuda çalışır.
- Sırlar (parolalar) repoya ve sohbete girmez; sudo ve yönetici parolasını kullanıcı kendi terminalinde girer.

## Review Focus

1. Aynı numarayla iki farklı cihaz giriş yaparsa yalnızca ilki bağlanır, ikincisi 409 alır → Görev 2 ve 6 testleri.
2. Kuyruk klasörüne sembolik bağ / FIFO / bozuk JSON / beklenmeyen adlı `.json` düşerse root işçisi takılmaz, döngüye girmez, hata sonucu yazar → Görev 3 testleri.
3. Cihaz kuyruk zaman aşımından sonra aynı anahtarla yeniden girerse bağlanır, yeni cihaz anahtarı alır, eskisi geçersizleşir → Görev 6 testleri.
4. Üyeliği biten müşteri merkezi panele girer ama cihaz API'sinden giremez; üyelik yeniden açılınca girer → Görev 6 ve 7 testleri.
5. Bilinmeyen `Host`, portlu `Host`, müşteri çerezinin yönetim alanında kullanılması → 404 / doğru uygulama / giriş sayfası → Görev 9 testleri.

## Dosya Haritası (`scripts/sunucu/merkez/`)

| Dosya | Sorumluluk |
|---|---|
| `common.py` | (kopya, değişmez) `TZ`, `now_iso`, `read_rows`, `save_json` |
| `ozet.py` | (kopya, değişmez) gün özetleri ve arama dizini |
| `detay.py` | (kopya + bağlantı öneki) gün/kişi/arama HTML parçaları |
| `istatistik.py` | `wc-istatistik` (root): arşivleri `durum.arsivler()`'den alır |
| `durum.py` | **yeni** `wc-durum` (root): arşiv listesi, tünel el sıkışması, son gün, boyut → `durum.json` |
| `kuyruk.py` | **yeni** panel tarafı `ekle/sonuc/bekle` + root `wc-kuyruk` |
| `guvenlik.py` | **yeni** parola özeti, oturumlar (+gerekçe), giriş kilidi, hız sınırı |
| `veri.py` | **yeni** SQLite: müşteri, cihaz, hareket, eski arşiv |
| `kayitlar.py` | **yeni** müşterinin arşivlerinden 30 gün / gün / arama sayfaları |
| `web.py` | **yeni** CSS, sayfa çerçevesi, çerez, giriş/çıkış/CSRF/gerekçe ortak tabanı (`Taban`) |
| `musteri.py` | **yeni** `panel.wificorrect.com` |
| `yonetim.py` | **yeni** `yonetim.wificorrect.com` |
| `yonetici.py` | **yeni** `wc-yonetici parola <ad>` ve yönetici dosyası yazımı |
| `api.py` | **yeni** `api.wificorrect.com`: `/api/giris`, `/api/eslesme`, `/api/parola` |
| `merkez.py` | **yeni** tek süreç, `Host`'a göre yönlendirme |
| `wc-*.service/.timer/.path`, `Caddyfile`, `fail2ban-*.conf`, `hotspot-arsiv-saklama`, `kur.sh` | kurulum |
| `tests/test_*.py` | testler |
| `../dagit.sh` | sunucuya kopyala + kur |
| `../wificorrect-sunucu` | + `cihaz-kapat`, `yonetici-parola`; `cihaz-ekle` biten kaydı yeniden açar; `liste`, `tasi-paketle` |

Yollar (sunucu): veritabanı `/var/lib/wificorrect/merkez/merkez.db`, yönetici `/var/lib/wificorrect/merkez/yonetici.json`
(klasör `wcpanel` 700); kuyruk `/var/lib/wificorrect/kuyruk` (`wcpanel` 700); sonuçlar `/var/lib/wificorrect/kuyruk-sonuc`
(`root:wcpanel` 750, dosyalar 640); `durum.json`, `istatistik/`, `detay/` (`root:wcpanel`).

---

### Task 0: Spec düzeltmeleri

**Files:** Modify: `docs/superpowers/specs/2026-10-04-yonetim-merkezi-design.md`

- [ ] **Step 1:** Şu farkları spec'e işle (her biri ilgili bölümde tek cümle):
  - §2/§4: Cihaz API'si `wificorrect.com/api/` değil **`api.wificorrect.com/api/`** (kök alan adı hosting firmasında, 185.106.208.2; dokunulmaz). DNS: `yonetim` ve `api` A kayıtları → <dış-ip>; kök kayda dokunulmaz.
  - §2/§3: Kuyruk tablo değil **dosya**: panel `/var/lib/wificorrect/kuyruk/<id>.json` yazar, `wc-kuyruk` (root, `.path` birimi) sonucu `/var/lib/wificorrect/kuyruk-sonuc/<id>.json`'a koyar. Root veritabanına hiç dokunmaz.
  - §2: Arşiv/tünel durumu `wc-durum` (root, 5 dk) → `/var/lib/wificorrect/durum.json`; `wc-istatistik` arşivleri bu listeden alır (müşteri listesinden değil).
  - §3: Yönetici dosyası `/var/lib/wificorrect/merkez/yonetici.json`; komut `wificorrect-sunucu yonetici-parola <ad>` (`wc-yonetici`'ye iner).
  - §5/§8: Eski arşivi müşteriye bağlama **yalnızca yönetim ekranından** (Eski arşivler); `eski-arsiv-bagla` komutu yok.
  - §5: "Çevrimiçi" = tünel el sıkışması ≤ 10 dk; son eşitleme 26 saatten eskiyse sorunlu (06:00 eşitlemesi en çok ~24 saat arayla gelir).
  - §9: Test cihazının (`bocafe-test`) tünelinin kapatılması cihaz planına taşınır (cihaz yeni sürüme geçmeden kapatılırsa yedeği durur).
- [ ] **Step 2:** Commit: `git commit -am "docs(yonetim-merkezi): plan sırasında netleşen API alan adı, dosya kuyruğu, durum birimi"`

---

### Task 1: Kodu taşı ve detay bağlantılarına önek ekle

**Files:**
- Create: `scripts/sunucu/merkez/{common,ozet,detay,istatistik}.py`, `scripts/sunucu/merkez/{wc-istatistik.service,wc-istatistik.timer,hotspot-arsiv-saklama}`
- Create: `scripts/sunucu/merkez/tests/{test_wc_ozet,test_wc_detay,test_wc_istatistik}.py`
- Modify: `scripts/sunucu/merkez/detay.py`

**Interfaces:**
- Produces: `detay.arama_formu(q="", onek="")`, `detay.gunler_html(gunler, sayilar, onek="")`,
  `detay.gun_icerik(gun, ozet, onek="")`, `detay.ara_icerik(q, sonuclar, onek="")` — bütün iç bağlantılar `onek` ile başlar.

- [ ] **Step 1: Kopyala**

```bash
S=/c/Users/HUAWEI/Desktop/openwrt-kafe-paneli; R=/c/Users/HUAWEI/Desktop/wificorrect/rza/scripts/sunucu/merkez
mkdir -p $R/tests
cp $S/sunucu/panel/{ozet,detay,istatistik}.py $S/sunucu/panel/{wc-istatistik.service,wc-istatistik.timer} $S/sunucu/hotspot-arsiv-saklama $R/
cp /c/Users/HUAWEI/Desktop/openwrt/files/usr/lib/hotspot/common.py $R/
cp $S/tests/test_wc_{ozet,detay,istatistik}.py $R/tests/
```

Her test dosyasında iki `sys.path.insert` satırını tek satırla değiştir: `sys.path.insert(0, os.path.join(HERE, ".."))`.
`test_wc_istatistik.py`'deki `import panel_auth` satırını ve `test_main_hesabi_olan_kafeleri_isler`,
`test_guvensiz_hesap_adi_ve_bozuk_kafe_digerlerini_durdurmaz` testlerini sil (Görev 4 yenisini yazar).

- [ ] **Step 2: Önek testi** — `tests/test_wc_detay.py` sonuna (ana bloktan önce):

```python
def test_onek_butun_baglantilarda():
    on = "/m/100001/kayitlar"
    assert f'action="{on}/ara"' in D.arama_formu("x", on)
    assert f'href="{on}/gun/2026-09-30"' in D.gunler_html(["2026-09-30"], {}, on)
    assert f'href="{on}/"' in D.gun_icerik("2026-09-30", {"kisiler": []}, on)
    s = D.ara_icerik("ay", [("5330000001", {"ad": "Ayşe", "gunler": ["2026-09-30"]})], on)
    assert f'href="{on}/gun/2026-09-30#k5330000001"' in s and f'href="{on}/"' in s and f'action="{on}/ara"' in s
    assert 'href="/gun/' not in s and 'href="/"' not in s
```

(Dosyada modül `D` adıyla içe aktarılmıyorsa — ör. `import detay as X` — `D.` yerine o adı kullan.)

- [ ] **Step 3: Run** `python scripts/sunucu/merkez/tests/test_wc_detay.py` → FAIL (`onek` parametresi yok).

- [ ] **Step 4: Implement** — `detay.py`:

```python
def arama_formu(q="", onek=""):
    return (f'<form class="ara" method="get" action="{onek}/ara">'
            f'<input name="q" value="{e(q)}" maxlength="50" placeholder="Telefon ya da ad soyad" aria-label="Kişi ara">'
            '<button>Ara</button></form>')
```
`gunler_html(gunler, sayilar, onek="")`: `f'<li><a href="{onek}/gun/{g}">…'`.
`gun_icerik(gun, ozet, onek="")`: `f'<p><a href="{onek}/">← Özet</a></p>…'`.
`ara_icerik(q, sonuclar, onek="")`: `f'<a href="{onek}/gun/{g}#k{e(tel)}">'`, `f'<p><a href="{onek}/">← Özet</a></p>'`,
`arama_formu(q, onek)`.

- [ ] **Step 5: Run** üç test dosyasını → `test_wc_detay`, `test_wc_ozet` geçer; `test_wc_istatistik` geçer (main testleri silindi).
- [ ] **Step 6: Commit**

```bash
git add scripts/sunucu/merkez && git commit -m "Yönetim merkezi: özet/detay/istatistik eski panelden taşındı; detay bağlantılarına önek"
```

---

### Task 2: guvenlik.py ve veri.py

**Files:**
- Create: `scripts/sunucu/merkez/guvenlik.py`, `scripts/sunucu/merkez/veri.py`
- Test: `scripts/sunucu/merkez/tests/test_guvenlik.py`, `scripts/sunucu/merkez/tests/test_veri.py`

**Interfaces:**
- Produces (guvenlik): `YINELEME=120000`, `EN_AZ=10`, `ozet(pw, tuz, yineleme)->str`, `yeni_kayit(pw)->(tuz, ozet, yineleme)`,
  `dogru(pw, tuz, ozet, yineleme)->bool`, `bos_dogrulama(pw)`, `parola_uret(n=12)->str`,
  `Oturumlar.create(user, role, now)->token`, `.get(token, now)->dict|None` (anahtarlar `user, role, csrf, gerekce`),
  `.drop(token)`, `.drop_user(user)`, `.set_gerekce(token, metin, bitis)`,
  `GirisKilidi.attempt(ip, user, now)->bool`, `.succeed(ip, user, now)`, `HizSiniri(limit, pencere).izin(anahtar, now)->bool`
- Produces (veri): `Veri(yol, saat=time.time)`, `BaskaCihaz`, `anahtar_ozeti(str)->str`, yöntemler:
  `musteri_ekle(not_="")->(numara, parola)`, `musteri(n)->Row|None`, `musteriler()->list[Row]`,
  `parola_dogrula(n, pw)->Row|None`, `parola_koy(n, pw)->(tuz, ozet, yineleme)` (ValueError), `parola_sifirla(n)->str`,
  `giris_kaydet(n)`, `uyelik(n, "aktif"|"bitti")`, `bagli_cihaz(n)->Row|None`,
  `cihaz_bagla(n, wg_pub, ssh_pub, isletme_adi="", unvan="", surum="")->(cihaz_id, anahtar)` (BaskaCihaz),
  `cihaz_anahtarla(anahtar)->Row|None`, `cihaz_guncelle(cid, **alanlar)`, `eslesme_kaydet(cid, isletme_adi, unvan, surum)`,
  `serbest_birak(n, zorla=False)->Row|None`, `temizlendi(cid)`, `cihazlar()->{n: Row}`, `cihaz_gecmisi(n)->list[Row]`,
  `hareket(kim, ip, olay, musteri=None, ayrinti="")`, `hareketler(musteri=None, limit=300)->list[Row]`,
  `eski_arsiv_bagla(ad, n|None)`, `eski_arsivler()->{ad: n|None}`, `arsivler(n)->list[str]`

- [ ] **Step 1: Failing tests** — `tests/test_guvenlik.py`:

```python
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))
import guvenlik as G  # noqa: E402


def test_ozet_cihazla_ayni():
    assert G.ozet("parola-12345", "a1b2c3d4", 120000) == "ee38d08b3d6573ecc281263ad6259d2e7ee39f37d6b8958b3180906580a44495"


def test_yeni_kayit_ve_dogrulama():
    tuz, oz, y = G.yeni_kayit("parola-12345")
    assert len(tuz) == 32 and y == 120000 and G.dogru("parola-12345", tuz, oz, y)
    assert not G.dogru("parola-12346", tuz, oz, y) and not G.dogru(None, tuz, oz, y)


def test_parola_uret_karisan_harf_yok():
    for _ in range(200):
        p = G.parola_uret()
        assert len(p) == 12 and not set(p) & set("0O1lI8B")


def test_oturum_gerekce_ve_sure():
    s = G.Oturumlar()
    t = s.create("100001", "musteri", 100.0)
    assert s.get(t, 100.0)["gerekce"] is None
    s.set_gerekce(t, "Müşteri şikâyeti", 1900.0)
    assert s.get(t, 101.0)["gerekce"] == ("Müşteri şikâyeti", 1900.0)
    assert s.get(t, 101.0 + 1801) is None  # boşta 30 dk


def test_kilit_ve_hiz():
    k = G.GirisKilidi()
    assert all(k.attempt("1.1.1.1", "100001", 0) for _ in range(5))
    assert not k.attempt("1.1.1.1", "100001", 1) and not k.attempt("2.2.2.2", "100001", 1)  # hesap kilitli
    assert k.attempt("2.2.2.2", "100002", 1)
    assert k.attempt("1.1.1.1", "100001", 901)  # 15 dk sonra açılır
    h = G.HizSiniri(3, 60)
    assert [h.izin("ip", t) for t in (0, 1, 2, 3)] == [True, True, True, False]
    assert h.izin("ip", 61)


if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
```

`tests/test_veri.py`:

```python
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
    with tempfile.TemporaryDirectory() as tmp:
        v = yeni(tmp)
        n1, p1 = v.musteri_ekle("Bocafe, Göztepe")
        n2, _ = v.musteri_ekle()
        assert (n1, n2) == (100001, 100002) and len(p1) == 12
        assert v.parola_dogrula(n1, p1)["not_"] == "Bocafe, Göztepe"
        assert v.parola_dogrula(n1, "yanlis") is None and v.parola_dogrula(999999, p1) is None
        assert v.parola_dogrula("100001", p1) is None  # numara int olmalı
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
    with tempfile.TemporaryDirectory() as tmp:
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
    with tempfile.TemporaryDirectory() as tmp:
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
    with tempfile.TemporaryDirectory() as tmp:
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
    with tempfile.TemporaryDirectory() as tmp:
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
```

- [ ] **Step 2: Run** iki dosya → FAIL (modüller yok).

- [ ] **Step 3: Implement `guvenlik.py`**

```python
"""Yönetim merkezi güvenliği (spec §3, §10): parola özetleri (cihazla aynı biçim), oturumlar, giriş kilidi, hız sınırı."""
import collections
import hashlib
import hmac
import secrets
import threading

YINELEME = 120_000  # Rust hesap.rs ITER ile aynı: cihaz aynı özeti internetsiz doğrular
EN_AZ = 10
HARFLER = "abcdefghjkmnpqrstuvwxyzACDEFGHJKLMNPQRSTUVWXYZ2345679"  # 0/O, 1/l/I, 8/B karışmasın


def ozet(pw, tuz, yineleme):
    """PBKDF2-HMAC-SHA256; tuz metnin baytları (Rust: salt.as_bytes()), hex."""
    return hashlib.pbkdf2_hmac("sha256", pw.encode("utf-8"), tuz.encode("utf-8"), int(yineleme)).hex()


def yeni_kayit(pw):
    tuz = secrets.token_hex(16)
    return tuz, ozet(pw, tuz, YINELEME), YINELEME


def dogru(pw, tuz, oz, yineleme):
    return hmac.compare_digest(ozet(pw or "", tuz, yineleme), oz)


def bos_dogrulama(pw):
    """Olmayan hesapta da aynı süre geçsin (numara ya da kullanıcı adı yanıt süresinden anlaşılmasın)."""
    ozet(pw or "", "0" * 32, YINELEME)


def parola_uret(n=12):
    return "".join(secrets.choice(HARFLER) for _ in range(n))


class Oturumlar:
    """Bellek içi oturumlar: boşta 30 dk, en çok 12 saat. Servis yeniden başlarsa herkes yeniden girer."""

    def __init__(self, bosta=1800, en_cok=43200):
        self.bosta, self.en_cok = bosta, en_cok
        self._s = {}
        self.lock = threading.Lock()

    def create(self, user, role, now):
        token = secrets.token_urlsafe(32)
        with self.lock:
            self._s[token] = {"user": user, "role": role, "csrf": secrets.token_urlsafe(24),
                              "created": now, "last": now, "gerekce": None}
        return token

    def get(self, token, now):
        with self.lock:
            s = self._s.get(token or "")
            if s is None:
                return None
            if now - s["last"] > self.bosta or now - s["created"] > self.en_cok:
                del self._s[token]
                return None
            s["last"] = now
            return dict(s)

    def drop(self, token):
        with self.lock:
            self._s.pop(token or "", None)

    def drop_user(self, user):
        with self.lock:
            for t in [t for t, s in self._s.items() if s["user"] == user]:
                del self._s[t]

    def set_gerekce(self, token, metin, bitis):
        with self.lock:
            s = self._s.get(token or "")
            if s is not None:
                s["gerekce"] = (metin, bitis)


class GirisKilidi:
    """IP başına ve hesap başına kayan pencere: `limit` deneme `pencere` sn içinde → kilit. Deneme doğrulamadan ÖNCE
    sayılır; başarılı giriş yalnızca kendi denemesini ve o hesabın sayacını siler."""

    def __init__(self, limit=5, pencere=900):
        self.limit, self.pencere = limit, pencere
        self._f = {}
        self.lock = threading.Lock()

    def _son(self, k, now):
        self._f[k] = [t for t in self._f.get(k, []) if t > now - self.pencere]
        return self._f[k]

    def attempt(self, ip, user, now):
        keys = (("ip", ip), ("user", user))
        with self.lock:
            if any(len(self._son(k, now)) >= self.limit for k in keys):
                return False
            for k in keys:
                self._f[k].append(now)
            return True

    def succeed(self, ip, user, now):
        with self.lock:
            if now in self._f.get(("ip", ip), []):
                self._f[("ip", ip)].remove(now)
            self._f.pop(("user", user), None)


class HizSiniri:
    """Anahtar başına kayan pencere (API: IP başına dakikada 20 istek).
    ponytail: anahtarlar bellekten silinmez; çok sayıda farklı IP'de periyodik temizlik eklenir."""

    def __init__(self, limit=20, pencere=60):
        self.limit, self.pencere = limit, pencere
        self._z = collections.defaultdict(collections.deque)
        self.lock = threading.Lock()

    def izin(self, anahtar, now):
        with self.lock:
            q = self._z[anahtar]
            while q and q[0] <= now - self.pencere:
                q.popleft()
            if len(q) >= self.limit:
                return False
            q.append(now)
            return True
```

- [ ] **Step 4: Implement `veri.py`**

```python
"""Yönetim merkezi veritabanı (spec §3): müşteriler, cihazlar, hareketler, eski arşiv bağları.
SQLite; tek bağlantı + kilit (ThreadingHTTPServer iş parçacıkları için). Yalnızca panel (wcpanel) açar, root açmaz."""
import contextlib
import hashlib
import secrets
import sqlite3
import threading
import time

import common
import guvenlik

ILK_NUMARA = 100001
SEMA = """
CREATE TABLE IF NOT EXISTS musteri (
  numara INTEGER PRIMARY KEY, tuz TEXT NOT NULL, ozet TEXT NOT NULL, yineleme INTEGER NOT NULL,
  not_ TEXT NOT NULL DEFAULT '', uyelik TEXT NOT NULL DEFAULT 'aktif', bitis TEXT NOT NULL DEFAULT '',
  olusturma TEXT NOT NULL, son_giris TEXT NOT NULL DEFAULT '');
CREATE TABLE IF NOT EXISTS cihaz (
  id INTEGER PRIMARY KEY AUTOINCREMENT, musteri INTEGER NOT NULL REFERENCES musteri(numara),
  durum TEXT NOT NULL, tunel_ip TEXT NOT NULL DEFAULT '', wg_pub TEXT NOT NULL, ssh_pub TEXT NOT NULL,
  anahtar_ozet TEXT NOT NULL, isletme_adi TEXT NOT NULL DEFAULT '', unvan TEXT NOT NULL DEFAULT '',
  surum TEXT NOT NULL DEFAULT '', son_eslesme TEXT NOT NULL DEFAULT '', baglanma TEXT NOT NULL,
  ayrilma TEXT NOT NULL DEFAULT '');
CREATE UNIQUE INDEX IF NOT EXISTS cihaz_tek ON cihaz(musteri) WHERE durum != 'serbest';
CREATE TABLE IF NOT EXISTS hareket (
  id INTEGER PRIMARY KEY AUTOINCREMENT, zaman TEXT NOT NULL, kim TEXT NOT NULL, ip TEXT NOT NULL,
  olay TEXT NOT NULL, musteri INTEGER, ayrinti TEXT NOT NULL DEFAULT '');
CREATE INDEX IF NOT EXISTS hareket_musteri ON hareket(musteri, id);
CREATE TABLE IF NOT EXISTS eski_arsiv (ad TEXT PRIMARY KEY, musteri INTEGER REFERENCES musteri(numara));
"""
CIHAZ_ALANLARI = {"tunel_ip", "isletme_adi", "unvan", "surum", "son_eslesme"}


class BaskaCihaz(Exception):
    """Numaranın başka (serbest bırakılmamış) cihazı var ya da bu cihaz başka müşteriye bağlı."""


def anahtar_ozeti(anahtar):
    return hashlib.sha256(anahtar.encode("utf-8")).hexdigest()


class Veri:
    def __init__(self, yol, saat=time.time):
        self.saat = saat
        self.lock = threading.Lock()
        self.db = sqlite3.connect(yol, check_same_thread=False, isolation_level=None)
        self.db.row_factory = sqlite3.Row
        self.db.execute("PRAGMA foreign_keys = ON")
        self.db.executescript(SEMA)

    def _simdi(self):
        return common.now_iso(self.saat())

    @contextlib.contextmanager
    def _islem(self):
        """Kilitli tek işlem; içinde self._bir/_hepsi çağrılmaz (kilit yeniden girilmez), db doğrudan kullanılır."""
        with self.lock:
            self.db.execute("BEGIN IMMEDIATE")
            try:
                yield self.db
            except BaseException:
                self.db.execute("ROLLBACK")
                raise
            self.db.execute("COMMIT")

    def _bir(self, sql, *a):
        with self.lock:
            return self.db.execute(sql, a).fetchone()

    def _hepsi(self, sql, *a):
        with self.lock:
            return self.db.execute(sql, a).fetchall()

    # --- müşteriler ---
    def musteri_ekle(self, not_=""):
        pw = guvenlik.parola_uret()
        tuz, oz, y = guvenlik.yeni_kayit(pw)
        with self._islem() as db:
            son = db.execute("SELECT MAX(numara) FROM musteri").fetchone()[0]
            numara = max(son or 0, ILK_NUMARA - 1) + 1
            db.execute("INSERT INTO musteri (numara, tuz, ozet, yineleme, not_, olusturma) VALUES (?, ?, ?, ?, ?, ?)",
                       (numara, tuz, oz, y, (not_ or "").strip()[:200], self._simdi()))
        return numara, pw

    def musteri(self, numara):
        return self._bir("SELECT * FROM musteri WHERE numara = ?", numara)

    def musteriler(self):
        return self._hepsi("SELECT * FROM musteri ORDER BY numara")

    def parola_dogrula(self, numara, pw):
        m = self.musteri(numara) if type(numara) is int else None
        if m is None:
            guvenlik.bos_dogrulama(pw)
            return None
        return m if guvenlik.dogru(pw, m["tuz"], m["ozet"], m["yineleme"]) else None

    def parola_koy(self, numara, pw):
        if not isinstance(pw, str) or len(pw) < guvenlik.EN_AZ:
            raise ValueError(f"Parola en az {guvenlik.EN_AZ} karakter olmalı.")
        tuz, oz, y = guvenlik.yeni_kayit(pw)
        with self._islem() as db:
            db.execute("UPDATE musteri SET tuz = ?, ozet = ?, yineleme = ? WHERE numara = ?", (tuz, oz, y, numara))
        return tuz, oz, y

    def parola_sifirla(self, numara):
        pw = guvenlik.parola_uret()
        self.parola_koy(numara, pw)
        return pw

    def giris_kaydet(self, numara):
        with self._islem() as db:
            db.execute("UPDATE musteri SET son_giris = ? WHERE numara = ?", (self._simdi(), numara))

    def uyelik(self, numara, durum):
        if durum not in ("aktif", "bitti"):
            raise ValueError(durum)
        with self._islem() as db:
            db.execute("UPDATE musteri SET uyelik = ?, bitis = ? WHERE numara = ?",
                       (durum, self._simdi() if durum == "bitti" else "", numara))

    # --- cihazlar ---
    def bagli_cihaz(self, numara):
        return self._bir("SELECT * FROM cihaz WHERE musteri = ? AND durum != 'serbest'", numara)

    def cihaz_bagla(self, numara, wg_pub, ssh_pub, isletme_adi="", unvan="", surum=""):
        """Müşterinin serbest bırakılmamış cihazı yoksa bu cihazı bağlar; aynı cihaz (aynı wg_pub) yeniden denerse yeni
        anahtar verir (eskisi geçersizleşir). Dönen: (cihaz_id, cihaz_anahtari) — anahtar yalnızca burada açık görünür."""
        anahtar = secrets.token_urlsafe(32)
        with self._islem() as db:
            baska = db.execute("SELECT 1 FROM cihaz WHERE wg_pub = ? AND durum != 'serbest' AND musteri != ?",
                               (wg_pub, numara)).fetchone()
            c = db.execute("SELECT * FROM cihaz WHERE musteri = ? AND durum != 'serbest'", (numara,)).fetchone()
            if baska or (c is not None and (c["wg_pub"] != wg_pub or c["durum"] != "bagli")):
                raise BaskaCihaz()
            if c is not None:
                db.execute("UPDATE cihaz SET anahtar_ozet = ?, ssh_pub = ?, isletme_adi = ?, unvan = ?, surum = ? WHERE id = ?",
                           (anahtar_ozeti(anahtar), ssh_pub, isletme_adi, unvan, surum, c["id"]))
                return c["id"], anahtar
            cur = db.execute(
                "INSERT INTO cihaz (musteri, durum, wg_pub, ssh_pub, anahtar_ozet, isletme_adi, unvan, surum, baglanma) "
                "VALUES (?, 'bagli', ?, ?, ?, ?, ?, ?, ?)",
                (numara, wg_pub, ssh_pub, anahtar_ozeti(anahtar), isletme_adi, unvan, surum, self._simdi()))
            return cur.lastrowid, anahtar

    def cihaz_anahtarla(self, anahtar):
        if not isinstance(anahtar, str) or not anahtar:
            return None
        return self._bir("SELECT * FROM cihaz WHERE anahtar_ozet = ? AND durum != 'serbest'", anahtar_ozeti(anahtar))

    def cihaz_guncelle(self, cid, **alanlar):
        if not alanlar or not set(alanlar) <= CIHAZ_ALANLARI:
            raise ValueError(sorted(alanlar))
        with self._islem() as db:
            db.execute(f"UPDATE cihaz SET {', '.join(k + ' = ?' for k in alanlar)} WHERE id = ?", (*alanlar.values(), cid))

    def eslesme_kaydet(self, cid, isletme_adi, unvan, surum):
        self.cihaz_guncelle(cid, isletme_adi=isletme_adi, unvan=unvan, surum=surum, son_eslesme=self._simdi())

    def serbest_birak(self, numara, zorla=False):
        """Dönen: etkilenen cihaz satırı (yoksa None). Zorla: cihaz beklenmeden serbest (arızalı/kayıp cihaz)."""
        with self._islem() as db:
            c = db.execute("SELECT * FROM cihaz WHERE musteri = ? AND durum != 'serbest'", (numara,)).fetchone()
            if c is None:
                return None
            if zorla:
                db.execute("UPDATE cihaz SET durum = 'serbest', ayrilma = ? WHERE id = ?", (self._simdi(), c["id"]))
            else:
                db.execute("UPDATE cihaz SET durum = 'serbest_birakiliyor' WHERE id = ?", (c["id"],))
            return c

    def temizlendi(self, cid):
        with self._islem() as db:
            db.execute("UPDATE cihaz SET durum = 'serbest', ayrilma = ? WHERE id = ?", (self._simdi(), cid))

    def cihazlar(self):
        return {c["musteri"]: c for c in self._hepsi("SELECT * FROM cihaz WHERE durum != 'serbest'")}

    def cihaz_gecmisi(self, numara):
        return self._hepsi("SELECT * FROM cihaz WHERE musteri = ? ORDER BY id DESC", numara)

    # --- hareketler ---
    def hareket(self, kim, ip, olay, musteri=None, ayrinti=""):
        with self._islem() as db:
            db.execute("INSERT INTO hareket (zaman, kim, ip, olay, musteri, ayrinti) VALUES (?, ?, ?, ?, ?, ?)",
                       (self._simdi(), str(kim)[:64], str(ip)[:64], olay, musteri, str(ayrinti)[:500]))

    def hareketler(self, musteri=None, limit=300):
        if musteri is None:
            return self._hepsi("SELECT * FROM hareket ORDER BY id DESC LIMIT ?", limit)
        return self._hepsi("SELECT * FROM hareket WHERE musteri = ? ORDER BY id DESC LIMIT ?", musteri, limit)

    # --- eski arşivler ---
    def eski_arsiv_bagla(self, ad, numara):
        with self._islem() as db:
            db.execute("INSERT INTO eski_arsiv (ad, musteri) VALUES (?, ?) "
                       "ON CONFLICT(ad) DO UPDATE SET musteri = excluded.musteri", (ad, numara))

    def eski_arsivler(self):
        return {r["ad"]: r["musteri"] for r in self._hepsi("SELECT * FROM eski_arsiv")}

    def arsivler(self, numara):
        """Müşterinin arşiv adları: kendi numarası önce (aynı gün iki yerdeyse o geçerli), sonra bağlı eski arşivler."""
        return [str(numara)] + [r["ad"] for r in self._hepsi("SELECT ad FROM eski_arsiv WHERE musteri = ? ORDER BY ad", numara)]
```

- [ ] **Step 5: Run** iki test dosyası → `TUM TESTLER GECTI`.
- [ ] **Step 6: Commit** `git add scripts/sunucu/merkez && git commit -m "Yönetim merkezi: parola/oturum güvenliği ve SQLite veri katmanı"`

---

### Task 3: kuyruk.py (panel tarafı + root işçisi)

**Files:**
- Create: `scripts/sunucu/merkez/kuyruk.py`, `scripts/sunucu/merkez/wc-kuyruk.path`, `scripts/sunucu/merkez/wc-kuyruk.service`
- Test: `scripts/sunucu/merkez/tests/test_kuyruk.py`

**Interfaces:**
- Produces: `ekle(islem, veri, kuyruk=KUYRUK)->kimlik`, `sonuc(kimlik, sonuc_dizini=SONUC)->dict|None`,
  `bekle(kimlik, sure=15.0, aralik=0.5, sonuc_dizini=SONUC, uyku=time.sleep)->dict|None`,
  `isle(is_, calistir, kayit, wg_dir)->dict` (`{"durum": "tamam", "tunel_ip", "sunucu_pub", "uc_nokta"}` ya da
  `{"durum": "hata", "hata"}`), `main(kuyruk, sonuc_dizini, calistir, kayit, wg_dir, grup, simdi)->int`.
  `calistir(komut: list[str]) -> (kod: int, stdout: str, stderr: str)`.

- [ ] **Step 1: Failing tests** — `tests/test_kuyruk.py`:

```python
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
        kimlik = K.ekle("cihaz-ekle", {"numara": "100001"}, tmp)
        with open(os.path.join(tmp, kimlik + ".json"), encoding="utf-8") as f:
            assert json.load(f) == {"numara": "100001", "islem": "cihaz-ekle"}
        assert [a for a in os.listdir(tmp) if a.endswith(".tmp")] == []
        uykular = []
        assert K.bekle(kimlik, sure=1.0, aralik=0.5, sonuc_dizini=tmp, uyku=uykular.append) is None and len(uykular) == 3
        with open(os.path.join(tmp, kimlik + ".json"), "w", encoding="utf-8") as f:
            json.dump({"durum": "tamam"}, f)
        assert K.bekle(kimlik, sonuc_dizini=tmp, uyku=uykular.append) == {"durum": "tamam"}


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
                    {"islem": "kabuk", "numara": "100001"}):
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
        assert K.main(kq, sd, s, s.kayit, s.wg, grup=None, simdi=2_000_000_000.0) == 0
        assert os.listdir(kq) == []
        assert K.sonuc(iyi, sd)["tunel_ip"] == "10.99.0.10"
        assert K.sonuc("0123456789abcdef", sd)["durum"] == "hata" and K.sonuc("fedcba9876543210", sd)["durum"] == "hata"
        os.utime(os.path.join(sd, iyi + ".json"), (1, 1))  # 1 günden eski sonuç temizlenir
        K.main(kq, sd, s, s.kayit, s.wg, grup=None, simdi=2_000_000_000.0)
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


if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
```

- [ ] **Step 2: Run** → FAIL (modül yok).

- [ ] **Step 3: Implement `kuyruk.py`**

```python
#!/usr/bin/env python3
"""Root işleri kuyruğu (spec §2). Panel (wcpanel) iş dosyası yazar ve sonucu bekler; wc-kuyruk (root, systemd .path ile
anında) işi doğrulayıp wificorrect-sunucu ile yapar, sonucu yalnızca root'un yazabildiği klasöre koyar.
Panelden gelen hiçbir metin kabuğa verilmez: alanlar düzenli ifadeyle doğrulanır, komutlar liste argümanla çalışır."""
import json
import os
import re
import secrets
import shutil
import stat
import subprocess
import sys
import time

KUYRUK = "/var/lib/wificorrect/kuyruk"
SONUC = "/var/lib/wificorrect/kuyruk-sonuc"
SUNUCU = "/usr/local/sbin/wificorrect-sunucu"
KAYIT = "/etc/wireguard/wificorrect/kayit.csv"
WG_DIR = "/etc/wireguard/wificorrect"
NUMARA_RE = re.compile(r"[1-9]\d{5}")
WG_RE = re.compile(r"[A-Za-z0-9+/]{43}=")
SSH_RE = re.compile(r"ssh-ed25519 [A-Za-z0-9+/]+=*( [A-Za-z0-9@._-]+)?")
IS_RE = re.compile(r"[0-9a-f]{16}\.json")


# --- panel tarafı (wcpanel) ---
def ekle(islem, veri, kuyruk=KUYRUK):
    kimlik = secrets.token_hex(8)
    gecici = os.path.join(kuyruk, f".{kimlik}.tmp")  # .path birimi yalnızca *.json görür
    with open(gecici, "w", encoding="utf-8") as f:
        json.dump(dict(veri, islem=islem), f)
    os.replace(gecici, os.path.join(kuyruk, kimlik + ".json"))
    return kimlik


def sonuc(kimlik, sonuc_dizini=SONUC):
    try:
        with open(os.path.join(sonuc_dizini, kimlik + ".json"), encoding="utf-8") as f:
            v = json.load(f)
    except (FileNotFoundError, ValueError):
        return None
    return v if isinstance(v, dict) else None


def bekle(kimlik, sure=15.0, aralik=0.5, sonuc_dizini=SONUC, uyku=time.sleep):
    for _ in range(int(sure / aralik) + 1):
        s = sonuc(kimlik, sonuc_dizini)
        if s is not None:
            return s
        uyku(aralik)
    return None


# --- root tarafı (wc-kuyruk) ---
def is_oku(yol):
    """İş dosyası güvenilmez (wcpanel'in klasöründe): sembolik bağ izlenmez, FIFO'da beklenmez, en çok 4 KB."""
    try:
        fd = os.open(yol, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0))
    except OSError:
        return None
    try:
        st = os.fstat(fd)
        if not stat.S_ISREG(st.st_mode) or st.st_size > 4096:
            return None
        ham = os.read(fd, 4097)
    finally:
        os.close(fd)
    try:
        v = json.loads(ham.decode("utf-8"))
    except ValueError:
        return None
    return v if isinstance(v, dict) else None


def kayit_satiri(ad, kayit=KAYIT):
    """kayit.csv'deki aktif cihaz satırı: [tur, ad, ip, wg, yedek-kullanıcısı] ya da None."""
    try:
        with open(kayit, encoding="utf-8") as f:
            for s in f.read().splitlines():
                p = s.split(";")
                if len(p) >= 5 and p[0] == "cihaz" and p[1] == ad:
                    return p
    except FileNotFoundError:
        pass
    return None


def _oku(yol):
    with open(yol, encoding="utf-8") as f:
        return f.read().strip()


def _hata(metin):
    return {"durum": "hata", "hata": (metin or "bilinmeyen hata").strip()[:300]}


def isle(is_, calistir, kayit=KAYIT, wg_dir=WG_DIR):
    numara = is_.get("numara")
    if not isinstance(numara, str) or not NUMARA_RE.fullmatch(numara):
        return _hata("gecersiz numara")
    if is_.get("islem") == "cihaz-kapat":
        if kayit_satiri(numara, kayit):
            kod, _, hata = calistir([SUNUCU, "cihaz-kapat", numara])
            if kod != 0:
                return _hata(hata)
        return {"durum": "tamam"}
    if is_.get("islem") != "cihaz-ekle":
        return _hata("bilinmeyen islem")
    wg, ssh = is_.get("wg_pub"), is_.get("ssh_pub")
    if not (isinstance(wg, str) and WG_RE.fullmatch(wg) and isinstance(ssh, str) and SSH_RE.fullmatch(ssh)):
        return _hata("gecersiz anahtar")
    s = kayit_satiri(numara, kayit)
    if s and s[3] != wg:  # müşterinin eski cihazı hâlâ tünelde: önce kapat
        kod, _, hata = calistir([SUNUCU, "cihaz-kapat", numara])
        if kod != 0:
            return _hata(hata)
        s = None
    if s is None:
        kod, _, hata = calistir([SUNUCU, "cihaz-ekle", numara, wg, ssh])
        s = kayit_satiri(numara, kayit)
        if kod != 0 or s is None:
            return _hata(hata or "cihaz eklenemedi")
    return {"durum": "tamam", "tunel_ip": s[2], "sunucu_pub": _oku(os.path.join(wg_dir, "sunucu.pub")),
            "uc_nokta": _oku(os.path.join(wg_dir, "uc-nokta"))}


def _calistir(komut):
    try:
        r = subprocess.run(komut, capture_output=True, text=True, timeout=120)
        return r.returncode, r.stdout, r.stderr
    except (OSError, subprocess.SubprocessError) as e:
        return 1, "", repr(e)


def main(kuyruk=KUYRUK, sonuc_dizini=SONUC, calistir=_calistir, kayit=KAYIT, wg_dir=WG_DIR, grup="wcpanel", simdi=None):
    simdi = time.time() if simdi is None else simdi
    for ad in sorted(os.listdir(kuyruk)):
        yol = os.path.join(kuyruk, ad)
        if not ad.endswith(".json"):
            continue
        if not IS_RE.fullmatch(ad):  # .path birimi her *.json'da tetiklenir: tanınmayanı sil, döngü olmasın
            os.unlink(yol)
            continue
        is_ = is_oku(yol)
        s = isle(is_, calistir, kayit, wg_dir) if is_ is not None else _hata("is okunamadi")
        hedef = os.path.join(sonuc_dizini, ad)
        with open(hedef + ".tmp", "w", encoding="utf-8") as f:
            json.dump(s, f)
        os.chmod(hedef + ".tmp", 0o640)
        if grup:
            shutil.chown(hedef + ".tmp", group=grup)
        os.replace(hedef + ".tmp", hedef)
        os.unlink(yol)
        print(f"{ad}: {(is_ or {}).get('islem', '?')} {s['durum']} {s.get('hata', '')}", file=sys.stderr)
    for ad in os.listdir(sonuc_dizini):  # ponytail: okunmuş/okunmamış ayrılmaz; 1 günden eski sonuçlar silinir
        y = os.path.join(sonuc_dizini, ad)
        if os.path.getmtime(y) < simdi - 86400:
            os.unlink(y)
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 4: Birimler**

`wc-kuyruk.path`:
```ini
[Unit]
Description=WifiCorrect yönetim merkezi root işleri (tünel/yedek hesabı) kuyruğu

[Path]
PathExistsGlob=/var/lib/wificorrect/kuyruk/*.json
Unit=wc-kuyruk.service

[Install]
WantedBy=multi-user.target
```
`wc-kuyruk.service`:
```ini
[Unit]
Description=WifiCorrect yönetim merkezi root işleri

[Service]
Type=oneshot
Environment=PYTHONDONTWRITEBYTECODE=1
ExecStart=/usr/local/sbin/wc-kuyruk
TimeoutStartSec=10min
```

- [ ] **Step 5: Run** `python scripts/sunucu/merkez/tests/test_kuyruk.py` → geçer.
- [ ] **Step 6: Commit** `git add scripts/sunucu/merkez && git commit -m "Yönetim merkezi: root işleri için dosya kuyruğu (wc-kuyruk)"`

---

### Task 4: durum.py (wc-durum) ve istatistik'in arşiv listesi

**Files:**
- Create: `scripts/sunucu/merkez/durum.py`, `scripts/sunucu/merkez/wc-durum.service`, `scripts/sunucu/merkez/wc-durum.timer`
- Modify: `scripts/sunucu/merkez/istatistik.py` (`main`), `scripts/sunucu/merkez/hotspot-arsiv-saklama`
- Test: `scripts/sunucu/merkez/tests/test_durum.py`, `scripts/sunucu/merkez/tests/test_wc_istatistik.py`

**Interfaces:**
- Produces: `durum.KAYIT`, `durum.YENI="/srv/wificorrect-arsiv"`, `durum.ESKI="/srv/hotspot-arsiv"`, `durum.DURUM`, `durum.AD_RE`,
  `durum.arsivler(kayit, yeni, eski)->{ad: {"tur": "cihaz"|"bitti"|"eski", "pub": str, "kaynaklar": [klasör]}}`,
  `durum.el_sikismalari(metin)->{pub: epoch}`, `durum.son_gun(kaynaklar)->str|None`, `durum.boyut(kaynaklar)->int`,
  `durum.main(kayit, yeni, eski, cikti, wg=None, simdi=None, grup="wcpanel")` → `durum.json`:
  `{"zaman": iso, "arsivler": {ad: {"tur", "son_gun", "boyut", "el_sikisma"}}}`
- Produces: `istatistik.main(kayit=None, yeni=None, eski=None, cikti=CIKTI, detay=DETAY, grup=PANEL_GRUBU)->int`

- [ ] **Step 1: Failing tests** — `tests/test_durum.py`:

```python
import json
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))
import durum as D  # noqa: E402

PUB = "A" * 43 + "="


def ortam(tmp):
    kayit = os.path.join(tmp, "kayit.csv")
    with open(kayit, "w", encoding="utf-8") as f:
        f.write(f"yonetici;pc;10.99.0.2;{PUB};\ncihaz;100001;10.99.0.10;{PUB};wfc-100001\n"
                f"bitti;bocafe-test;10.99.0.11;{'B' * 43}=;wfc-bocafe-test;2026-10-04\n")
    yeni, eski = os.path.join(tmp, "yeni"), os.path.join(tmp, "eski")
    os.makedirs(os.path.join(yeni, "100001", "veri", "gunluk", "2026-10-03"))
    os.makedirs(os.path.join(yeni, "bocafe-test", "veri", "gunluk", "2026-10-01"))
    os.makedirs(os.path.join(yeni, "..kotu", "veri"))
    os.makedirs(os.path.join(eski, "bocafe", "gunluk", "2026-09-20"))
    with open(os.path.join(yeni, "100001", "veri", "zincir.txt"), "w") as f:
        f.write("x" * 100)
    return kayit, yeni, eski


def test_arsivler():
    with tempfile.TemporaryDirectory() as tmp:
        kayit, yeni, eski = ortam(tmp)
        a = D.arsivler(kayit, yeni, eski)
        assert sorted(a) == ["100001", "bocafe", "bocafe-test"]
        assert a["100001"] == {"tur": "cihaz", "pub": PUB, "kaynaklar": [os.path.join(yeni, "100001", "veri")]}
        assert a["bocafe-test"]["tur"] == "bitti" and a["bocafe"]["tur"] == "eski"


def test_yardimcilar():
    assert D.el_sikismalari(f"{PUB}\t1790000000\nbozuk\n{'B' * 43}=\t0\n") == {PUB: 1790000000}
    with tempfile.TemporaryDirectory() as tmp:
        kayit, yeni, eski = ortam(tmp)
        k = [os.path.join(yeni, "100001", "veri")]
        assert D.son_gun(k) == "2026-10-03" and D.son_gun([os.path.join(tmp, "yok")]) is None
        assert D.boyut(k) == 100


def test_main():
    with tempfile.TemporaryDirectory() as tmp:
        kayit, yeni, eski = ortam(tmp)
        cikti = os.path.join(tmp, "durum.json")
        D.main(kayit, yeni, eski, cikti, wg=f"{PUB}\t1790000000\n", simdi=1_790_000_100.0, grup=None)
        with open(cikti, encoding="utf-8") as f:
            d = json.load(f)
        assert d["arsivler"]["100001"] == {"tur": "cihaz", "son_gun": "2026-10-03", "boyut": 100, "el_sikisma": 1790000000}
        assert d["arsivler"]["bocafe"]["el_sikisma"] is None and d["zaman"].startswith("2026-")


if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
```

`tests/test_wc_istatistik.py` sonuna (dosyadaki `gun_yaz` yardımcısıyla):

```python
def test_main_arsivleri_isler_yeni_once():
    with tempfile.TemporaryDirectory() as tmp:
        yeni, eski = os.path.join(tmp, "yeni"), os.path.join(tmp, "eski")
        kayit = os.path.join(tmp, "kayit.csv")
        with open(kayit, "w", encoding="utf-8") as f:
            f.write("cihaz;bocafe;10.99.0.10;" + "A" * 43 + "=;wfc-bocafe\n")
        gun_yaz(os.path.join(yeni, "bocafe", "veri"), "2026-10-01", ["5334553132"])
        gun_yaz(os.path.join(eski, "bocafe"), "2026-09-20", ["5334553132", "5551112233"])
        gun_yaz(os.path.join(eski, "bocafe"), "2026-10-01", ["5550000000", "5551111111", "5552222222"])
        cikti, detay = os.path.join(tmp, "ist"), os.path.join(tmp, "detay")
        assert I.main(kayit, yeni, eski, cikti, detay, None) == 0
        assert oku(os.path.join(cikti, "bocafe.json")) == {"2026-09-20": 2, "2026-10-01": 1}


def test_main_bozuk_arsiv_digerlerini_durdurmaz():
    with tempfile.TemporaryDirectory() as tmp:
        yeni, eski = os.path.join(tmp, "yeni"), os.path.join(tmp, "eski")
        kayit = os.path.join(tmp, "kayit.csv")
        open(kayit, "w").close()
        os.makedirs(os.path.join(eski, "a-kafe"))
        with open(os.path.join(eski, "a-kafe", "gunluk"), "w") as f:  # klasör olması gereken yerde dosya
            f.write("x")
        gun_yaz(os.path.join(eski, "b-kafe"), "2026-10-01", ["5334553132"])
        cikti, detay = os.path.join(tmp, "ist"), os.path.join(tmp, "detay")
        I.main(kayit, yeni, eski, cikti, detay, None)
        assert oku(os.path.join(cikti, "b-kafe.json")) == {"2026-10-01": 1}
```

- [ ] **Step 2: Run** iki dosya → FAIL.

- [ ] **Step 3: Implement `durum.py`**

```python
#!/usr/bin/env python3
"""wc-durum (root, 5 dk'da bir): arşivlerin ve tünelin durumu → /var/lib/wificorrect/durum.json (root:wcpanel 640).
Panel /srv'yi ve WireGuard'ı göremez; müşteri listesindeki son gelen gün, arşiv boyutu ve tünel bilgisi buradan gelir.
Root, panelin veritabanına dokunmaz: arşivler klasörlerden ve kayit.csv'den bulunur."""
import os
import re
import shutil
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))
import common  # noqa: E402

KAYIT = "/etc/wireguard/wificorrect/kayit.csv"
YENI = "/srv/wificorrect-arsiv"
ESKI = "/srv/hotspot-arsiv"
DURUM = "/var/lib/wificorrect/durum.json"
AD_RE = re.compile(r"[a-z0-9][a-z0-9._-]{0,31}")
GUN_RE = re.compile(r"\d{4}-\d{2}-\d{2}")


def _kayit(kayit):
    """kayit.csv → {arşiv adı: (tur, wg açık anahtarı)} (yalnızca cihaz/bitti satırları)."""
    out = {}
    try:
        with open(kayit, encoding="utf-8") as f:
            for s in f.read().splitlines():
                p = s.split(";")
                if len(p) >= 5 and p[0] in ("cihaz", "bitti") and p[4].startswith("wfc-"):
                    out[p[4][4:]] = (p[0], p[3])
    except FileNotFoundError:
        pass
    return out


def arsivler(kayit=KAYIT, yeni=YENI, eski=ESKI):
    """{ad: {tur, pub, kaynaklar}}: yeni arşiv (cihaz yedekleri) önce; aynı ad eski arşivde de varsa ikinci kaynak olur."""
    k, out = _kayit(kayit), {}
    if os.path.isdir(yeni):
        for ad in sorted(os.listdir(yeni)):
            d = os.path.join(yeni, ad, "veri")
            if AD_RE.fullmatch(ad) and not os.path.islink(os.path.join(yeni, ad)) and os.path.isdir(d):
                tur, pub = k.get(ad, ("bitti", ""))
                out[ad] = {"tur": tur, "pub": pub, "kaynaklar": [d]}
    if os.path.isdir(eski):
        for ad in sorted(os.listdir(eski)):
            d = os.path.join(eski, ad)
            if AD_RE.fullmatch(ad) and not os.path.islink(d) and os.path.isdir(d):
                out.setdefault(ad, {"tur": "eski", "pub": "", "kaynaklar": []})["kaynaklar"].append(d)
    return out


def el_sikismalari(metin):
    out = {}
    for s in metin.splitlines():
        p = s.split("\t")
        if len(p) == 2 and p[1].isdigit() and int(p[1]) > 0:
            out[p[0]] = int(p[1])
    return out


def son_gun(kaynaklar):
    gunler = []
    for k in kaynaklar:
        g = os.path.join(k, "gunluk")
        if os.path.isdir(g) and not os.path.islink(g):
            gunler += [d for d in os.listdir(g) if GUN_RE.fullmatch(d)]
    return max(gunler) if gunler else None


def boyut(kaynaklar):
    # ponytail: her çalıştırmada tüm arşivi tarar; yüzlerce müşteride günde bire indirilir
    toplam = 0
    for k in kaynaklar:
        for kok, _, dosyalar in os.walk(k):
            for d in dosyalar:
                try:
                    toplam += os.lstat(os.path.join(kok, d)).st_size
                except OSError:
                    pass
    return toplam


def _wg():
    try:
        return subprocess.run(["wg", "show", "wg0", "latest-handshakes"], capture_output=True, text=True, timeout=10).stdout
    except (OSError, subprocess.SubprocessError):
        return ""


def main(kayit=KAYIT, yeni=YENI, eski=ESKI, cikti=DURUM, wg=None, simdi=None, grup="wcpanel"):
    simdi = time.time() if simdi is None else simdi
    el = el_sikismalari(_wg() if wg is None else wg)
    out = {ad: {"tur": a["tur"], "son_gun": son_gun(a["kaynaklar"]), "boyut": boyut(a["kaynaklar"]),
                "el_sikisma": el.get(a["pub"]) if a["pub"] else None}
           for ad, a in arsivler(kayit, yeni, eski).items()}
    common.save_json(cikti, {"zaman": common.now_iso(simdi), "arsivler": out})
    os.chmod(cikti, 0o640)
    if grup:
        shutil.chown(cikti, group=grup)
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 4: istatistik main** — `istatistik.py`'de `HESAPLAR` sabitini, `import panel_auth` satırını ve eski `main`'i sil; yerine:

```python
def main(kayit=None, yeni=None, eski=None, cikti=CIKTI, detay=DETAY, grup=PANEL_GRUBU):
    import durum  # durum → common; döngü yok ama istatistik'i hafif tutmak için burada
    import ozet  # istatistik'i içe aktarır; döngüsel içe aktarma olmasın diye burada
    kod = 0
    for ad, a in durum.arsivler(kayit or durum.KAYIT, yeni or durum.YENI, eski or durum.ESKI).items():
        hatalar = []
        for kaynak in a["kaynaklar"]:  # yeni arşiv önce: aynı gün eskide de varsa atlanır
            try:
                hatalar += guncelle(kaynak, os.path.join(cikti, ad + ".json"))
                hatalar += ozet.detay_guncelle(kaynak, os.path.join(detay, ad), grup)
            except Exception as e:  # bir arşivin bozuk klasörü diğerlerini durdurmasın
                hatalar.append(repr(e))
        for hata in hatalar:
            print(f"{ad}: {hata}", file=sys.stderr)
            kod = 1
    return kod
```
Modül docstring'indeki "kafe" ifadelerini "arşiv" yap. `hotspot-arsiv-saklama` ilk döngüsü:
`for d in /srv/hotspot-arsiv/*/gunluk/????-??-?? /srv/wificorrect-arsiv/*/veri/gunluk/????-??-??; do`.

- [ ] **Step 5: Birimler** — `wc-durum.service`:
```ini
[Unit]
Description=WifiCorrect yönetim merkezi: arşiv ve tünel durumu

[Service]
Type=oneshot
Environment=PYTHONDONTWRITEBYTECODE=1
ExecStart=/usr/local/sbin/wc-durum
TimeoutStartSec=10min
```
`wc-durum.timer`:
```ini
[Unit]
Description=WifiCorrect yönetim merkezi durumu, 5 dakikada bir

[Timer]
OnBootSec=1min
OnUnitActiveSec=5min

[Install]
WantedBy=timers.target
```

- [ ] **Step 6: Run** `test_durum.py`, `test_wc_istatistik.py`, `test_wc_ozet.py` → geçer.
- [ ] **Step 7: Commit** `git add scripts/sunucu/merkez && git commit -m "Yönetim merkezi: wc-durum (arşiv/tünel durumu); istatistik arşivleri klasörlerden alır"`

---

### Task 5: kayitlar.py ve web.py (ortak taban)

**Files:**
- Create: `scripts/sunucu/merkez/kayitlar.py`, `scripts/sunucu/merkez/web.py`
- Test: `scripts/sunucu/merkez/tests/test_kayitlar.py`, `scripts/sunucu/merkez/tests/yardim.py` (testlerin ortak kurulumu)

**Interfaces:**
- Consumes: `detay.*` (öneklerle), `guvenlik.Oturumlar/GirisKilidi`, `veri.Veri.hareket`
- Produces (kayitlar): `Kayitlar(istatistik=ISTATISTIK, detay_dizini=DETAY)`, `.sayilar(arsivler)->(dict, guncel|None)`,
  `.gunler(arsivler)->{gun: arşiv}`, `.ozet_html(baslik, arsivler, simdi, onek)->str`, `.gun_html(arsivler, gun, onek)->str|None`,
  `.ara_html(arsivler, sorgu, onek)->str`; `son_30_gun(simdi)->list[date]`
- Produces (web): `e(s)`, `CSS`, `GEREKCE_SN=1800`, `yanit_html(govde, durum=200, cerez=None)`, `yonlendir(yer, cerez=None)`,
  `cerez_degeri(baslik, ad)`, `guvenli_donus(p)`, `log(msg)`, `kart(icerik)`, `bilgi_tablosu(satirlar)`, sınıf `Taban`:
  - sınıf alanları: `rol`, `cerez`, `kullanici_etiketi`, `giris_basligi`, `giris_aciklama`
  - `__init__(self, veri, kayitlar, saat=time.time)`
  - alt sınıfın yazdıkları: `dogrula(kul, pw)->kimlik|None`, `hesap_var(kimlik)->bool`, `girdi(kimlik, ip)`,
    `kim(ot)->str`, `hareket_musterisi(ot, yol)->int|None`, `menu(ot)->str`,
    `sayfalar(ot, token, yontem, yol, sorgu, form, ip)->yanıt|None`
  - verdikleri: `istek(yontem, yol, basliklar, govde, sorgu="", ip="")->(durum, başlıklar, bayt)`,
    `sayfa(baslik, icerik, ot, genis=False)->str`, `mesaj(ot, baslik, metin)->str`,
    `gerekce_iste(ot, tam_yol, musteri, ip)->yanıt|None`, `kayit_sayfasi(ot, numara, alt, sorgu, onek, ip, baslik)->yanıt|None`
  - yanıt biçimi: `(durum: int, basliklar: dict, govde: bytes)`; çerez `(ad, token|None)` (None = sil)

- [ ] **Step 1: Test yardımcısı** — `tests/yardim.py`:

```python
"""Uygulama testlerinin ortak kurulumu: geçici veritabanı, özet klasörleri, sahte saat, istek yardımcıları."""
import json
import os
import re
import sys
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))
import kayitlar  # noqa: E402
import veri  # noqa: E402


class Saat:
    t = 1_790_000_000.0  # 2026-09-21 ~ 17:00 (+03)

    def __call__(self):
        return self.t


def ortam(tmp):
    saat = Saat()
    v = veri.Veri(os.path.join(tmp, "m.db"), saat=saat)
    ist, det = os.path.join(tmp, "ist"), os.path.join(tmp, "detay")
    os.makedirs(ist)
    os.makedirs(det)
    return v, kayitlar.Kayitlar(ist, det), saat


def sayi_yaz(tmp, ad, sayilar):
    with open(os.path.join(tmp, "ist", ad + ".json"), "w", encoding="utf-8") as f:
        json.dump(sayilar, f)


def gun_yaz(tmp, ad, gun, kisiler=(), dizin=None):
    d = os.path.join(tmp, "detay", ad)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, gun + ".json"), "w", encoding="utf-8") as f:
        json.dump({"gun": gun, "kisiler": list(kisiler)}, f)
    if dizin is not None:
        with open(os.path.join(d, "kisiler.json"), "w", encoding="utf-8") as f:
            json.dump(dizin, f)


def cerez(yanit):
    return yanit[1]["Set-Cookie"].split(";")[0]


def csrf(yanit):
    return re.search(r'name="csrf" value="([^"]+)"', yanit[2].decode()).group(1)


def al(app, yol, c="", ip="203.0.113.5"):
    yol, _, sorgu = yol.partition("?")
    return app.istek("GET", yol, {"Cookie": c}, "", sorgu, ip)


def gonder(app, yol, c, alanlar, ip="203.0.113.5"):
    return app.istek("POST", yol, {"Cookie": c}, urllib.parse.urlencode(alanlar), "", ip)


def giris(app, kul, pw, ip="203.0.113.5"):
    return app.istek("POST", "/giris", {}, urllib.parse.urlencode({"kullanici": kul, "sifre": pw}), "", ip)


def gerekce_ver(app, c, metin="Müşteri şikâyeti"):
    f = al(app, "/gerekce?donus=%2F", c)
    return gonder(app, "/gerekce", c, {"csrf": csrf(f), "gerekce": metin, "donus": "/"})
```

- [ ] **Step 2: Failing test** — `tests/test_kayitlar.py`:

```python
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import yardim as Y  # noqa: E402
import kayitlar as K  # noqa: E402


def test_birden_cok_arsiv_birlesir_ilki_gecerli():
    with tempfile.TemporaryDirectory() as tmp:
        _, k, saat = Y.ortam(tmp)
        dun = K.son_30_gun(saat())[0].isoformat()
        Y.sayi_yaz(tmp, "100001", {dun: 5})
        Y.sayi_yaz(tmp, "bocafe", {dun: 99, "2026-09-01": 7})
        assert k.sayilar(["100001", "bocafe"])[0] == {dun: 5, "2026-09-01": 7}
        Y.gun_yaz(tmp, "100001", "2026-09-20", dizin={"5330000001": {"ad": "Ayşe", "gunler": ["2026-09-20"]}})
        Y.gun_yaz(tmp, "bocafe", "2026-09-20", dizin={"5330000001": {"ad": "Ayşe", "gunler": ["2026-09-01"]}})
        Y.gun_yaz(tmp, "bocafe", "2026-09-01")
        assert k.gunler(["100001", "bocafe"]) == {"2026-09-20": "100001", "2026-09-01": "bocafe"}
        s = k.ara_html(["100001", "bocafe"], "q=ay%C5%9Fe", "/m/100001/kayitlar")
        assert 'href="/m/100001/kayitlar/gun/2026-09-20#k5330000001"' in s and "/gun/2026-09-01#k" in s


def test_ozet_gun_ve_bilinmeyen():
    with tempfile.TemporaryDirectory() as tmp:
        _, k, saat = Y.ortam(tmp)
        dun = K.son_30_gun(saat())[0].isoformat()
        Y.sayi_yaz(tmp, "100001", {dun: 14})
        Y.gun_yaz(tmp, "100001", "2026-09-20", [{"telefon": "5330000001", "ad": "<b>x</b>"}])
        s = k.ozet_html("Bocafe <Göztepe>", ["100001"], saat(), "")
        assert "Bocafe &lt;Göztepe&gt;" in s and ">14<" in s and 'href="/gun/2026-09-20"' in s
        assert "&lt;b&gt;x&lt;/b&gt;" in k.gun_html(["100001"], "2026-09-20", "")
        assert k.gun_html(["100001"], "2026-09-21", "") is None
        assert k.gun_html(["100001"], "../../etc", "") is None
        assert "Henüz veri yok" in k.ozet_html("X", ["yok"], saat(), "")


if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
```

- [ ] **Step 3: Run** → FAIL.

- [ ] **Step 4: Implement `kayitlar.py`**

```python
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
```

- [ ] **Step 5: Implement `web.py`**

```python
"""Yönetim merkezi web tabanı: CSS, sayfa çerçevesi, çerezler, giriş/çıkış/CSRF/gerekçe (spec §5–§6, §10).
Uygulamalar (yönetim, müşteri) Taban'dan türer ve yalnızca kendi sayfalarını yazar."""
import hmac
import html
import sys
import time
import urllib.parse

import detay
import guvenlik

GEREKCE_SN = 1800
GEREKCE_ONERILERI = ("Emniyet / savcılık talebi", "Müşteri şikâyeti", "Müşterinin kendi talebi", "Teknik sorun incelemesi")
CSS = """*{box-sizing:border-box}body{margin:0;font:16px/1.5 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;background:#f5f7fa;color:#1d2330}
header{background:#fff;border-bottom:1px solid #e3e7ee;padding:12px 16px;display:flex;align-items:center;justify-content:space-between;gap:12px;flex-wrap:wrap}
.marka{font-weight:700;color:#0a6ebd;font-size:18px;text-decoration:none}nav{display:flex;gap:12px;align-items:center;flex-wrap:wrap}nav form{margin:0}
main{max-width:640px;margin:0 auto;padding:16px}main.genis{max-width:1100px}.kart{background:#fff;border:1px solid #e3e7ee;border-radius:12px;padding:16px;margin-bottom:16px}
h1{font-size:20px;margin:0 0 4px}.alt{color:#5b6475;font-size:14px;margin:0}
label{display:block;font-size:14px;margin:12px 0 4px}input,select{width:100%;padding:10px 12px;border:1px solid #c9d0db;border-radius:8px;font:inherit}
input[type=checkbox]{width:auto}button{background:#0a6ebd;color:#fff;border:0;border-radius:999px;padding:10px 18px;font:inherit;cursor:pointer;margin-top:16px}
button.ikincil{background:#fff;color:#0a6ebd;border:1px solid #0a6ebd;margin:0;padding:4px 14px}button.tehlike{background:#a4262c}
.hata{background:#fdecec;color:#a4262c;padding:10px 12px;border-radius:8px;margin:12px 0 0}.kotu{color:#a4262c}
.tamam{background:#e7f6ec;color:#1e6b34;padding:10px 12px;border-radius:8px;margin:12px 0 0}
.sir{font:600 22px ui-monospace,Consolas,monospace;letter-spacing:1px;user-select:all}
table{width:100%;border-collapse:collapse;margin-bottom:8px}td,th{padding:6px 4px;border-bottom:1px solid #eef1f5;font-size:14px;text-align:left;vertical-align:top}
td.cubuk{width:100%}td.cubuk span{display:block;height:10px;background:#0a6ebd;border-radius:5px;min-width:2px}
td.n{text-align:right;font-weight:600;min-width:3ch}.bos{color:#9aa3b2}a{color:#0a6ebd}h2{font-size:17px;margin:0 0 8px}h3{font-size:15px;margin:12px 0 4px}
details{border:1px solid #e3e7ee;border-radius:10px;margin:8px 0;background:#fff}summary{cursor:pointer;padding:10px 12px}details>div{padding:0 12px 12px}
.kaydir{overflow-x:auto}table.detay th{font-size:13px;color:#5b6475}ul.gunler{list-style:none;margin:0;padding:0 12px 12px}ul.gunler li{padding:4px 0}
form.ara{display:flex;gap:8px}form.ara input{flex:1}form.ara button{margin:0}"""
e = detay.e
SINIF_GENIS = ' class="genis"'


def log(msg):
    print(msg, file=sys.stderr, flush=True)  # journal; fail2ban GIRIS_HATALI satırlarını okur


def cerez_degeri(baslik, ad):
    """Cookie başlığından `ad` çerezini okur (SimpleCookie ilk bozuk çerezde durduğu için elle)."""
    for parca in (baslik or "").split(";"):
        k, _, v = parca.strip().partition("=")
        if k == ad:
            return v
    return None


def _cerez_basligi(cerez):
    ad, token = cerez
    if token is None:
        return f"{ad}=; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=0"
    return f"{ad}={token}; HttpOnly; Secure; SameSite=Strict; Path=/"


def yanit_html(govde, durum=200, cerez=None):
    b = {"Content-Type": "text/html; charset=utf-8", "Cache-Control": "no-store"}
    if cerez:
        b["Set-Cookie"] = _cerez_basligi(cerez)
    return durum, b, govde.encode("utf-8")


def yonlendir(yer, cerez=None):
    b = {"Location": yer, "Cache-Control": "no-store"}
    if cerez:
        b["Set-Cookie"] = _cerez_basligi(cerez)
    return 303, b, b""


def guvenli_donus(p):
    return p if isinstance(p, str) and p.startswith("/") and not p.startswith("//") and not any(c in p for c in "\\\r\n") else "/"


def kart(icerik):
    return f'<div class="kart">{icerik}</div>'


def bilgi_tablosu(satirlar):
    return "<table>" + "".join(f"<tr><th>{e(k)}</th><td>{v}</td></tr>" for k, v in satirlar) + "</table>"


def _cerceve(baslik, icerik, nav="", genis=False):
    return ('<!doctype html><html lang="tr"><head><meta charset="utf-8">'
            '<meta name="viewport" content="width=device-width,initial-scale=1">'
            f'<title>{e(baslik)} · WifiCorrect</title><style>{CSS}</style></head><body>'
            f'<header><a class="marka" href="/">WifiCorrect</a>{nav}</header>'
            f'<main{SINIF_GENIS if genis else ""}>{icerik}</main></body></html>')


class Taban:
    rol = ""
    cerez = ""
    kullanici_etiketi = "Kullanıcı adı"
    giris_basligi = "Giriş"
    giris_aciklama = ""

    def __init__(self, veri, kayitlar, saat=time.time):
        self.veri, self.kayitlar, self.saat = veri, kayitlar, saat
        self.oturumlar, self.kilit = guvenlik.Oturumlar(), guvenlik.GirisKilidi()

    # --- alt sınıfın yazdıkları ---
    def dogrula(self, kul, pw):
        raise NotImplementedError

    def hesap_var(self, kimlik):
        raise NotImplementedError

    def girdi(self, kimlik, ip):
        raise NotImplementedError

    def kim(self, ot):
        raise NotImplementedError

    def hareket_musterisi(self, ot, yol):
        raise NotImplementedError

    def menu(self, ot):
        return ""

    def sayfalar(self, ot, token, yontem, yol, sorgu, form, ip):
        return None

    # --- ortak ---
    def sayfa(self, baslik, icerik, ot, genis=False):
        nav = ""
        if ot:
            nav = (f'<nav>{self.menu(ot)}<form method="post" action="/cikis">'
                   f'<input type="hidden" name="csrf" value="{e(ot["csrf"])}"><button class="ikincil">Çıkış</button></form></nav>')
        return _cerceve(baslik, icerik, nav, genis)

    def mesaj(self, ot, baslik, metin):
        return self.sayfa(baslik, kart(f'<h1>{e(baslik)}</h1><p>{e(metin)}</p><p><a href="/">Ana sayfa</a></p>'), ot)

    def _giris_html(self, hata=""):
        h = f'<p class="hata">{e(hata)}</p>' if hata else ""
        return self.sayfa("Giriş", kart(
            f'<h1>{e(self.giris_basligi)}</h1><p class="alt">{e(self.giris_aciklama)}</p>'
            '<form method="post" action="/giris">'
            f'<label for="k">{e(self.kullanici_etiketi)}</label>'
            '<input id="k" name="kullanici" autocomplete="username" autocapitalize="none" spellcheck="false" required>'
            '<label for="s">Parola</label><input id="s" name="sifre" type="password" autocomplete="current-password" required>'
            f'{h}<button>Giriş yap</button></form>'), None)

    def istek(self, yontem, yol, basliklar, govde, sorgu="", ip=""):
        simdi = self.saat()
        token = cerez_degeri(basliklar.get("Cookie"), self.cerez)
        ot = self.oturumlar.get(token, simdi)
        if ot and not self.hesap_var(ot["user"]):  # hesap silinmiş / değişmiş
            self.oturumlar.drop(token)
            ot = None
        form = dict(urllib.parse.parse_qsl(govde)) if yontem == "POST" else {}
        if yol == "/giris":
            if yontem == "POST":
                return self._giris(form, ip, simdi)
            return yonlendir("/") if ot else yanit_html(self._giris_html())
        if not ot:
            return yonlendir("/giris")
        if yontem == "POST" and not hmac.compare_digest(form.get("csrf", "").encode(), ot["csrf"].encode()):
            return yanit_html(self.mesaj(ot, "Geçersiz istek", "Sayfayı yenileyip tekrar deneyin."), 403)
        if (yontem, yol) == ("POST", "/cikis"):
            self.oturumlar.drop(token)
            return yonlendir("/giris", (self.cerez, None))
        if yol == "/gerekce":
            return self._gerekce(ot, token, yontem, sorgu, form, ip, simdi)
        r = self.sayfalar(ot, token, yontem, yol, sorgu, form, ip)
        return r or yanit_html(self.mesaj(ot, "Bulunamadı", "Böyle bir sayfa yok."), 404)

    def _giris(self, form, ip, simdi):
        kul = form.get("kullanici", "").strip().lower()
        if not self.kilit.attempt(ip, kul, simdi):
            log(f"GIRIS_KILITLI alan={self.rol} kullanici={kul!r} ip={ip}")
            return yanit_html(self._giris_html("Çok fazla deneme. 15 dakika sonra tekrar deneyin."), 429)
        kimlik = self.dogrula(kul, form.get("sifre", ""))
        if kimlik is None:
            log(f"GIRIS_HATALI alan={self.rol} kullanici={kul!r} ip={ip}")
            return yanit_html(self._giris_html(f"{self.kullanici_etiketi} veya parola hatalı."), 401)
        self.kilit.succeed(ip, kul, simdi)
        self.girdi(kimlik, ip)
        return yonlendir("/", (self.cerez, self.oturumlar.create(kimlik, self.rol, simdi)))

    def _gerekce(self, ot, token, yontem, sorgu, form, ip, simdi):
        if yontem == "GET":
            donus = guvenli_donus((urllib.parse.parse_qs(sorgu).get("donus") or ["/"])[0])
            return yanit_html(self._gerekce_html(ot, donus))
        metin, donus = " ".join(form.get("gerekce", "").split()), guvenli_donus(form.get("donus", "/"))
        if not 5 <= len(metin) <= 200:
            return yanit_html(self._gerekce_html(ot, donus, "Gerekçe 5–200 karakter olmalı."), 400)
        self.oturumlar.set_gerekce(token, metin, simdi + GEREKCE_SN)
        self.veri.hareket(self.kim(ot), ip, "GEREKCE", self.hareket_musterisi(ot, donus), f"{donus} · {metin}")
        return yonlendir(donus)

    def _gerekce_html(self, ot, donus, hata=""):
        secenekler = "".join(f'<option value="{e(x)}">' for x in GEREKCE_ONERILERI)
        h = f'<p class="hata">{e(hata)}</p>' if hata else ""
        return self.sayfa("Gerekçe", kart(
            '<h1>Gerekçe</h1><p>Müşterilerin kişisel verisine bakmak için gerekçe yazın.</p>'
            f'<p class="alt">Gerekçe {GEREKCE_SN // 60} dakika geçerlidir; bu süredeki her görüntüleme gerekçeyle kaydedilir.</p>'
            f'<form method="post" action="/gerekce"><input type="hidden" name="csrf" value="{e(ot["csrf"])}">'
            f'<input type="hidden" name="donus" value="{e(donus)}">'
            '<label for="g">Gerekçe</label><input id="g" name="gerekce" list="oneriler" minlength="5" maxlength="200" required>'
            f'<datalist id="oneriler">{secenekler}</datalist>{h}<button>Devam</button></form>'), ot)

    def gerekce_iste(self, ot, tam_yol, musteri, ip):
        """Geçerli gerekçe yoksa gerekçe sayfasına yönlendirir; varsa görüntülemeyi kaydeder ve None döner."""
        g = ot.get("gerekce")
        if not g or g[1] <= self.saat():
            return yonlendir("/gerekce?donus=" + urllib.parse.quote(tam_yol, safe=""))
        self.veri.hareket(self.kim(ot), ip, "GORUNTULEME", musteri, f"{tam_yol} · {g[0]}")
        return None

    def kayit_sayfasi(self, ot, numara, alt, sorgu, onek, ip, baslik):
        """alt: '/', '/gun/<gün>', '/ara'. Kişi verisi açan sayfalar gerekçe ister (sayılar ve boş arama formu istemez)."""
        arsivler = self.veri.arsivler(numara)
        if alt == "/":
            return yanit_html(self.sayfa(baslik, self.kayitlar.ozet_html(baslik, arsivler, self.saat(), onek), ot))
        q = (urllib.parse.parse_qs(sorgu).get("q") or [""])[0].strip()
        if alt.startswith("/gun/") or (alt == "/ara" and q):
            r = self.gerekce_iste(ot, onek + alt + ("?" + sorgu if sorgu else ""), numara, ip)
            if r:
                return r
        if alt.startswith("/gun/"):
            gun = alt[len("/gun/"):]
            h = self.kayitlar.gun_html(arsivler, gun, onek)
            if h is None:
                return yanit_html(self.mesaj(ot, "Bulunamadı", "Bu gün için kayıt yok."), 404)
            return yanit_html(self.sayfa(detay.tarih(gun), h, ot))
        if alt == "/ara":
            return yanit_html(self.sayfa("Arama", self.kayitlar.ara_html(arsivler, sorgu, onek), ot))
        return None
```

- [ ] **Step 6: Run** `python scripts/sunucu/merkez/tests/test_kayitlar.py` → geçer.
- [ ] **Step 7: Commit** `git add scripts/sunucu/merkez && git commit -m "Yönetim merkezi: kayıt görünümü (çok arşivli) ve ortak web tabanı"`

---

### Task 6: api.py — cihaz API'si

**Files:**
- Create: `scripts/sunucu/merkez/api.py`
- Test: `scripts/sunucu/merkez/tests/test_api.py`

**Interfaces:**
- Consumes: `veri.Veri` (Görev 2), `kuyruk.ekle/bekle` (Görev 3), `guvenlik.GirisKilidi/HizSiniri`
- Produces: `Api(veri, saat=time.time, kuyruk_ekle=kuyruk.ekle, kuyruk_bekle=kuyruk.bekle)`,
  `.istek(yontem, yol, basliklar, govde, sorgu="", ip="")->(durum, başlıklar, bayt)`; gövde/yanıt JSON.
  Hata yanıtı `{"hata": "<Türkçe metin>", "kod": "<kısa kod>"}`; kodlar: `gecersiz`, `kilit`, `hatali`, `uyelik`,
  `baska_cihaz`, `kayit`, `taninmadi`, `hiz`, `yok`.

- [ ] **Step 1: Failing tests** — `tests/test_api.py`:

```python
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


def post(api, yol, govde, ip="198.51.100.7"):
    d, b, g = api.istek("POST", yol, {}, json.dumps(govde) if not isinstance(govde, str) else govde, "", ip)
    return d, json.loads(g)


def giris(api, n, pw, wg=WG, ip="198.51.100.7"):
    return post(api, "/api/giris", {"numara": str(n), "parola": pw, "wg_pub": wg, "ssh_pub": SSH,
                                    "isletme_adi": "Bocafe", "unvan": "Bocafe Ltd.", "surum": "1.0"}, ip)


def test_ilk_giris_baglar_ve_kuyruga_atar():
    with tempfile.TemporaryDirectory() as tmp:
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
    with tempfile.TemporaryDirectory() as tmp:
        api, v, q, _ = kur(tmp)
        n, pw = v.musteri_ekle()
        assert giris(api, n, "yanlis-parola")[1]["kod"] == "hatali"
        assert giris(api, 999999, "yanlis-parola")[1]["kod"] == "hatali"
        for _ in range(4):
            giris(api, n, "yanlis-parola", ip="198.51.100.8")
        assert giris(api, n, pw)[0] == 429  # numara kilitli
        assert post(api, "/api/giris", "{bozuk")[0] == 400
        assert post(api, "/api/giris", {"numara": "abc", "parola": "x"})[1]["kod"] == "gecersiz"
        assert post(api, "/api/giris", {"numara": "100009", "parola": "x" * 10, "wg_pub": "kotu", "ssh_pub": SSH})[1]["kod"] == "hatali"
        assert api.istek("GET", "/api/giris", {}, "", "", "1.1.1.1")[0] == 404
        assert api.istek("POST", "/baska", {}, "{}", "", "1.1.1.1")[0] == 404
        assert api.istek("POST", "/api/giris", {}, "x" * 5000, "", "1.1.1.1")[0] == 413
        assert q.isler == []


def test_baska_cihaz_409_ve_ayni_cihaz_yeniden_dener():
    with tempfile.TemporaryDirectory() as tmp:
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
    with tempfile.TemporaryDirectory() as tmp:
        api, v, _, _ = kur(tmp)
        n, pw = v.musteri_ekle()
        v.uyelik(n, "bitti")
        assert giris(api, n, pw)[1]["kod"] == "uyelik"
        v.uyelik(n, "aktif")
        assert giris(api, n, pw)[0] == 200


def test_eslesme_parola_ve_serbest_birakma():
    with tempfile.TemporaryDirectory() as tmp:
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
        assert q.isler[-1] == ("cihaz-kapat", {"numara": str(n)})
        assert post(api, "/api/eslesme", {"cihaz_anahtari": a})[1]["kod"] == "taninmadi"
        assert v.bagli_cihaz(n) is None


def test_metinler_kirpilir_ve_hiz_siniri():
    with tempfile.TemporaryDirectory() as tmp:
        api, v, _, _ = kur(tmp)
        n, pw = v.musteri_ekle()
        a = giris(api, n, pw)[1]["cihaz_anahtari"]
        post(api, "/api/eslesme", {"cihaz_anahtari": a, "isletme_adi": "x" * 500, "unvan": "a\x00b", "surum": 5})
        c = v.bagli_cihaz(n)
        assert c["isletme_adi"] == "" and c["unvan"] == "" and c["surum"] == ""
        kodlar = [post(api, "/api/eslesme", {"cihaz_anahtari": a}, ip="203.0.113.99")[0] for _ in range(21)]
        assert kodlar[-1] == 429 and 429 not in kodlar[:20]


if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
```

- [ ] **Step 2: Run** → FAIL.

- [ ] **Step 3: Implement `api.py`**

```python
"""Cihaz API'si (spec §4): api.wificorrect.com/api/{giris,eslesme,parola}. JSON gövde ≤ 4 KB, IP başına dakikada 20 istek.
Cihazın kimliği ilk girişte verilen cihaz anahtarıdır (merkez yalnızca SHA-256'sını tutar)."""
import json
import re
import time

import guvenlik
import kuyruk
import veri as veri_modulu

GOVDE_SINIRI = 4096
NUMARA_RE = re.compile(r"[1-9]\d{5}")
YEDEK_SUNUCU = "10.99.0.1"


def _json(durum, veri):
    return durum, {"Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store"}, \
        json.dumps(veri, ensure_ascii=False).encode("utf-8")


def _hata(durum, kod, metin):
    return _json(durum, {"hata": metin, "kod": kod})


def _metin(v):
    """Cihazın bildirdiği metin: ≤ 200 yazdırılabilir karakter, değilse boş."""
    return v if isinstance(v, str) and len(v) <= 200 and v.isprintable() else ""


def _parola_alanlari(m):
    return {"tuz": m["tuz"], "ozet": m["ozet"], "yineleme": m["yineleme"]}


class Api:
    def __init__(self, veri, saat=time.time, kuyruk_ekle=kuyruk.ekle, kuyruk_bekle=kuyruk.bekle):
        self.veri, self.saat = veri, saat
        self.kuyruk_ekle, self.kuyruk_bekle = kuyruk_ekle, kuyruk_bekle
        self.kilit, self.hiz = guvenlik.GirisKilidi(), guvenlik.HizSiniri(20, 60)

    def istek(self, yontem, yol, basliklar, govde, sorgu="", ip=""):
        uclar = {"/api/giris": self.giris, "/api/eslesme": self.eslesme, "/api/parola": self.parola}
        if yontem != "POST" or yol not in uclar:
            return _hata(404, "yok", "Bulunamadı.")
        if len(govde) > GOVDE_SINIRI:
            return _hata(413, "gecersiz", "İstek çok büyük.")
        if not self.hiz.izin(ip, self.saat()):
            return _hata(429, "hiz", "Çok fazla istek, biraz sonra tekrar deneyin.")
        try:
            b = json.loads(govde)
        except ValueError:
            b = None
        if not isinstance(b, dict):
            return _hata(400, "gecersiz", "Geçersiz istek.")
        return uclar[yol](b, ip)

    def giris(self, b, ip):
        simdi = self.saat()
        numara, pw = b.get("numara"), b.get("parola")
        if not isinstance(numara, str) or not NUMARA_RE.fullmatch(numara) or not isinstance(pw, str):
            return _hata(400, "gecersiz", "Müşteri numarası 6 haneli olmalı.")
        if not self.kilit.attempt(ip, numara, simdi):
            return _hata(429, "kilit", "Çok fazla deneme. 15 dakika sonra tekrar deneyin.")
        m = self.veri.parola_dogrula(int(numara), pw)
        if m is None:
            if self.veri.musteri(int(numara)) is not None:
                self.veri.hareket(numara, ip, "CIHAZ_GIRIS_HATALI", int(numara))
            return _hata(401, "hatali", "Müşteri numarası veya parola hatalı.")
        self.kilit.succeed(ip, numara, simdi)
        if m["uyelik"] != "aktif":
            return _hata(403, "uyelik", "Üyeliğiniz sona ermiş. Hizmet sağlayıcınıza başvurun.")
        wg, ssh = b.get("wg_pub"), b.get("ssh_pub")
        if not (isinstance(wg, str) and kuyruk.WG_RE.fullmatch(wg) and isinstance(ssh, str) and kuyruk.SSH_RE.fullmatch(ssh)):
            return _hata(400, "gecersiz", "Cihaz anahtarları geçersiz.")
        try:
            cid, anahtar = self.veri.cihaz_bagla(m["numara"], wg, ssh, _metin(b.get("isletme_adi")),
                                                 _metin(b.get("unvan")), _metin(b.get("surum")))
        except veri_modulu.BaskaCihaz:
            self.veri.hareket(numara, ip, "CIHAZ_REDDEDILDI", m["numara"], "numara başka cihazda")
            return _hata(409, "baska_cihaz", "Bu numara başka bir cihazda kullanılıyor.")
        s = self.kuyruk_bekle(self.kuyruk_ekle("cihaz-ekle", {"numara": numara, "wg_pub": wg, "ssh_pub": ssh}))
        if not s or s.get("durum") != "tamam":
            self.veri.hareket(numara, ip, "CIHAZ_KAYIT_HATA", m["numara"], (s or {}).get("hata", "zaman aşımı"))
            return _hata(503, "kayit", "Cihaz kaydı tamamlanamadı, biraz sonra tekrar deneyin.")
        self.veri.cihaz_guncelle(cid, tunel_ip=s["tunel_ip"])
        self.veri.hareket(numara, ip, "CIHAZ_BAGLANDI", m["numara"], f"cihaz={cid} tunel={s['tunel_ip']}")
        return _json(200, dict(_parola_alanlari(m), cihaz_anahtari=anahtar, tunel_ip=s["tunel_ip"],
                               sunucu_pub=s["sunucu_pub"], uc_nokta=s["uc_nokta"],
                               yedek_hedefi=f"wfc-{numara}@{YEDEK_SUNUCU}:"))

    def eslesme(self, b, ip):
        c = self.veri.cihaz_anahtarla(b.get("cihaz_anahtari"))
        if c is None:
            return _hata(401, "taninmadi", "Cihaz tanınmadı.")
        if c["durum"] == "serbest_birakiliyor":
            if b.get("temizlendi") is True:
                self.veri.temizlendi(c["id"])
                self.kuyruk_ekle("cihaz-kapat", {"numara": str(c["musteri"])})  # sonucu beklenmez
                self.veri.hareket(f"cihaz:{c['id']}", ip, "CIHAZ_SERBEST", c["musteri"])
            return _json(200, {"durum": "serbest"})
        self.veri.eslesme_kaydet(c["id"], _metin(b.get("isletme_adi")), _metin(b.get("unvan")), _metin(b.get("surum")))
        m = self.veri.musteri(c["musteri"])
        return _json(200, dict(_parola_alanlari(m), durum="bagli", uyelik=m["uyelik"]))

    def parola(self, b, ip):
        c = self.veri.cihaz_anahtarla(b.get("cihaz_anahtari"))
        if c is None:
            return _hata(401, "taninmadi", "Cihaz tanınmadı.")
        if not self.kilit.attempt(ip, f"cihaz:{c['id']}", self.saat()):
            return _hata(429, "kilit", "Çok fazla deneme. 15 dakika sonra tekrar deneyin.")
        if self.veri.parola_dogrula(c["musteri"], b.get("eski")) is None:
            return _hata(401, "hatali", "Mevcut parola yanlış.")
        try:
            tuz, oz, y = self.veri.parola_koy(c["musteri"], b.get("yeni"))
        except ValueError as h:
            return _hata(400, "gecersiz", str(h))
        self.veri.hareket(str(c["musteri"]), ip, "PAROLA_DEGISTI", c["musteri"], "cihazdan")
        return _json(200, {"tuz": tuz, "ozet": oz, "yineleme": y})
```

- [ ] **Step 4: Run** `python scripts/sunucu/merkez/tests/test_api.py` → geçer.
- [ ] **Step 5: Commit** `git add scripts/sunucu/merkez && git commit -m "Yönetim merkezi: cihaz API'si (ilk giriş, günlük eşitleme, parola)"`

---

### Task 7: musteri.py — panel.wificorrect.com

**Files:**
- Create: `scripts/sunucu/merkez/musteri.py`
- Test: `scripts/sunucu/merkez/tests/test_musteri.py`

**Interfaces:**
- Consumes: `web.Taban`, `veri.Veri`, `kayitlar.Kayitlar`
- Produces: `Musteri(veri, kayitlar, saat=time.time)`; çerez `wc_musteri`; yollar `/`, `/gun/<g>`, `/ara`, `/parola`

- [ ] **Step 1: Failing tests** — `tests/test_musteri.py`:

```python
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import yardim as Y  # noqa: E402
import kayitlar  # noqa: E402
import musteri as M  # noqa: E402

WG, SSH = "A" * 43 + "=", "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI c"


def kur(tmp):
    v, k, saat = Y.ortam(tmp)
    return M.Musteri(v, k, saat), v, saat


def test_giris_ve_kendi_kayitlari():
    with tempfile.TemporaryDirectory() as tmp:
        app, v, saat = kur(tmp)
        n, pw = v.musteri_ekle()
        n2, _ = v.musteri_ekle()
        dun = kayitlar.son_30_gun(saat())[0].isoformat()
        Y.sayi_yaz(tmp, str(n), {dun: 14})
        Y.sayi_yaz(tmp, str(n2), {dun: 777})
        assert Y.giris(app, str(n), "yanlis")[0] == 401
        assert Y.giris(app, "abc", pw)[0] == 401
        c = Y.cerez(Y.giris(app, str(n), pw))
        assert c.startswith("wc_musteri=")
        s = Y.al(app, "/", c)[2].decode()
        assert ">14<" in s and "777" not in s and f"Müşteri {n}" in s
        cid, _ = v.cihaz_bagla(n, WG, SSH, "Bocafe Göztepe")
        assert "Bocafe Göztepe" in Y.al(app, "/", c)[2].decode()
        assert v.musteri(n)["son_giris"] and [h["olay"] for h in v.hareketler(n)] == ["GIRIS"]


def test_gerekce_olmadan_kisi_verisi_yok():
    with tempfile.TemporaryDirectory() as tmp:
        app, v, saat = kur(tmp)
        n, pw = v.musteri_ekle()
        Y.gun_yaz(tmp, str(n), "2026-09-20", [{"telefon": "5330000001", "ad": "Ayşe"}])
        c = Y.cerez(Y.giris(app, str(n), pw))
        r = Y.al(app, "/gun/2026-09-20", c)
        assert r[0] == 303 and r[1]["Location"] == "/gerekce?donus=%2Fgun%2F2026-09-20"
        assert Y.al(app, "/ara", c)[0] == 200 and Y.al(app, "/ara?q=ay", c)[0] == 303
        assert Y.gerekce_ver(app, c, "x")[0] == 400
        assert Y.gerekce_ver(app, c)[0] == 303
        assert "Ayşe" in Y.al(app, "/gun/2026-09-20", c)[2].decode()
        olaylar = [h["olay"] for h in v.hareketler(n)]
        assert olaylar[:2] == ["GORUNTULEME", "GEREKCE"]
        saat.t += 1801
        c = Y.cerez(Y.giris(app, str(n), pw))
        assert Y.al(app, "/gun/2026-09-20", c)[0] == 303  # yeni oturum, gerekçe yok


def test_parola_degistirme_ve_uyelik_bitti_girer():
    with tempfile.TemporaryDirectory() as tmp:
        app, v, _ = kur(tmp)
        n, pw = v.musteri_ekle()
        v.uyelik(n, "bitti")
        c = Y.cerez(Y.giris(app, str(n), pw))  # üyelik bitse de kayıtlarına bakar
        f = Y.al(app, "/parola", c)
        assert Y.gonder(app, "/parola", c, {"csrf": Y.csrf(f), "mevcut": "yanlis", "yeni": "yeni-parola-1", "tekrar": "yeni-parola-1"})[0] == 400
        assert Y.gonder(app, "/parola", c, {"csrf": Y.csrf(f), "mevcut": pw, "yeni": "yeni-parola-1", "tekrar": "baska"})[0] == 400
        r = Y.gonder(app, "/parola", c, {"csrf": Y.csrf(f), "mevcut": pw, "yeni": "yeni-parola-1", "tekrar": "yeni-parola-1"})
        assert r[0] == 200 and "06:00" in r[2].decode() and v.parola_dogrula(n, "yeni-parola-1")
        assert Y.al(app, "/", c)[0] == 303  # eski oturum kapandı
        assert Y.al(app, "/", Y.cerez(r))[0] == 200


def test_baska_musterinin_yollari_yok():
    with tempfile.TemporaryDirectory() as tmp:
        app, v, _ = kur(tmp)
        n, pw = v.musteri_ekle()
        c = Y.cerez(Y.giris(app, str(n), pw))
        for yol in ("/m/100002/", "/m/100001/kayitlar/", "/hareketler", "/yeni"):
            assert Y.al(app, yol, c)[0] == 404, yol


if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
```

- [ ] **Step 2: Run** → FAIL.

- [ ] **Step 3: Implement `musteri.py`**

```python
"""Müşteri paneli (spec §6): panel.wificorrect.com — müşteri numarası + parola; kayıtlar (gün/kişi/arama) ve parola.
Üyeliği bitmiş müşteri de girer (yalnızca okuma)."""
import time

import guvenlik
import web
from web import e, kart, yanit_html


def parola_html(ot, hata="", tamam=""):
    m = f'<p class="hata">{e(hata)}</p>' if hata else (f'<p class="tamam">{e(tamam)}</p>' if tamam else "")
    return kart(
        '<h1>Parola değiştir</h1><p class="alt">Yeni parola en az 10 karakter olmalı.</p>'
        f'<form method="post" action="/parola"><input type="hidden" name="csrf" value="{e(ot["csrf"])}">'
        '<label for="m">Mevcut parola</label><input id="m" name="mevcut" type="password" autocomplete="current-password" required>'
        '<label for="y">Yeni parola</label><input id="y" name="yeni" type="password" autocomplete="new-password" minlength="10" required>'
        '<label for="t">Yeni parola (tekrar)</label><input id="t" name="tekrar" type="password" autocomplete="new-password" minlength="10" required>'
        f'{m}<button>Değiştir</button></form><p><a href="/">← Özete dön</a></p>')


class Musteri(web.Taban):
    rol = "musteri"
    cerez = "wc_musteri"
    kullanici_etiketi = "Müşteri numarası"
    giris_basligi = "Müşteri girişi"
    giris_aciklama = "Size verilen müşteri numarası ve parolayla girin."

    def dogrula(self, kul, pw):
        if not (kul.isdigit() and len(kul) == 6):
            guvenlik.bos_dogrulama(pw)
            return None
        m = self.veri.parola_dogrula(int(kul), pw)
        return str(m["numara"]) if m else None

    def hesap_var(self, kimlik):
        return self.veri.musteri(int(kimlik)) is not None

    def girdi(self, kimlik, ip):
        self.veri.giris_kaydet(int(kimlik))
        self.veri.hareket(kimlik, ip, "GIRIS", int(kimlik), "müşteri paneli")

    def kim(self, ot):
        return ot["user"]

    def hareket_musterisi(self, ot, yol):
        return int(ot["user"])

    def menu(self, ot):
        return '<a href="/">Özet</a><a href="/parola">Parola</a>'

    def sayfalar(self, ot, token, yontem, yol, sorgu, form, ip):
        n = int(ot["user"])
        if yontem == "GET" and (yol == "/" or yol.startswith("/gun/") or yol == "/ara"):
            c = self.veri.bagli_cihaz(n)
            baslik = c["isletme_adi"] if c is not None and c["isletme_adi"] else f"Müşteri {n}"
            return self.kayit_sayfasi(ot, n, yol, sorgu, "", ip, baslik)
        if yol == "/parola" and yontem == "GET":
            return yanit_html(self.sayfa("Parola", parola_html(ot), ot))
        if yol == "/parola" and yontem == "POST":
            if self.veri.parola_dogrula(n, form.get("mevcut", "")) is None:
                return yanit_html(self.sayfa("Parola", parola_html(ot, hata="Mevcut parola yanlış."), ot), 400)
            if form.get("yeni", "") != form.get("tekrar", ""):
                return yanit_html(self.sayfa("Parola", parola_html(ot, hata="Yeni parolalar aynı değil."), ot), 400)
            try:
                self.veri.parola_koy(n, form.get("yeni", ""))
            except ValueError as h:
                return yanit_html(self.sayfa("Parola", parola_html(ot, hata=str(h)), ot), 400)
            self.oturumlar.drop_user(ot["user"])
            yeni = self.oturumlar.create(ot["user"], self.rol, self.saat())
            self.veri.hareket(ot["user"], ip, "PAROLA_DEGISTI", n, "müşteri paneli")
            tamam = "Parolanız değiştirildi. Cihazınızda ertesi sabah 06:00'dan sonra geçerli olur."
            return yanit_html(self.sayfa("Parola", parola_html(self.oturumlar.get(yeni, self.saat()), tamam=tamam), ot),
                              cerez=(self.cerez, yeni))
        return None
```

- [ ] **Step 4: Run** → geçer.
- [ ] **Step 5: Commit** `git add scripts/sunucu/merkez && git commit -m "Yönetim merkezi: müşteri paneli (panel.wificorrect.com)"`

---

### Task 8: yonetici.py ve yonetim.py — yonetim.wificorrect.com

**Files:**
- Create: `scripts/sunucu/merkez/yonetici.py`, `scripts/sunucu/merkez/yonetim.py`
- Test: `scripts/sunucu/merkez/tests/test_yonetim.py`

**Interfaces:**
- Consumes: `web.Taban`, `veri.Veri`, `kayitlar.Kayitlar`, `kuyruk.ekle`, `durum.json` biçimi (Görev 4)
- Produces: `yonetici.YONETICI`, `yonetici.yaz(yol, kullanici, pw)` (ValueError < 12), `yonetici.oku(yol)->dict|None`,
  `yonetici.main(argv, yol, sor, sahip, birak)->int`;
  `Yonetim(veri, kayitlar, saat=time.time, yonetici_yolu=YONETICI, durum_yolu=DURUM, kuyruk_ekle=kuyruk.ekle)`;
  çerez `wc_yonetim`; yollar `/`, `/yeni`, `/m/<n>`, `/m/<n>/{parola-sifirla,serbest,uyelik,hareketler}`,
  `/m/<n>/kayitlar/…`, `/hareketler`, `/eski-arsivler`, `/hesabim`

- [ ] **Step 1: Failing tests** — `tests/test_yonetim.py`:

```python
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
    with tempfile.TemporaryDirectory() as tmp:
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
    with tempfile.TemporaryDirectory() as tmp:
        app, v, _, c, _ = kur(tmp)
        f = Y.al(app, "/yeni", c)
        r = Y.gonder(app, "/yeni", c, {"csrf": Y.csrf(f), "not": "Bocafe <Göztepe>"})
        numara, pw = re.findall(r'class="sir">([^<]+)<', r[2].decode())
        assert r[0] == 200 and numara == "100001" and v.parola_dogrula(100001, pw)
        assert pw not in Y.al(app, "/m/100001", c)[2].decode()
        assert "Bocafe &lt;Göztepe&gt;" in Y.al(app, "/", c)[2].decode()
        assert v.hareketler(100001)[0]["olay"] == "MUSTERI_ACILDI"


def test_liste_durumlar_sorunlu_ustte_ve_arama():
    with tempfile.TemporaryDirectory() as tmp:
        app, v, saat, c, _ = kur(tmp, {"100002": {"tur": "cihaz", "son_gun": "2026-09-01", "boyut": 2048, "el_sikisma": 1_790_000_000 - 60}})
        n1, _ = v.musteri_ekle("Sağlam kafe")
        n2, _ = v.musteri_ekle("Gecikmiş kafe")
        cid, _ = v.cihaz_bagla(n2, WG, SSH, "Kafe <B>")
        s = Y.al(app, "/", c)[2].decode()
        assert s.index("Gecikmiş kafe") < s.index("Sağlam kafe")
        assert "GECİKTİ" in s and "çevrimiçi" in s and "Kafe &lt;B&gt;" in s and "cihaz yok" in s
        s = Y.al(app, "/?q=sa%C4%9Flam", c)[2].decode()
        assert "Sağlam kafe" in s and "Gecikmiş kafe" not in s


def test_parola_sifirla_serbest_birak_uyelik():
    with tempfile.TemporaryDirectory() as tmp:
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
        assert v.bagli_cihaz(n) is None and isler == [("cihaz-kapat", {"numara": str(n)})]
        Y.gonder(app, f"/m/{n}/uyelik", c, {"csrf": t, "islem": "bitir"})
        assert v.musteri(n)["uyelik"] == "bitti"
        Y.gonder(app, f"/m/{n}/uyelik", c, {"csrf": t, "islem": "ac"})
        assert v.musteri(n)["uyelik"] == "aktif"
        olaylar = [h["olay"] for h in v.hareketler(n)]
        for o in ("PAROLA_SIFIRLANDI", "SERBEST_BIRAKMA_ISTENDI", "CIHAZ_ZORLA_AYRILDI", "UYELIK_BITTI", "UYELIK_ACILDI"):
            assert o in olaylar, o
        assert Y.al(app, "/m/999999", c)[0] == 404 and Y.al(app, "/m/abc", c)[0] == 404


def test_musteri_kayitlari_gerekce_ister():
    with tempfile.TemporaryDirectory() as tmp:
        app, v, saat, c, _ = kur(tmp)
        n, _ = v.musteri_ekle()
        dun = kayitlar.son_30_gun(saat())[0].isoformat()
        Y.sayi_yaz(tmp, str(n), {dun: 9})
        Y.gun_yaz(tmp, str(n), "2026-09-20", [{"telefon": "5330000001", "ad": "Ayşe"}])
        assert ">9<" in Y.al(app, f"/m/{n}/kayitlar/", c)[2].decode()
        r = Y.al(app, f"/m/{n}/kayitlar/gun/2026-09-20", c)
        assert r[0] == 303 and "donus=%2Fm%2F100001%2Fkayitlar%2Fgun" in r[1]["Location"]
        Y.gerekce_ver(app, c)
        assert "Ayşe" in Y.al(app, f"/m/{n}/kayitlar/gun/2026-09-20", c)[2].decode()
        assert v.hareketler(n)[0]["olay"] == "GORUNTULEME" and v.hareketler(n)[0]["kim"] == "admin"


def test_eski_arsiv_baglama_ve_hesabim():
    with tempfile.TemporaryDirectory() as tmp:
        app, v, _, c, _ = kur(tmp, {"bocafe": {"tur": "eski", "son_gun": "2026-09-20", "boyut": 10, "el_sikisma": None},
                                    "100001": {"tur": "cihaz", "son_gun": None, "boyut": 0, "el_sikisma": None}})
        n, _ = v.musteri_ekle()
        f = Y.al(app, "/eski-arsivler", c)
        s = f[2].decode()
        assert "<td>bocafe</td>" in s and "<td>100001</td>" not in s  # müşteri arşivi eski arşiv adayı değil
        assert Y.gonder(app, "/eski-arsivler", c, {"csrf": Y.csrf(f), "ad": "yok-boyle", "musteri": str(n)})[0] == 400
        assert Y.gonder(app, "/eski-arsivler", c, {"csrf": Y.csrf(f), "ad": "bocafe", "musteri": "999999"})[0] == 400
        assert Y.gonder(app, "/eski-arsivler", c, {"csrf": Y.csrf(f), "ad": "bocafe", "musteri": str(n)})[0] == 303
        assert v.arsivler(n) == [str(n), "bocafe"]
        h = Y.al(app, "/hesabim", c)
        assert Y.gonder(app, "/hesabim", c, {"csrf": Y.csrf(h), "mevcut": PW, "yeni": "kisa", "tekrar": "kisa"})[0] == 400
        assert Y.gonder(app, "/hesabim", c, {"csrf": Y.csrf(h), "mevcut": PW, "yeni": "yeni-yonetici-12", "tekrar": "yeni-yonetici-12"})[0] == 200
        assert Y.giris(app, "serkan", "yeni-yonetici-12")[0] == 303


if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
```

- [ ] **Step 2: Run** → FAIL.

- [ ] **Step 3: Implement `yonetici.py`**

```python
#!/usr/bin/env python3
"""Kullanım: wc-yonetici parola <kullanıcı-adı>
Yönetim merkezi (yonetim.wificorrect.com) yönetici hesabını açar ya da parolasını değiştirir (tek hesap).
Root çalıştırılır; dosyaya dokunmadan önce wcpanel'e iner (klasör onun: root olarak yazmak sembolik bağ izleyebilirdi)."""
import getpass
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))
import common  # noqa: E402
import guvenlik  # noqa: E402

YONETICI = "/var/lib/wificorrect/merkez/yonetici.json"
KUL_RE = re.compile(r"[a-z0-9._-]{3,32}")
EN_AZ = 12


def yaz(yol, kullanici, pw):
    if not KUL_RE.fullmatch(kullanici or ""):
        raise ValueError("Kullanıcı adı 3-32 karakter: küçük harf, rakam, . _ -")
    if not isinstance(pw, str) or len(pw) < EN_AZ:
        raise ValueError(f"Parola en az {EN_AZ} karakter olmalı.")
    tuz, oz, y = guvenlik.yeni_kayit(pw)
    common.save_json(yol, {"kullanici": kullanici, "tuz": tuz, "ozet": oz, "yineleme": y})
    os.chmod(yol, 0o600)


def oku(yol):
    try:
        with open(yol, encoding="utf-8") as f:
            v = json.load(f)
    except (FileNotFoundError, ValueError):
        return None
    return v if isinstance(v, dict) and {"kullanici", "tuz", "ozet", "yineleme"} <= set(v) else None


def sor():
    s1 = getpass.getpass("Parola: ")
    s2 = getpass.getpass("Parola (tekrar): ")
    return s1 if s1 == s2 else None


def birak(sahip):
    if hasattr(os, "geteuid") and os.geteuid() == 0:
        import pwd
        p = pwd.getpwnam(sahip)
        os.setgroups([])
        os.setgid(p.pw_gid)
        os.setuid(p.pw_uid)


def main(argv, yol=YONETICI, sor=sor, sahip="wcpanel", birak=birak):
    if len(argv) != 2 or argv[0] != "parola" or not KUL_RE.fullmatch(argv[1]):
        print(__doc__.splitlines()[0], file=sys.stderr)
        return 2
    if sahip:
        birak(sahip)
    pw = sor()
    if pw is None:
        print("Parolalar aynı değil.", file=sys.stderr)
        return 1
    try:
        yaz(yol, argv[1], pw)
    except ValueError as h:
        print(h, file=sys.stderr)
        return 1
    print("tamam")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
```

- [ ] **Step 4: Implement `yonetim.py`**

```python
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
```

- [ ] **Step 5: Run** `python scripts/sunucu/merkez/tests/test_yonetim.py` → geçer. Başarısız bir onay testinde HTML'e bakıp
beklentiyi değil kodu düzelt (ör. "çevrimiçi" metni `_ozet`'te üretilir).
- [ ] **Step 6: Commit** `git add scripts/sunucu/merkez && git commit -m "Yönetim merkezi: yönetim ekranları (müşteriler, cihaz, serbest bırakma, eski arşivler)"`

---

### Task 9: merkez.py — tek süreç ve alan adı yönlendirmesi

**Files:**
- Create: `scripts/sunucu/merkez/merkez.py`, `scripts/sunucu/merkez/wc-merkez.service`
- Test: `scripts/sunucu/merkez/tests/test_merkez.py`

**Interfaces:**
- Consumes: `Yonetim`, `Musteri`, `Api`, `veri.Veri`, `kayitlar.Kayitlar`
- Produces: `ALANLAR`, `Merkez(uygulamalar: dict)`, `.istek(yontem, yol, basliklar, govde, sorgu, ip)`, `Isleyici`, `main()`

- [ ] **Step 1: Failing tests** — `tests/test_merkez.py`:

```python
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import yardim as Y  # noqa: E402
import merkez as MZ  # noqa: E402


class Sahte:
    def __init__(self, ad):
        self.ad = ad

    def istek(self, yontem, yol, basliklar, govde, sorgu, ip):
        return 200, {}, f"{self.ad} {yol} {ip}".encode()


def test_host_yonlendirme():
    m = MZ.Merkez({"yonetim": Sahte("Y"), "musteri": Sahte("M"), "api": Sahte("A")})
    assert m.istek("GET", "/", {"Host": "yonetim.wificorrect.com"}, "", "", "1.1.1.1")[2] == b"Y / 1.1.1.1"
    assert m.istek("GET", "/", {"Host": "PANEL.wificorrect.com:443"}, "", "", "1.1.1.1")[2].startswith(b"M")
    assert m.istek("POST", "/api/giris", {"Host": "api.wificorrect.com"}, "", "", "1.1.1.1")[2].startswith(b"A")
    for h in ("wificorrect.com", "evil.com", "", "yonetim.wificorrect.com.evil.com"):
        assert m.istek("GET", "/", {"Host": h}, "", "", "1.1.1.1")[0] == 404, h


def test_musteri_cerezi_yonetimde_gecmez():
    import musteri, yonetim, yonetici
    with tempfile.TemporaryDirectory() as tmp:
        v, k, saat = Y.ortam(tmp)
        yonetici.yaz(os.path.join(tmp, "y.json"), "serkan", "yonetici-parola-1")
        m = MZ.Merkez({"yonetim": yonetim.Yonetim(v, k, saat, os.path.join(tmp, "y.json"), os.path.join(tmp, "d.json")),
                       "musteri": musteri.Musteri(v, k, saat)})
        n, pw = v.musteri_ekle()
        import urllib.parse
        r = m.istek("POST", "/giris", {"Host": "panel.wificorrect.com"},
                    urllib.parse.urlencode({"kullanici": str(n), "sifre": pw}), "", "1.1.1.1")
        c = Y.cerez(r)
        assert m.istek("GET", "/", {"Host": "panel.wificorrect.com", "Cookie": c}, "", "", "1.1.1.1")[0] == 200
        assert m.istek("GET", "/", {"Host": "yonetim.wificorrect.com", "Cookie": c}, "", "", "1.1.1.1")[0] == 303


def test_istemci_ip_son_xff():
    assert MZ.istemci_ip({"X-Forwarded-For": "6.6.6.6, 203.0.113.5"}) == "203.0.113.5"
    assert MZ.istemci_ip({}) == "yerel"


if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
```

- [ ] **Step 2: Run** → FAIL.

- [ ] **Step 3: Implement `merkez.py`**

```python
#!/usr/bin/env python3
"""WifiCorrect yönetim merkezi (spec 2026-10-04): tek süreç, Caddy arkasında 127.0.0.1:8081. Alan adına göre:
yonetim.wificorrect.com (hizmet sağlayıcı), panel.wificorrect.com (müşteri), api.wificorrect.com (cihazlar)."""
import os
import sys
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))
import api  # noqa: E402
import kayitlar  # noqa: E402
import musteri  # noqa: E402
import veri  # noqa: E402
import web  # noqa: E402
import yonetim  # noqa: E402

DB = "/var/lib/wificorrect/merkez/merkez.db"
ALANLAR = {"yonetim.wificorrect.com": "yonetim", "panel.wificorrect.com": "musteri", "api.wificorrect.com": "api"}
GOVDE_SINIRI = 10_000
GUVENLIK_BASLIKLARI = (("X-Content-Type-Options", "nosniff"), ("Referrer-Policy", "no-referrer"), ("X-Frame-Options", "DENY"),
                       ("Content-Security-Policy", "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:"))


def istemci_ip(basliklar):
    """Yalnızca 127.0.0.1 dinlenir; Caddy gelen X-Forwarded-For'a gerçek istemciyi SONA ekler."""
    return (basliklar.get("X-Forwarded-For") or "").split(",")[-1].strip() or "yerel"


class Merkez:
    def __init__(self, uygulamalar):
        self.uygulamalar = uygulamalar

    def istek(self, yontem, yol, basliklar, govde, sorgu, ip):
        host = (basliklar.get("Host") or "").split(":")[0].strip().lower()
        app = self.uygulamalar.get(ALANLAR.get(host, ""))
        if app is None:
            return 404, {"Content-Type": "text/plain; charset=utf-8"}, "Bulunamadı".encode("utf-8")
        return app.istek(yontem, yol, basliklar, govde, sorgu, ip)


class Isleyici(BaseHTTPRequestHandler):
    app = None
    server_version = "wificorrect"
    sys_version = ""

    def _cevapla(self, yontem):
        parca = urllib.parse.urlsplit(self.path)
        try:
            uzunluk = int(self.headers.get("Content-Length") or 0)
        except ValueError:
            uzunluk = -1
        if not 0 <= uzunluk <= GOVDE_SINIRI:
            durum, basliklar, icerik = 413, {"Content-Type": "text/plain; charset=utf-8"}, "İstek çok büyük".encode()
        else:
            govde = self.rfile.read(uzunluk).decode("utf-8", "replace")
            try:
                durum, basliklar, icerik = self.app.istek(yontem, parca.path, self.headers, govde, parca.query,
                                                          istemci_ip(self.headers))
            except Exception as ex:  # ayrıntı journal'a, kullanıcıya genel mesaj
                web.log(f"HATA {yontem} {parca.path}: {ex!r}")
                durum, basliklar, icerik = 500, {"Content-Type": "text/plain; charset=utf-8"}, "Bir sorun oluştu".encode()
        self.send_response(durum)
        for k, v in list(basliklar.items()) + list(GUVENLIK_BASLIKLARI):
            self.send_header(k, v)
        self.send_header("Content-Length", str(len(icerik)))
        self.end_headers()
        self.wfile.write(icerik)

    def do_GET(self):
        self._cevapla("GET")

    def do_POST(self):
        self._cevapla("POST")

    def log_message(self, *args):
        pass


def main():
    os.umask(0o077)
    v, k = veri.Veri(DB), kayitlar.Kayitlar()
    Isleyici.app = Merkez({"yonetim": yonetim.Yonetim(v, k), "musteri": musteri.Musteri(v, k), "api": api.Api(v)})
    srv = ThreadingHTTPServer(("127.0.0.1", 8081), Isleyici)
    srv.daemon_threads = True
    srv.serve_forever()


if __name__ == "__main__":
    main()
```

`wc-merkez.service`:
```ini
[Unit]
Description=WifiCorrect yönetim merkezi (127.0.0.1:8081: yonetim, panel, api)
After=network.target

[Service]
User=wcpanel
Group=wcpanel
Environment=PYTHONDONTWRITEBYTECODE=1
ExecStart=/usr/bin/python3 /usr/local/lib/wificorrect/merkez.py
Restart=on-failure
RestartSec=3
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ReadWritePaths=/var/lib/wificorrect/merkez /var/lib/wificorrect/kuyruk
InaccessiblePaths=/srv

[Install]
WantedBy=multi-user.target
```

- [ ] **Step 4: Run** bütün testler: `for f in scripts/sunucu/merkez/tests/test_*.py; do python "$f" | tail -1; done` → hepsi geçer.
- [ ] **Step 5: Commit** `git add scripts/sunucu/merkez && git commit -m "Yönetim merkezi: tek süreç, alan adına göre yönlendirme (wc-merkez)"`

---

### Task 10: Sunucu komutları, kurulum betikleri

**Files:**
- Modify: `scripts/sunucu/wificorrect-sunucu`
- Create: `scripts/sunucu/merkez/{Caddyfile,fail2ban-wc-merkez.conf,fail2ban-wc-merkez-filtre.conf,kur.sh}`, `scripts/sunucu/dagit.sh`

**Interfaces:**
- Produces: `wificorrect-sunucu cihaz-kapat <ad>`, `wificorrect-sunucu yonetici-parola <kullanıcı>`; `cihaz-ekle` biten
  kaydı yeniden açar; `liste` biten cihazları da gösterir; `tasi-paketle` merkez durumunu da paketler.

- [ ] **Step 1: `wificorrect-sunucu`** — kullanım başlığına ekle ve `kullanim()`'ı `sed -n '2,13p'` yap:

```sh
#   wificorrect-sunucu cihaz-kapat <ad>                tüneli ve yedek hesabını kapatır; arşiv kalır (yönetim merkezi çağırır)
#   wificorrect-sunucu yonetici-parola <kullanici>     yonetim.wificorrect.com yönetici hesabı / parolası
```

`cihaz-ekle)` içinde `grep -q "^cihaz;$2;" "$KAYIT" && …` satırından sonra:
```sh
	sed -i "/^bitti;$2;/d" "$KAYIT"   # biten kayıt yeniden açılıyor: arşiv ve yedek klasörü korunur
```

Yeni dallar (`tasi-paketle)`'den önce):
```sh
cihaz-kapat)
	[ $# -eq 2 ] || kullanim
	ad_kontrol "$2"
	satir=$(grep "^cihaz;$2;" "$KAYIT") || { echo "$2 aktif bir cihaz değil" >&2; exit 1; }
	pub=$(printf '%s' "$satir" | cut -d';' -f4)
	u=$(printf '%s' "$satir" | cut -d';' -f5)
	# WireGuard eşi: "# cihaz <ad>" satırı ve ardından gelen [Peer] / PublicKey / AllowedIPs satırları çıkar
	awk -v a="# cihaz $2" '$0==a{atla=1;next} atla&&/^(\[Peer\]|PublicKey = |AllowedIPs = |PersistentKeepalive = )/{next} {atla=0;print}' "$CONF" > "$CONF.yeni"
	if grep -qF "PublicKey = $pub" "$CONF.yeni"; then rm -f "$CONF.yeni"; echo "eş çıkarılamadı, wg0.conf'a dokunulmadı" >&2; exit 1; fi
	chmod 600 "$CONF.yeni" && mv "$CONF.yeni" "$CONF"
	uygula
	[ -n "$u" ] && : > "$ARSIV/${u#wfc-}/.ssh/authorized_keys"
	awk -F';' -v OFS=';' -v a="$2" -v t="$(date +%F)" '$1=="cihaz" && $2==a {$1="bitti"; print $0, t; next} {print}' "$KAYIT" > "$KAYIT.yeni" && mv "$KAYIT.yeni" "$KAYIT"
	echo "Cihaz kapatıldı: $2 (tünel ve yedek kapalı; arşiv duruyor)"
	;;
yonetici-parola)
	[ $# -eq 2 ] || kullanim
	exec /usr/local/sbin/wc-yonetici parola "$2"
	;;
```
`liste)` döngüsü: `while IFS=';' read -r tur ad ip pub u tarih; do`; `bitti` satırında `d="bitti $tarih"` (el sıkışma
yerine); son yedek araması `[ "$tur" = cihaz ] || [ "$tur" = bitti ]` için.

`tasi-paketle)` tar listesi:
```sh
	ek=""; for p in var/lib/wificorrect srv/hotspot-arsiv etc/caddy/Caddyfile; do [ -e "/$p" ] && ek="$ek $p"; done
	tar -C / -cpf "$2" etc/wireguard etc/sysctl.d/99-wificorrect-vpn.conf etc/systemd/system/wificorrect-vpn.service "${ARSIV#/}" usr/local/sbin/wificorrect-sunucu $ek
```
ve mesaja "Yönetim merkezi için yeni sunucuda scripts/sunucu/dagit.sh çalıştırın." ekle. `tasi-kur)` sonuna:
`id wcpanel >/dev/null 2>&1 && chown -R wcpanel:wcpanel /var/lib/wificorrect/merkez /var/lib/wificorrect/kuyruk 2>/dev/null || true`.

- [ ] **Step 2: awk denemesi**

```bash
printf '[Interface]\nAddress = 10.99.0.1/24\n\n# cihaz a\n[Peer]\nPublicKey = PA=\nAllowedIPs = 10.99.0.10/32\n\n# cihaz b\n[Peer]\nPublicKey = PB=\nAllowedIPs = 10.99.0.11/32\n' > /tmp/wg.conf
awk -v a="# cihaz a" '$0==a{atla=1;next} atla&&/^(\[Peer\]|PublicKey = |AllowedIPs = |PersistentKeepalive = )/{next} {atla=0;print}' /tmp/wg.conf
```
Expected: `cihaz a` bloğu yok; `[Interface]` ve `cihaz b` bloğu duruyor. `sh -n scripts/sunucu/wificorrect-sunucu` → çıktı yok.

- [ ] **Step 3: Caddyfile ve fail2ban**

`Caddyfile`:
```
yonetim.wificorrect.com, panel.wificorrect.com, api.wificorrect.com {
	reverse_proxy 127.0.0.1:8081
	header {
		Strict-Transport-Security "max-age=31536000"
		-Server
	}
}
```
`fail2ban-wc-merkez-filtre.conf` (→ `/etc/fail2ban/filter.d/wc-merkez.conf`):
```ini
[Definition]
failregex = GIRIS_HATALI .* ip=<HOST>$
journalmatch = _SYSTEMD_UNIT=wc-merkez.service
```
`fail2ban-wc-merkez.conf` (→ `/etc/fail2ban/jail.d/wc-merkez.local`):
```ini
[wc-merkez]
enabled = true
backend = systemd
filter = wc-merkez
port = http,https
maxretry = 10
findtime = 900
bantime = 86400
```

- [ ] **Step 4: `merkez/kur.sh`**

```bash
#!/bin/bash
# Yönetim merkezi kurulumu/güncellemesi (Ubuntu, root). scripts/sunucu/dagit.sh çağırır; tekrar çalıştırmak güvenli.
set -euo pipefail
cd "$(dirname "$0")"
sed -i 's/\r$//' ./*.py ./*.service ./*.timer ./*.path ./*.conf Caddyfile hotspot-arsiv-saklama
L=/usr/local/lib/wificorrect
command -v caddy >/dev/null || DEBIAN_FRONTEND=noninteractive apt-get install -y -q caddy >/dev/null
id wcpanel >/dev/null 2>&1 || useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin wcpanel
install -d -m 755 $L /var/lib/wificorrect /var/lib/wificorrect/istatistik
install -d -o root -g wcpanel -m 750 /var/lib/wificorrect/detay /var/lib/wificorrect/kuyruk-sonuc
install -d -o wcpanel -g wcpanel -m 700 /var/lib/wificorrect/merkez /var/lib/wificorrect/kuyruk
rm -f $L/panel.py $L/panel_auth.py $L/hesap.py   # eski kafe paneli
install -m 644 common.py ozet.py detay.py guvenlik.py veri.py kayitlar.py web.py musteri.py yonetim.py api.py $L/
install -m 755 merkez.py istatistik.py durum.py kuyruk.py yonetici.py $L/
ln -sf $L/istatistik.py /usr/local/sbin/wc-istatistik
ln -sf $L/durum.py /usr/local/sbin/wc-durum
ln -sf $L/kuyruk.py /usr/local/sbin/wc-kuyruk
ln -sf $L/yonetici.py /usr/local/sbin/wc-yonetici
rm -f /usr/local/sbin/wc-hesap
install -m 644 wc-merkez.service wc-kuyruk.path wc-kuyruk.service wc-durum.service wc-durum.timer \
  wc-istatistik.service wc-istatistik.timer /etc/systemd/system/
install -m 644 Caddyfile /etc/caddy/Caddyfile
install -m 755 hotspot-arsiv-saklama /etc/cron.daily/
install -m 644 fail2ban-wc-merkez-filtre.conf /etc/fail2ban/filter.d/wc-merkez.conf
install -m 644 fail2ban-wc-merkez.conf /etc/fail2ban/jail.d/wc-merkez.local
# eski kafe paneli: durdur, hesap dosyasını silmeden kenara al
if systemctl list-unit-files wc-panel.service >/dev/null 2>&1; then systemctl disable -q --now wc-panel 2>/dev/null || true; rm -f /etc/systemd/system/wc-panel.service; fi
[ -f /etc/wificorrect/hesaplar.json ] && mv -n /etc/wificorrect/hesaplar.json /etc/wificorrect/hesaplar.json.eski
systemctl daemon-reload
systemctl enable -q --now wc-kuyruk.path wc-durum.timer wc-istatistik.timer
systemctl enable -q wc-merkez
systemctl restart wc-merkez
systemctl start wc-durum.service
systemctl reload caddy 2>/dev/null || systemctl restart caddy
systemctl reload fail2ban 2>/dev/null || systemctl restart fail2ban
[ -s /var/lib/wificorrect/merkez/yonetici.json ] || echo "Yönetici hesabı yok: sudo wificorrect-sunucu yonetici-parola <kullanıcı-adı>"
echo "kurulum tamam: wc-merkez $(systemctl is-active wc-merkez), kuyruk $(systemctl is-active wc-kuyruk.path)"
```

- [ ] **Step 5: `dagit.sh`**

```bash
#!/usr/bin/env bash
# Sunucu tarafını (wificorrect-sunucu + yönetim merkezi) merkez sunucuya kurar. sudo parolasını terminalde siz girersiniz.
# Kullanım: scripts/sunucu/dagit.sh [gbserver@192.168.1.109]
set -euo pipefail
host=${1:-gbserver@192.168.1.109}
cd "$(dirname "$0")"
tar --exclude=__pycache__ -cf - wificorrect-sunucu merkez \
  | ssh -i ~/.ssh/gbserver "$host" 'rm -rf /tmp/wfc-sunucu && mkdir /tmp/wfc-sunucu && tar -C /tmp/wfc-sunucu -xf -'
ssh -t -i ~/.ssh/gbserver "$host" 'sudo install -m 755 /tmp/wfc-sunucu/wificorrect-sunucu /usr/local/sbin/ && sudo bash /tmp/wfc-sunucu/merkez/kur.sh'
```

- [ ] **Step 6:** `bash -n scripts/sunucu/merkez/kur.sh scripts/sunucu/dagit.sh` → çıktı yok.
- [ ] **Step 7: Commit** `git add scripts/sunucu && git commit -m "wificorrect-sunucu: cihaz-kapat, yonetici-parola; yönetim merkezi kurulum betikleri"`

---

### Task 11: Dağıtım ve uçtan uca doğrulama

**Files:** Modify: `docs/KURULUM_GUNLUGU.md`

- [ ] **Step 1: Sunucuda testler** — tar'ı kopyala (kurmadan) ve testleri Linux'ta koş (sembolik bağ/FIFO testleri burada çalışır):
```bash
cd /c/Users/HUAWEI/Desktop/wificorrect/rza/scripts/sunucu && tar --exclude=__pycache__ -cf - wificorrect-sunucu merkez | ssh -i ~/.ssh/gbserver gbserver@192.168.1.109 'rm -rf /tmp/wfc-sunucu && mkdir /tmp/wfc-sunucu && tar -C /tmp/wfc-sunucu -xf - && cd /tmp/wfc-sunucu/merkez && for f in tests/test_*.py; do python3 "$f" | tail -1; done'
```
Expected: her dosya `TUM TESTLER GECTI`.
- [ ] **Step 2: DNS** — Güzel Hosting DNS yönetiminde `yonetim` ve `api` A kayıtları → <dış-ip> (kullanıcı panele girer,
kaydı tarayıcıdan ekleriz; kök kayda dokunulmaz). `nslookup yonetim.wificorrect.com 8.8.8.8` ve `api` → <dış-ip>.
- [ ] **Step 3: Kurulum** — kullanıcı terminalinde: `scripts/sunucu/dagit.sh` (sudo parolasını kendisi girer), ardından
`sudo wificorrect-sunucu yonetici-parola <kullanıcı-adı>` (parolayı kendisi girer).
Doğrula: `systemctl is-active wc-merkez wc-kuyruk.path wc-durum.timer`; `curl -s -o /dev/null -w "%{http_code}" https://yonetim.wificorrect.com/giris` → 200;
`https://api.wificorrect.com/api/giris` GET → 404 (JSON); `https://wificorrect.com` değişmedi.
- [ ] **Step 4: Bocafe** — kullanıcı yönetime girer: "Yeni müşteri" → not "Bocafe" → 100001 (parolayı kullanıcı saklar).
Eski arşivler → `bocafe` → 100001. Müşteri sayfasında Kayıtlar eski günleri gösterir.
- [ ] **Step 5: Uçtan uca (deneme müşterisi)** — yönetimde "Deneme" müşterisi aç (100002). Cihazda geçici anahtar üret
(uygulamadan): `ssh wificorrect 'd=$(mktemp -d); wg genkey | tee $d/k | wg pubkey; ssh-keygen -q -t ed25519 -N "" -f $d/s -C deneme; cat $d/s.pub; rm -rf $d'`.
PC'den: `curl -s https://api.wificorrect.com/api/giris -H 'Content-Type: application/json' --data-binary @-` ile
`{"numara":"100002","parola":"<kullanıcının verdiği deneme parolası — kullanıcı terminalde yazar>","wg_pub":"…","ssh_pub":"…"}`
→ 200, `tunel_ip` 10.99.0.x. Sunucuda `sudo wificorrect-sunucu liste` → `cihaz 100002`. Yönetimde 100002 → "Cihaz ulaşılamaz,
zorla ayır" → birkaç saniyede `liste`'de `bitti 100002`. Sonra "Üyeliği bitir".
(Parolayı ajan yazmaz: curl komutunu kullanıcı kendi terminalinde çalıştırır ya da deneme müşterisinin parolası yalnızca
kullanıcıya gösterilir ve kullanıcı yapıştırır.)
- [ ] **Step 6: Günlük** — KURULUM_GUNLUGU'na: yeni birimler, DNS kayıtları, eski panelin kapatılması (`hesaplar.json.eski`),
Bocafe = 100001, deneme müşterisi 100002 (bitti), `bocafe-test` tünelinin cihaz planında kapatılacağı.
- [ ] **Step 7: Commit** `git commit -am "Kurulum günlüğü: yönetim merkezi (yonetim/panel/api.wificorrect.com) kuruldu"`
