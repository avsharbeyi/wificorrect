# Kurulum ISO'su — uygulama planı

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Boş bir cihaza tek onayla kurulan WifiCorrect ISO'su; admin parolası ve NetGSM bilgileri merkezden gelir.

**Architecture:** Merkez (Python) cihaz başına admin parolası ve tek SMS ayar takımı tutar, `/api/giris` ve
`/api/eslesme` yanıtlarına ekler. Cihaz (Rust) bunları bağlanırken ve her eşitlemede uygular. ISO = Debian 13 netinst
+ preseed + `dpkg-deb` ile üretilen `wificorrect_<sürüm>_amd64.deb`; GitHub Actions her `v*` etiketinde üretir.

**Tech Stack:** Rust 1.85 (cihaz), Python 3.13 stdlib + SQLite (merkez), dpkg-deb, xorriso, Debian preseed, QEMU/OVMF (duman testi).

**Spec:** `docs/superpowers/specs/2026-10-05-kurulum-iso-design.md`

## Global Constraints

- Kalıba cihaza/işletmeye özel hiçbir şey girmez; kalıp yalnızca repodan üretilir (KURULUM_GUNLUGU "Hedef").
- Açık admin parolası cihaza **gönderilmez**; yalnızca `{tuz, ozet, yineleme}` (PBKDF2-HMAC-SHA256, tuz metnin baytları, hex, 120000).
- Sırlar repoya ve kalıba girmez. Hizmet sağlayıcının **açık** SSH anahtarı yalnızca GitHub değişkeni `WFC_SSH_PUB`'dan.
- Merkez: yalnızca standart kütüphane. Cihaz: yeni crate yok.
- Kullanıcıya görünen metinler Türkçe; kod tanımlayıcıları mevcut kodla aynı dilde (Türkçe ağırlıklı).
- Cihaz testleri **cihazda** çalışır: `bash scripts/test.sh wificorrect [süzgeç]` (bellek sınırlı; release+LTO test derlemesi cihazı kilitler).
- Merkez testleri: `python scripts/sunucu/merkez/tests/test_<ad>.py` (son satır `TUM TESTLER GECTI`), hepsi: `sh scripts/sunucu/merkez/tests/hepsi.sh python`.
- Canlı cihaz (1537344) **silinmez**; ISO yalnızca QEMU'da ve boş yedek makinede denenir.
- Gerçek SMS yalnızca kullanıcının açık onayıyla. Merkez kurulumu (`dagit.sh`) sudo parolasını kullanıcı girer.

## Review Focus

1. Merkez deneme modu kapalı ama NetGSM alanı eksik bir SMS ayarı gönderirse cihaz uygulamamalı (aksi halde portal çöker — 2026-10-05'te yaşandı). → Task 4 testi `ek_eksik_sms_uygulanmaz`.
2. Aynı admin özeti her eşitlemede yeniden yazılırsa admin her gün oturumdan düşer. → Task 1 (özet bir kez üretilir) + Task 4 `set_admin_ozet` değişmeyince `false`.
3. Yönetimde SMS ayarı hiç girilmemişken yanıtta `sms` alanı olmamalı; cihaz yerel ayarını korumalı. → Task 2 `test_sms_ayari_yoksa_alan_yok`.
4. Paket güncellemesi panelin ürettiği dosyaları (ağ, Wi-Fi, yasaklı siteler) ve `ayarlar.toml`'u ezmemeli. → Task 7 kapsayıcı testi.
5. İki Ethernet adının J1900'dakinden farklı olduğu makinede ilk açılışta ağ rolleri doğru seçilmeli. → Task 6 `ilk_rota_ve_varsayilan`.

---

### Task 1: Merkez veritabanı — cihaz admin parolası ve SMS ayarı

**Files:**
- Modify: `scripts/sunucu/merkez/veri.py`
- Test: `scripts/sunucu/merkez/tests/test_veri.py`

**Interfaces:**
- Produces: `cihaz` sütunları `admin_parola, admin_tuz, admin_ozet, admin_yineleme`; `Veri.admin_parola_yenile(numara) -> str|None`;
  `Veri.sms_ayari() -> dict|None` (`{"mock": bool, "usercode", "password", "msgheader", "appkey"}`);
  `Veri.sms_ayari_koy(d: dict) -> None` (ValueError: Türkçe mesaj).

- [ ] **Step 1: Failing tests** — `tests/test_veri.py` sonuna (`if __name__` bloğundan önce):

```python
def test_cihaz_admin_parolasi_bir_kez_uretilir_ve_yenilenir():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        v = yeni(tmp)
        n, _ = v.musteri_ekle()
        cid, _ = v.cihaz_bagla(n, "A" * 43 + "=", "ssh-ed25519 AAAA c")
        c = v.bagli_cihaz(n)
        assert len(c["admin_parola"]) == 16 and c["admin_yineleme"] == 120000
        assert G.dogru(c["admin_parola"], c["admin_tuz"], c["admin_ozet"], c["admin_yineleme"])
        v.cihaz_bagla(n, "A" * 43 + "=", "ssh-ed25519 AAAA c")  # aynı cihaz yeniden giriş: parola değişmez
        assert v.bagli_cihaz(n)["admin_ozet"] == c["admin_ozet"]
        yeni_pw = v.admin_parola_yenile(n)
        c2 = v.bagli_cihaz(n)
        assert yeni_pw == c2["admin_parola"] != c["admin_parola"] and G.dogru(yeni_pw, c2["admin_tuz"], c2["admin_ozet"], 120000)
        assert v.admin_parola_yenile(1234567) is None  # bağlı cihaz yok


def test_sms_ayari():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        v = yeni(tmp)
        assert v.sms_ayari() is None
        try:
            v.sms_ayari_koy({"mock": False, "usercode": "850", "password": "", "msgheader": "BASLIK", "appkey": ""})
            assert False
        except ValueError as h:
            assert "password" in str(h)
        v.sms_ayari_koy({"mock": True, "usercode": "", "password": "", "msgheader": "", "appkey": ""})
        assert v.sms_ayari()["mock"] is True
        v.sms_ayari_koy({"mock": False, "usercode": "8503027084", "password": "gizli-1", "msgheader": "gztp.blgsyr", "appkey": ""})
        assert v.sms_ayari() == {"mock": False, "usercode": "8503027084", "password": "gizli-1", "msgheader": "gztp.blgsyr", "appkey": ""}


def test_eski_cihaz_tablosuna_admin_sutunlari_eklenir():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        yol = os.path.join(tmp, "m.db")
        db = sqlite3.connect(yol)
        db.executescript(V.SEMA.replace("  admin_parola TEXT NOT NULL DEFAULT '', admin_tuz TEXT NOT NULL DEFAULT '',\n"
                                        "  admin_ozet TEXT NOT NULL DEFAULT '', admin_yineleme INTEGER NOT NULL DEFAULT 0,\n", ""))
        db.close()
        v = V.Veri(yol, saat=lambda: 1_790_000_000.0)
        assert {"admin_parola", "admin_ozet"} <= {r[1] for r in v.db.execute("PRAGMA table_info(cihaz)")}
```

Dosyanın başındaki içe aktarmalara `import sqlite3` ve `import guvenlik as G` ekle (yoksa).

- [ ] **Step 2: Run** `python scripts/sunucu/merkez/tests/test_veri.py` — Expected: FAIL (`KeyError`/`AttributeError: admin_parola`).

- [ ] **Step 3: Implement** — `veri.py`:

`SEMA` içindeki `cihaz` tablosunda `ayrilma TEXT NOT NULL DEFAULT ''` satırından önce:

```
  admin_parola TEXT NOT NULL DEFAULT '', admin_tuz TEXT NOT NULL DEFAULT '',
  admin_ozet TEXT NOT NULL DEFAULT '', admin_yineleme INTEGER NOT NULL DEFAULT 0,
```

`SEMA` sonuna: `CREATE TABLE IF NOT EXISTS ayar (anahtar TEXT PRIMARY KEY, deger TEXT NOT NULL);`

`YENI_SUTUNLAR` altına:

```python
YENI_CIHAZ_SUTUNLARI = (("admin_parola", "TEXT NOT NULL DEFAULT ''"), ("admin_tuz", "TEXT NOT NULL DEFAULT ''"),
                        ("admin_ozet", "TEXT NOT NULL DEFAULT ''"), ("admin_yineleme", "INTEGER NOT NULL DEFAULT 0"))
SMS_ALANLARI = ("usercode", "password", "msgheader", "appkey")
ADMIN_PAROLA_UZUNLUK = 16
```

`_gocur` başına:

```python
        var_c = {r[1] for r in self.db.execute("PRAGMA table_info(cihaz)")}
        for ad, tip in YENI_CIHAZ_SUTUNLARI:
            if ad not in var_c:
                self.db.execute(f"ALTER TABLE cihaz ADD COLUMN {ad} {tip}")
```

Modül düzeyinde yardımcı:

```python
def _admin_alanlari():
    """Cihazın admin parolası: açık (yönetimde görünür, kullanıcı kararı 2026-10-05) + cihaza giden özet."""
    pw = guvenlik.parola_uret(ADMIN_PAROLA_UZUNLUK)
    tuz, oz, y = guvenlik.yeni_kayit(pw)
    return pw, tuz, oz, y
```

`cihaz_bagla` içindeki `INSERT` (yeni cihaz) sütunlarına `admin_parola, admin_tuz, admin_ozet, admin_yineleme` ve değerlerine `*_admin_alanlari()` ekle
(yeniden giriş dalı — `if c is not None:` — admin alanlarına dokunmaz):

```python
            cur = db.execute(
                "INSERT INTO cihaz (musteri, durum, wg_pub, ssh_pub, anahtar_ozet, isletme_adi, unvan, surum, baglanma, "
                "admin_parola, admin_tuz, admin_ozet, admin_yineleme) VALUES (?, 'bagli', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (numara, wg_pub, ssh_pub, anahtar_ozeti(anahtar), isletme_adi, unvan, surum, self._simdi(), *_admin_alanlari()))
```

Yeni yöntemler (`cihazlar` yönteminden önce):

```python
    def admin_parola_yenile(self, numara):
        """Bağlı cihazın admin parolasını yeniler; cihaz bir sonraki eşitlemede alır. Dönen: yeni parola ya da None."""
        pw, tuz, oz, y = _admin_alanlari()
        with self._islem() as db:
            cur = db.execute("UPDATE cihaz SET admin_parola = ?, admin_tuz = ?, admin_ozet = ?, admin_yineleme = ? "
                             "WHERE musteri = ? AND durum != 'serbest'", (pw, tuz, oz, y, numara))
        return pw if cur.rowcount else None

    # --- bütün cihazlara giden SMS (NetGSM) ayarı ---
    def sms_ayari(self):
        r = self._bir("SELECT deger FROM ayar WHERE anahtar = 'sms'")
        return json.loads(r["deger"]) if r else None

    def sms_ayari_koy(self, d):
        a = {"mock": bool(d.get("mock"))}
        for k in SMS_ALANLARI:
            a[k] = str(d.get(k) or "").strip()
        eksik = [k for k in ("usercode", "password", "msgheader") if not a[k]]
        if not a["mock"] and eksik:
            raise ValueError(f"Deneme modu kapalıyken şu alanlar gerekli: {', '.join(eksik)}")
        with self._islem() as db:
            # ponytail: NetGSM şifresi açık saklanır (cihaza gönderilmesi gerekir); veritabanı yalnızca wcpanel'in (600)
            db.execute("INSERT OR REPLACE INTO ayar (anahtar, deger) VALUES ('sms', ?)", (json.dumps(a, ensure_ascii=False),))
```

`import json` ekle.

- [ ] **Step 4: Run** `python scripts/sunucu/merkez/tests/test_veri.py` — Expected: `TUM TESTLER GECTI`. Ardından `sh scripts/sunucu/merkez/tests/hepsi.sh python` — hepsi geçer.

- [ ] **Step 5: Commit** — `git add scripts/sunucu/merkez/veri.py scripts/sunucu/merkez/tests/test_veri.py && git commit -m "merkez: cihaz başına admin parolası, ortak SMS ayarı"`

---

### Task 2: Merkez API — yanıtlara admin özeti ve SMS ayarı

**Files:**
- Modify: `scripts/sunucu/merkez/api.py`
- Test: `scripts/sunucu/merkez/tests/test_api.py`

**Interfaces:**
- Consumes: Task 1 — `cihaz` admin sütunları, `Veri.sms_ayari()`.
- Produces: `/api/giris` ve `/api/eslesme` (`durum=bagli`) yanıtında `"admin": {"tuz","ozet","yineleme"}` ve SMS ayarı kayıtlıysa
  `"sms": {"mock": bool, "provider": "netgsm", "netgsm": {"usercode","password","msgheader","appkey"}}`.

- [ ] **Step 1: Failing tests** — `tests/test_api.py` sonuna:

```python
def test_giris_ve_eslesme_admin_ozeti_tasir():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        api, v, _, _ = kur(tmp)
        n, pw = v.musteri_ekle()
        d, j = giris(api, n, pw)
        c = v.bagli_cihaz(n)
        assert d == 200 and j["admin"] == {"tuz": c["admin_tuz"], "ozet": c["admin_ozet"], "yineleme": 120000}
        assert c["admin_parola"] not in json.dumps(j)  # açık parola gitmez
        d, j2 = post(api, "/api/eslesme", {"cihaz_anahtari": j["cihaz_anahtari"]})
        assert d == 200 and j2["admin"] == j["admin"]  # aynı özet: cihaz admin oturumlarını boşuna kapatmaz


def test_sms_ayari_yoksa_alan_yok_varsa_gider():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        api, v, _, _ = kur(tmp)
        n, pw = v.musteri_ekle()
        d, j = giris(api, n, pw)
        assert "sms" not in j
        v.sms_ayari_koy({"mock": False, "usercode": "8503027084", "password": "gizli-1", "msgheader": "gztp.blgsyr", "appkey": ""})
        d, j2 = post(api, "/api/eslesme", {"cihaz_anahtari": j["cihaz_anahtari"]})
        assert j2["sms"] == {"mock": False, "provider": "netgsm",
                             "netgsm": {"usercode": "8503027084", "password": "gizli-1", "msgheader": "gztp.blgsyr", "appkey": ""}}
        d, j3 = post(api, "/api/eslesme", {"cihaz_anahtari": "yanlis"})
        assert d == 401 and "sms" not in j3
```

- [ ] **Step 2: Run** `python scripts/sunucu/merkez/tests/test_api.py` — Expected: FAIL (`KeyError: 'admin'`).

- [ ] **Step 3: Implement** — `api.py`, `_lisans` altına:

```python
def _cihaz_ekleri(veri, c):
    """Cihaza giden admin parola özeti (açık parola gitmez) ve kayıtlıysa bütün cihazların SMS ayarı."""
    ek = {"admin": {"tuz": c["admin_tuz"], "ozet": c["admin_ozet"], "yineleme": c["admin_yineleme"]}}
    s = veri.sms_ayari()
    if s is not None:
        ek["sms"] = {"mock": s["mock"], "provider": "netgsm", "netgsm": {k: s[k] for k in veri_modulu.SMS_ALANLARI}}
    return ek
```

`giris` sonunda `cihaz_guncelle` sonrası cihaz satırını yeniden oku ve ekle:

```python
        c = self.veri.bagli_cihaz(m["numara"])
        return _json(200, dict(_parola_alanlari(m), **_lisans(self.veri, m), **_cihaz_ekleri(self.veri, c),
                               cihaz_anahtari=anahtar, tunel_ip=s["tunel_ip"], sunucu_pub=s["sunucu_pub"], uc_nokta=s["uc_nokta"],
                               yedek_hedefi=f"wfc-{numara}@{YEDEK_SUNUCU}:"))
```

`eslesme` son satırı:

```python
        return _json(200, dict(_parola_alanlari(m), **_lisans(self.veri, m), **_cihaz_ekleri(self.veri, c),
                               durum="bagli", uyelik=m["uyelik"]))
```

(`veri_modulu` zaten içe aktarılı — `BaskaCihaz` için; değilse `import veri as veri_modulu`.)

- [ ] **Step 4: Run** `python scripts/sunucu/merkez/tests/test_api.py` — Expected: `TUM TESTLER GECTI`; `sh scripts/sunucu/merkez/tests/hepsi.sh python` hepsi geçer.

- [ ] **Step 5: Commit** — `git commit -am "merkez API: admin parola özeti ve SMS ayarı cihaza"`

---

### Task 3: Yönetim — admin parolası gösterimi/yenileme, SMS ayarları sayfası

**Files:**
- Modify: `scripts/sunucu/merkez/yonetim.py`
- Test: `scripts/sunucu/merkez/tests/test_yonetim.py`

**Interfaces:**
- Consumes: `Veri.admin_parola_yenile`, `Veri.sms_ayari`, `Veri.sms_ayari_koy`, `cihaz["admin_parola"]`.
- Produces: `POST /m/<n>/admin-parola`, `GET|POST /sms`; menüde "SMS ayarları"; hareketler `ADMIN_PAROLA_YENILENDI`, `SMS_AYARI`.

- [ ] **Step 1: Failing tests** — `tests/test_yonetim.py` sonuna:

```python
def test_admin_parolasi_gorunur_ve_yenilenir():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        app, v, _, c, _ = kur(tmp)
        n, _ = v.musteri_ekle()
        v.cihaz_bagla(n, WG, SSH)
        eski = v.bagli_cihaz(n)["admin_parola"]
        s = Y.al(app, f"/m/{n}", c)[2].decode()
        assert eski in s and "admin parolasını yenile" in s.lower()
        Y.gonder(app, f"/m/{n}/admin-parola", c, {"csrf": Y.csrf(Y.al(app, f"/m/{n}", c))})
        assert v.bagli_cihaz(n)["admin_parola"] != eski
        assert v.hareketler(n)[0]["olay"] == "ADMIN_PAROLA_YENILENDI"


def test_sms_ayarlari_sayfasi():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        app, v, _, c, _ = kur(tmp)
        s = Y.al(app, "/sms", c)[2].decode()
        assert 'name="usercode"' in s and 'name="password"' in s
        t = Y.csrf(Y.al(app, "/sms", c))
        r = Y.gonder(app, "/sms", c, {"csrf": t, "usercode": "850", "msgheader": "BASLIK"})  # deneme modu kapalı, şifre yok
        assert r[0] == 400 and "password" in r[2].decode() and v.sms_ayari() is None
        Y.gonder(app, "/sms", c, {"csrf": t, "usercode": "8503027084", "password": "gizli-1", "msgheader": "gztp.blgsyr"})
        assert v.sms_ayari()["password"] == "gizli-1" and v.sms_ayari()["mock"] is False
        s = Y.al(app, "/sms", c)[2].decode()
        assert "gizli-1" not in s and "tanımlı" in s  # şifre geri gösterilmez
        Y.gonder(app, "/sms", c, {"csrf": t, "usercode": "8503027084", "password": "", "msgheader": "gztp.blgsyr", "mock": "1"})
        assert v.sms_ayari()["password"] == "gizli-1" and v.sms_ayari()["mock"] is True  # boş şifre = aynı kalır
        assert any(h["olay"] == "SMS_AYARI" and "gizli-1" not in h["ayrinti"] for h in v.hareketler())
```

- [ ] **Step 2: Run** `python scripts/sunucu/merkez/tests/test_yonetim.py` — Expected: FAIL (sayfada parola yok / `/sms` 404).

- [ ] **Step 3: Implement** — `yonetim.py`:

`menu`: `'<a href="/eski-arsivler">Eski arşivler</a>'` önüne `'<a href="/sms">SMS ayarları</a>'` ekle.

`sayfalar` içinde `if yol == "/hesabim":` satırından önce:

```python
        if yol == "/sms":
            return self.sms(ot, yontem, form, ip)
```

`/m/<n>` dalında `if (yontem, alt) == ("POST", "/serbest"):` önüne:

```python
        if (yontem, alt) == ("POST", "/admin-parola"):
            if self.veri.admin_parola_yenile(n) is None:
                return yanit_html(self.mesaj(ot, "Bağlı cihaz yok", "Admin parolası yalnızca bağlı cihaz için yenilenir."), 400)
            self.veri.hareket("admin", ip, "ADMIN_PAROLA_YENILENDI", n)
            return yonlendir(f"/m/{n}")
```

`musteri_sayfasi` içinde cihaz bilgi tablosuna `("Tünel adresi", …)` satırından önce:

```python
                ("Cihaz admin parolası", f'<span class="sir">{e(c["admin_parola"])}</span>' if c["admin_parola"]
                 else '<span class="bos">yok (bu özellikten önce bağlandı) — yenileyin</span>'),
```

ve `cihaz += (f'<form method="post" action="/m/{n}/serbest">…` satırından önce:

```python
            cihaz += (f'<form method="post" action="/m/{n}/admin-parola"><input type="hidden" name="csrf" value="{cs}">'
                      '<button>Admin parolasını yenile</button></form>'
                      '<p class="alt">Cihaz yeni parolayı bir sonraki eşitlemede (her gün 06:00) alır.</p>')
```

Yeni yöntem (`lisans` yönteminden önce):

```python
    def sms(self, ot, yontem, form, ip):
        """Bütün cihazların NetGSM ayarı. Şifre yalnızca yazılır; boş bırakılırsa eskisi kalır."""
        eski = self.veri.sms_ayari() or {"mock": True, "usercode": "", "password": "", "msgheader": "", "appkey": ""}
        hata = ""
        if yontem == "POST":
            yeni = {"mock": form.get("mock") == "1", "usercode": form.get("usercode", ""), "msgheader": form.get("msgheader", ""),
                    "appkey": form.get("appkey", ""), "password": form.get("password", "") or eski["password"]}
            try:
                self.veri.sms_ayari_koy(yeni)
            except ValueError as h:
                hata = str(h)
            else:
                degisen = [k for k in ("mock", "usercode", "msgheader", "appkey", "password") if yeni[k] != eski[k]]
                self.veri.hareket("admin", ip, "SMS_AYARI", None, "değişen=" + ",".join(degisen))  # değer yazılmaz
                return yonlendir("/sms")
            eski = dict(yeni, password=eski["password"])
        cs = e(ot["csrf"])
        sifre = "tanımlı" if eski["password"] else "tanımsız"
        icerik = (f'<h1>SMS ayarları</h1>{f"<p class=hata>{e(hata)}</p>" if hata else ""}'
                  f'<form method="post" action="/sms"><input type="hidden" name="csrf" value="{cs}">'
                  f'<label><input type="checkbox" name="mock" value="1"{" checked" if eski["mock"] else ""}> Deneme modu (SMS gönderilmez)</label>'
                  f'<label for="u">NetGSM kullanıcı kodu</label><input id="u" name="usercode" value="{e(eski["usercode"])}" autocomplete="off">'
                  f'<label for="p">API şifresi ({sifre})</label><input id="p" name="password" type="password" autocomplete="new-password" '
                  'placeholder="değiştirmek için yazın (boş = aynı kalır)">'
                  f'<label for="b">Mesaj başlığı</label><input id="b" name="msgheader" value="{e(eski["msgheader"])}" maxlength="11">'
                  f'<label for="a">Uygulama anahtarı (opsiyonel)</label><input id="a" name="appkey" value="{e(eski["appkey"])}">'
                  '<button>Kaydet</button></form>'
                  '<p class="alt">Bütün bağlı cihazlara bir sonraki eşitlemede (her gün 06:00) gider.</p>')
        return yanit_html(self.sayfa("SMS ayarları", kart(icerik), ot), 400 if hata else 200)
```

`hareket(..., musteri=None, ...)` imzası `veri.hareket(kim, ip, olay, musteri=None, ayrinti="")` — uygun.

- [ ] **Step 4: Run** `python scripts/sunucu/merkez/tests/test_yonetim.py` — Expected: `TUM TESTLER GECTI`; `hepsi.sh` hepsi geçer.

- [ ] **Step 5: Commit** — `git commit -am "yönetim: cihaz admin parolası ve SMS ayarları sayfası"`

---

### Task 4: Cihaz — merkez yanıtındaki ekleri ayrıştırma ve uygulama

**Files:**
- Modify: `src/merkez.rs`, `src/hesap.rs`, `src/ayar.rs`
- Test: aynı dosyaların `#[cfg(test)]` modülleri

**Interfaces:**
- Consumes: Task 2 yanıt biçimi.
- Produces:
  - `ayar::Sms { mock, provider, #[serde(default)] merkez: bool }`
  - `hesap::Hesaplar::set_admin_ozet(&self, tuz: &str, ozet: &str, yineleme: u32) -> Result<bool, String>` (değiştiyse `true`)
  - `hesap::Hesaplar::ozet(&self, user: &str) -> Option<String>`
  - `merkez::SmsAyari { mock: bool, usercode, password, msgheader, appkey: String }`
  - `merkez::Ek { admin: Option<(String, String, u32)>, sms: Option<SmsAyari> }` (Default, Clone, Debug, PartialEq)
  - `merkez::Giris.ek: Ek`; `merkez::Eslesme::Bagli { uyelik: String, ek: Ek }`
  - `merkez::ek_uygula(cfg: &mut Config, hesaplar: &Hesaplar, ek: &Ek) -> Result<(bool, bool), String>` → (admin değişti, sms değişti)

- [ ] **Step 1: Failing tests** — `src/hesap.rs` test modülüne:

```rust
    #[test]
    fn admin_ozeti_merkezden() {
        let h = Hesaplar::new(tmp_path("admin-ozet")); // modüldeki mevcut geçici dosya yardımcısını kullan
        let oz = digest("merkez-parola-1", "ab12", 120_000);
        assert!(h.set_admin_ozet("ab12", &oz, 120_000).unwrap());
        assert!(!h.set_admin_ozet("ab12", &oz, 120_000).unwrap()); // aynı: değişmedi
        assert_eq!(h.verify(ADMIN, "merkez-parola-1"), Some(Rol::Hizmet));
        assert_eq!(h.ozet(ADMIN), Some(oz));
        assert!(h.set_admin_ozet("", "zz", 120_000).is_err() && h.set_admin_ozet("ab", "ab", 10).is_err()); // bozuk özet yazılmaz
    }
```

(`tmp_path` adı modülde farklıysa — ör. `tmp` — onu kullan; yeni yardımcı yazma.)

`src/merkez.rs` test modülüne:

```rust
    const EK: &str = r#""admin":{"tuz":"ab12","ozet":"OZET","yineleme":120000},
        "sms":{"mock":false,"provider":"netgsm","netgsm":{"usercode":"8503027084","password":"gizli-1","msgheader":"gztp.blgsyr","appkey":""}}"#;

    fn ekli(govde: &str, admin_ozet: &str) -> &'static str {
        let g = govde.trim_end_matches('}').to_string() + "," + &EK.replace("OZET", admin_ozet) + "}";
        Box::leak(g.into_boxed_str())
    }

    #[test]
    fn giris_ve_eslesme_ekleri() {
        let oz = crate::hesap::digest("merkez-parola-1", "ab12", 120_000);
        let (h, _) = sahte(vec![Ok((200, ekli(GIRIS_OK, &oz)))]);
        let g = giris(N, "parola-12345", "W", "S", "", "", &*h, 0.0).unwrap();
        assert_eq!(g.ek.admin, Some(("ab12".into(), oz.clone(), 120_000)));
        assert_eq!(g.ek.sms.as_ref().map(|s| (s.mock, s.msgheader.as_str())), Some((false, "gztp.blgsyr")));
        let (h, _) = sahte(vec![Ok((200, GIRIS_OK))]); // eski merkez: ek yok
        assert_eq!(giris(N, "parola-12345", "W", "S", "", "", &*h, 0.0).unwrap().ek, Ek::default());
        let mut m = ornek();
        let b = ekli(r#"{"durum":"bagli","tuz":"a1b2c3d4","ozet":"ee38d08b3d6573ecc281263ad6259d2e7ee39f37d6b8958b3180906580a44495","yineleme":120000,"uyelik":"aktif"}"#, &oz);
        let (h, _) = sahte(vec![Ok((200, b))]);
        assert!(matches!(eslesme(&mut m, "", "", false, &*h, 1.0), Ok(Eslesme::Bagli { ek, .. }) if ek.admin.is_some() && ek.sms.is_some()));
    }

    #[test]
    fn ek_uygula_sms_ve_admin() {
        let d = tmp("ek");
        let hs = crate::hesap::Hesaplar::new(d.join("hesaplar.json"));
        let oz = crate::hesap::digest("merkez-parola-1", "ab12", 120_000);
        let ek = Ek { admin: Some(("ab12".into(), oz, 120_000)),
                      sms: Some(SmsAyari { mock: false, usercode: "8503027084".into(), password: "gizli-1".into(), msgheader: "gztp.blgsyr".into(), appkey: String::new() }) };
        let mut cfg = Config::default();
        assert_eq!(ek_uygula(&mut cfg, &hs, &ek).unwrap(), (true, true));
        assert!(!cfg.sms.mock && cfg.sms.merkez && cfg.netgsm.password == "gizli-1" && cfg.sms.provider == "netgsm");
        assert!(cfg.sms_missing().is_empty());
        assert_eq!(ek_uygula(&mut cfg, &hs, &ek).unwrap(), (false, false)); // aynı değerler: dokunulmaz
    }

    #[test]
    fn ek_eksik_sms_uygulanmaz() {
        // deneme modu kapalı ama başlık yok: uygulanırsa portal açılmaz (2026-10-05)
        let d = tmp("ek-eksik");
        let hs = crate::hesap::Hesaplar::new(d.join("hesaplar.json"));
        let ek = Ek { admin: None, sms: Some(SmsAyari { mock: false, usercode: "850".into(), password: "x".into(), msgheader: String::new(), appkey: String::new() }) };
        let mut cfg = Config::default();
        assert!(ek_uygula(&mut cfg, &hs, &ek).is_err());
        assert!(cfg.sms.mock && !cfg.sms.merkez);
    }
```

- [ ] **Step 2: Run** `bash scripts/test.sh wificorrect "ek_ admin_ozeti"` — Expected: derleme hatası (`Ek`, `set_admin_ozet` yok).
  (Süzgeç tek kelime alır; iki kez çalıştır: `... ek_` ve `... admin_ozeti`.)

- [ ] **Step 3: Implement**

`src/ayar.rs` — `Sms`:

```rust
pub struct Sms {
    pub mock: bool,
    pub provider: String,
    /// Bağlı cihazda NetGSM bilgileri ve deneme modu yönetim merkezinden gelir; panelde salt okunur.
    #[serde(default)]
    pub merkez: bool,
}
```

`Default`: `Sms { mock: true, provider: "netgsm".into(), merkez: false }`.

`src/hesap.rs` — `impl Hesaplar` içine:

```rust
    /// Merkezden gelen admin özeti. Dönen: değişti mi (aynıysa dosyaya dokunulmaz, oturumlar düşmez).
    pub fn set_admin_ozet(&self, tuz: &str, ozet: &str, yineleme: u32) -> Result<bool, String> {
        if tuz.is_empty() || ozet.len() != 64 || !ozet.bytes().all(|b| b.is_ascii_hexdigit()) || yineleme < 10_000 {
            return Err("merkezden gelen admin özeti geçersiz".into());
        }
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut m = self.load()?;
        if m.get(ADMIN).is_some_and(|h| h.tuz == tuz && h.ozet == ozet && h.yineleme == yineleme) {
            return Ok(false);
        }
        m.insert(ADMIN.into(), Hesap { rol: Rol::Hizmet, tuz: tuz.into(), ozet: ozet.into(), yineleme });
        self.save(&m)?;
        Ok(true)
    }

    pub fn ozet(&self, user: &str) -> Option<String> {
        self.load().ok()?.get(user).map(|h| h.ozet.clone())
    }
```

`src/merkez.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct SmsAyari {
    pub mock: bool,
    pub usercode: String,
    pub password: String,
    pub msgheader: String,
    pub appkey: String,
}

/// Merkezin bağlı cihaza gönderdiği ekler (spec 2026-10-05 §5-6). Eski merkez göndermezse boş.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ek {
    pub admin: Option<(String, String, u32)>,
    pub sms: Option<SmsAyari>,
}

fn ek(v: &Value) -> Ek {
    let sms = v.get("sms").and_then(|s| {
        let n = s.get("netgsm")?;
        Some(SmsAyari { mock: s.get("mock")?.as_bool()?, usercode: metin(n, "usercode")?, password: metin(n, "password")?,
                        msgheader: metin(n, "msgheader")?, appkey: metin(n, "appkey").unwrap_or_default() })
    });
    Ek { admin: v.get("admin").and_then(ozet_alanlari), sms }
}

/// Bağlanırken ve her eşitlemede: admin özeti hesaplara, SMS ayarı `cfg`'ye (kaydetmek çağıranın işi).
/// Deneme modu kapalı ama zorunlu alan eksik SMS ayarı uygulanmaz (portal eksik ayarla açılmaz).
pub fn ek_uygula(cfg: &mut Config, hesaplar: &crate::hesap::Hesaplar, ek: &Ek) -> Result<(bool, bool), String> {
    let admin = match &ek.admin {
        Some((tuz, ozet, y)) => hesaplar.set_admin_ozet(tuz, ozet, *y)?,
        None => false,
    };
    let Some(s) = &ek.sms else { return Ok((admin, false)) };
    if !s.mock && (s.usercode.is_empty() || s.password.is_empty() || s.msgheader.is_empty()) {
        return Err("merkezden gelen SMS ayarı eksik (deneme modu kapalı); uygulanmadı".into());
    }
    let once = (cfg.sms.clone(), cfg.netgsm.usercode.clone(), cfg.netgsm.password.clone(), cfg.netgsm.msgheader.clone(), cfg.netgsm.appkey.clone());
    cfg.sms.mock = s.mock;
    cfg.sms.provider = "netgsm".into();
    cfg.sms.merkez = true;
    cfg.netgsm.usercode.clone_from(&s.usercode);
    cfg.netgsm.password.clone_from(&s.password);
    cfg.netgsm.msgheader.clone_from(&s.msgheader);
    cfg.netgsm.appkey.clone_from(&s.appkey);
    let sonra = (cfg.sms.clone(), cfg.netgsm.usercode.clone(), cfg.netgsm.password.clone(), cfg.netgsm.msgheader.clone(), cfg.netgsm.appkey.clone());
    Ok((admin, once != sonra))
}
```

(`Sms` için `#[derive(PartialEq)]` yoksa ekle.)

`Giris` yapısına `pub ek: Ek,`; `giris` içindeki `Some(Giris { … })` sonuna `ek: ek(&v),`.
`Eslesme::Bagli { uyelik: String, ek: Ek }`; `eslesme` içinde `Ok(Eslesme::Bagli { uyelik: …, ek: ek(&v) })`.
Mevcut `Eslesme::Bagli { uyelik }` desenleri (`ctl.rs`, testler) `Eslesme::Bagli { uyelik, .. }` olur.

- [ ] **Step 4: Run** `bash scripts/test.sh wificorrect ek_` ve `bash scripts/test.sh wificorrect admin_ozeti` — Expected: geçer. Ardından tam takım `bash scripts/test.sh wificorrect` — `test result: ok`.

- [ ] **Step 5: Commit** — `git commit -am "cihaz: merkezden admin özeti ve SMS ayarı (ayrıştırma, uygulama)"`

---

### Task 5: Cihaz — bağlanma/eşitlemede uygulama, admin oturumu, salt okunur SMS alanları

**Files:**
- Modify: `src/panel.rs`, `src/ctl.rs`
- Test: `src/panel.rs` test modülü

**Interfaces:**
- Consumes: Task 4 — `ek_uygula`, `Giris.ek`, `Eslesme::Bagli { ek, .. }`, `Hesaplar::ozet`, `Sms.merkez`.
- Produces: denetim olayları `ADMIN_PAROLA_MERKEZ`, `SMS_AYARI_MERKEZ`, `MERKEZ_EK_HATA`; admin oturumunun `surum`'u = admin özeti.

- [ ] **Step 1: Failing tests** — `src/panel.rs` test modülüne:

```rust
    #[test]
    fn admin_oturumu_parola_degisince_duser() {
        let e = env();
        let (tok, _) = setup_and_login(&e, "admin", "hizmet-parola-1");
        assert_eq!(e.p.handle(&req("GET", "/admin-ayarlari", &[], Some(&tok))).status, 200);
        let oz = crate::hesap::digest("merkez-parola-1", "ab12", 120_000);
        e.p.hesaplar.set_admin_ozet("ab12", &oz, 120_000).unwrap();
        let r = e.p.handle(&req("GET", "/admin-ayarlari", &[], Some(&tok)));
        assert!(r.status == 303 && r.headers.iter().any(|(_, v)| v == "/giris"));
    }

    #[test]
    fn merkezden_sms_alanlari_salt_okunur() {
        let e = env();
        let (tok, csrf) = setup_and_login(&e, "admin", "hizmet-parola-1");
        let mut c = Config::load(&e.p.cfg_path).unwrap();
        (c.sms.merkez, c.sms.mock, c.netgsm.usercode, c.netgsm.password, c.netgsm.msgheader) =
            (true, false, "8503027084".into(), "gizli-1".into(), "gztp.blgsyr".into());
        c.save(&e.p.cfg_path).unwrap();
        let page = e.p.handle(&req("GET", "/admin-ayarlari", &[], Some(&tok))).body;
        assert!(page.contains("merkezden yönetiliyor") && !page.contains("name=\"netgsm.usercode\"") && !page.contains("gizli-1"));
        e.p.handle(&req("POST", "/admin-ayarlari", &[("csrf", &csrf), ("netgsm.usercode", "999"), ("limits.sms_global_day", "250")], Some(&tok)));
        let c = Config::load(&e.p.cfg_path).unwrap();
        assert_eq!((c.netgsm.usercode.as_str(), c.sms.mock, c.limits.sms_global_day), ("8503027084", false, 250)); // sms.mock kutusu yok sayıldı
    }
```

(`c.limits.sms_global_day` alan adı `ayar.rs`'deki `Limits` ile aynı olmalı; farklıysa oradaki adı kullan.)

- [ ] **Step 2: Run** `bash scripts/test.sh wificorrect admin_oturumu` ve `... merkezden_sms` — Expected: FAIL.

- [ ] **Step 3: Implement** — `src/panel.rs`:

Sabit (ALANLAR yakınında):

```rust
/// Bağlı cihazda yönetim merkezinden gelen alanlar (spec 2026-10-05 §6): panelde salt okunur, formdan değişmez.
const MERKEZ_SMS: &[&str] = &["sms.mock", "sms.provider", "netgsm.usercode", "netgsm.msgheader", "netgsm.appkey", "netgsm.password"];
```

`validate` içinde `for a in fields(admin) {` hemen altına:

```rust
        if cfg.sms.merkez && MERKEZ_SMS.contains(&a.key) {
            continue;
        }
```

`ayarlar` (render) içinde `let html = match a.tur {` satırından önce:

```rust
            if cfg.sms.merkez && MERKEZ_SMS.contains(&a.key) {
                let deger = if matches!(a.tur, Tur::Gizli(_)) { (if cur.is_empty() { "tanımsız" } else { "tanımlı" }).to_string() }
                            else if matches!(a.tur, Tur::Evet) { (if cur == "1" { "açık" } else { "kapalı" }).to_string() } else { cur.clone() };
                let html = format!("<p class=\"not\">{}: <b>{}</b> (merkezden yönetiliyor)</p>", h(a.label), h(&deger));
                match groups.iter_mut().find(|(g, _)| *g == a.grup) {
                    Some((_, s)) => s.push_str(&html),
                    None => groups.push((a.grup, html)),
                }
                continue;
            }
```

Admin girişi (`giris_post`): `Some(r) => (r, String::new()),` → `Some(r) => (r, self.hesaplar.ozet(hesap::ADMIN).unwrap_or_default()),`

Oturum denetimi (`if o.rol == Rol::Sahip && !self.sahip_oturumu_gecerli(&o) {`) → 

```rust
        let gecerli = match o.rol {
            Rol::Sahip => self.sahip_oturumu_gecerli(&o),
            Rol::Hizmet => self.hesaplar.ozet(hesap::ADMIN).is_some_and(|z| hesap::ct_eq(&z, &o.surum)),
        };
        if !gecerli {
```

`merkeze_baglan` içinde `crate::merkez::uygula(&mut yeni, &g);` sonrasına:

```rust
        let ek = crate::merkez::ek_uygula(&mut yeni, &self.hesaplar, &g.ek);
```

`self.audit(&yeni, req, None, "PANEL_MERKEZ_BAGLANDI", …)` sonrasına:

```rust
        match ek {
            Ok((admin, sms)) => {
                if admin { self.audit(&yeni, req, None, "ADMIN_PAROLA_MERKEZ", ""); }
                if sms {
                    self.audit(&yeni, req, None, "SMS_AYARI_MERKEZ", "");
                    (self.runner)(&cmd(&["systemctl", "restart", "wificorrect-portal"]));
                }
            }
            Err(e) => self.audit(&yeni, req, None, "MERKEZ_EK_HATA", &e),
        }
```

`src/ctl.rs` — `merkez-eslesme` `Ok(Bagli { uyelik })` dalı:

```rust
                Ok(crate::merkez::Eslesme::Bagli { uyelik, ek }) => {
                    if let Err(e) = crate::merkez::kaydet(p, &m) {
                        eprintln!("{e}");
                        return ExitCode::from(1);
                    }
                    denetim("MERKEZ_ESLESME", format!("uyelik={uyelik}"));
                    let mut yeni = cfg.clone();
                    match crate::merkez::ek_uygula(&mut yeni, &crate::hesap::Hesaplar::new(crate::hesap::PATH), &ek) {
                        Ok((admin, sms)) => {
                            if admin { denetim("ADMIN_PAROLA_MERKEZ", String::new()); }
                            if sms {
                                match yeni.save(&cfg_path) {
                                    Ok(()) => {
                                        denetim("SMS_AYARI_MERKEZ", String::new()); // şifre yazılmaz
                                        runner(&["systemctl".into(), "restart".into(), "wificorrect-portal".into()]);
                                    }
                                    Err(e) => denetim("MERKEZ_EK_HATA", e),
                                }
                            }
                        }
                        Err(e) => denetim("MERKEZ_EK_HATA", e),
                    }
                    ExitCode::SUCCESS
                }
```

`cfg_path`: `ctl.rs`'de ayar yolunun tutulduğu değişkeni kullan (`main.rs` `WFC_AYAR` ya da `/etc/wificorrect/ayarlar.toml`); adı farklıysa onu kullan.

- [ ] **Step 4: Run** iki testi, sonra tam takım `bash scripts/test.sh wificorrect` — Expected: `test result: ok`.

- [ ] **Step 5: Commit** — `git commit -am "cihaz: merkez eklerini bağlanma/eşitlemede uygula; admin oturumu özete bağlı; SMS alanları salt okunur"`

---

### Task 6: Cihaz — ilk kurulumda ağ rollerini belirleme (`ctl ag-ilk`)

**Files:**
- Modify: `src/ag.rs`, `src/ctl.rs`
- Test: `src/ag.rs` test modülü

**Interfaces:**
- Produces: `ag::ilk(cfg: &Config, y: &Yollar, sys: &Path, rota: Option<&str>) -> Result<bool, String>` (ag.toml zaten varsa `Ok(false)`, hiçbir şeye dokunmaz);
  `ag::rota_arayuzu(ip_route: &str) -> Option<String>`; `wificorrect ctl ag-ilk` (kurulum paketi çağırır).

- [ ] **Step 1: Failing test** — `src/ag.rs` test modülüne:

```rust
    #[test]
    fn ilk_rota_ve_varsayilan() {
        fn sys_kur(ad: &str, adlar: &[&str]) -> std::path::PathBuf {
            let d = std::env::temp_dir().join(format!("wfc-ag-ilk-{}-{ad}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            for n in adlar {
                std::fs::create_dir_all(d.join("net").join(n).join("device")).unwrap();
            }
            d
        }
        assert_eq!(rota_arayuzu("default via 192.168.1.1 dev eno1 proto dhcp src 192.168.1.50 metric 100\n").as_deref(), Some("eno1"));
        assert_eq!(rota_arayuzu(""), None);
        let cfg = Config::default();
        // farklı donanım: kurulumun internete çıktığı arayüz internet alır
        let d = sys_kur("farkli", &["eno1", "enp2s0"]);
        let y = Yollar::test(&d); // mevcut test yardımcısı; yoksa testlerde kullanılan Yollar kurulumunu kullan
        assert!(ilk(&cfg, &y, &d.join("net"), Some("eno1")).unwrap());
        let ag = load(&y.ag);
        assert_eq!((ag.wan.as_str(), ag.lan.clone()), ("eno1", vec!["enp2s0".to_string()]));
        assert!(std::fs::read_to_string(&y.interfaces).unwrap().contains("bridge_ports enp2s0"));
        assert!(!ilk(&cfg, &y, &d.join("net"), Some("enp2s0")).unwrap()); // ag.toml var: dokunulmaz
        // rota yok, J1900 adları var: varsayılan
        let d = sys_kur("j1900", &["enp1s0", "enp3s0"]);
        let y = Yollar::test(&d);
        assert!(ilk(&cfg, &y, &d.join("net"), None).unwrap());
        assert_eq!(load(&y.ag).wan, "enp3s0");
        // tek Ethernet: hata
        let d = sys_kur("tek", &["eno1"]);
        assert!(ilk(&cfg, &Yollar::test(&d), &d.join("net"), Some("eno1")).is_err());
    }
```

(`Yollar` test kurucusu modülde başka adla varsa — mevcut testlerin kullandığı — onu kullan; yoksa test modülüne
`impl Yollar { fn test(d: &Path) -> Yollar { Yollar { ag: d.join("ag.toml"), interfaces: d.join("interfaces"), nft: d.join("arayuzler.nft"), hostapd: d.join("hostapd.conf"), issue: d.join("issue"), durum: d.join("durum") } } }` ekle.)

- [ ] **Step 2: Run** `bash scripts/test.sh wificorrect ilk_rota` — Expected: derleme hatası (`ilk` yok).

- [ ] **Step 3: Implement** — `src/ag.rs`:

```rust
/// `ip -o route show default` çıktısından arayüz adı.
pub fn rota_arayuzu(ip_route: &str) -> Option<String> {
    let mut it = ip_route.split_whitespace();
    while let Some(w) = it.next() {
        if w == "dev" {
            return it.next().map(String::from);
        }
    }
    None
}

/// Kurulum (paket postinst): ag.toml yoksa rolleri belirler ve dosyaları yazar; ağı geçirmez (kurucunun içinde çalışır).
/// İnternet alan: kurulumun internete çıktığı arayüz (varsayılan rota) → yoksa bu kutunun varsayılanı → yoksa Ethernet 1.
pub fn ilk(cfg: &Config, y: &Yollar, sys: &Path, rota: Option<&str>) -> Result<bool, String> {
    if y.ag.exists() {
        return Ok(false);
    }
    let eths = ethernets(sys);
    let v = Ag::default();
    let wan = match rota {
        Some(r) if eths.iter().any(|e| e == r) => r.to_string(),
        _ if eths.contains(&v.wan) && v.lan.iter().all(|l| eths.contains(l)) => v.wan.clone(),
        _ => eths.first().cloned().ok_or("Ethernet bulunamadı")?,
    };
    let ag = Ag { lan: eths.iter().filter(|e| **e != wan).cloned().collect(), wan, ..Ag::default() };
    if ag.lan.is_empty() {
        return Err("En az iki Ethernet gerekli (biri internet alır, biri misafirlere verir).".into());
    }
    save(&y.ag, &ag)?;
    write_files(cfg, &ag, y)?;
    Ok(true)
}
```

`src/ctl.rs` komut eşlemesine (`"ag-uygula"` dalının yanına, ayrı kol):

```rust
        // Kurulum paketi (postinst): ilk açılıştan önce ağ rolleri ve dosyaları; ag.toml varsa dokunmaz
        Some("ag-ilk") => {
            let rota = std::process::Command::new("ip").args(["-o", "route", "show", "default"]).output()
                .ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).and_then(|s| crate::ag::rota_arayuzu(&s));
            match crate::ag::ilk(&cfg, &crate::ag::Yollar::sistem(), std::path::Path::new("/sys/class/net"), rota.as_deref()) {
                Ok(yazildi) => {
                    println!("{}", if yazildi { "ağ rolleri yazıldı" } else { "ag.toml var, dokunulmadı" });
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(1)
                }
            }
        }
```

Kullanım metnindeki `ag-uygula | …` satırına `| ag-ilk` ekle.

- [ ] **Step 4: Run** `bash scripts/test.sh wificorrect ilk_rota` sonra tam takım — Expected: geçer.

- [ ] **Step 5: Commit** — `git commit -am "cihaz: ctl ag-ilk — kurulumda ağ rolleri (farklı Ethernet adları)"`

---

### Task 7: `.deb` paketi ve açılış ayarları

**Files:**
- Create: `deploy/debian/etc/default/grub.d/wificorrect.cfg`, `paket/postinst`, `paket/control.in`, `scripts/paket.sh`, `scripts/paket-test.sh`
- Modify: `.github/workflows/test.yml`

**Interfaces:**
- Consumes: Task 6 `wificorrect ctl ag-ilk`.
- Produces: `scripts/paket.sh <ikili> <sürüm> <çıktı-klasörü>` → `<çıktı>/wificorrect_<sürüm>_amd64.deb`.

- [ ] **Step 1: Test betiği (önce başarısız)** — `scripts/paket-test.sh`:

```sh
#!/bin/sh
# .deb'i temiz Debian 13 kapsayıcısında kurar ve denetler (CI). Kullanım: scripts/paket-test.sh dist/wificorrect_X_amd64.deb
set -eu
deb=$1
docker run --rm -v "$PWD/$(dirname "$deb"):/p:ro" debian:trixie sh -euc "
  apt-get update -q >/dev/null
  DEBIAN_FRONTEND=noninteractive apt-get install -y -q /p/$(basename "$deb") >/dev/null
  test -x /usr/local/bin/wificorrect
  /usr/local/bin/wificorrect surum
  test -f /etc/wificorrect/ayarlar.toml && test \"\$(stat -c %a /etc/wificorrect/ayarlar.toml)\" = 600
  grep -q GRUB_TERMINAL=console /etc/default/grub.d/wificorrect.cfg
  test -d /srv/5651 && test \"\$(stat -c %a /srv/5651)\" = 700
  test -L /etc/systemd/system/multi-user.target.wants/wificorrect-panel.service
  test ! -e /etc/network/interfaces.d/wificorrect.dpkg-new
  echo 'site_name = \"Kalsin\"' >> /etc/wificorrect/ayarlar.toml
  echo 'elle' > /etc/wificorrect/yasak-siteler.conf
  DEBIAN_FRONTEND=noninteractive dpkg -i /p/$(basename "$deb") >/dev/null
  grep -q Kalsin /etc/wificorrect/ayarlar.toml
  grep -q elle /etc/wificorrect/yasak-siteler.conf
  echo PAKET TESTI GECTI
"
```

Run: `sh scripts/paket-test.sh dist/yok.deb` — Expected: FAIL (dosya yok). (Docker yalnızca CI'da; yerelde Windows'ta çalıştırılmaz.)

- [ ] **Step 2: Açılış ayarı** — `deploy/debian/etc/default/grub.d/wificorrect.cfg`:

```sh
# WifiCorrect (2026-10-05, cihazda ölçüldü): monitörsüz açılışta grafik GRUB takılıyordu → metin konsolu.
# J1900 (Bay Trail) derin C-state donması → intel_idle.max_cstate=1. Seri konsol: QEMU duman testi ve servis.
GRUB_TERMINAL=console
GRUB_TIMEOUT=3
GRUB_CMDLINE_LINUX_DEFAULT="quiet intel_idle.max_cstate=1 console=tty0 console=ttyS0,115200n8"
```

- [ ] **Step 3: Paket dosyaları** — `paket/control.in`:

```
Package: wificorrect
Version: @SURUM@
Architecture: amd64
Maintainer: WifiCorrect <destek@wificorrect.com>
Depends: dnsmasq, hostapd, iw, wireless-regdb, conntrack, nftables, wireguard-tools, rsync, curl, ca-certificates, openssl, openssh-server, openssh-client, bridge-utils, ifupdown, dhcpcd-base, iproute2, unattended-upgrades
Section: net
Priority: optional
Description: WifiCorrect 5651 uyumlu SMS doğrulamalı misafir internet cihazı
```

`paket/postinst`:

```sh
#!/bin/sh
# Kurulum ve güncelleme. Panelin ürettiği dosyalar (ağ, Wi-Fi, yasaklı siteler) ve ayarlar pakette yok → ezilmez.
set -e
[ "$1" = configure ] || exit 0
install -d -m 700 /srv/5651 /srv/hotspot /srv/hotspot/state
install -d -m 755 /var/lib/wificorrect
[ -e /etc/wificorrect/ayarlar.toml ] || install -m 600 /etc/wificorrect/ayarlar.ornek.toml /etc/wificorrect/ayarlar.toml
[ -e /etc/wificorrect/yasak-siteler.conf ] || : > /etc/wificorrect/yasak-siteler.conf
/usr/local/bin/wificorrect ctl ag-ilk || echo "wificorrect: ağ rolleri belirlenemedi; panel → Portlar" >&2
systemctl mask hostapd.service >/dev/null 2>&1 || true
systemctl disable nftables.service >/dev/null 2>&1 || true
systemctl enable dnsmasq.service wificorrect-guvenlik.service wificorrect-ag-acilis.service wificorrect-kaydedici.service \
  wificorrect-panel.service wificorrect-portal.service wificorrect-wifi.service wificorrect-gece.timer wificorrect-gun-kapat.timer >/dev/null
update-grub >/dev/null 2>&1 || true
# Çalışan cihazda güncelleme: servisler yeni ikiliyle açılsın (kurucunun içinde systemd çalışmaz)
if [ -d /run/systemd/system ]; then
  systemctl daemon-reload
  systemctl try-restart wificorrect-portal.service wificorrect-kaydedici.service wificorrect-panel.service
fi
```

`scripts/paket.sh`:

```sh
#!/bin/sh
# deploy/debian + ikili → .deb (dpkg-deb, ek araç yok). Kullanım: scripts/paket.sh target/release/wificorrect 0.3.0 dist
set -eu
ikili=$1 surum=$2 cikti=$3
kok=$(mktemp -d)
cd "$(dirname "$0")/.."
cp -a deploy/debian/. "$kok/"
# panelin / programın ürettiği dosyalar pakete girmez (güncellemede ezilmesin)
rm -f "$kok/etc/network/interfaces.d/wificorrect" "$kok/etc/wificorrect/arayuzler.nft" \
      "$kok/etc/issue.d/wificorrect.issue" "$kok/etc/wificorrect/yasak-siteler.conf"
install -D -m 755 "$ikili" "$kok/usr/local/bin/wificorrect"
mkdir -p "$kok/DEBIAN"
sed "s/@SURUM@/$surum/" paket/control.in > "$kok/DEBIAN/control"
install -m 755 paket/postinst "$kok/DEBIAN/postinst"
# conffiles yok: /etc altındaki ürün dosyaları her güncellemede repodaki haline döner (kural: kalıp yalnızca repodan)
mkdir -p "$cikti"
dpkg-deb --root-owner-group --build "$kok" "$cikti/wificorrect_${surum}_amd64.deb"
rm -rf "$kok"
```

`ayarlar.ornek.toml` pakette `/etc/wificorrect/ayarlar.ornek.toml` olarak kalır (postinst ilk kurulumda kopyalar).

- [ ] **Step 4: CI** — `.github/workflows/test.yml`'a iş ekle:

```yaml
  paket:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - run: sudo apt-get update -q && sudo apt-get install -y -q wireguard-tools openssh-client
      - uses: dtolnay/rust-toolchain@1.85
      - uses: Swatinem/rust-cache@v2
      - run: cargo build --release --locked
      - run: sh scripts/paket.sh target/release/wificorrect 0.0.0-ci dist
      - run: sh scripts/paket-test.sh dist/wificorrect_0.0.0-ci_amd64.deb
```

- [ ] **Step 5: Run** — dalı it, CI'da `paket` işi: Expected son satır `PAKET TESTI GECTI`. Başarısızsa kapsayıcı çıktısını oku, kök nedeni düzelt.

- [ ] **Step 6: Commit** — `git add deploy/debian/etc/default paket scripts/paket.sh scripts/paket-test.sh .github/workflows/test.yml && git commit -m "paket: .deb, postinst, açılış ayarları repoda; CI kapsayıcı testi"`

---

### Task 8: ISO üretimi, sürüm iş akışı, QEMU duman testi

**Files:**
- Create: `iso/preseed.cfg`, `iso/son.sh`, `iso/grub.cfg`, `iso/txt.cfg`, `scripts/iso.sh`, `scripts/iso-test.sh`
- Modify: `.github/workflows/release.yml`

**Interfaces:**
- Consumes: Task 7 `.deb`.
- Produces: `scripts/iso.sh <deb> <sürüm> <çıktı-klasörü>` → `<çıktı>/wificorrect-kurulum-<sürüm>.iso`; ortam `WFC_SSH_PUB` (opsiyonel).

- [ ] **Step 1: Duman testi (önce başarısız)** — `scripts/iso-test.sh`:

```sh
#!/bin/sh
# ISO'yu QEMU'da boş diske kurar, diskten açar, seri konsolda servisleri görür. KVM yoksa atlar (rapora yazılır).
# Kullanım: scripts/iso-test.sh dist/wificorrect-kurulum-X.iso
set -eu
iso=$1 d=$(mktemp -d)
[ -w /dev/kvm ] || { echo "ISO DUMAN TESTI ATLANDI: KVM yok"; exit 0; }
qemu-img create -f qcow2 "$d/disk.qcow2" 8G >/dev/null
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$d/vars.fd"
ortak="-enable-kvm -m 2048 -smp 2 -nographic -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd
 -drive if=pflash,format=raw,file=$d/vars.fd -drive file=$d/disk.qcow2,if=virtio
 -netdev user,id=n0 -device virtio-net-pci,netdev=n0 -netdev user,id=n1,restrict=on -device virtio-net-pci,netdev=n1"
# 1) kurulum: menü kendiliğinden başlamaz → 15 sn sonra Enter; kurucu bitince makineyi kapatır
( sleep 15; echo "sendkey ret" ) | timeout 2400 qemu-system-x86_64 $ortak -cdrom "$iso" -boot d \
  -monitor stdio -serial "file:$d/kurulum.log" >/dev/null
# 2) diskten açılış: seri konsolda servisler ve yönetim adresi
timeout 300 qemu-system-x86_64 $ortak -serial "file:$d/acilis.log" -monitor none -display none &
sleep 240
kill %1 2>/dev/null || true
grep -q "WifiCorrect yonetim paneli" "$d/acilis.log" || { tail -50 "$d/acilis.log"; echo "ISO DUMAN TESTI BASARISIZ"; exit 1; }
grep -q "WifiCorrect yonetim adresi" "$d/acilis.log" || { tail -50 "$d/acilis.log"; echo "ISO DUMAN TESTI BASARISIZ (issue)"; exit 1; }
echo "ISO DUMAN TESTI GECTI"
```

Run (CI'da, Step 6 sonrası) — ISO yokken Expected: FAIL.

- [ ] **Step 2: Preseed** — `iso/preseed.cfg`:

```
# WifiCorrect kurulumu: soru sorulmaz. Disk = kurulum ortamı olmayan ilk disk, tamamı silinir.
d-i debian-installer/locale string tr_TR.UTF-8
d-i keyboard-configuration/xkb-keymap select tr
d-i netcfg/choose_interface select auto
d-i netcfg/get_hostname string wificorrect
d-i netcfg/get_domain string
d-i netcfg/hostname string wificorrect
d-i hw-detect/load_firmware boolean false
d-i mirror/country string manual
d-i mirror/http/hostname string deb.debian.org
d-i mirror/http/directory string /debian
d-i mirror/http/proxy string
d-i passwd/root-login boolean true
d-i passwd/make-user boolean false
d-i passwd/root-password-crypted password !
d-i clock-setup/utc boolean true
d-i time/zone string Europe/Istanbul
d-i clock-setup/ntp boolean true
d-i partman/early_command string \
  media=$(mount | awk '$3=="/cdrom"{print $1}' | sed -E 's|/dev/||; s/p?[0-9]+$//'); \
  for d in $(list-devices disk); do [ "${d#/dev/}" = "$media" ] && continue; debconf-set partman-auto/disk "$d"; break; done
d-i partman-auto/method string regular
d-i partman-auto/choose_recipe select atomic
d-i partman-lvm/device_remove_lvm boolean true
d-i partman-md/device_remove_md boolean true
d-i partman-efi/non_efi_system boolean true
d-i partman-partitioning/confirm_write_new_label boolean true
d-i partman/choose_partition select finish
d-i partman/confirm boolean true
d-i partman/confirm_nooverwrite boolean true
d-i apt-setup/non-free-firmware boolean true
d-i apt-setup/services-select multiselect security, updates
tasksel tasksel/first multiselect
d-i pkgsel/include string openssh-server
d-i pkgsel/upgrade select full-upgrade
popularity-contest popularity-contest/participate boolean false
d-i grub-installer/only_debian boolean true
d-i grub-installer/bootdev string default
d-i grub-installer/force-efi-extra-removable boolean true
d-i preseed/late_command string sh /cdrom/wificorrect/son.sh
d-i finish-install/reboot_in_progress note
d-i debian-installer/exit/poweroff boolean true
```

(`force-efi-extra-removable`: bu kartın UEFI'sinde BootOrder yoktu — `efibootmgr` "No BootOrder"; yedek yol `EFI/BOOT/BOOTX64.EFI` yazılır.)

`iso/son.sh`:

```sh
#!/bin/sh
# Kurucunun son adımı: WifiCorrect paketi (bağımlılıklar internetten) + hizmet sağlayıcının açık SSH anahtarı.
set -e
cp /cdrom/wificorrect/wificorrect_*_amd64.deb /target/tmp/
if [ -s /cdrom/wificorrect/authorized_keys ]; then
  mkdir -p /target/root/.ssh && chmod 700 /target/root/.ssh
  cp /cdrom/wificorrect/authorized_keys /target/root/.ssh/authorized_keys && chmod 600 /target/root/.ssh/authorized_keys
fi
in-target sh -c 'DEBIAN_FRONTEND=noninteractive apt-get install -y -q /tmp/wificorrect_*_amd64.deb && rm -f /tmp/wificorrect_*_amd64.deb'
```

`iso/grub.cfg` (UEFI menüsü; zaman aşımı yok):

```
set timeout=-1
set default=0
menuentry "WifiCorrect kur - DISKTEKI HER SEY SILINIR" {
  linux /install.amd/vmlinuz auto=true priority=critical preseed/file=/cdrom/preseed.cfg locale=tr_TR.UTF-8 keyboard-configuration/xkb-keymap=tr console=ttyS0,115200n8 console=tty0 ---
  initrd /install.amd/initrd.gz
}
```

`iso/txt.cfg` (BIOS/isolinux menüsü):

```
default wificorrect
timeout 0
prompt 0
label wificorrect
  menu label WifiCorrect kur - DISKTEKI HER SEY SILINIR
  kernel /install.amd/vmlinuz
  append auto=true priority=critical preseed/file=/cdrom/preseed.cfg locale=tr_TR.UTF-8 keyboard-configuration/xkb-keymap=tr vga=788 initrd=/install.amd/initrd.gz console=ttyS0,115200n8 console=tty0 ---
```

- [ ] **Step 3: ISO betiği** — `scripts/iso.sh`:

```sh
#!/bin/sh
# Debian 13 netinst (imzası doğrulanır) + preseed + .deb → kurulum ISO'su. Kullanım: scripts/iso.sh <deb> <sürüm> <çıktı>
set -eu
deb=$1 surum=$2 cikti=$3
cd "$(dirname "$0")/.."
u=https://cdimage.debian.org/debian-cd/current/amd64/iso-cd
d=$(mktemp -d)
curl -fsSL "$u/SHA256SUMS" -o "$d/SHA256SUMS"
curl -fsSL "$u/SHA256SUMS.sign" -o "$d/SHA256SUMS.sign"
gpgv --keyring /usr/share/keyrings/debian-role-keys.gpg "$d/SHA256SUMS.sign" "$d/SHA256SUMS"
ad=$(grep -o 'debian-13[^ ]*-amd64-netinst.iso' "$d/SHA256SUMS" | head -1)
curl -fsSL "$u/$ad" -o "$d/$ad"
(cd "$d" && grep " $ad\$" SHA256SUMS | sha256sum -c -)
mkdir -p "$d/ek/wificorrect" "$cikti"
cp "$deb" iso/son.sh "$d/ek/wificorrect/"
[ -n "${WFC_SSH_PUB:-}" ] && printf '%s\n' "$WFC_SSH_PUB" > "$d/ek/wificorrect/authorized_keys"
xorriso -indev "$d/$ad" -outdev "$cikti/wificorrect-kurulum-$surum.iso" \
  -map iso/preseed.cfg /preseed.cfg -map "$d/ek/wificorrect" /wificorrect \
  -map iso/grub.cfg /boot/grub/grub.cfg -map iso/txt.cfg /isolinux/txt.cfg \
  -boot_image any replay
rm -rf "$d"
```

(Netinst'te `isolinux/menu.cfg` `txt.cfg`'yi içerir; `timeout 0` + `default` ile yalnızca bu giriş kalır. Ayrıntı farklıysa — `menu.cfg`'de başka `default` — ISO açılıp `isolinux/` içeriği okunarak düzeltilir ve buraya Ruling yazılır.)

- [ ] **Step 4: Sürüm iş akışı** — `.github/workflows/release.yml` `Paketle` adımı yerine:

```yaml
      - run: sudo apt-get install -y -q xorriso debian-keyring qemu-system-x86 qemu-utils ovmf
      - name: Paketle
        env:
          WFC_SSH_PUB: ${{ vars.WFC_SSH_PUB }}
        run: |
          s="${GITHUB_REF_NAME#v}"
          mkdir -p dist
          install -m 755 target/release/wificorrect dist/wificorrect
          sh scripts/paket.sh target/release/wificorrect "$s" dist
          sh scripts/paket-test.sh "dist/wificorrect_${s}_amd64.deb"
          sh scripts/iso.sh "dist/wificorrect_${s}_amd64.deb" "$s" dist
          sudo chmod 666 /dev/kvm || true
          sh scripts/iso-test.sh "dist/wificorrect-kurulum-${s}.iso"
          tar -czf "dist/wificorrect-sunucu-${GITHUB_REF_NAME}.tar.gz" --exclude=__pycache__ -C scripts sunucu
          (cd dist && sha256sum wificorrect *.deb *.iso *.tar.gz > SHA256SUMS)
```

`Release` adımındaki dosya listesi: `dist/wificorrect dist/*.deb dist/*.iso dist/*.tar.gz dist/SHA256SUMS`.
(`-cihaz-…-x86_64.tar.gz` artık üretilmez: yerini `.deb` alır.)

- [ ] **Step 5: Elle çalıştırılabilir iş akışı (etiketsiz deneme)** — `release.yml` `on:` altına `workflow_dispatch:` ekle; `Release` adımına
  `if: startsWith(github.ref, 'refs/tags/')` koy, böylece dalda elle çalıştırılınca yalnızca derler ve test eder. Elle çalıştırmada
  `GITHUB_REF_NAME` dal adıdır → `s` için `${GITHUB_REF_NAME#v}` yerine etiket yoksa `0.0.0-<kısa sha>`:

```sh
          case "$GITHUB_REF" in refs/tags/v*) s="${GITHUB_REF_NAME#v}" ;; *) s="0.0.0-${GITHUB_SHA%"${GITHUB_SHA#???????}"}" ;; esac
```

- [ ] **Step 6: Run** — dalı it, `gh workflow run release.yml --ref kurulum-iso -R avsharbeyi/wificorrect`. Expected: `PAKET TESTI GECTI`,
  `ISO DUMAN TESTI GECTI` (ya da `ATLANDI: KVM yok` — o durumda bunu rapora yaz). Başarısızsa `kurulum.log`/`acilis.log`
  son satırlarından kök nedeni bul (ör. preseed sorusu takıldıysa kurulum 40 dk zaman aşımına düşer: hangi soru olduğu seri
  günlükte görünmez → `DEBCONF_DEBUG=5` ekleyip yeniden dene).

- [ ] **Step 7: Commit** — `git add iso scripts/iso.sh scripts/iso-test.sh .github/workflows/release.yml && git commit -m "ISO: preseed, .deb, imzalı netinst; sürümde ISO + QEMU duman testi"`

---

### Task 9: Belgeler, merkez ve canlı cihaz güncellemesi, kabul

**Files:**
- Modify: `docs/KURULUM_GUNLUGU.md`, `docs/superpowers/specs/2026-10-05-kurulum-iso-design.md` (gerekirse Ruling'ler)

- [ ] **Step 1: KURULUM_GUNLUGU** — yeni bölüm "Kurulum ISO'su (2026-10-05)": admin parolası/NetGSM merkezden, `ag-ilk`, paket
  (ürettiği/ezmediği dosyalar), ISO menüsü ve teknisyen notu (Ethernet 1 modeme, monitör+klavye, ~15 dk, cihaz kendiliğinden
  kapanır), GitHub değişkeni `WFC_SSH_PUB`, KVM durumu.
- [ ] **Step 2: Canlı cihaz** — `bash scripts/gelistir.sh` (yeni ikili), `ssh wificorrect 'systemctl restart wificorrect-panel wificorrect-portal'`.
  Cihaz bağlı ama merkezde `admin_parola` boş (eski bağ) → yönetimde "Admin parolasını yenile" ile üretilir (Step 3 sonrası).
- [ ] **Step 3: Merkez** — kullanıcı terminalde `scripts/sunucu/dagit.sh` (sudo parolası kullanıcıda). Sonra yönetimde: SMS ayarları
  (kullanıcı girer; mevcut değerler cihazdaki gibi), Göztepe Bilgisayar → "Admin parolasını yenile".
- [ ] **Step 4: Eşitleme** — `ssh wificorrect 'wificorrect ctl merkez-eslesme; grep -E "ADMIN_PAROLA_MERKEZ|SMS_AYARI_MERKEZ|MERKEZ_EK_HATA" /srv/5651/gunluk/$(date +%F)/denetim.csv | tail -3'`
  Expected: `ADMIN_PAROLA_MERKEZ` ve (değer farklıysa) `SMS_AYARI_MERKEZ`; panel Admin ayarları'nda NetGSM "merkezden yönetiliyor";
  admin yönetimde görünen parolayla girer.
- [ ] **Step 5: Sürüm** — kullanıcı onayıyla `v0.3.0` etiketi; iş akışı `.deb` + ISO yayınlar.
- [ ] **Step 6: Gerçek donanım kabulü (kullanıcıyla)** — boş yedek makinede ISO: monitörsüz ikinci açılış, numarayla bağlanma,
  yönetimdeki admin parolasıyla giriş, deneme modu kapalıyken **kullanıcı onayıyla** tek gerçek SMS.
- [ ] **Step 7: Commit** — `git commit -am "docs: kurulum ISO'su günlüğü"`
