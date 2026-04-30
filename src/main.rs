use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use eframe::egui;
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use rfd::FileDialog;
use serde::Serialize;
use tempfile::TempDir;

const SAMPLE_RATE: u32 = 16_000;
const MAX_MEDIA_SIZE: u64 = 2_147_483_648;
const FRAME_MS: usize = 30;
const CHUNK_SECONDS: f32 = 30.0;
const ENERGY_THRESHOLD: f32 = 0.012;
const SILENCE_HANGOVER_FRAMES: usize = 8;

fn main() -> Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "MP4/WEBM Transkript Araci",
        options,
        Box::new(|_cc| Box::<TranskriptApp>::default()),
    )
    .map_err(|e| anyhow!("UI baslatilamadi: {e}"))
}

#[derive(Default)]
struct TranskriptApp {
    media_path: String,
    model_path: String,
    output_dir: String,
    status: String,
    in_progress: bool,
    rx: Option<Receiver<WorkerMsg>>,
}

impl eframe::App for TranskriptApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_windows_xp_theme(ctx);

        if let Some(rx) = &self.rx {
            loop {
                match rx.try_recv() {
                    Ok(WorkerMsg::Progress(msg)) => self.status = msg,
                    Ok(WorkerMsg::Done(msg)) => {
                        self.status = msg;
                        self.in_progress = false;
                        self.rx = None;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.in_progress = false;
                        self.rx = None;
                        break;
                    }
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Transkriptit");
            ui.label("Demo Versiyonu");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Video Dosya:");
                ui.text_edit_singleline(&mut self.media_path);
                if ui.button("Sec").clicked() {
                    if let Some(path) = FileDialog::new()
                        .add_filter("Video", &["mp4", "webm"])
                        .pick_file()
                    {
                        self.media_path = path.display().to_string();
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label("Model (ggml*.bin):");
                ui.text_edit_singleline(&mut self.model_path);
                if ui.button("Sec").clicked() {
                    if let Some(path) = FileDialog::new().pick_file() {
                        self.model_path = path.display().to_string();
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label("Cikti Klasoru:");
                ui.text_edit_singleline(&mut self.output_dir);
                if ui.button("Sec").clicked() {
                    if let Some(path) = FileDialog::new().pick_folder() {
                        self.output_dir = path.display().to_string();
                    }
                }
            });

            ui.separator();

            let can_start = !self.in_progress
                && !self.media_path.trim().is_empty()
                && !self.model_path.trim().is_empty()
                && !self.output_dir.trim().is_empty();
            if ui
                .add_enabled(can_start, egui::Button::new("Transkripti Baslat"))
                .clicked()
            {
                let media = self.media_path.clone();
                let model = self.model_path.clone();
                let out = self.output_dir.clone();
                let (tx, rx) = mpsc::channel();
                self.rx = Some(rx);
                self.in_progress = true;
                self.status = "Pipeline baslatiliyor...".to_string();

                thread::spawn(move || {
                    let _ = tx.send(WorkerMsg::Progress("Validation yapiliyor...".to_string()));
                    let result = run_pipeline(&media, &model, &out, |m| {
                        let _ = tx.send(WorkerMsg::Progress(m));
                    });
                    match result {
                        Ok(paths) => {
                            let msg = format!(
                                "Tamamlandi.\nSRT: {}\nVTT: {}\nJSON: {}",
                                paths.srt.display(),
                                paths.vtt.display(),
                                paths.json.display()
                            );
                            let _ = tx.send(WorkerMsg::Done(msg));
                        }
                        Err(e) => {
                            let _ = tx.send(WorkerMsg::Done(format!("Hata: {e:#}")));
                        }
                    }
                });
            }

            if self.in_progress {
                ui.label("Durum: Calisiyor...");
            }
            
            ui.label(&self.status);
        });

        ctx.request_repaint();
    }
}

fn apply_windows_xp_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.visuals = egui::Visuals::light();
    style.visuals.panel_fill = egui::Color32::from_rgb(212, 230, 254);
    style.visuals.window_fill = egui::Color32::from_rgb(236, 244, 255);
    style.visuals.override_text_color = Some(egui::Color32::from_rgb(15, 45, 90));
    style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(224, 238, 255);
    style.visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(140, 175, 220));
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(178, 212, 252);
    style.visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(82, 129, 196));
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(146, 197, 251);
    style.visuals.widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(51, 106, 179));
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(111, 174, 246);
    style.visuals.widgets.active.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(34, 87, 160));
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(49, 106, 197);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(255, 255, 255);
    style.visuals.faint_bg_color = egui::Color32::from_rgb(238, 247, 255);
    style.visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(76, 116, 170));
    style.visuals.widgets.noninteractive.rounding = egui::Rounding::same(2.0);
    style.visuals.widgets.inactive.rounding = egui::Rounding::same(2.0);
    style.visuals.widgets.hovered.rounding = egui::Rounding::same(2.0);
    style.visuals.widgets.active.rounding = egui::Rounding::same(2.0);
    style.visuals.window_rounding = egui::Rounding::same(4.0);
    ctx.set_style(style);
}

enum WorkerMsg {
    Progress(String),
    Done(String),
}

#[derive(Debug)]
struct OutputPaths {
    srt: PathBuf,
    vtt: PathBuf,
    json: PathBuf,
}

#[derive(Debug, Clone)]
struct SpeechChunk {
    start_sec: f32,
    samples: Vec<f32>,
}

#[derive(Debug, Clone)]
struct TimedSegment {
    start_sec: f32,
    end_sec: f32,
    text: String,
}

#[derive(Serialize)]
struct JsonSegment {
    start: f32,
    end: f32,
    text: String,
}

fn run_pipeline(
    media_path: &str,
    model_path: &str,
    output_dir: &str,
    mut progress: impl FnMut(String),
) -> Result<OutputPaths> {
    let media = PathBuf::from(media_path);
    let model = PathBuf::from(model_path);
    let out_dir = PathBuf::from(output_dir);

    validate_media(&media)?;
    if !model.exists() {
        return Err(anyhow!("Model dosyasi bulunamadi: {}", model.display()));
    }
    fs::create_dir_all(&out_dir)?;

    let ts = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let base = out_dir.join(format!("transkript_{ts}"));
    let srt_path = base.with_extension("srt");
    let vtt_path = base.with_extension("vtt");
    let json_path = base.with_extension("json");

    let mut srt_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&srt_path)?;
    let mut vtt_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&vtt_path)?;
    let mut json_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&json_path)?;
    writeln!(vtt_file, "WEBVTT\n")?;
    writeln!(json_file, "{{\"segments\":[")?;

    progress("FFmpeg ile PCM cikariliyor...".to_string());
    let temp = TempDir::new()?;
    let wav_path = temp.path().join("full_audio.wav");
    extract_wav_with_ffmpeg(&media, &wav_path)?;

    progress("VAD + 30sn chunking yapiliyor...".to_string());
    let chunks = detect_and_chunk(&wav_path)?;
    if chunks.is_empty() {
        return Err(anyhow!("Konusma bulunamadi (VAD sonucu bos)."));
    }

    let mut srt_index = 1usize;
    let mut first_json = true;

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        progress(format!(
            "Chunk {}/{} isleniyor (offset {:.2}s)...",
            chunk_idx + 1,
            chunks.len(),
            chunk.start_sec
        ));
        let chunk_wav = temp.path().join(format!("chunk_{chunk_idx:04}.wav"));
        write_chunk_wav(&chunk_wav, &chunk.samples)?;

        let chunk_base = temp.path().join(format!("chunk_{chunk_idx:04}"));
        run_whisper_cli(&chunk_wav, &chunk_base, &model)?;
        let chunk_srt = chunk_base.with_extension("srt");
        if !chunk_srt.exists() {
            continue;
        }

        let raw = fs::read_to_string(&chunk_srt)
            .with_context(|| format!("Chunk SRT okunamadi: {}", chunk_srt.display()))?;
        let mut segments = parse_srt_segments(&raw)?;
        for seg in &mut segments {
            seg.start_sec += chunk.start_sec;
            seg.end_sec += chunk.start_sec;
        }

        // Segment bazli flush: her chunk sonrasinda dosyalara anlik yazim.
        for seg in segments {
            write_srt_segment(&mut srt_file, srt_index, &seg)?;
            write_vtt_segment(&mut vtt_file, &seg)?;
            write_json_segment(&mut json_file, &seg, &mut first_json)?;
            srt_file.flush()?;
            vtt_file.flush()?;
            json_file.flush()?;
            srt_index += 1;
        }
    }

    writeln!(json_file, "\n]}}")?;

    Ok(OutputPaths {
        srt: srt_path,
        vtt: vtt_path,
        json: json_path,
    })
}

fn validate_media(path: &Path) -> Result<()> {
    let meta = fs::metadata(path)
        .with_context(|| format!("Video dosyasi bulunamadi veya okunamiyor: {}", path.display()))?;
    if meta.len() > MAX_MEDIA_SIZE {
        return Err(anyhow!("Dosya 2GB ustu, reddedildi."));
    }

    let mut f = File::open(path)?;
    let mut header = [0u8; 12];
    f.read_exact(&mut header)?;

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "mp4" => {
            if &header[4..8] != b"ftyp" {
                return Err(anyhow!("Gecersiz MP4 container (ftyp yok)."));
            }
        }
        "webm" => {
            // WEBM, EBML tabanlidir; baslik 0x1A45DFA3 ile baslar ve "webm" doctyp'i icerir.
            if header[0..4] != [0x1A, 0x45, 0xDF, 0xA3] {
                return Err(anyhow!("Gecersiz WEBM (EBML magic yok)."));
            }
            let mut probe = vec![0u8; 4096];
            let n = f.read(&mut probe)?;
            let haystack = String::from_utf8_lossy(&probe[..n]).to_ascii_lowercase();
            if !haystack.contains("webm") {
                return Err(anyhow!("Gecersiz WEBM (doctype webm bulunamadi)."));
            }
        }
        _ => return Err(anyhow!("Desteklenmeyen dosya uzantisi. Sadece .mp4 veya .webm.")),
    }
    Ok(())
}

fn extract_wav_with_ffmpeg(input_media: &Path, output_wav: &Path) -> Result<()> {
    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            input_media
                .to_str()
                .ok_or_else(|| anyhow!("Video path UTF-8 degil"))?,
            "-map",
            "0:a:0",
            "-ar",
            "16000",
            "-ac",
            "1",
            "-f",
            "wav",
            output_wav
                .to_str()
                .ok_or_else(|| anyhow!("WAV path UTF-8 degil"))?,
        ])
        .output()
        .context("ffmpeg calistirilamadi. Sisteminizde ffmpeg kurulu mu?")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.contains("does not contain any stream")
            || stderr.contains("Stream map '0:a:0' matches no streams")
        {
            return Err(anyhow!(
                "Secilen videoda ses stream'i bulunamadi. Lutfen ses iceren bir MP4/WEBM sec."
            ));
        }
        return Err(anyhow!("ffmpeg ses cikaramadi: {stderr}"));
    }
    Ok(())
}

fn detect_and_chunk(wav_path: &Path) -> Result<Vec<SpeechChunk>> {
    let mut reader = WavReader::open(wav_path)?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != SAMPLE_RATE {
        return Err(anyhow!(
            "Beklenen WAV mono/16kHz degil: channels={}, sr={}",
            spec.channels,
            spec.sample_rate
        ));
    }

    let samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Int, 16) => reader
            .samples::<i16>()
            .filter_map(|s| s.ok())
            .map(|x| x as f32 / i16::MAX as f32)
            .collect(),
        (SampleFormat::Float, 32) => reader.samples::<f32>().filter_map(|s| s.ok()).collect(),
        _ => return Err(anyhow!("Desteklenmeyen WAV format.")),
    };

    let frame_len = SAMPLE_RATE as usize * FRAME_MS / 1000;
    let max_chunk_samples = (CHUNK_SECONDS * SAMPLE_RATE as f32) as usize;
    let mut chunks = Vec::new();

    let mut in_speech = false;
    let mut speech_start = 0usize;
    let mut silence_count = 0usize;

    for (i, frame) in samples.chunks(frame_len).enumerate() {
        if frame.is_empty() {
            continue;
        }
        let rms = (frame.iter().map(|v| v * v).sum::<f32>() / frame.len() as f32).sqrt();
        if rms >= ENERGY_THRESHOLD {
            if !in_speech {
                in_speech = true;
                speech_start = i * frame_len;
            }
            silence_count = 0;
        } else if in_speech {
            silence_count += 1;
            if silence_count >= SILENCE_HANGOVER_FRAMES {
                let speech_end = i * frame_len;
                push_chunked_segment(
                    &samples[speech_start.min(samples.len())..speech_end.min(samples.len())],
                    speech_start,
                    max_chunk_samples,
                    &mut chunks,
                );
                in_speech = false;
            }
        }
    }

    if in_speech && speech_start < samples.len() {
        push_chunked_segment(
            &samples[speech_start..],
            speech_start,
            max_chunk_samples,
            &mut chunks,
        );
    }

    Ok(chunks)
}

fn push_chunked_segment(
    segment_samples: &[f32],
    global_start_sample: usize,
    max_chunk_samples: usize,
    out: &mut Vec<SpeechChunk>,
) {
    for (idx, chunk) in segment_samples.chunks(max_chunk_samples).enumerate() {
        if chunk.len() < SAMPLE_RATE as usize / 2 {
            continue;
        }
        let chunk_offset = idx * max_chunk_samples;
        let start_sample = global_start_sample + chunk_offset;
        out.push(SpeechChunk {
            start_sec: start_sample as f32 / SAMPLE_RATE as f32,
            samples: chunk.to_vec(),
        });
    }
}

fn write_chunk_wav(path: &Path, samples: &[f32]) -> Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec)?;
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_sample(v)?;
    }
    writer.finalize()?;
    Ok(())
}

fn run_whisper_cli(chunk_wav: &Path, out_prefix: &Path, model_path: &Path) -> Result<()> {
    let status = Command::new("whisper-cli")
        .args([
            "-m",
            model_path
                .to_str()
                .ok_or_else(|| anyhow!("Model path UTF-8 degil"))?,
            "-f",
            chunk_wav
                .to_str()
                .ok_or_else(|| anyhow!("Chunk path UTF-8 degil"))?,
            "-l",
            "tr",
            "--output-srt",
            "--output-vtt",
            "--output-json",
            "--output-file",
            out_prefix
                .to_str()
                .ok_or_else(|| anyhow!("Out prefix UTF-8 degil"))?,
        ])
        .status()
        .context("whisper-cli calistirilamadi. whisper.cpp binary PATH icinde mi?")?;
    if !status.success() {
        return Err(anyhow!("whisper-cli chunk transkriptinde hata verdi."));
    }
    Ok(())
}

fn parse_srt_segments(content: &str) -> Result<Vec<TimedSegment>> {
    let mut segments = Vec::new();
    let blocks = content.split("\n\n");
    for block in blocks {
        let lines: Vec<&str> = block.lines().collect();
        if lines.len() < 3 {
            continue;
        }
        let time_line = lines[1];
        let mut parts = time_line.split("-->");
        let start = parts
            .next()
            .ok_or_else(|| anyhow!("SRT start parse edilemedi"))?
            .trim();
        let end = parts
            .next()
            .ok_or_else(|| anyhow!("SRT end parse edilemedi"))?
            .trim();
        let text = lines[2..].join(" ").trim().to_string();
        if text.is_empty() {
            continue;
        }
        segments.push(TimedSegment {
            start_sec: parse_srt_time(start)?,
            end_sec: parse_srt_time(end)?,
            text,
        });
    }
    Ok(segments)
}

fn parse_srt_time(s: &str) -> Result<f32> {
    let parts: Vec<&str> = s.split([':', ',']).collect();
    if parts.len() != 4 {
        return Err(anyhow!("SRT timestamp format gecersiz: {s}"));
    }
    let h: f32 = parts[0].parse()?;
    let m: f32 = parts[1].parse()?;
    let sec: f32 = parts[2].parse()?;
    let ms: f32 = parts[3].parse()?;
    Ok(h * 3600.0 + m * 60.0 + sec + ms / 1000.0)
}

fn format_srt_time(sec: f32) -> String {
    let total_ms = (sec.max(0.0) * 1000.0).round() as u64;
    let h = total_ms / 3_600_000;
    let m = (total_ms % 3_600_000) / 60_000;
    let s = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{h:02}:{m:02}:{s:02},{ms:03}")
}

fn format_vtt_time(sec: f32) -> String {
    let total_ms = (sec.max(0.0) * 1000.0).round() as u64;
    let h = total_ms / 3_600_000;
    let m = (total_ms % 3_600_000) / 60_000;
    let s = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}

fn write_srt_segment(file: &mut File, idx: usize, segment: &TimedSegment) -> Result<()> {
    writeln!(file, "{idx}")?;
    writeln!(
        file,
        "{} --> {}",
        format_srt_time(segment.start_sec),
        format_srt_time(segment.end_sec)
    )?;
    writeln!(file, "{}\n", segment.text)?;
    Ok(())
}

fn write_vtt_segment(file: &mut File, segment: &TimedSegment) -> Result<()> {
    writeln!(
        file,
        "{} --> {}",
        format_vtt_time(segment.start_sec),
        format_vtt_time(segment.end_sec)
    )?;
    writeln!(file, "{}\n", segment.text)?;
    Ok(())
}

fn write_json_segment(file: &mut File, segment: &TimedSegment, first_json: &mut bool) -> Result<()> {
    let obj = JsonSegment {
        start: segment.start_sec,
        end: segment.end_sec,
        text: segment.text.clone(),
    };
    if !*first_json {
        writeln!(file, ",")?;
    }
    serde_json::to_writer_pretty(file, &obj)?;
    *first_json = false;
    Ok(())
}
