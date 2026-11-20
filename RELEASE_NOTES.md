## ئوقۇ (Oqu) 0.3.2

**Düzeltmeler**
- Android System WebView yüklü değilse, kapalıysa ya da sürümü cihazla uyuşmuyorsa PDF dışındaki kitaplar (EPUB, DOCX, FB2…) açılırken uygulama artık çökmüyor. Bunun yerine bir uyarı ekranı çıkıyor. Ekrandaki **WebView'i güncelle** düğmesi, WebView'in Play Store sayfasını açar.
- Okuma sırasında WebView çökerse uygulama kapanmıyor, aynı uyarı ekranı gösteriliyor.

**Hangi APK'yı indirmeliyim?**
- `oqu-0.3.2-arm64-v8a.apk`: çoğu güncel telefon (önerilen)
- `oqu-0.3.2-armeabi-v7a.apk`: eski 32 bit telefonlar
- `oqu-0.3.2-x86_64.apk`: emülatörler ve Chromebook'lar
- `oqu-0.3.2-universal.apk`: tüm cihazlar (daha büyük)

Android 8.0 veya üstü gerekir.

## ئوقۇ (Oqu) 0.1.0 — ilk sürüm

Uygurca ve sağdan sola yazılar için Android belge okuyucu. Çekirdeği Rust ile yazıldı.

**Biçimler:** PDF (Chromium'un PDFium motoru), EPUB 2/3, Word DOCX, ODT, FB2, HTML, TXT, Markdown, CBZ

**Uygurca**
- Arama ى/ي, ې, ە, ۇ/ۆ/ۈ, ۋ, ڭ harflerini birbirinden ayırır; hece başındaki ئ yok sayılır.
- Latin harfle arama yapılabilir: `kitab` yazınca «كىتاب» bulunur.
- Uygurca metin otomatik tanınır (`lang="ug"`), sağdan sola düzen ve şekillendirme buna göre yapılır.
- UKIJ Tuz, Ekran ve Basma fontları uygulamada gömülü. Kitapların istediği UKIJ, ALKATIP ve Microsoft Uighur fontları bunlarla gösterilir.
- Arayüz Uygurca, Türkçe, Arapça ve İngilizce.

**Okuyucu (Acrobat benzeri)**
- PDF: sürekli kaydırma veya tek sayfa (sağdan sola çevirme), iki parmakla yakınlaştırma, küçük resimler, içindekiler, arama vurguları, gece modu, okuma modu, vurgulama, not, yer imi.
- Kitaplar: sayfalı veya kaydırmalı okuma; yazı tipi, boyut, satır aralığı, tema (açık, sepya, koyu, siyah), seçili metni vurgulama.

**Hangi APK'yı indirmeliyim?**
- `oqu-0.1.0-arm64-v8a.apk`: çoğu güncel telefon (önerilen)
- `oqu-0.1.0-armeabi-v7a.apk`: eski 32 bit telefonlar
- `oqu-0.1.0-x86_64.apk`: emülatörler ve Chromebook'lar
- `oqu-0.1.0-universal.apk`: tüm cihazlar (daha büyük)

Android 8.0 veya üstü gerekir. Bu sürüm internet izni istemez. 0.3.1 ile gelen güncelleme kontrolünden beri uygulama interneti yalnızca GitHub'dan yeni sürümü denetleyip indirmek için kullanır; belgeler cihazdan çıkmaz.
