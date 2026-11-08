# ئوقۇ (Oqu) — Sahifa okuyucu

Uygurca ve sağdan sola (RTL) yazılar için Android belge okuyucu. Çekirdeği Rust ile yazıldı. PDF, EPUB, Word (DOCX), ODT, FB2, HTML, TXT, Markdown ve CBZ dosyalarını açar.

**İndir:** [Releases](../../releases) sayfasından telefonuna uygun APK'yı al. Çoğu telefon için `arm64-v8a`, emin değilsen `universal`. Android 8.0 veya üstü gerekir.

## Motorlar
| Biçim | Motor | Neden |
|---|---|---|
| PDF | **PDFium** (Chromium, `pdfium-render` 0.9.5 + chromium/8125 ikilileri) | Chrome/Android ile aynı motor, BSD/Apache lisansı, kararlı ve hızlı. MuPDF bir tık daha hızlı olabilir ama AGPL lisanslı ve Rust'tan Android'e derlemesi zor. |
| EPUB, DOCX, ODT, FB2, HTML, TXT, MD, CBZ | Rust ayrıştırıcıları → temizlenmiş (X)HTML → **Android WebView (Blink + HarfBuzz + ICU)** | Arapça/Farsça/Urduca/İbranice için en iyi şekillendirme ve bidi desteği bu. |

## Uygurca desteği
- **Arama:** Uygurca belgelerde ى/ي, ې, ە, ۇ/ۆ/ۈ, ۋ, ڭ, ھ birbirinden ayrı tutulur. Yalnızca hece başındaki ئ yok sayılır; eski metinlerde ە yerine yazılan ه de eşleşir.
- **Latin harfli arama (ULY):** `kitab`, `ürümchide`, `séning` gibi aramalar Arap harfli metinde bulunur.
- **Dil tanıma:** Etiketi olmayan veya yanlış etiketli (ar/en/fa) Uygurca metinler `lang="ug"` olarak işaretlenir. Böylece şekillendirme ve klavye Uygurcaya göre çalışır.
- **Fontlar:** UKIJ Tuz, UKIJ Tuz Bold, UKIJ Ekran ve UKIJ Tuz Basma uygulamaya gömülüdür (OFL-1.1). Kitapların istediği ama telefonda olmayan "UKIJ Tuz Tom", "ALKATIP …" ve "Microsoft Uighur" adları bu dosyalara yönlendirilir.
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

Fontlar (SIL OFL 1.1): UKIJ (Uygur Bilgisayar Bilimi Derneği), Noto Naskh Arabic, Noto Nastaliq Urdu, Noto Sans Hebrew, Vazirmatn, Amiri.
