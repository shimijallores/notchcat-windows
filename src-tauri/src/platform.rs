use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

use base64::Engine;
use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

use crate::store::now_ms;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// ---------------------------------------------------------------------------
// Battery
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatteryStatus {
    pub percent: Option<u8>,
    pub charging: bool,
}

pub fn read_battery_status() -> BatteryStatus {
    let mut sps = SYSTEM_POWER_STATUS::default();
    let ok = unsafe { GetSystemPowerStatus(&mut sps) };
    if ok.is_err() || sps.BatteryFlag == 128 || sps.BatteryFlag == 255 || sps.BatteryLifePercent > 100 {
        return BatteryStatus::default();
    }
    let charging = sps.ACLineStatus == 1 || (sps.BatteryFlag & 8) != 0;
    BatteryStatus {
        percent: Some(sps.BatteryLifePercent),
        charging,
    }
}

// ---------------------------------------------------------------------------
// Sound Effects (in-memory gentle PCM chimes via WinMM PlaySoundW)
// ---------------------------------------------------------------------------

#[cfg(windows)]
#[link(name = "winmm")]
extern "system" {
    fn PlaySoundW(psz_sound: *const u16, hmod: isize, fdw_sound: u32) -> i32;
}

pub fn play_timer_sound(name: &str) {
    if name.eq_ignore_ascii_case("none") || name.trim().is_empty() {
        return;
    }
    let freqs: &'static [f64] = match name {
        "Hero" => &[523.25, 659.25, 783.99],
        "Ping" => &[1046.5],
        "Pop" => &[698.46, 880.0],
        "Purr" => &[392.0, 493.88],
        "Submarine" => &[440.0, 554.37],
        "Tink" => &[1318.51],
        _ => &[880.0, 1174.66], // Glass default
    };
    let wav = build_chime_wav(freqs);
    std::thread::spawn(move || {
        #[cfg(windows)]
        unsafe {
            // SND_MEMORY = 0x0004, SND_SYNC = 0x0000, SND_NODEFAULT = 0x0002
            PlaySoundW(wav.as_ptr() as *const u16, 0, 0x0004 | 0x0002);
        }
    });
}

fn build_chime_wav(freqs: &[f64]) -> Vec<u8> {
    let sample_rate = 22_050u32;
    let note_samples = (sample_rate as f64 * 0.16) as usize;
    let tail_samples = (sample_rate as f64 * 0.25) as usize;
    let total_samples = freqs.len() * note_samples + tail_samples;
    let mut pcm: Vec<i16> = vec![0; total_samples];

    for (idx, &freq) in freqs.iter().enumerate() {
        let start = idx * note_samples;
        let dur_samples = note_samples + tail_samples;
        for i in 0..dur_samples {
            if start + i >= total_samples {
                break;
            }
            let t = i as f64 / sample_rate as f64;
            let attack = (t / 0.008).min(1.0);
            let decay = (-t * 7.5).exp();
            let amp = 0.22 * attack * decay;
            let sample = (2.0 * std::f64::consts::PI * freq * t).sin()
                + 0.25 * (2.0 * std::f64::consts::PI * (freq * 2.0) * t).sin();
            let mixed = pcm[start + i] as f64 + sample * amp * 32767.0;
            pcm[start + i] = mixed.clamp(-32767.0, 32767.0) as i16;
        }
    }

    let data_bytes = (pcm.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_bytes as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());
    for s in pcm {
        wav.extend_from_slice(&s.to_le_bytes());
    }
    wav
}

// ---------------------------------------------------------------------------
// Launch at Login & Shell Open Helpers
// ---------------------------------------------------------------------------

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const STARTUP_APPROVED_KEY: &str =
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const RUN_VALUE: &str = "NotchCat";
const STARTUP_ENABLED_HEX: &str = "020000000000000000000000";
const STARTUP_DISABLED_HEX: &str = "030000000000000000000000";

fn reg_command(args: &[&str]) -> std::io::Result<std::process::Output> {
    let mut cmd = Command::new("reg");
    cmd.args(args);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.output()
}

fn parse_startup_approved_enabled(reg_output: &str) -> Option<bool> {
    for line in reg_output.lines() {
        let trimmed = line.trim();
        if let Some(pos) = trimmed.find("REG_BINARY") {
            let hex = trimmed[pos + "REG_BINARY".len()..].trim();
            if hex.len() >= 2 {
                if let Ok(first_byte) = u8::from_str_radix(&hex[..2], 16) {
                    // Windows StartupApproved uses an even leading byte (0x02/0x06) for enabled
                    // and an odd leading byte (0x03/0x07) for disabled.
                    return Some(first_byte % 2 == 0);
                }
            }
        }
    }
    None
}

fn write_run_entry() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let quoted = format!("\"{}\"", exe.display());
    let out = reg_command(&[
        "add",
        RUN_KEY,
        "/v",
        RUN_VALUE,
        "/t",
        "REG_SZ",
        "/d",
        &quoted,
        "/f",
    ])
    .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}

fn write_startup_approved(enabled: bool) -> Result<(), String> {
    let hex = if enabled {
        STARTUP_ENABLED_HEX
    } else {
        STARTUP_DISABLED_HEX
    };
    let out = reg_command(&[
        "add",
        STARTUP_APPROVED_KEY,
        "/v",
        RUN_VALUE,
        "/t",
        "REG_BINARY",
        "/d",
        hex,
        "/f",
    ])
    .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}

/// Ensures NotchCat is registered under HKCU\...\Run so it appears in Windows Settings → Apps →
/// Startup and Task Manager → Startup apps, preserving the user's enabled/disabled toggle if
/// already present and defaulting to disabled on first registration.
pub fn ensure_startup_registered() {
    let has_run = reg_command(&["query", RUN_KEY, "/v", RUN_VALUE])
        .map(|o| o.status.success())
        .unwrap_or(false);
    let _ = write_run_entry();
    if !has_run {
        let has_approved = reg_command(&["query", STARTUP_APPROVED_KEY, "/v", RUN_VALUE])
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !has_approved {
            let _ = write_startup_approved(false);
        }
    }
}

pub fn is_launch_at_login() -> bool {
    let has_run = reg_command(&["query", RUN_KEY, "/v", RUN_VALUE])
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !has_run {
        return false;
    }
    match reg_command(&["query", STARTUP_APPROVED_KEY, "/v", RUN_VALUE]) {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            parse_startup_approved_enabled(&stdout).unwrap_or(true)
        }
        _ => true,
    }
}

pub fn set_launch_at_login(enabled: bool) -> Result<(), String> {
    write_run_entry()?;
    write_startup_approved(enabled)?;
    Ok(())
}

pub fn open_external_url(url: &str) {
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "start", "", url]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let _ = cmd.spawn();
}

pub fn reveal_folder(path: &Path) {
    let mut cmd = Command::new("explorer");
    cmd.arg(path);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let _ = cmd.spawn();
}

// ---------------------------------------------------------------------------
// Music Service (Notification parser + SMTC Windows Media Controls + Pastel Tint)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MusicPlayer {
    Spotify,
    AppleMusic,
    SystemMedia,
}

impl MusicPlayer {
    pub fn display_name(&self) -> &'static str {
        match self {
            MusicPlayer::Spotify => "Spotify",
            MusicPlayer::AppleMusic => "Apple Music",
            MusicPlayer::SystemMedia => "Media",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub elapsed: f64,
    pub updated_at_ms: i64,
    pub playing: bool,
    pub player: MusicPlayer,
    pub artwork_data_url: Option<String>,
    pub pastel_tint: Option<[u8; 3]>,
}

impl NowPlayingTrack {
    pub fn position_at(&self, now_ms: i64) -> f64 {
        if !self.playing {
            return self.elapsed.clamp(0.0, self.duration.max(0.0));
        }
        let dt = ((now_ms - self.updated_at_ms).max(0) as f64) / 1000.0;
        let pos = self.elapsed + dt;
        if self.duration > 0.0 {
            pos.clamp(0.0, self.duration)
        } else {
            pos.max(0.0)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicSnapshot {
    pub track: Option<NowPlayingTrack>,
    pub effective_playing_source: Option<MusicPlayer>,
    pub has_session: bool,
}

pub struct MusicService {
    pub wanted: bool,
    pub fake_mode: bool,
    pub track: Option<NowPlayingTrack>,
    pub effective_playing_source: Option<MusicPlayer>,
    pub has_smtc_session: bool,
    art_cache_key: Option<String>,
    art_cache_url: Option<String>,
    art_cache_tint: Option<[u8; 3]>,
    art_miss_count: u8,
    art_fallback_attempted_key: Option<String>,
}

impl MusicService {
    pub fn new() -> Self {
        Self {
            wanted: false,
            fake_mode: false,
            track: None,
            effective_playing_source: None,
            has_smtc_session: false,
            art_cache_key: None,
            art_cache_url: None,
            art_cache_tint: None,
            art_miss_count: 0,
            art_fallback_attempted_key: None,
        }
    }

    pub fn snapshot(&self, now_ms: i64) -> MusicSnapshot {
        let mut track = self.track.clone();
        if let Some(t) = track.as_mut() {
            t.elapsed = t.position_at(now_ms);
            t.updated_at_ms = now_ms;
        }
        MusicSnapshot {
            track,
            effective_playing_source: self.effective_playing_source,
            has_session: self.has_smtc_session || self.track.is_some(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.track.as_ref().map(|t| t.playing).unwrap_or(false)
    }

    pub fn seed_fake_music(&mut self, playing: bool, now_ms: i64) {
        self.fake_mode = true;
        self.track = Some(NowPlayingTrack {
            id: "fake:lofi-study".to_string(),
            title: "Midnight Tea".to_string(),
            artist: "NotchCat Lo-Fi".to_string(),
            album: "Purr Beats".to_string(),
            duration: 185.0,
            elapsed: 42.0,
            updated_at_ms: now_ms,
            playing,
            player: MusicPlayer::Spotify,
            artwork_data_url: None,
            pastel_tint: Some([228, 222, 240]),
        });
        self.effective_playing_source = if playing {
            Some(MusicPlayer::Spotify)
        } else {
            None
        };
    }

    /// Parses Spotify / Apple Music playback notifications identically to macOS `MusicService.swift`.
    pub fn handle_notification(
        &mut self,
        sender: &str,
        name: Option<&str>,
        info: &serde_json::Map<String, serde_json::Value>,
        now_ms: i64,
    ) {
        let player = if sender.contains("spotify") || name.unwrap_or("").contains("spotify") {
            MusicPlayer::Spotify
        } else if sender.contains("Music") || name.unwrap_or("").contains("Music") {
            MusicPlayer::AppleMusic
        } else {
            MusicPlayer::SystemMedia
        };

        let state_str = info
            .get("Player State")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if state_str == "Stopped" {
            if self.track.as_ref().map(|t| t.player) == Some(player) {
                self.track = None;
                self.effective_playing_source = None;
            }
            return;
        }

        let playing = state_str == "Playing";

        // Do not let a paused non-active player clobber an actively playing player.
        if !playing {
            if let Some(active_source) = self.effective_playing_source {
                if active_source != player {
                    return;
                }
            }
        }

        let title = info
            .get("Name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if title.is_empty() {
            return;
        }
        let artist = info
            .get("Artist")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let album = info
            .get("Album")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let track_id = info
            .get("Track ID")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}:{}:{}", player.display_name(), title, artist));

        let raw_duration = info
            .get("Duration")
            .or_else(|| info.get("Total Time"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        // Apple Music reports Total Time in milliseconds when large, Spotify reports Duration in seconds
        let duration = if player == MusicPlayer::AppleMusic && raw_duration > 1000.0 {
            raw_duration / 1000.0
        } else if player == MusicPlayer::Spotify
            && info.get("Duration").is_none()
            && info.get("Total Time").is_some()
            && raw_duration > 1000.0
        {
            raw_duration / 1000.0
        } else {
            raw_duration
        };

        let elapsed = info
            .get("Playback Position")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        self.effective_playing_source = if playing { Some(player) } else { None };
        let prev_art = if self
            .track
            .as_ref()
            .map(|t| t.id == track_id)
            .unwrap_or(false)
        {
            self.track
                .as_ref()
                .map(|t| (t.artwork_data_url.clone(), t.pastel_tint))
                .unwrap_or((None, None))
        } else {
            (None, None)
        };

        self.track = Some(NowPlayingTrack {
            id: track_id,
            title,
            artist,
            album,
            duration,
            elapsed,
            updated_at_ms: now_ms,
            playing,
            player,
            artwork_data_url: prev_art.0,
            pastel_tint: prev_art.1,
        });
    }

    pub fn update_from_smtc(&mut self, poll: SmtcPollResult, now_ms: i64) {
        if self.fake_mode {
            return;
        }
        self.has_smtc_session = poll.has_session;
        let Some(item) = poll.track else {
            if !self.has_smtc_session {
                self.track = None;
                self.effective_playing_source = None;
            }
            return;
        };

        let key = format!("{}:{}:{}", item.player.display_name(), item.title, item.artist);
        if self.art_cache_key.as_deref() != Some(&key) {
            self.art_cache_key = Some(key.clone());
            self.art_cache_url = None;
            self.art_cache_tint = None;
            self.art_miss_count = 0;
        }
        if self.art_cache_url.is_none() {
            if let Some(bytes) = item.artwork_bytes.as_deref() {
                let (url, tint) = process_artwork_bytes(bytes);
                if url.is_some() {
                    self.art_cache_url = url;
                    self.art_cache_tint = tint;
                } else {
                    self.art_miss_count = self.art_miss_count.saturating_add(1);
                }
            } else {
                self.art_miss_count = self.art_miss_count.saturating_add(1);
            }
        }

        self.effective_playing_source = if item.playing {
            Some(item.player)
        } else {
            None
        };
        self.track = Some(NowPlayingTrack {
            id: key,
            title: item.title,
            artist: item.artist,
            album: item.album,
            duration: item.duration,
            elapsed: item.elapsed,
            updated_at_ms: now_ms,
            playing: item.playing,
            player: item.player,
            artwork_data_url: self.art_cache_url.clone(),
            pastel_tint: self.art_cache_tint,
        });
    }

    pub fn toggle_fake_play_pause(&mut self, now_ms: i64) -> bool {
        if !self.fake_mode {
            return false;
        }
        if let Some(t) = self.track.as_mut() {
            t.elapsed = t.position_at(now_ms);
            t.updated_at_ms = now_ms;
            t.playing = !t.playing;
            self.effective_playing_source = if t.playing { Some(t.player) } else { None };
        }
        true
    }
}

pub fn process_artwork_bytes(bytes: &[u8]) -> (Option<String>, Option<[u8; 3]>) {
    let Ok(img) = image::load_from_memory(bytes) else {
        return (None, None);
    };
    let thumb = img.thumbnail(144, 144);
    let sample = thumb.thumbnail(16, 16).to_rgb8();
    let mut sum_r = 0u64;
    let mut sum_g = 0u64;
    let mut sum_b = 0u64;
    let mut count = 0u64;
    for p in sample.pixels() {
        sum_r += p[0] as u64;
        sum_g += p[1] as u64;
        sum_b += p[2] as u64;
        count += 1;
    }
    let tint = if count > 0 {
        let r = (sum_r / count) as f64;
        let g = (sum_g / count) as f64;
        let b = (sum_b / count) as f64;
        // Same 0.58 mix toward white as Mac MusicService.pastelTint
        let w = 255.0;
        let k = 0.58;
        Some([
            (r * (1.0 - k) + w * k).round().clamp(0.0, 255.0) as u8,
            (g * (1.0 - k) + w * k).round().clamp(0.0, 255.0) as u8,
            (b * (1.0 - k) + w * k).round().clamp(0.0, 255.0) as u8,
        ])
    } else {
        None
    };

    let mut png_buf = Vec::new();
    let url = if thumb
        .write_to(&mut Cursor::new(&mut png_buf), image::ImageFormat::Png)
        .is_ok()
    {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_buf);
        Some(format!("data:image/png;base64,{b64}"))
    } else {
        None
    };
    (url, tint)
}

pub struct SmtcTrackData {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub elapsed: f64,
    pub playing: bool,
    pub player: MusicPlayer,
    pub artwork_bytes: Option<Vec<u8>>,
}

pub struct SmtcPollResult {
    pub has_session: bool,
    pub track: Option<SmtcTrackData>,
}

pub fn poll_windows_smtc(last_art_key: Option<&str>) -> SmtcPollResult {
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus,
    };

    let Ok(op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() else {
        return SmtcPollResult {
            has_session: false,
            track: None,
        };
    };
    let Ok(mgr) = op.get() else {
        return SmtcPollResult {
            has_session: false,
            track: None,
        };
    };
    let Ok(session) = mgr.GetCurrentSession() else {
        return SmtcPollResult {
            has_session: false,
            track: None,
        };
    };

    let source_id = session
        .SourceAppUserModelId()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let player = if source_id.contains("spotify") {
        MusicPlayer::Spotify
    } else if source_id.contains("apple") || source_id.contains("itunes") {
        MusicPlayer::AppleMusic
    } else {
        MusicPlayer::SystemMedia
    };

    let playing = session
        .GetPlaybackInfo()
        .and_then(|i| i.PlaybackStatus())
        .map(|s| s == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
        .unwrap_or(false);

    let (elapsed, duration) = session
        .GetTimelineProperties()
        .map(|tl| {
            let pos = tl.Position().map(|d| d.Duration as f64 / 10_000_000.0).unwrap_or(0.0);
            let end = tl.EndTime().map(|d| d.Duration as f64 / 10_000_000.0).unwrap_or(0.0);
            (pos.max(0.0), end.max(0.0))
        })
        .unwrap_or((0.0, 0.0));

    let Ok(media_op) = session.TryGetMediaPropertiesAsync() else {
        return SmtcPollResult {
            has_session: true,
            track: None,
        };
    };
    let Ok(props) = media_op.get() else {
        return SmtcPollResult {
            has_session: true,
            track: None,
        };
    };

    let title = props
        .Title()
        .map(|s| s.to_string_lossy().trim().to_string())
        .unwrap_or_default();
    if title.is_empty() {
        return SmtcPollResult {
            has_session: true,
            track: None,
        };
    }
    let artist = props
        .Artist()
        .map(|s| s.to_string_lossy().trim().to_string())
        .unwrap_or_default();
    let album = props
        .AlbumTitle()
        .map(|s| s.to_string_lossy().trim().to_string())
        .unwrap_or_default();

    let new_key = format!("{}:{}:{}", player.display_name(), title, artist);
    let artwork_bytes = if last_art_key != Some(new_key.as_str()) {
        read_smtc_thumbnail_bytes(&props)
    } else {
        None
    };

    SmtcPollResult {
        has_session: true,
        track: Some(SmtcTrackData {
            title,
            artist,
            album,
            duration,
            elapsed,
            playing,
            player,
            artwork_bytes,
        }),
    }
}

fn read_smtc_thumbnail_bytes(
    props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
) -> Option<Vec<u8>> {
    use windows::core::Interface;
    use windows::Storage::Streams::{Buffer, DataReader, IInputStream, InputStreamOptions};

    let thumb_ref = props.Thumbnail().ok()?;
    let stream = thumb_ref.OpenReadAsync().ok()?.get().ok()?;
    let raw_size = stream.Size().ok()?;
    if raw_size == 0 || raw_size > 8 * 1024 * 1024 {
        return None;
    }
    let size = raw_size as u32;

    if let Ok(input) = Interface::cast::<IInputStream>(&stream) {
        if let Ok(reader) = DataReader::CreateDataReader(&input) {
            if let Some(loaded) = reader
                .LoadAsync(size)
                .ok()
                .and_then(|op| op.get().ok())
                .filter(|&n| n > 0)
            {
                let mut out = vec![0u8; loaded as usize];
                if reader.ReadBytes(&mut out).is_ok() {
                    let _ = reader.DetachStream();
                    return Some(out);
                }
            }
            let _ = reader.DetachStream();
        }
    }

    let _ = stream.Seek(0);
    let buf = Buffer::Create(size).ok()?;
    let read_buf = stream
        .ReadAsync(&buf, size, InputStreamOptions::None)
        .ok()?
        .get()
        .ok()?;
    let len = read_buf.Length().ok()? as usize;
    if len == 0 {
        return None;
    }
    let data = DataReader::FromBuffer(&read_buf).ok()?;
    let mut out = vec![0u8; len];
    data.ReadBytes(&mut out).ok()?;
    Some(out)
}

fn fetch_itunes_artwork_bytes(title: &str, artist: &str) -> Option<Vec<u8>> {
    let title = title.trim();
    if title.is_empty() {
        return None;
    }
    let term = if artist.trim().is_empty() {
        title.to_string()
    } else {
        format!("{} {}", artist.trim(), title)
    };

    let fetch = async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .user_agent("NotchCat-Windows")
            .build()
            .ok()?;
        let resp = client
            .get("https://itunes.apple.com/search")
            .query(&[
                ("media", "music"),
                ("entity", "song"),
                ("limit", "1"),
                ("term", term.as_str()),
            ])
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body: serde_json::Value = resp.json().await.ok()?;
        let art_url = body
            .get("results")?
            .as_array()?
            .first()?
            .get("artworkUrl100")?
            .as_str()?
            .replace("100x100bb", "300x300bb");
        let img_resp = client.get(&art_url).send().await.ok()?;
        if !img_resp.status().is_success() {
            return None;
        }
        let bytes = img_resp.bytes().await.ok()?;
        if bytes.is_empty() || bytes.len() > 4 * 1024 * 1024 {
            return None;
        }
        Some(bytes.to_vec())
    };

    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.block_on(fetch)
    } else {
        tokio::runtime::Runtime::new().ok()?.block_on(fetch)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MediaControlAction {
    PlayPause,
    Next,
    Previous,
}

pub fn send_smtc_control(action: MediaControlAction) -> bool {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;
    let Ok(mgr) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .and_then(|op| op.get())
    else {
        return false;
    };
    let Ok(session) = mgr.GetCurrentSession() else {
        return false;
    };
    match action {
        MediaControlAction::PlayPause => session
            .TryTogglePlayPauseAsync()
            .and_then(|op| op.get())
            .unwrap_or(false),
        MediaControlAction::Next => session
            .TrySkipNextAsync()
            .and_then(|op| op.get())
            .unwrap_or(false),
        MediaControlAction::Previous => session
            .TrySkipPreviousAsync()
            .and_then(|op| op.get())
            .unwrap_or(false),
    }
}

pub fn launch_spotify_app() {
    open_external_url("spotify:");
}

// ---------------------------------------------------------------------------
// Calendar Store (ICS Subscription Parser)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum CalendarAccess {
    NotDetermined,
    Authorized,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub is_all_day: bool,
    pub color: [u8; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarSnapshot {
    pub access: CalendarAccess,
    pub events: Vec<CalendarEvent>,
}

pub struct CalendarStore {
    pub access: CalendarAccess,
    pub events: Vec<CalendarEvent>,
}

impl CalendarStore {
    pub fn new() -> Self {
        Self {
            access: CalendarAccess::NotDetermined,
            events: Vec::new(),
        }
    }

    pub fn snapshot(&self) -> CalendarSnapshot {
        CalendarSnapshot {
            access: self.access.clone(),
            events: self.events.clone(),
        }
    }
}

pub fn parse_ics_events_for_range(
    ics_text: &str,
    range_start_ms: i64,
    range_end_ms: i64,
) -> Vec<CalendarEvent> {
    // Unfold RFC 5545 folded lines
    let normalized = ics_text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<String> = Vec::new();
    for raw in normalized.split('\n') {
        if raw.starts_with(' ') || raw.starts_with('\t') {
            if let Some(last) = lines.last_mut() {
                last.push_str(&raw[1..]);
            }
        } else {
            lines.push(raw.to_string());
        }
    }

    let palette: [[u8; 3]; 4] = [
        [124, 152, 232],
        [104, 184, 148],
        [232, 148, 108],
        [176, 132, 224],
    ];

    let mut out = Vec::new();
    let mut in_event = false;
    let mut props: HashMap<String, (String, String)> = HashMap::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "BEGIN:VEVENT" {
            in_event = true;
            props.clear();
            continue;
        }
        if trimmed == "END:VEVENT" {
            if in_event {
                if let Some(ev) = build_ics_event(&props, range_start_ms, range_end_ms, &palette) {
                    out.push(ev);
                }
            }
            in_event = false;
            props.clear();
            continue;
        }
        if !in_event {
            continue;
        }
        if let Some((left, val)) = trimmed.split_once(':') {
            let (name, params) = match left.split_once(';') {
                Some((n, p)) => (n.to_uppercase(), p.to_uppercase()),
                None => (left.to_uppercase(), String::new()),
            };
            props.insert(name, (params, val.to_string()));
        }
    }

    out.sort_by(|a, b| {
        b.is_all_day
            .cmp(&a.is_all_day)
            .then_with(|| a.start_ms.cmp(&b.start_ms))
    });
    out
}

fn build_ics_event(
    props: &HashMap<String, (String, String)>,
    range_start_ms: i64,
    range_end_ms: i64,
    palette: &[[u8; 3]; 4],
) -> Option<CalendarEvent> {
    if props
        .get("STATUS")
        .map(|(_, v)| v.eq_ignore_ascii_case("CANCELLED"))
        .unwrap_or(false)
    {
        return None;
    }
    let (start_params, start_val) = props.get("DTSTART")?;
    let (start_ms, is_all_day) = parse_ics_datetime(start_params, start_val)?;
    let end_ms = props
        .get("DTEND")
        .and_then(|(p, v)| parse_ics_datetime(p, v).map(|(ms, _)| ms))
        .unwrap_or_else(|| {
            if is_all_day {
                start_ms + 86_400_000
            } else {
                start_ms + 3_600_000
            }
        });

    if end_ms <= range_start_ms || start_ms >= range_end_ms {
        return None;
    }

    let raw_title = props
        .get("SUMMARY")
        .map(|(_, v)| unescape_ics_text(v))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Untitled event".to_string());
    let uid = props
        .get("UID")
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| format!("{start_ms}:{raw_title}"));

    let hash = uid
        .bytes()
        .fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize));
    let color = palette[hash % palette.len()];

    Some(CalendarEvent {
        id: uid,
        title: raw_title,
        start_ms,
        end_ms,
        is_all_day,
        color,
    })
}

fn unescape_ics_text(s: &str) -> String {
    s.replace("\\n", " ")
        .replace("\\N", " ")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
        .trim()
        .to_string()
}

fn parse_ics_datetime(params: &str, value: &str) -> Option<(i64, bool)> {
    let v = value.trim();
    if params.contains("VALUE=DATE") || (v.len() == 8 && v.chars().all(|c| c.is_ascii_digit())) {
        let d = NaiveDate::parse_from_str(&v[..8], "%Y%m%d").ok()?;
        let dt = d.and_time(NaiveTime::MIN);
        let local_dt = Local.from_local_datetime(&dt).earliest()?;
        return Some((local_dt.timestamp_millis(), true));
    }
    if v.ends_with('Z') && v.len() >= 16 {
        let ndt = NaiveDateTime::parse_from_str(v.trim_end_matches('Z'), "%Y%m%dT%H%M%S").ok()?;
        return Some((Utc.from_utc_datetime(&ndt).timestamp_millis(), false));
    }
    if v.len() >= 15 {
        let ndt = NaiveDateTime::parse_from_str(&v[..15], "%Y%m%dT%H%M%S").ok()?;
        let local_dt = Local.from_local_datetime(&ndt).earliest()?;
        return Some((local_dt.timestamp_millis(), false));
    }
    None
}

pub async fn fetch_ics_text(url: &str) -> Result<String, String> {
    let trimmed = url.trim();
    if let Some(path_str) = trimmed.strip_prefix("file://") {
        let clean = path_str.trim_start_matches('/');
        return std::fs::read_to_string(clean).map_err(|e| e.to_string());
    }
    let http_url = if let Some(rest) = trimmed.strip_prefix("webcal://") {
        format!("https://{rest}")
    } else {
        trimmed.to_string()
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&http_url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Calendar feed returned HTTP {}", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Update Checker (GitHub Releases + NOTCHCAT_UPDATE_URL override)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub version: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

pub fn current_app_version() -> String {
    std::env::var("NOTCHCAT_VERSION")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

pub fn is_newer_version(candidate: &str, current: &str) -> bool {
    let parse_parts = |s: &str| -> Vec<u64> {
        s.trim()
            .trim_start_matches(['v', 'V'])
            .split(['-', '+'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let a = parse_parts(candidate);
    let b = parse_parts(current);
    let len = a.len().max(b.len());
    for i in 0..len {
        let av = *a.get(i).unwrap_or(&0);
        let bv = *b.get(i).unwrap_or(&0);
        if av != bv {
            return av > bv;
        }
    }
    false
}

pub fn release_from_bytes(bytes: &[u8]) -> Option<ReleaseInfo> {
    let gh: GhRelease = serde_json::from_slice(bytes).ok()?;
    let version = gh.tag_name.trim().trim_start_matches(['v', 'V']).to_string();
    if version.is_empty() {
        return None;
    }
    let asset_url = gh
        .assets
        .iter()
        .find(|a| a.name.ends_with(".exe") || a.name.ends_with(".msi"))
        .or_else(|| {
            gh.assets
                .iter()
                .find(|a| a.name.ends_with(".dmg") || a.name.ends_with(".zip"))
        })
        .map(|a| a.browser_download_url.clone())
        .unwrap_or(gh.html_url);

    Some(ReleaseInfo {
        version,
        url: asset_url,
    })
}

pub struct UpdateChecker {
    pub available: Option<ReleaseInfo>,
    pub last_checked_ms: i64,
    pub status_message: Option<String>,
}

impl UpdateChecker {
    pub fn new() -> Self {
        Self {
            available: None,
            last_checked_ms: 0,
            status_message: None,
        }
    }
}

pub async fn fetch_latest_release() -> Result<Option<ReleaseInfo>, String> {
    let url = std::env::var("NOTCHCAT_UPDATE_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            "https://api.github.com/repos/shimijallores/notchcat-windows/releases/latest".to_string()
        });

    let bytes = if let Some(path_str) = url.strip_prefix("file://") {
        let clean = path_str.trim_start_matches('/');
        std::fs::read(clean).map_err(|e| e.to_string())?
    } else if Path::new(&url).exists() {
        std::fs::read(&url).map_err(|e| e.to_string())?
    } else {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("NotchCat-Windows")
            .build()
            .map_err(|e| e.to_string())?;
        let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("Update check returned HTTP {}", resp.status()));
        }
        resp.bytes().await.map_err(|e| e.to_string())?.to_vec()
    };

    let Some(rel) = release_from_bytes(&bytes) else {
        return Ok(None);
    };
    let current = current_app_version();
    if is_newer_version(&rel.version, &current) {
        Ok(Some(rel))
    } else {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Shared Platform State Bundle
// ---------------------------------------------------------------------------

pub struct PlatformState {
    pub battery: Mutex<BatteryStatus>,
    pub music: Mutex<MusicService>,
    pub calendar: Mutex<CalendarStore>,
    pub updater: Mutex<UpdateChecker>,
}

impl PlatformState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            battery: Mutex::new(read_battery_status()),
            music: Mutex::new(MusicService::new()),
            calendar: Mutex::new(CalendarStore::new()),
            updater: Mutex::new(UpdateChecker::new()),
        })
    }
}

pub fn poll_music_now(platform: &PlatformState) {
    let last_key = {
        let m = platform.music.lock().unwrap();
        if m.fake_mode {
            return;
        }
        m.art_cache_key.clone().filter(|_| m.art_cache_url.is_some())
    };
    let poll = poll_windows_smtc(last_key.as_deref());
    let fallback_query = {
        let mut m = platform.music.lock().unwrap();
        m.update_from_smtc(poll, now_ms());
        if m.art_cache_url.is_none() && m.art_miss_count >= 2 {
            let track_info = m
                .track
                .as_ref()
                .map(|t| (t.title.clone(), t.artist.clone()));
            if let (Some(key), Some((title, artist))) = (m.art_cache_key.clone(), track_info) {
                if m.art_fallback_attempted_key.as_deref() != Some(key.as_str()) {
                    m.art_fallback_attempted_key = Some(key.clone());
                    Some((key, title, artist))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };

    if let Some((key, title, artist)) = fallback_query {
        if let Some(bytes) = fetch_itunes_artwork_bytes(&title, &artist) {
            let (url, tint) = process_artwork_bytes(&bytes);
            if url.is_some() {
                let mut m = platform.music.lock().unwrap();
                if m.art_cache_key.as_deref() == Some(key.as_str()) && m.art_cache_url.is_none() {
                    m.art_cache_url = url.clone();
                    m.art_cache_tint = tint;
                    if let Some(t) = m.track.as_mut() {
                        t.artwork_data_url = url;
                        t.pastel_tint = tint;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_approved_binary_parser_reads_enabled_and_disabled() {
        let enabled_out = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run\r\n    NotchCat    REG_BINARY    020000000000000000000000\r\n";
        let disabled_out = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run\r\n    NotchCat    REG_BINARY    03000000A1B2C3D401000000\r\n";
        assert_eq!(parse_startup_approved_enabled(enabled_out), Some(true));
        assert_eq!(parse_startup_approved_enabled(disabled_out), Some(false));
        assert_eq!(parse_startup_approved_enabled("no match"), None);
    }

    #[test]
    fn update_version_comparison_and_json_parse() {
        assert!(is_newer_version("v1.0.7", "1.0.6"));
        assert!(!is_newer_version("1.0.6", "1.0.6"));
        assert!(!is_newer_version("1.0.5", "1.0.6"));

        let payload = br#"{
            "tag_name": "v1.2.0",
            "html_url": "https://github.com/test/releases/v1.2.0",
            "assets": [
                {"name": "NotchCat-1.2.0-setup.exe", "browser_download_url": "https://cdn.example/setup.exe"}
            ]
        }"#;
        let rel = release_from_bytes(payload).unwrap();
        assert_eq!(rel.version, "1.2.0");
        assert_eq!(rel.url, "https://cdn.example/setup.exe");
    }

    #[test]
    fn music_notification_arbitration_and_tint() {
        let mut music = MusicService::new();
        let mut sp = serde_json::Map::new();
        sp.insert("Player State".into(), "Playing".into());
        sp.insert("Name".into(), "Night Owl".into());
        sp.insert("Artist".into(), "LoFi Cat".into());
        sp.insert("Duration".into(), serde_json::json!(180.0));
        sp.insert("Playback Position".into(), serde_json::json!(20.0));

        music.handle_notification("com.spotify.client.PlaybackStateChanged", None, &sp, 1000);
        assert_eq!(music.effective_playing_source, Some(MusicPlayer::Spotify));
        assert_eq!(music.track.as_ref().unwrap().position_at(6000), 25.0);

        // Paused Apple Music shouldn't clobber playing Spotify
        let mut am = serde_json::Map::new();
        am.insert("Player State".into(), "Paused".into());
        am.insert("Name".into(), "Old Track".into());
        am.insert("Total Time".into(), serde_json::json!(200_000.0));
        music.handle_notification("com.apple.Music.playerInfo", None, &am, 7000);
        assert_eq!(music.track.as_ref().unwrap().title, "Night Owl");
    }

    #[test]
    fn ics_parser_extracts_todays_timed_and_all_day_events() {
        let today = Local::now().date_naive();
        let ymd = today.format("%Y%m%d").to_string();
        let ics = format!(
            "BEGIN:VCALENDAR\r\n\
             BEGIN:VEVENT\r\n\
             UID:ev-1\r\n\
             DTSTART;VALUE=DATE:{ymd}\r\n\
             SUMMARY:All Day Cat\r\n\
             END:VEVENT\r\n\
             BEGIN:VEVENT\r\n\
             UID:ev-2\r\n\
             DTSTART:{ymd}T140000\r\n\
             DTEND:{ymd}T150000\r\n\
             SUMMARY:Standup\\, Team\r\n\
             END:VEVENT\r\n\
             END:VCALENDAR\r\n"
        );
        let start_s = crate::store::today_start();
        let end_s = crate::store::add_local_days(start_s, 1);
        let events = parse_ics_events_for_range(&ics, (start_s * 1000.0) as i64, (end_s * 1000.0) as i64);
        assert_eq!(events.len(), 2);
        assert!(events[0].is_all_day);
        assert_eq!(events[0].title, "All Day Cat");
        assert_eq!(events[1].title, "Standup, Team");
    }

    #[test]
    fn smtc_poll_reads_artwork_when_session_active() {
        let poll = poll_windows_smtc(None);
        if let Some(track) = poll.track {
            println!(
                "SMTC session active: player={:?}, title='{}', artist='{}', artwork_bytes={:?}",
                track.player,
                track.title,
                track.artist,
                track.artwork_bytes.as_ref().map(|b| b.len())
            );
            if track.player == MusicPlayer::Spotify {
                let bytes = track
                    .artwork_bytes
                    .expect("expected live Spotify SMTC session to yield artwork bytes");
                let (url, tint) = process_artwork_bytes(&bytes);
                assert!(
                    url.as_deref().map(|u| u.starts_with("data:image/png;base64,")).unwrap_or(false),
                    "expected decoded PNG data URL from Spotify artwork bytes"
                );
                assert!(tint.is_some(), "expected pastel tint from Spotify artwork");
            }
        }
    }
}
