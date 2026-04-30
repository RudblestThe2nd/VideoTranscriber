# MP4/WEBM Transkript Masaustu Uygulamasi

Bu proje, `.mp4` veya `.webm` dosyasini alip transkript cikarir.

## Ozellikler

- MP4 (`ftyp`) ve WEBM (EBML + doctype) validasyonu (2GB limit)
- FFmpeg subprocess ile mono/16kHz WAV extraction
- Enerji tabanli VAD ile konusma tespiti
- 30 saniye chunking
- Chunk bazli `whisper-cli` transkripsiyonu
- Timestamp offset duzeltme
- Anlik flush ile `.srt`, `.vtt`, `.json` cikti yazimi
- Basit masaustu arayuz (`egui`)

## Gereksinimler

- Rust toolchain (`cargo`)
- `ffmpeg` komutu PATH icinde
- `whisper-cli` komutu PATH icinde (whisper.cpp binary)
- Quantized model dosyasi (onerilen: `ggml-large-v3-q5_0.bin`)

## Calistirma

```bash
cargo run
```

Uygulama acildiginda:
1. MP4/WEBM dosyasini sec
2. Model dosyasini sec
3. Cikti klasorunu sec
4. `Transkripti Baslat` butonuna tikla

## Not

Arayuz sade tutulmustur; odak pipeline'in calisir olmasidir.
