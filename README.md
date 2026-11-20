# ئوقۇ (Oqu) — Sahifa okuyucu

Uygurca ve sağdan sola (RTL) yazılar için Android belge okuyucu. Çekirdeği Rust ile yazıldı. PDF, EPUB, Word (DOCX), ODT, FB2, HTML, TXT, Markdown ve CBZ dosyalarını açar.

**İndir:** [Releases](../../releases/latest) sayfasından **`arm64-v8a`** APK'sını indir; bu dosya çoğu telefon ve tablette, Android 17 dahil, çalışır. Emin değilsen `universal` dosyasını al. `old-32bit` dosyası yalnızca çok eski 32 bit telefonlar içindir; yeni cihazlar onu kurmaz. APK'yı WPS gibi bir ofis uygulamasıyla değil, **Dosyalar** uygulamasından ya da Chrome'un **İndirilenler** bölümünden aç. Android 8.0 veya üstü gerekir.

## Motorlar
| Biçim | Motor | Neden |
|---|---|---|
| PDF | **PDFium** (Chromium, `pdfium-render` 0.9.5 + chromium/8125 ikilileri) | Chrome/Android ile aynı motor, BSD/Apache lisansı, kararlı ve hızlı. MuPDF bir tık daha hızlı olabilir ama AGPL lisanslı ve Rust'tan Android'e derlemesi zor. |
| EPUB, DOCX, ODT, FB2, HTML, TXT, MD, CBZ | Rust ayrıştırıcıları → temizlenmiş (X)HTML → **Android WebView (Blink + HarfBuzz + ICU)** | Arapça/Farsça/Urduca/İbranice için en iyi şekillendirme ve bidi desteği bu. |

## Android System WebView sorunları
PDF dışındaki kitaplar telefondaki **Android System WebView** ile gösterilir. WebView yüklü değilse, kapalıysa ya da güncellemesi bozuk olduğu için sürümü cihazla uyuşmuyorsa, 0.4.0'dan beri uygulama çökmüyor; bunun yerine bir uyarı ekranı çıkıyor. Okuma sırasında WebView çökerse de uygulama kapanmıyor, aynı ekran gösteriliyor.
- **WebView'i güncelle** düğmesi, telefondaki WebView'in Play Store sayfasını açar. WebView'i ve Chrome'u birlikte güncel sürüme getir.
- Sorun yeni bir güncellemeden sonra başladıysa: Ayarlar → Uygulamalar → *Android System WebView* (veya *Google Chrome*) → ⋮ → **Güncellemeleri kaldır**.
- PDF'ler WebView kullanmaz; bu durumda da açılır.

## Cihazdaki belgeler
- **Dosyalar** sekmesi, telefondaki ve SD karttaki PDF, EPUB, DOCX, ODT, FB2, HTML, MD, TXT ve CBZ dosyalarını otomatik bulup listeler. Sekme her açıldığında liste arka planda yenilenir.
- Biçime göre süzme ve ada göre arama yapılabilir. Dokunulan belge açılır ve Son açılanlar'a eklenir; dosya kopyalanmaz, yerinde açılır.
- Bunun için Android 11 ve üstünde **Tüm dosyalara erişim**, Android 8–10'da depolama izni gerekir. Dosyalar yalnızca cihazda okunur, hiçbir yere gönderilmez.
- Gizli klasörler, `Android/data` ve `Android/obb` taranmaz. Çok küçük TXT/HTML/MD dosyaları (2 KB altı) uygulama artığı sayılıp atlanır.

## Düzenleme, imza ve yazdırma
- **Doldur ve İmzala (PDF):**
  - Ekleyebileceklerin: metin, parmakla çizilen imza (kaydedilir ve tekrar kullanılabilir), tarih, ✓ / ✗ ve beyazla örtme.
  - Eklenen öğeler sürüklenip boyutlandırılabilir.
  - Kaydedince öğeler PDF'in içine yazılır. Uygurca metin Android'in kendi motoruyla şekillendirildiği için harfler doğru bağlanır.
  - Uygulamada yapılan vurgu ve notlar, gerçek PDF açıklaması (annotation) olarak dosyaya gömülür. Böylece başka PDF okuyucularında da görünürler.
- **Sayfalar:** Sayfa döndürme ve silme. Önceden döndürülmüş sayfalarda da koordinatlar doğru hesaplanır.
- **Yazdırma:** PDF'ler Android'in yazdırma servisine olduğu gibi (vektörel) gönderilir. Kitaplarda o anki bölüm açık temada yazdırılır.

## İki sayfa görünümü
- **PDF:** Görünüm menüsünde üç seçenek var: sürekli kaydırma, tek sayfa ve iki sayfa.
  - Telefon yan çevrilince otomatik olarak iki sayfaya geçer; dik tutunca önceki görünüme döner. Bu davranış menüden kapatılabilir.
  - Görünüm değişince kaldığın sayfa korunur.
  - İstersen kapak sayfası ayrı gösterilir.
- **Sağdan sola belgeler (Uygurca, Arapça):** Basılı kitap gibi açılır.
  - 1. sayfa sağda, 2. sayfa solda durur.
  - Sayfalar sağdan sola çevrilir: parmağını soldan sağa kaydırınca ileri gider.
  - Tek başına kalan kapak sayfası da kitap düzenine göre yerleşir: soldan sağa kitaplarda sağda, sağdan sola kitaplarda solda.
- **Kitaplar (EPUB, Word vb.):** Yatay ekranda iki sütun gösterilir. Metin ayarlarından seçilir: "Yan çevirince", "Her zaman" veya "Hiçbir zaman".
  - Sağdan sola kitaplarda metin sağ sütunda başlar.

## Otomatik güncelleme
- **Kontrol:** Uygulama günde bir kez (internet varken) GitHub'daki son sürüme bakar. Yeni sürüm varsa telefona bildirim gelir.
- **Açılışta:** Uygulama açıldığında "Yeni sürüm" penceresi değişiklik notlarıyla gösterilir.
- **Güncelle:** Cihazın işlemcisine uygun APK (çoğu cihazda arm64) uygulamanın içinde indirilir. SHA-256 sağlama toplamı sürümdeki listeyle karşılaştırılır ve Android'in kurulum ekranı açılır. İlk seferde Android, uygulamaya güncelleme yükleme izni verilmesini ister.
- **Ayarlar:** "Güncellemeleri otomatik kontrol et" anahtarı ve "Şimdi kontrol et" butonu Ayarlar sekmesinde.
- **İnternet kullanımı:** Yalnızca `api.github.com` ve GitHub'ın sürüm dosyalarına bağlanılır. Belgeler hiçbir yere gönderilmez.

## Okuma ayarları (kitaplar)
- **Erişim:** Üst çubuktaki **Aa** butonu yazı tipini, yazı boyutunu ve arka planı tek dokunuşla açar.
- **Temalar:** Açık, Sepya, Yeşil, Gece kâğıdı, Koyu ve Siyah. Koyu temalarda uygulamanın çubukları ve pencereleri de koyulaşır.
- **Yazı boyutu:** A− / A+ butonları ya da kaydırıcıyla ayarlanır.
- **Yazı tipleri:** Her seçenek kendi yazı tipiyle gösterilir. UKIJ Tuz/Ekran/Basma, Naskh, Vazirmatn, Amiri, Nastaliq, Hebrew ve içe aktarılan fontlar var.
- **Diğer:** Satır aralığı, kenar boşluğu, iki yana yaslama, yazı yönü, sayfalı veya kaydırmalı düzen.

## Tablet görünümü
Ekran genişliği 600dp'yi geçince (tabletler, katlanabilir telefonlar) uygulama telefondan farklı görünür:
- **Ana ekran:** Gezinme çubuğu yanda durur. Belgeler kapak resimli bir ızgarada gösterilir: PDF'lerde ilk sayfa, EPUB/FB2/CBZ'de kitabın kendi kapağı.
- **Okuyucu:** Sayfa küçük resimleri, içindekiler ve notlar belgenin yanında sabit bir panelde açık kalır. PDF'lerde panel üç sekmeli, kitaplarda iki sekmelidir.
- **Kitap metni:** Satırlar rahat okunur bir genişlikte, ortalanmış olarak gösterilir.
- **Uygurca ve Arapça arayüz:** Gezinme çubuğu ve panel otomatik olarak sağ tarafa geçer.

## Dil ve yön
- **Arayüz:** Dil Ayarlar'dan seçilir (Sistem, ئۇيغۇرچە, Türkçe, English, العربية). Uygurca ve Arapça seçilince arayüz otomatik olarak sağdan sola, diğer dillerde soldan sağa olur.
- **Belge içeriği:** Arayüz dilinden bağımsızdır, yönü belgenin kendi diline göre otomatik belirlenir. Örneğin Türkçe arayüzde açılan Uygurca kitap sağdan sola gösterilir.

## Uygurca desteği
- **Arama:** Uygurca belgelerde ى/ي, ې, ە, ۇ/ۆ/ۈ, ۋ, ڭ, ھ birbirinden ayrı tutulur. Yalnızca hece başındaki ئ yok sayılır; eski metinlerde ە yerine yazılan ه de eşleşir.
- **Latin harfli arama (ULY):** `kitab`, `ürümchide`, `séning` gibi aramalar Arap harfli metinde bulunur.
- **Dil tanıma:** Etiketi olmayan veya yanlış etiketli (ar/en/fa) Uygurca metinler `lang="ug"` olarak işaretlenir. Böylece şekillendirme ve klavye Uygurcaya göre çalışır.
- **Fontlar:** UKIJ yazı tiplerinin tamamı (79 aile, 98 dosya: Tuz, Ekran, Basma, Kufi, Nasq, Sulus, Diwani, Qolyazma, Tughra, CJK…) uygulamaya gömülüdür (OFL-1.1 / LGPL-3). Kitap hangi UKIJ fontunu isterse o font kullanılır. Telefonda olmayan "ALKATIP …" ve "Microsoft Uighur" adları en yakın UKIJ fontuna yönlendirilir. Okuma ayarlarındaki **Tüm UKIJ fontları** listesinden hepsi seçilebilir.
- **Arayüz:** Uygurca çeviri (`values-ug`) var.

## Rust çekirdeği (`rust/`)
- `pdf.rs`: PDFium'un tek bir worker thread'de çalışması; parça (tile) render, içindekiler, linkler, sayfa bazlı arama.
- `pdf_layout.rs`: PDF'te ters veya karışık sırayla gelen Arapça/İbranice metni glif koordinatlarından mantıksal sıraya geri kurar (lam-elif, hareke, sayılar).
- `rtl.rs`: Yön tespiti. Arama normalizasyonu: tashkeel/niqqud yoksayma, elif/ye/kef varyantları, sunum biçimleri (NFKC), Arap-Hint rakamları.
- `reflow/`: EPUB 2/3 (nav + NCX, `page-progression-direction`), DOCX (`w:bidi`, `bidiVisual`, listeler, dipnotlar, görseller), ODT, FB2, HTML, TXT (cp1256/1255 otomatik), Markdown, CBZ.
- `library.rs`: Son açılanlar, ilerleme, yer imleri, vurgular ve notlar (JSON).
- Kotlin'e UniFFI ile bağlanır.

## Derleme
Gerekenler: Rust (`rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`), `cargo install cargo-ndk`, Android SDK + NDK, JDK 17 veya 21.
```bash
./build-rust.sh                        # PDFium'u indirir, Rust → jniLibs, Kotlin bağlamaları
cd android && ./gradlew assembleDebug  # veya assembleRelease
```
Sürüm imzası `~/.android/oqu-keystore.properties` dosyasından okunur. Dosya yoksa debug anahtarı kullanılır.
Masaüstü testi: `cd rust && cargo test && cargo run --example smoke -- <libpdfium> <dosyalar>`

**Yayınlama:** `./release.sh` Rust çekirdeğini ve APK'ları derler, yayın anahtarıyla imzalar, `SHA256SUMS.txt` dosyasını üretir, `vX.Y.Z` etiketini gönderir ve notları `RELEASE_NOTES.md` dosyasından alarak GitHub Release'i oluşturur. Önce taslak olarak oluşturmak için `./release.sh --draft`. Gerekenler: yukarıdaki derleme araçları, yayın anahtarı ve oturum açılmış GitHub CLI (`gh auth login`).

Fontlar: UKIJ (Uygur Bilgisayar Bilimi Derneği; OFL-1.1 veya LGPL-3), SIL OFL 1.1: Noto Naskh Arabic, Noto Nastaliq Urdu, Noto Sans Hebrew, Vazirmatn, Amiri.
