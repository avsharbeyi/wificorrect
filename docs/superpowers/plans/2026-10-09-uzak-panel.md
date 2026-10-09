# Uzak panel (yalnızca panel.wificorrect.com'dan giriş) + admin kilidi — uygulama planı

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** İşletme sahibi panel.wificorrect.com'da numara+parolayla girer ve cihazının paneline tarayıcıdan ulaşır; Admin ayarları ve Panel hareketleri merkezden belirlenen admin parolasıyla açılır.

**Architecture:** Merkez (Python) giriş sonrası tek kullanımlık belirteçle `cihaz.wificorrect.com`'a geçirir, orada kendi çerezini tutar; Caddy `forward_auth` ile merkeze sorup isteği WireGuard üzerinden cihazın 8443'üne aktarır, kimliği `X-WFC-Kullanici` başlığıyla taşır. Cihaz (Rust) bu başlığa yalnızca merkezin tünel adresinden güvenir; bağlıyken başka yerden panel açmaz; admin kilidi oturuma 30 dk yazılır.

**Tech Stack:** Python 3.13 stdlib (merkez), Rust 1.85 + tiny_http (cihaz), Caddy 2 (forward_auth, reverse_proxy).

**Spec:** `docs/superpowers/specs/2026-10-08-uzak-panel-design.md` (2026-10-09 sürümü)

## Global Constraints
- Giriş yalnızca müşteri numarası + parola; cihazda ayrı `admin` kullanıcı girişi yok.
- Admin parolası merkezden gelir (cihazdaki `hesaplar.json` → `admin` özeti); Admin ayarları ve Panel hareketleri onunla açılır, 30 dk.
- Cihaz `X-WFC-Kullanici` / `X-Forwarded-For` başlıklarına yalnızca istek merkezin tünel adresinden geliyorsa güvenir: cihazın `uzak.adres` değerinin ilk üç sekizlisi + `.1` (ör. 10.99.0.11 → 10.99.0.1).
- Merkez oturumları bellek içi; boşta 30 dk, en çok 12 saat. Geçiş belirteci tek kullanımlık, 60 sn.
- Merkez yalnızca standart kütüphane; cihaz yeni crate yok.
- Kullanıcıya görünen metinler Türkçe.
- Merkez testleri: `python scripts/sunucu/merkez/tests/test_<ad>.py` (son satır `TUM TESTLER GECTI`), hepsi `sh scripts/sunucu/merkez/tests/hepsi.sh python`. Cihaz testleri cihazda: `bash scripts/test.sh wificorrect [süzgeç]`.
- Canlı cihaza ve merkeze kurulum yalnızca Task 4'te (merkez sudo'su kullanıcıda).

## Review Focus
1. Tünel dışından (modem ağından) gelen, `X-WFC-Kullanici` başlığı taşıyan istek oturum açmamalı → Task 2 `baslik_yalniz_merkezden`.
2. Bağlı cihazda dükkân içinden panel/giriş açılmamalı, ama bağlı olmayan cihaz eşleştirme ekranını açmalı → Task 2 `yerel_erisim_bagliyken_kapali`.
3. Merkezde parola değişince / lisans kapanınca açık cihaz oturumu düşmeli → Task 1 `test_cihaz_oturumu_parola_lisans`.
4. Admin kilidi açıkken merkezde admin parolası değişirse kilit kapanmalı → Task 3 `admin_kilidi_parola_degisince_kapanir`.
5. Geçiş belirteci ikinci kez ya da 60 sn sonra kullanılamamalı → Task 1 `test_belirtec_tek_kullanim`.

---

### Task 1: Merkez — geçiş belirteci, cihaz oturumu, `cihaz.wificorrect.com` uygulaması, giriş sonrası yönlendirme

**Files:**
- Create: `scripts/sunucu/merkez/cihaz.py`, `scripts/sunucu/merkez/tests/test_cihaz.py`
- Modify: `scripts/sunucu/merkez/guvenlik.py` (GecisBelirtecleri), `scripts/sunucu/merkez/merkez.py` (ALANLAR + bağlama), `scripts/sunucu/merkez/musteri.py` (giriş sonrası `/cihaz`, menü), `scripts/sunucu/merkez/web.py` (giriş sonrası yönlendirmeyi alt sınıfın seçebilmesi), `scripts/sunucu/merkez/tests/hepsi.sh` gerekmez (test_*.py kendiliğinden)

**Interfaces:**
- Produces:
  - `guvenlik.GecisBelirtecleri()` → `.uret(numara: str, simdi: float) -> str`, `.tuket(token: str, simdi: float) -> str|None` (tek kullanım, 60 sn)
  - `cihaz.Cihaz(veri, gecis, musteri_app, saat=time.time)` → `.istek(yontem, yol, basliklar, govde, sorgu, ip)`; yollar: `/cihaz-yetki`, `/_giris`, `/_cikis`
  - Çerez `wcc` (HttpOnly, Secure, SameSite=Lax, Path=/); yanıt başlıkları `X-WFC-Cihaz`, `X-WFC-Kullanici`
  - `musteri.Musteri.cihaz_adresi(numara) -> str|None` (bağlı, tünel adresi olan, üyeliği ve lisansı açık cihazın tünel IP'si)
  - panel.wificorrect.com `GET /cihaz` → belirteç → `303 https://cihaz.wificorrect.com/_giris?t=…` (cihaz yoksa merkez sayfası + "Cihazınıza şu an ulaşılamıyor")

- [ ] **Step 1: Failing tests** — `tests/test_cihaz.py`:

```python
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import yardim as Y  # noqa: E402
import cihaz as C  # noqa: E402
import guvenlik as G  # noqa: E402
import musteri as M  # noqa: E402

WG, SSH = "A" * 43 + "=", "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI c"
CIHAZ = {"Host": "cihaz.wificorrect.com"}


def kur(tmp):
    v, k, saat = Y.ortam(tmp)
    gecis = G.GecisBelirtecleri()
    m = M.Musteri(v, k, saat, gecis=gecis)
    c = C.Cihaz(v, gecis, m, saat)
    n, pw = v.musteri_ekle(parola="musteri-parola-1")
    v.cihaz_bagla(n, WG, SSH)
    v.cihaz_guncelle(v.bagli_cihaz(n)["id"], tunel_ip="10.99.0.11")
    return v, m, c, saat, n


def gir(m, n, pw="musteri-parola-1"):
    return Y.giris(m, str(n), pw)


def cerez(yanit, ad):
    return yanit[1]["Set-Cookie"].split(";")[0].split("=", 1)[1]


def test_belirtec_tek_kullanim():
    g = G.GecisBelirtecleri()
    t = g.uret("1234567", 1000.0)
    assert g.tuket(t, 1030.0) == "1234567" and g.tuket(t, 1031.0) is None
    t2 = g.uret("1234567", 1000.0)
    assert g.tuket(t2, 1061.0) is None and g.tuket("yok", 1000.0) is None


def test_giris_cihaza_gecirir_ve_yetki_baslik_verir():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        v, m, c, saat, n = kur(tmp)
        r = gir(m, n)
        assert r[0] == 303 and r[1]["Location"] == "/cihaz"
        cz = Y.cerez(r)
        r = m.istek("GET", "/cihaz", {"Cookie": cz}, "", "", "203.0.113.5")
        assert r[0] == 303 and r[1]["Location"].startswith("https://cihaz.wificorrect.com/_giris?t=")
        t = r[1]["Location"].split("t=", 1)[1]
        r = c.istek("GET", "/_giris", CIHAZ, "", "t=" + t, "203.0.113.5")
        assert r[0] == 303 and r[1]["Location"] == "/" and "SameSite=Lax" in r[1]["Set-Cookie"]
        wcc = cerez(r, "wcc")
        r = c.istek("GET", "/cihaz-yetki", dict(CIHAZ, Cookie=f"wcc={wcc}"), "", "", "203.0.113.5")
        assert r[0] == 200 and r[1]["X-WFC-Cihaz"] == "10.99.0.11" and r[1]["X-WFC-Kullanici"] == str(n)
        assert c.istek("GET", "/_giris", CIHAZ, "", "t=" + t, "203.0.113.5")[0] == 303  # ikinci kez: giriş sayfasına
        r = c.istek("GET", "/cihaz-yetki", CIHAZ, "", "", "203.0.113.5")
        assert r[0] == 302 and r[1]["Location"] == "https://panel.wificorrect.com/giris"


def test_cihaz_oturumu_parola_lisans():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        v, m, c, saat, n = kur(tmp)
        wcc = c.oturumlar.create(str(n), "musteri", saat(), v.musteri(n)["ozet"])
        yetki = lambda: c.istek("GET", "/cihaz-yetki", dict(CIHAZ, Cookie=f"wcc={wcc}"), "", "", "1.2.3.4")[0]
        assert yetki() == 200
        v.askiya_al(n, True)
        assert yetki() == 302  # lisans kapalı
        v.askiya_al(n, False)
        v.parola_koy(n, "yeni-parola-12")
        assert yetki() == 302  # parola değişti: oturum düştü
        wcc = c.oturumlar.create(str(n), "musteri", saat(), v.musteri(n)["ozet"])
        v.serbest_birak(n, zorla=True)
        assert yetki() == 302  # cihaz bağlı değil


def test_cikis_iki_oturumu_kapatir_ve_cihazsiz_musteri_merkezde_kalir():
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        v, m, c, saat, n = kur(tmp)
        cz = Y.cerez(gir(m, n))
        wcc = c.oturumlar.create(str(n), "musteri", saat(), v.musteri(n)["ozet"])
        r = c.istek("GET", "/_cikis", dict(CIHAZ, Cookie=f"wcc={wcc}"), "", "", "1.2.3.4")
        assert r[0] == 303 and r[1]["Location"] == "https://panel.wificorrect.com/giris" and "Max-Age=0" in r[1]["Set-Cookie"]
        assert c.istek("GET", "/cihaz-yetki", dict(CIHAZ, Cookie=f"wcc={wcc}"), "", "", "1.2.3.4")[0] == 302
        assert m.istek("GET", "/", {"Cookie": cz}, "", "", "1.2.3.4")[0] == 303  # merkez oturumu da kapandı → giriş
        n2, _ = v.musteri_ekle(parola="musteri-parola-2")  # cihazı yok
        r = gir(m, n2, "musteri-parola-2")
        assert r[1]["Location"] == "/"
        s = m.istek("GET", "/cihaz", {"Cookie": Y.cerez(r)}, "", "", "1.2.3.4")[2].decode()
        assert "ulaşılamıyor" in s


if __name__ == "__main__":
    for _name, _fn in sorted(globals().items()):
        if _name.startswith("test_") and callable(_fn):
            _fn()
            print("ok ", _name)
    print("TUM TESTLER GECTI")
```

- [ ] **Step 2: Run** `python scripts/sunucu/merkez/tests/test_cihaz.py` — Expected: `ModuleNotFoundError: cihaz`.

- [ ] **Step 3: Implement**
  - `guvenlik.py`:

```python
class GecisBelirtecleri:
    """panel → cihaz.wificorrect.com geçişi: tek kullanımlık, 60 sn (URL'de taşınır, çerez değil)."""
    SURE = 60

    def __init__(self):
        self._b, self.lock = {}, threading.Lock()

    def uret(self, numara, simdi):
        t = secrets.token_urlsafe(32)
        with self.lock:
            self._b = {k: v for k, v in self._b.items() if v[1] > simdi}
            self._b[t] = (numara, simdi + self.SURE)
        return t

    def tuket(self, token, simdi):
        with self.lock:
            v = self._b.pop(token or "", None)
        return v[0] if v and v[1] > simdi else None
```
  - `web.py`: `Taban._giris` başarı dönüşünü `yonlendir(self.giris_sonrasi(kimlik), (self.cerez, …))` yap; `Taban.giris_sonrasi(self, kimlik): return "/"` (alt sınıf değiştirir).
  - `musteri.py`: `__init__(self, veri, kayitlar, saat=time.time, gecis=None)` (`super().__init__`; `self.gecis = gecis or guvenlik.GecisBelirtecleri()`); `cihaz_adresi(numara)`: `c = veri.bagli_cihaz(n)`; `None` değilse ve `c["durum"] == "bagli"` ve `c["tunel_ip"]` ve `veri.musteri(n)["uyelik"] == "aktif"` ve `veri.lisans_durumu(m)[0] == "aktif"` → `c["tunel_ip"]`; `giris_sonrasi(kimlik)`: `"/cihaz" if self.cihaz_adresi(int(kimlik)) else "/"`; `sayfalar`'a `GET /cihaz`: adres varsa `yonlendir("https://cihaz.wificorrect.com/_giris?t=" + self.gecis.uret(ot["user"], self.saat()))`, yoksa `yanit_html(self.sayfa("Cihaz paneli", kart('<h1>Cihazınıza şu an ulaşılamıyor</h1><p>…kapalı, internetsiz ya da lisansı kapalı olabilir. Yedek arşiviniz aşağıda.</p><p><a href="/">Yedek arşiv</a></p>'), ot))`; `menu`: `'<a href="/cihaz">Cihaz paneli</a><a href="/">Yedek arşiv</a><a href="/parola">Parola</a>'`.
  - `cihaz.py`:

```python
"""cihaz.wificorrect.com (spec 2026-10-09): Caddy forward_auth buraya sorar; geçiş belirteci → çerez; çıkış."""
import time
import urllib.parse

import guvenlik
import web

PANEL_GIRIS = "https://panel.wificorrect.com/giris"
CEREZ = "wcc"


def _cerez(token):
    if token is None:
        return f"{CEREZ}=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0"
    return f"{CEREZ}={token}; HttpOnly; Secure; SameSite=Lax; Path=/"  # panelden gelen yönlendirmede de gönderilsin


class Cihaz:
    def __init__(self, veri, gecis, musteri_app, saat=time.time):
        self.veri, self.gecis, self.musteri_app, self.saat = veri, gecis, musteri_app, saat
        self.oturumlar = guvenlik.Oturumlar()

    def _gecerli(self, token):
        """Dönen: (numara, tünel adresi) ya da None. Parola değiştiyse / lisans, üyelik kapandıysa / cihaz yoksa düşer."""
        ot = self.oturumlar.get(token, self.saat())
        if not ot:
            return None
        n = int(ot["user"])
        m = self.veri.musteri(n)
        adres = self.musteri_app.cihaz_adresi(n)
        if m is None or m["ozet"] != ot["surum"] or adres is None:
            self.oturumlar.drop(token)
            return None
        return ot["user"], adres

    def istek(self, yontem, yol, basliklar, govde, sorgu="", ip=""):
        token = web.cerez_degeri(basliklar.get("Cookie"), CEREZ)
        if yol == "/cihaz-yetki":
            g = self._gecerli(token)
            if g is None:
                return 302, {"Location": PANEL_GIRIS, "Cache-Control": "no-store"}, b""
            return 200, {"X-WFC-Kullanici": g[0], "X-WFC-Cihaz": g[1], "Cache-Control": "no-store"}, b""
        if yol == "/_giris":
            numara = self.gecis.tuket((urllib.parse.parse_qs(sorgu).get("t") or [""])[0], self.saat())
            m = self.veri.musteri(int(numara)) if numara else None
            if m is None:
                return 303, {"Location": PANEL_GIRIS, "Cache-Control": "no-store"}, b""
            yeni = self.oturumlar.create(numara, "musteri", self.saat(), m["ozet"])
            return 303, {"Location": "/", "Set-Cookie": _cerez(yeni), "Cache-Control": "no-store"}, b""
        if yol == "/_cikis":
            ot = self.oturumlar.get(token, self.saat())
            if ot:
                self.oturumlar.drop_user(ot["user"])
                self.musteri_app.oturumlar.drop_user(ot["user"])
            return 303, {"Location": PANEL_GIRIS, "Set-Cookie": _cerez(None), "Cache-Control": "no-store"}, b""
        return 404, {"Content-Type": "text/plain; charset=utf-8"}, "Bulunamadı".encode("utf-8")
```
    (`guvenlik.Oturumlar`'da `drop_user` yoksa ekle: o kullanıcının bütün oturumlarını siler. `create`/`get` imzaları mevcut koddakiyle aynı; oturum sözlüğünde `user`, `surum` alanları var. Boşta 30 dk / 12 saat sınırı `Oturumlar`'ın varsayılanı.)
  - `merkez.py`: `ALANLAR["cihaz.wificorrect.com"] = "cihaz"`; `main()`: `gecis = guvenlik.GecisBelirtecleri(); m = musteri.Musteri(v, k, gecis=gecis)`; uygulamalara `"cihaz": cihaz.Cihaz(v, gecis, m)`.
  - Mevcut `test_musteri.py` giriş sonrası `Location == "/"` bekliyorsa: cihazı olmayan müşteride değişmez; bağlı+tünelli müşteride artık `/cihaz` — o testleri yeni davranışa göre güncelle (Ruling yaz).

- [ ] **Step 4: Run** `python scripts/sunucu/merkez/tests/test_cihaz.py` → `TUM TESTLER GECTI`; `sh scripts/sunucu/merkez/tests/hepsi.sh python` → hepsi geçer.
- [ ] **Step 5: Commit** — `merkez: cihaz.wificorrect.com geçişi (belirteç, cihaz oturumu, forward_auth yetkisi), girişten sonra cihaz paneline`

---

### Task 2: Cihaz — merkezden gelen kimlik, yerel erişim kapalı, eşleştirme ekranı, çıkış

**Files:** Modify `src/panel.rs` (Req, `handle` sarmalayıcı, `Panel::handle`, giriş sayfası, çıkış, Kayıtlar bölümüne arşiv bağlantısı). Test: `src/panel.rs` test modülü.

**Interfaces:**
- `Req` yeni alanlar: `pub merkez_kullanici: Option<String>` (X-WFC-Kullanici), `pub ileten_ip: Option<String>` (X-Forwarded-For'un son öğesi). `#[derive(Default)]` ekle; bütün `Req { … }` kurucuları `..Default::default()` ya da yeni alanlarla.
- `fn merkez_adresi(cfg: &Config) -> Option<String>`: `cfg.uzak.adres` "a.b.c.d" ise "a.b.c.1", yoksa None.
- `Panel::handle` başında: `let merkezden = merkez_adresi(&cfg).is_some_and(|m| m == req.ip);` merkezden ise `ip = ileten_ip.unwrap_or(req.ip)` (denetim/kilit için), aksi halde iki yeni alan yok sayılır.

- [ ] **Step 1: Failing tests** (`src/panel.rs` tests; `MUSTERI` sabiti ve `setup_and_login` mevcut — merkez.json bağlı kurulur):

```rust
    fn merkez_req(path: &str, kullanici: &str) -> Req {
        Req { method: "GET".into(), path: path.into(), ip: "10.99.0.1".into(), merkez_kullanici: Some(kullanici.into()),
              ileten_ip: Some("203.0.113.9".into()), ..Default::default() }
    }

    fn uzak_adresli(e: &Env) {
        let mut c = Config::load(&e.p.cfg_path).unwrap();
        c.uzak.adres = "10.99.0.11".into();
        c.save(&e.p.cfg_path).unwrap();
    }

    #[test]
    fn baslik_yalniz_merkezden() {
        let e = env();
        setup_and_login(&e, "mudur", "sahip-parola-12"); // cihazı MUSTERI'ye bağlar
        uzak_adresli(&e);
        let r = e.p.handle(&merkez_req("/cihazlar", MUSTERI));
        assert_eq!(r.status, 200, "{}", r.body);
        let cerez = r.headers.iter().find(|(k, _)| k == "Set-Cookie").expect("oturum çerezi").1.clone();
        assert!(cerez.starts_with("wfc="));
        // aynı başlık modem ağından: yok sayılır
        let mut r2 = merkez_req("/cihazlar", MUSTERI);
        r2.ip = "192.168.1.50".into();
        assert!(e.p.handle(&r2).body.contains("panel.wificorrect.com"));
        // başka müşteri numarası: red
        assert_ne!(e.p.handle(&merkez_req("/cihazlar", "7654321")).status, 200);
        // denetimde gerçek istemci IP'si
        let gun = ortak::day_of(&ortak::now_iso((e.p.clock)())).to_string();
        let root = Config::load(&e.p.cfg_path).unwrap().main.log_root;
        assert!(std::fs::read_to_string(ortak::day_file(&root, &gun, "denetim.csv")).unwrap().contains("ip=203.0.113.9"));
    }

    #[test]
    fn yerel_erisim_bagliyken_kapali() {
        let e = env();
        // bağlı değil: yerelden yalnızca eşleştirme (giriş) ekranı
        let g = e.p.handle(&req("GET", "/giris", &[], None)).body;
        assert!(g.contains("action=\"/giris\"") && g.contains("Müşteri numarası"));
        assert_eq!(loc(&e.p.handle(&req("GET", "/cihazlar", &[], None))), "/giris");
        setup_and_login(&e, "mudur", "sahip-parola-12");
        uzak_adresli(&e);
        // bağlı: yerelden giriş formu da yok, yönlendirme notu var
        let g = e.p.handle(&req("GET", "/giris", &[], None)).body;
        assert!(!g.contains("action=\"/giris\"") && g.contains("panel.wificorrect.com"));
        assert_ne!(e.p.handle(&req("POST", "/giris", &[("kullanici", MUSTERI), ("parola", "sahip-parola-12")], None)).status, 303);
    }

    #[test]
    fn merkezden_cikis_merkeze_yonlenir_ve_arsiv_baglantisi() {
        let e = env();
        setup_and_login(&e, "mudur", "sahip-parola-12");
        uzak_adresli(&e);
        let r = e.p.handle(&merkez_req("/kayitlar", MUSTERI));
        assert!(r.body.contains("https://panel.wificorrect.com/") && r.body.contains("yedek arşiv"));
        let tok = r.headers.iter().find(|(k, _)| k == "Set-Cookie").unwrap().1.trim_start_matches("wfc=").split(';').next().unwrap().to_string();
        let csrf = r.body.split("name=\"csrf\" value=\"").nth(1).unwrap().split('"').next().unwrap().to_string();
        let mut c = merkez_req("/cikis", MUSTERI);
        (c.method, c.token, c.form) = ("POST".into(), Some(tok), [("csrf".to_string(), csrf)].into_iter().collect());
        assert_eq!(loc(&e.p.handle(&c)), "/_cikis");
    }
```
  (**Test yardımcıları:** bağlı cihazda yerel istek artık reddedildiği için mevcut testlerin kullandığı `req(…)` ve
  `get(…)` yardımcıları varsayılan olarak merkezden gelen istek kurar: `ip: "10.99.0.1"`, `merkez_kullanici: Some(MUSTERI)`,
  `ileten_ip: Some(IP)`; `env()` ayarına `uzak.adres = "10.99.0.11"` yazılır. Yerel davranışı sınayan testler `Req`'i
  açıkça (`ip: "192.168.1.50"`, başlıksız) kurar. `setup_and_login` bugün giriş için yerel `/giris` kullanıyor; bağlıyken yerel giriş kapanacağı için yardımcıyı merkez başlığıyla oturum açacak şekilde değiştir: merkez.json + `uzak.adres` kur, `merkez_req("/", MUSTERI)` ile çerezi al, CSRF'i `/sifre` gövdesinden al. `"admin"` kullanıcısı için Task 3 bu yardımcıyı genişletir; Task 2'de admin kullanan testler `setup_and_login(&e,"admin",…)` çağrısını Task 3'e kadar korur — bu yüzden bu görevde yardımcı `"admin"` için bugünkü davranışı (yerel admin girişi) sürdürmeli; Task 3 kaldırır.)

- [ ] **Step 2: Run** `bash scripts/test.sh wificorrect baslik_yalniz`, `… yerel_erisim`, `… merkezden_cikis` — Expected: derleme hatası (`merkez_kullanici` yok).

- [ ] **Step 3: Implement**
  - `Req`: alanlar + `Default`. tiny_http sarmalayıcı (`fn handle(p, req)`): `X-WFC-Kullanici` başlığını ve `X-Forwarded-For`'un son öğesini (virgülle bölünmüş, kırpılmış) `Req`'e koy.
  - `Panel::handle` başı (Config yüklendikten, müşteri ağı kontrolünden sonra):

```rust
        let merkezden = merkez_adresi(&cfg).is_some_and(|m| m == req.ip);
        let req = &Req { ip: if merkezden { req.ileten_ip.clone().unwrap_or_else(|| req.ip.clone()) } else { req.ip.clone() },
                         merkez_kullanici: if merkezden { req.merkez_kullanici.clone() } else { None }, ..clone_req(req) };
        let bagli = crate::merkez::oku(&self.merkez_path);
        // bağlıyken panel yalnızca merkez üzerinden (2026-10-09, kullanıcı kararı)
        if bagli.is_some() && !merkezden {
            return self.yalniz_merkez(&cfg, req);
        }
```
    `yalniz_merkez`: 200, `self.page(cfg, req, None, "Giriş", "<div class=\"kart dar\"><p>Bu cihazın paneli yalnızca <a href=\"https://panel.wificorrect.com\">panel.wificorrect.com</a> üzerinden açılır. Müşteri numaranız ve parolanızla oradan girin.</p></div>")`.
  - Merkezden ve `merkez_kullanici == Some(m.numara)` ise: geçerli oturum (çerez) yoksa ya da oturumun kullanıcısı farklıysa → `self.oturumlar.create(&m.numara, Rol::Sahip, now, &m.ozet)`; isteği bu oturumla işle; yanıta `Set-Cookie: wfc=<token>; Path=/; Secure; HttpOnly; SameSite=Lax` ekle (SameSite=Lax: merkezden yönlendirmeyle gelinir). `merkez_kullanici` var ama numara uyuşmuyorsa → 403 metin "Bu cihaz bu müşteriye ait değil.".
  - Bağlı değilken (`bagli.is_none()`): yalnızca `/giris` GET/POST (bugünkü eşleştirme akışı = `merkeze_baglan`); diğer her yol → `redirect("/giris")`. Giriş sayfası başlığı/metni: "Cihazı eşleştir — müşteri numarası ve parola".
  - `("POST", "/cikis")`: oturum merkezden açıldıysa (istek merkezden) yönlendirme `/_cikis`, değilse `/giris`.
  - Kayıtlar grubundaki `gunler` bölümünün gövdesine (yalnızca merkezden): `<p class="not"><a href="https://panel.wificorrect.com/">Merkezdeki yedek arşiv</a> — cihaz ulaşılamazken de açılır.</p>`.

- [ ] **Step 4: Run** üç test + tam takım `bash scripts/test.sh wificorrect` — Expected: geçer (eski yerel giriş testleri yeni yardımcıyla).
- [ ] **Step 5: Commit** — `cihaz: panel yalnızca merkez üzerinden (X-WFC-Kullanici yalnızca merkezin tünel adresinden), bağlı değilken yalnızca eşleştirme`

---

### Task 3: Cihaz — admin kilidi, "Admin ayarları" menüsü, admin kullanıcı girişi kalkar

**Files:** Modify `src/hesap.rs` (Oturum `admin_kadar: f64`, `admin_surum: String`; `Oturumlar::set_admin(token, until, surum)`), `src/panel.rs` (MENU, BOLUMLER, handle rol hesaplama, `/admin-kilidi`, fabrika parola doğrulaması, giriş), testler.

**Interfaces:**
- `MENU`: `[("/", "Özet"), ("/cihazlar", "Cihazlar"), ("/kayitlar", "Kayıtlar"), ("/ayarlar", "Ayarlar"), ("/admin", "Admin ayarları")]`.
- `BOLUMLER`: Ayarlar grubundan `admin-ayarlari` çıkar, Kayıtlar grubundan `panel-hareketleri` çıkar; yeni grup `/admin`: `("/admin","/admin-ayarlari","admin-ayarlari","Admin ayarları",true)`, `("/admin","/panel-hareketleri","panel-hareketleri","Panel hareketleri",true)`.
- Etkin rol: `handle` içinde oturum alındıktan sonra `o.rol = if o.admin_kadar > now && self.hesaplar.ozet(ADMIN).is_some_and(|z| ct_eq(&z, &o.admin_surum)) { Rol::Hizmet } else { Rol::Sahip }` (mevcut `hizmet` kontrolleri aynen çalışır).
- `GET /admin` ve kilitli iken `/admin-ayarlari`, `/panel-hareketleri`: admin parola formu (`POST /admin-kilidi`, alanlar `csrf`, `parola`, `donus`). `POST /admin-kilidi`: `self.guard.allowed(ip, "admin-kilidi:<numara>")`; `self.hesaplar.verify(ADMIN, parola) == Some(Rol::Hizmet)` → `set_admin(token, now+1800, ozet)`, denetim `PANEL_ADMIN_ACILDI`, `redirect(donus)`; değilse `guard.failed`, denetim `PANEL_ADMIN_HATALI`, formu hatayla göster. Admin özeti yoksa form yerine "Admin parolası belirlenmemiş; hizmet sağlayıcınız merkezden belirler."
- Admin ayarları bölümünün üstünde "Admin kilidi 30 dk açık; kapat" (`POST /admin-kilidi/kapat`).
- `giris_post`: `user == ADMIN` dalı kalkar (yalnızca müşteri numarası). Fabrika: `self.hesaplar.verify(ADMIN, pw) != Some(Rol::Hizmet)`.

- [ ] **Step 1: Failing tests**:

```rust
    #[test]
    fn admin_kilidi_ve_menu() {
        let e = env();
        let (tok, csrf) = setup_and_login(&e, "mudur", "sahip-parola-12");
        let ozet = e.p.handle(&req("GET", "/", &[], Some(&tok))).body; // (yardımcı merkez başlığıyla çalışır)
        assert_eq!(menu_linkleri(&ozet), ["/", "/cihazlar", "/kayitlar", "/ayarlar", "/admin"]);
        let a = e.p.handle(&req("GET", "/admin", &[], Some(&tok))).body;
        assert!(a.contains("action=\"/admin-kilidi\"") && !a.contains("name=\"netgsm.usercode\""));
        assert!(e.p.handle(&req("GET", "/panel-hareketleri", &[], Some(&tok))).body.contains("action=\"/admin-kilidi\""));
        let r = e.p.handle(&req("POST", "/admin-kilidi", &[("csrf", &csrf), ("parola", "yanlis-parola-1"), ("donus", "/admin")], Some(&tok)));
        assert!(r.body.contains("hatalı"));
        let r = e.p.handle(&req("POST", "/admin-kilidi", &[("csrf", &csrf), ("parola", "hizmet-parola-1"), ("donus", "/admin")], Some(&tok)));
        assert_eq!(loc(&r), "/admin");
        let a = e.p.handle(&req("GET", "/admin", &[], Some(&tok))).body;
        assert!(a.contains("name=\"netgsm.usercode\"") && a.contains("id=\"panel-hareketleri\""));
        // yerel admin kullanıcı girişi yok
        assert!(!e.p.handle(&req("POST", "/giris", &[("kullanici", "admin"), ("parola", "hizmet-parola-1")], None)).headers.iter().any(|(k, _)| k == "Set-Cookie"));
    }

    #[test]
    fn admin_kilidi_parola_degisince_kapanir_ve_sure_dolunca() {
        let e = env();
        let (tok, csrf) = setup_and_login(&e, "mudur", "sahip-parola-12");
        e.p.handle(&req("POST", "/admin-kilidi", &[("csrf", &csrf), ("parola", "hizmet-parola-1"), ("donus", "/admin")], Some(&tok)));
        assert!(e.p.handle(&req("GET", "/admin", &[], Some(&tok))).body.contains("name=\"netgsm.usercode\""));
        let oz = crate::hesap::digest("merkez-parola-1", "ab12", 120_000);
        e.p.hesaplar.set_admin_ozet("ab12", &oz, 120_000).unwrap();
        assert!(e.p.handle(&req("GET", "/admin", &[], Some(&tok))).body.contains("action=\"/admin-kilidi\""));
    }
```
  (Süre dolumu: test saatini 31 dk ileri alabiliyorsa — `env()`'deki saat düzeneğine bak — ekle; yoksa `Oturumlar` birim testinde `admin_kadar` sınırını sına.)

- [ ] **Step 2: Run** — Expected: FAIL.
- [ ] **Step 3: Implement** yukarıdaki arayüzü. `setup_and_login(&e, "admin", pw)`: müşteri olarak oturum aç + `POST /admin-kilidi` (pw) — böylece eski admin testleri anlamını korur. Eski yerel `admin` girişini sınayan testler yeni davranışa göre güncellenir (Ruling yaz).
- [ ] **Step 4: Run** iki test + tam takım — Expected: geçer.
- [ ] **Step 5: Commit** — `cihaz: Admin ayarları ve Panel hareketleri merkezden gelen admin parolasıyla açılır (30 dk); admin kullanıcı girişi kalktı`

---

### Task 4: Caddy, belgeler, kurulum ve uçtan uca deneme

**Files:** Modify `scripts/sunucu/merkez/Caddyfile`, `scripts/sunucu/merkez/kur.sh` (cihaz.py kopyalanır), `docs/KURULUM_GUNLUGU.md`.

- [ ] **Step 1:** `kur.sh`'deki `install -m 644 … api.py $L/` satırına `cihaz.py` ekle.
- [ ] **Step 2:** Caddyfile'a:

```
cihaz.wificorrect.com {
	request_header -X-WFC-Kullanici
	request_header -X-WFC-Cihaz
	@merkez path /_giris /_cikis
	handle @merkez {
		reverse_proxy 127.0.0.1:8081
	}
	handle {
		forward_auth 127.0.0.1:8081 {
			uri /cihaz-yetki
			copy_headers X-WFC-Kullanici X-WFC-Cihaz
		}
		reverse_proxy https://{http.request.header.X-WFC-Cihaz}:8443 {
			transport http {
				tls_insecure_skip_verify
			}
			header_up -X-WFC-Cihaz
		}
	}
	header {
		Strict-Transport-Security "max-age=31536000"
		-Server
	}
}
```
  (`caddy validate --config Caddyfile --adapter caddyfile` ile sunucuda doğrulanır; yer tutuculu üst akış Caddy 2.7+ ister — sürüm `caddy version`. Uymazsa Ruling yazıp alternatif: forward_auth yanıtındaki `X-WFC-Cihaz` ile `reverse_proxy {http.request.header.X-WFC-Cihaz}:8443`.)
- [ ] **Step 3:** KURULUM_GUNLUGU'ya kısa bölüm (akış, güvenlik kuralı, admin kilidi, yerel erişim).
- [ ] **Step 4 (kullanıcıyla):** merkez kurulumu (`sunucu-guncelle.sh`, sudo), `caddy validate`; cihaz `scripts/gelistir.sh` + servis yeniden başlatma.
- [ ] **Step 5 (uçtan uca):** sunucudan `curl -sk -o /dev/null -w '%{http_code}' https://10.99.0.11:8443/` (tünelden erişim); kullanıcı telefondan panel.wificorrect.com → numara+parola → cihaz paneli → Admin ayarları (admin parolası) → çıkış.
- [ ] **Step 6: Commit** — `uzak panel: Caddy cihaz.wificorrect.com, kurulum, günlük`
