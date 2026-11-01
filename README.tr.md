# Live Meeting Assistant

[English](README.md) | Türkçe

Sanal dersler için bir masaüstü yardımcısı. Bilgisayarınızın sistem sesini dinler, bunu Whisper ile yerel olarak yazıya döker, adınız, bir anahtar kelime ya da sınıfa yöneltilmiş bir soru geçtiğinde sizi uyarır ve — bir tetikleyici ya da **Ask now** kısayolu ile — bir LLM'den *ne sorulduğunu* ve *önerilen bir cevabı* ister. Dersin sonunda Obsidian kasanıza İngilizce bir özet ve tam transkripti yazar. Her toplantı uygulamasıyla çalışır (Teams, Zoom, bir tarayıcı oynatıcısı), çünkü konuşmacıların çaldığı sesi yakalar.

## Özellikler

- **Sistem sesinin yerel transkripsiyonu** Whisper ile (Windows'ta CUDA, macOS'ta Metal, CPU'ya geri dönüş). Ses yalnız bellekte işlenir.
- **İsim / anahtar kelime / soru uyarıları**: adınızın varyantlarının kelime sınırı eşleşmesi (uzun varyantlar için bir harf toleranslı), anahtar kelime listesi ve LLM'in onayladığı genel soru sezgiseli. Tekrarları önlemek için terim ve özellik başına bekleme süresi.
- **Ask now** kısayolu (varsayılan Windows'ta `Ctrl+Shift+Space`, macOS'ta `Cmd+Shift+Space`) ve isim ile sınıf soruları tetiklendiğinde otomatik analiz.
- Son ~350 kelimenin (ve isteğe bağlı bir ekran görüntüsü bölgesinin) **LLM analizi**: sorulan soru ve kısa önerilen cevap, bir güven düzeyiyle birlikte uygulamada ve bildirim olarak gösterilir.
- Obsidian klasörüne yazılan **ders notları**: özet, notlar ve yapılacaklar içeren bir özet dosyası ile özel bir tam transkript.

## Gizlilik

- **Ses bilgisayardan çıkmaz ve diske yazılmaz.** Bellekteki bir halka tamponda durur ve transkripsiyondan sonra silinir.
- **Ekran görüntüleri varsayılan olarak kapalıdır ve diske yazılmaz.** Bellekte yakalanır ve istekten sonra silinir. Yalnız seçili bir bölge, seçili bir pencere ya da tam monitör yakalanır ve yalnız açıkça etkinleştirildiğinde.
- **Metin, yapılandırdığınız LLM sağlayıcısına gider** (varsayılan OpenCode Go): her analiz için son ~350 kelime, genel soru sınıflandırması için son ~150 kelime, özet üretirken tam transkript ve etkinse ekran görüntüsü bölgesi. Ses gönderilmez.
- **API anahtarı işletim sisteminin anahtarlığında saklanır** (Windows Kimlik Bilgileri Yöneticisi / macOS Anahtarlık), settings JSON dosyasında veya loglarda asla. Ayarlar işletim sisteminin uygulama yapılandırma dizininde; Whisper modelleri işletim sisteminin uygulama verisi dizininde durur.
- **Bu depo hiçbir ders verisi, ayar, transkript, anahtar veya model dosyası içermez.** `.gitignore` bunları dışarıda tutar.

## İndirme

Hazır kurulumlar [Releases](../../releases) sayfasındadır:

- **Windows** (`…_x64-setup.exe` veya `.msi`): **CUDA 12** kurulu bir **NVIDIA RTX GPU** gerekir. Diğer GPU'lar veya yalnız CPU için kendiniz derleyin (aşağıda).
- **macOS** (`.dmg`): Apple Silicon. Uygulama imzalı değildir: ilk seferde sağ tıklayıp **Open** seçin.

İlk çalıştırmada **Settings**'i açın, API anahtarınızı girin, Whisper modelini indirin ve notlar klasörünü seçin. Windows, imzasız kurulum için SmartScreen uyarısı gösterebilir (**More info → Run anyway**).

## Gereksinimler (kaynaktan derleme)

- **Windows 11**: NVIDIA GPU + CUDA Toolkit 12.x önerilir (yayın derlemesi CUDA'yı açar); bunlar olmadan CPU'da çalışır.
- **macOS 13+** (Apple Silicon, Metal); Window ekran görüntüsü modu macOS 14 veya üstünü gerektirir. Sistem sesi yakalama ve ekran görüntüleri için Sistem Ayarları → Gizlilik ve Güvenlik'te **Screen Recording** iznini verin; izin olmadan uygulama bir izin hatası bildirir.
- **Rust** stable (1.90 veya üstü) ve **Tauri CLI** (`cargo install tauri-cli --version ^2`).
- **Node 22**.
- **CMake** ve **libclang** (whisper-rs derlemek için).
  - Windows: `pip install --user libclang` veya LLVM kurun.
  - macOS: Xcode komut satırı araçları (`xcode-select --install`).

## Derleme ve çalıştırma

```sh
npm --prefix ui ci

# Windows: scripts/build.ps1 CMake'i (Visual Studio içinde de) ve libclang'i sizin için bulur
powershell -File scripts/build.ps1        # yayın paketi (.msi)
powershell -File scripts/build.ps1 dev    # geliştirme

# macOS (Xcode komut satırı araçları clang ve araç zincirini sağlar)
cargo tauri build --features metal        # .app/.dmg
cargo tauri dev --features metal
```

Windows'ta düz bir `cargo tauri build`, `LIBCLANG_PATH` (ve `PATH` üzerinde CMake) ayarlanmadıkça "Unable to find libclang" ile başarısız olur; `scripts/env.ps1` bunları ayarlar.

Kalite kapısı (format, clippy, testler, audit, deny, arayüz typecheck/lint/test/build):

```powershell
powershell -File scripts/check.ps1
```

Kapı `CHECK PASSED` ile bitmelidir.

## İlk çalıştırma

1. **Settings**'i açın ve API base URL'ini, API anahtarını ve modeli ayarlayın. Varsayılanlar `https://opencode.ai/inference/go/anthropic` ve `deepseek-v4.1-flash`; Anthropic-Messages uyumlu her uç nokta çalışır.
2. Whisper modelini (varsayılan `large-v3-turbo-q5_0`) **Download** düğmesiyle indirin. Modeller Hugging Face'teki whisper.cpp deposundan alınır ve checksum ile doğrulanır.
3. **notes folder**'ınızı seçin, ör. `<vault>/okul`. Kurslar bu klasörün alt klasörleridir.
4. **name variants**'ınızı ekleyin, **keywords**'ü ayarlayın (varsayılanlar: homework, assignment, exam, quiz, deadline, attendance) ve isterseniz **hotkey**'i değiştirin. İsterseniz bir **explanation language** (ör. Turkish, Deutsch) ayarlayın; her önerilen cevabın altında 1–2 cümlelik bir açıklama alırsınız; kapalı bırakmak için boş bırakın.
5. Bir **screenshot mode** seçin: Off (varsayılan), Region (bir ekran bölgesi seçin), Window (pencereyi adıyla seçin) veya Full ve yakalanacak monitörü seçin. Window, seçilen tek bir pencereyi adıyla yakalar; macOS'ta macOS 14 veya üstünü gerektirir.
6. Bir kurs seçin ve **Start**'a basın. Uygulama pencere tepsiye gizliyken dinler; pencereyi kapatmak oturumu durdurmaz.

**Pencere ekran görüntüleri:** seçilen pencere başka pencerelerin arkasında olsa da yakalanabilir, ancak Chromium tabanlı uygulamalar bakmadığınız bir pencereyi çizmeyi durdurabilir. Bu olduğunda uygulama siyah bir resim göndermek yerine durumu bildirir; Settings'te Microsoft Teams ile henüz denenmemiş bir yeniden başlatma komutu çözümü bulunur.

## Kasa çıktısı

`Algorithms 101` kursu için 2026-10-22'de bir oturum şunları yazar:

```
<notes>/Algorithms 101/2026-10-22-ders.md          # özet, visibility: internal
<notes>/Algorithms 101/2026-10-22-transcript.md    # tam transkript, visibility: private
```

Aynı kursun aynı gün ikinci oturumu her iki dosyaya `-2` soneki alır (sonrasında `-3`, …). Mevcut dosyalar asla üzerine yazılmaz.

Özet frontmatter'ı:

```yaml
---
kind: note
project: okul
ders: "Algorithms 101"
tur: ders-ozeti
tarih: 2026-10-22
visibility: internal
---
```

Gövdede `## Summary`, `## Notes`, `## Action items` ve transkript dosyasına bir `## Transcript` wikilink'i bulunur. Transkript aynı frontmatter'ı `tur: transcript` ve `visibility: private` ile kullanır ve ders sürerken satırları `**[HH:MM:SS]** metin` biçiminde ekler.

## Proje düzeni

- `core/` — Rust mantığı: ses yakalama, Whisper STT, tetikleyiciler, LLM istemcisi, ekran görüntüleri, kasa yazımı, özetleme.
- `src-tauri/` — Tauri uygulaması: komutlar, olaylar, tepsi, bildirimler, genel kısayol.
- `ui/` — React + TypeScript arayüzü (ana görünüm, ayarlar, bölge seçici).
- `scripts/` — `check.ps1` kalite kapısı ve test fikstür yardımcısı.
- `docs/` — tasarım spesifikasyonu ve uygulama planı.

## Lisans

MIT. Bkz. [LICENSE](LICENSE).
