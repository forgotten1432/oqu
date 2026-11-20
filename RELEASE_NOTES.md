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

## ئوقۇ (Oqu) 0.3.1

**Okuma ayarları**
- Kitap okuyucunun üst çubuğunda her zaman görünen **Aa** düğmesi: yazı tipi, boyut ve arka plan tek dokunuşla değişir.
- Ayarlar yeniden düzenlendi. Önce tema örnekleri geliyor: açık, sepya, yeşil, gece kâğıdı, koyu, siyah. Sonra A−/A+ ile boyut, kendi yazı tipiyle önizlenen fontlar, en sonda satır aralığı ve düzen.
- Koyu okuma temaları okuyucunun çubuklarını ve panellerini de koyulaştırır.
- Açık ve koyu tema arasında geçiş yapınca okuyucu artık baştan yüklenmiyor.

**Uygulama içinden güncelleme**
- Uygulama günde bir kez GitHub Releases'ta yeni sürüm olup olmadığına bakar (yalnızca internet varken) ve yeni sürüm varsa bildirim gösterir.
- Güncelleme penceresi yenilikleri gösterir, cihaza uygun APK'yı indirir, SHA-256 özetini `SHA256SUMS.txt` ile karşılaştırır ve Android'in yükleyicisini açar. İlk seferde yükleme izni istenir.
- Ayarlarda: otomatik kontrolü açma/kapama, **Şimdi kontrol et** ve yüklü sürüm.
- İnternet yalnızca güncelleme kontrolü ve indirme için kullanılır; belgeler cihazdan çıkmaz.

## ئوقۇ (Oqu) 0.3.0

**İki sayfa görünümü**
- PDF'de yeni **İki sayfa** görünümü. Yatay tutunca kendiliğinden açılır, istenirse kapatılabilir. Kapak sayfası ayrı gösterilebilir. Görünüm veya yön değişince kalınan yer korunur.
- Uygurca ve Arapça gibi sağdan sola belgeler basılı kitap gibi açılır: ilk sayfa sağda, ileri gitmek için soldan sağa kaydırılır.
- Kitaplar yatayda ekranda iki sütun gösterir (otomatik / her zaman / hiçbir zaman). Sütunlar belgenin yazı yönünü izler.
- Alttaki araç çubukları kaydırılabilir oldu; uzun etiketler artık kesilmiyor.

**Tablet düzeni**
- 600 dp ve üstü ekranlarda alt çubuk yerine yan gezinme çubuğu var. Kitaplık kapak ızgarası olarak gösteriliyor (PDF'nin ilk sayfası, EPUB/FB2/CBZ kapağı).
- Okuyucuda küçük resimler, içindekiler ve notlar alttan açılan sayfa yerine sabit bir yan panelde.
- Geniş ekranda kitap metni okunaklı genişlikte tek sütun olarak ortalanır.
- Sağdan sola arayüzlerde gezinme çubuğu ve panel sağa geçer.

## ئوقۇ (Oqu) 0.2.0

**PDF doldur ve imzala**
- Metin, çizilmiş imza (sonra tekrar kullanmak üzere kaydedilir), tarih, onay/çarpı işareti ve beyaz örtü eklenebilir. Hepsi sürüklenip boyutlandırılabilir ve PDF'nin içine yazılır. Metni Android şekillendirdiği için Uygurca harfler doğru birleşir.
- Uygulamada eklenen vurgular ve notlar, kaydedince PDF'ye gerçek PDF açıklamaları olarak gömülür.
- Sayfa döndürme ve silme. Düzenlemeler ve arama döndürülmüş sayfalarda da doğru yerde çalışır.

**Yazdırma**
- PDF'ler olduğu gibi, kitaplar açık renkli düz bir düzenle yazdırılır.

**Uygulama dili**
- Uygulama içinden dil seçimi: sistem, Uygurca, Türkçe, İngilizce, Arapça. Arayüzün yönü dile göre kendiliğinden değişir; belgenin yönü her belge için ayrı kalır.

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
