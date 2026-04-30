# MP4/WEBM Transkript Masaüstü Uygulaması

Bu proje, `.mp4` veya `.webm` videoları masaüstünden seçip transkript üretir (`.srt`, `.vtt`, `.json`).

## Neden bu proje?

Bu proje, kişisel ihtiyacım için vibe coding yaklaşımıyla geliştirildi. Amaç: MP4/WEBM videolarını hızlıca SRT/VTT/JSON transkripte dönüştürmek.

## Özellikler

- MP4 (`ftyp`) ve WEBM (EBML + doctype) doğrulaması (2GB limit)
- FFmpeg ile mono/16kHz ses çıkarma
- Enerji tabanlı VAD ile konuşma tespiti
- 30 saniyelik chunking
- `whisper-cli` ile chunk bazlı transkripsiyon
- Timestamp offset düzeltmesi
- Anlık flush ile `.srt`, `.vtt`, `.json` yazımı
- Basit masaüstü arayüz (`egui`)

## Hızlı Başlangıç (Adım Adım)

> Aşağıdaki adımlar Ubuntu/Debian içindir.

### 1) Projeyi indir

```bash
git clone https://github.com/RudblestThe2nd/VideoTranscriber.git
cd VideoTranscriber
```

### 2) Rust / Cargo kur

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
cargo --version
```

### 3) FFmpeg kur

```bash
sudo apt update
sudo apt install -y ffmpeg
ffmpeg -version
```

### 4) whisper.cpp ve whisper-cli kur

```bash
sudo apt install -y git build-essential cmake
git clone https://github.com/ggerganov/whisper.cpp.git "$HOME/whisper.cpp"
cmake -S "$HOME/whisper.cpp" -B "$HOME/whisper.cpp/build"
cmake --build "$HOME/whisper.cpp/build" -j
```

`whisper-cli` komutunu PATH'e ekle:

```bash
echo 'export PATH="$PATH:$HOME/whisper.cpp/build/bin"' >> ~/.bashrc
source ~/.bashrc
whisper-cli --help
```

### 5) Model dosyasını indir (önerilen: Q5_0)

```bash
mkdir -p "$HOME/models"
wget -O "$HOME/models/ggml-large-v3-q5_0.bin" \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-q5_0.bin
ls -lh "$HOME/models/ggml-large-v3-q5_0.bin"
```

### 6) Uygulamayı çalıştır

```bash
cd VideoTranscriber
cargo run
```

## Uygulama İçinde Kullanım

Uygulama açıldığında:
1. MP4/WEBM dosyasını seç
2. Model olarak `ggml-large-v3-q5_0.bin` dosyasını seç
3. Çıktı klasörünü seç
4. `Transkripti Baslat` butonuna tıkla

## Doğrulama Komutları (Opsiyonel ama Önerilir)

Kurulum sonrası tek tek kontrol etmek için:

```bash
cargo --version
ffmpeg -version
whisper-cli --help
```

## Sık Karşılaşılan Hata

`Output file #0 does not contain any stream` hatası alırsan, seçilen videoda ses akışı olmayabilir.
Ses akışını kontrol etmek için:

```bash
ffprobe -v error -select_streams a -show_entries stream=index,codec_name -of compact "video.webm"
```

Çıktı boşsa videoda ses yoktur.
