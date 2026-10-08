"""Yönetim merkezi veritabanı (spec §3): müşteriler, cihazlar, hareketler, eski arşiv bağları.
SQLite; tek bağlantı + kilit (ThreadingHTTPServer iş parçacıkları için). Yalnızca panel (wcpanel) açar, root açmaz."""
import contextlib
import datetime
import hashlib
import json
import secrets
import sqlite3
import threading
import time

import common
import guvenlik

NUMARA_ARALIGI = (1_000_000, 9_999_999)  # 7 haneli, rastgele (sıralı değil: müşteri sayısı ve sırası anlaşılmasın)
SEMA = """
CREATE TABLE IF NOT EXISTS musteri (
  numara INTEGER PRIMARY KEY, tuz TEXT NOT NULL, ozet TEXT NOT NULL, yineleme INTEGER NOT NULL,
  not_ TEXT NOT NULL DEFAULT '', uyelik TEXT NOT NULL DEFAULT 'aktif', bitis TEXT NOT NULL DEFAULT '',
  olusturma TEXT NOT NULL, son_giris TEXT NOT NULL DEFAULT '',
  parola_acik TEXT NOT NULL DEFAULT '', lisans_bitis TEXT NOT NULL DEFAULT '', askida INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS cihaz (
  id INTEGER PRIMARY KEY AUTOINCREMENT, musteri INTEGER NOT NULL REFERENCES musteri(numara),
  durum TEXT NOT NULL, tunel_ip TEXT NOT NULL DEFAULT '', wg_pub TEXT NOT NULL, ssh_pub TEXT NOT NULL,
  anahtar_ozet TEXT NOT NULL, isletme_adi TEXT NOT NULL DEFAULT '', unvan TEXT NOT NULL DEFAULT '',
  surum TEXT NOT NULL DEFAULT '', son_eslesme TEXT NOT NULL DEFAULT '', baglanma TEXT NOT NULL,
  admin_parola TEXT NOT NULL DEFAULT '', admin_tuz TEXT NOT NULL DEFAULT '',
  admin_ozet TEXT NOT NULL DEFAULT '', admin_yineleme INTEGER NOT NULL DEFAULT 0,
  ayrilma TEXT NOT NULL DEFAULT '');
CREATE UNIQUE INDEX IF NOT EXISTS cihaz_tek ON cihaz(musteri) WHERE durum != 'serbest';
CREATE TABLE IF NOT EXISTS hareket (
  id INTEGER PRIMARY KEY AUTOINCREMENT, zaman TEXT NOT NULL, kim TEXT NOT NULL, ip TEXT NOT NULL,
  olay TEXT NOT NULL, musteri INTEGER, ayrinti TEXT NOT NULL DEFAULT '');
CREATE INDEX IF NOT EXISTS hareket_musteri ON hareket(musteri, id);
CREATE TABLE IF NOT EXISTS eski_arsiv (ad TEXT PRIMARY KEY, musteri INTEGER REFERENCES musteri(numara));
CREATE TABLE IF NOT EXISTS ayar (anahtar TEXT PRIMARY KEY, deger TEXT NOT NULL);
"""
CIHAZ_ALANLARI = {"tunel_ip", "isletme_adi", "unvan", "surum", "son_eslesme"}
# Sonradan eklenen müşteri sütunları (eski veritabanına ALTER ile eklenir)
YENI_SUTUNLAR = (("parola_acik", "TEXT NOT NULL DEFAULT ''"), ("lisans_bitis", "TEXT NOT NULL DEFAULT ''"),
                 ("askida", "INTEGER NOT NULL DEFAULT 0"))
YENI_CIHAZ_SUTUNLARI = (("admin_parola", "TEXT NOT NULL DEFAULT ''"), ("admin_tuz", "TEXT NOT NULL DEFAULT ''"),
                        ("admin_ozet", "TEXT NOT NULL DEFAULT ''"), ("admin_yineleme", "INTEGER NOT NULL DEFAULT 0"))
SMS_ALANLARI = ("usercode", "password", "msgheader", "appkey")
LISANS_GUN = 365


def _yil_sonra(gun):
    return (gun + datetime.timedelta(days=LISANS_GUN)).isoformat()


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
        self.db.execute("PRAGMA secure_delete = ON")  # değişen açık parolalar dosyanın boş sayfalarında kalmasın
        self.db.executescript(SEMA)
        self._gocur()

    def _gocur(self):
        """Eski veritabanı: eksik sütunlar eklenir; lisansı olmayan müşteriye oluşturmadan 1 yıl verilir."""
        var = {r[1] for r in self.db.execute("PRAGMA table_info(musteri)")}
        for ad, tip in YENI_SUTUNLAR:
            if ad not in var:
                self.db.execute(f"ALTER TABLE musteri ADD COLUMN {ad} {tip}")
        for r in self.db.execute("SELECT numara, olusturma FROM musteri WHERE lisans_bitis = ''").fetchall():
            self.db.execute("UPDATE musteri SET lisans_bitis = ? WHERE numara = ?",
                            (_yil_sonra(datetime.date.fromisoformat(r["olusturma"][:10])), r["numara"]))
        var_c = {r[1] for r in self.db.execute("PRAGMA table_info(cihaz)")}
        for ad, tip in YENI_CIHAZ_SUTUNLARI:
            if ad not in var_c:
                self.db.execute(f"ALTER TABLE cihaz ADD COLUMN {ad} {tip}")

    def bugun(self):
        return datetime.datetime.fromtimestamp(self.saat(), common.TZ).date()

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
    def musteri_ekle(self, not_="", parola=None):
        """Parolayı yönetici yazar (2026-10-08, kullanıcı kararı); verilmezse (testler) rastgele."""
        if parola is not None and (not isinstance(parola, str) or len(parola) < guvenlik.EN_AZ):
            raise ValueError(f"Parola en az {guvenlik.EN_AZ} karakter olmalı.")
        pw = parola if parola is not None else guvenlik.parola_uret()
        tuz, oz, y = guvenlik.yeni_kayit(pw)
        with self._islem() as db:
            while True:
                numara = NUMARA_ARALIGI[0] + secrets.randbelow(NUMARA_ARALIGI[1] - NUMARA_ARALIGI[0] + 1)
                if db.execute("SELECT 1 FROM musteri WHERE numara = ?", (numara,)).fetchone() is None:
                    break
            db.execute("INSERT INTO musteri (numara, tuz, ozet, yineleme, not_, olusturma, parola_acik, lisans_bitis) "
                       "VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                       (numara, tuz, oz, y, (not_ or "").strip()[:200], self._simdi(), pw, _yil_sonra(self.bugun())))
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
            # ponytail: parola yönetimde gösterilsin diye (kullanıcı kararı 2026-10-04) açık saklanır; veritabanı yalnızca
            # wcpanel'in (600). Sızarsa müşteri parolaları da sızar — şifreli saklama ya da yalnızca sıfırlama ile değiştirilebilir.
            db.execute("UPDATE musteri SET tuz = ?, ozet = ?, yineleme = ?, parola_acik = ? WHERE numara = ?", (tuz, oz, y, pw, numara))
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

    # --- lisans (yıllık; ödeme ekranı sonradan lisans_uzat çağırır) ---
    def lisans_durumu(self, m):
        """(durum, bitiş): askida | bitti | aktif."""
        bitis = m["lisans_bitis"]
        if m["askida"]:
            return "askida", bitis
        if bitis and bitis < self.bugun().isoformat():
            return "bitti", bitis
        return "aktif", bitis

    def lisans_uzat(self, numara, gun=LISANS_GUN):
        """Bitişin üstüne (süresi geçmişse bugünden) `gun` gün ekler. Dönen: yeni bitiş."""
        with self._islem() as db:
            m = db.execute("SELECT lisans_bitis FROM musteri WHERE numara = ?", (numara,)).fetchone()
            taban = self.bugun()
            if m and m["lisans_bitis"]:
                taban = max(taban, datetime.date.fromisoformat(m["lisans_bitis"]))
            yeni = (taban + datetime.timedelta(days=gun)).isoformat()
            db.execute("UPDATE musteri SET lisans_bitis = ? WHERE numara = ?", (yeni, numara))
        return yeni

    def lisans_ayarla(self, numara, tarih):
        """Dönen: kaydedilen tarih (YYYY-AA-GG). Geçersizse ValueError."""
        tarih = datetime.date.fromisoformat(tarih).isoformat()
        with self._islem() as db:
            db.execute("UPDATE musteri SET lisans_bitis = ? WHERE numara = ?", (tarih, numara))
        return tarih

    def askiya_al(self, numara, askida):
        with self._islem() as db:
            db.execute("UPDATE musteri SET askida = ? WHERE numara = ?", (1 if askida else 0, numara))

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

    def serbest_iptal(self, numara):
        """Henüz tamamlanmamış serbest bırakmayı geri alır (cihaz temizlenmeden önce)."""
        with self._islem() as db:
            db.execute("UPDATE cihaz SET durum = 'bagli' WHERE musteri = ? AND durum = 'serbest_birakiliyor'", (numara,))

    def temizlendi(self, cid):
        with self._islem() as db:
            db.execute("UPDATE cihaz SET durum = 'serbest', ayrilma = ? WHERE id = ?", (self._simdi(), cid))

    # --- bütün cihazlarda tek admin parolası (2026-10-08, kullanıcı kararı): yalnızca özet saklanır ---
    # ponytail: cihaz başına admin_* sütunları (2026-10-05) artık kullanılmıyor; eski veritabanında boş durur.
    def admin_parolasi(self):
        r = self._bir("SELECT deger FROM ayar WHERE anahtar = 'admin'")
        return json.loads(r["deger"]) if r else None

    def admin_parolasi_koy(self, pw):
        if not isinstance(pw, str) or len(pw) < guvenlik.EN_AZ:
            raise ValueError(f"Parola en az {guvenlik.EN_AZ} karakter olmalı.")
        tuz, oz, y = guvenlik.yeni_kayit(pw)
        with self._islem() as db:
            db.execute("INSERT OR REPLACE INTO ayar (anahtar, deger) VALUES ('admin', ?)",
                       (json.dumps({"tuz": tuz, "ozet": oz, "yineleme": y}),))

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
