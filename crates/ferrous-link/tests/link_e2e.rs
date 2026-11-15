//! Real sender ↔ receiver over loopback TCP.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ferrous_core::{LinkCodec, VideoFrame};
use ferrous_link::sender::SenderConfig;
use ferrous_link::{LinkReceiver, LinkSender};
use parking_lot::Mutex;

fn cfg(name: &str, codec: LinkCodec, pin: &str) -> SenderConfig {
    SenderConfig { name: name.into(), codec, width: 320, height: 180, fps: 30, port: 0, pin: pin.into(), audio: true, ffmpeg: PathBuf::from("ffmpeg"), external_stream: false }
}

fn frame(t: u8) -> VideoFrame {
    let mut d = vec![0u8; 320 * 180 * 4];
    for (i, px) in d.chunks_exact_mut(4).enumerate() {
        px.copy_from_slice(&[t, (i % 320) as u8, 200, if i % 320 < 20 { 64 } else { 255 }]);
    }
    VideoFrame::new(320, 180, d)
}

/// Pumps frames until the receiver has one (or times out).
fn pump(s: &LinkSender, r: &LinkReceiver, f: &VideoFrame, secs: u64) -> Option<VideoFrame> {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        s.push_frame(f);
        std::thread::sleep(Duration::from_millis(33));
        if let Some(got) = r.latest_frame() {
            return Some(got);
        }
    }
    None
}

#[test]
fn jpeg_link_with_pin_audio_and_stats() {
    let s = LinkSender::start(cfg("Gaming PC", LinkCodec::Jpeg { quality: 90, full_chroma: false }, "4321")).unwrap();
    let heard: Arc<Mutex<Vec<f32>>> = Arc::default();
    let h2 = heard.clone();
    let r = LinkReceiver::start(&format!("127.0.0.1:{}", s.port), "4321", PathBuf::from("ffmpeg"), Some(Arc::new(move |a: &[f32]| h2.lock().extend_from_slice(a))));
    let f = frame(100);
    let got = pump(&s, &r, &f, 10).expect("no frame received");
    assert_eq!((got.width, got.height), (320, 180));
    assert!((got.data[0] as i32 - 100).abs() < 8, "blue channel ~100, got {}", got.data[0]);
    for _ in 0..30 {
        s.push_frame(&f);
        s.push_audio(&[0.5f32; 960]);
        std::thread::sleep(Duration::from_millis(40));
    }
    let a = heard.lock();
    assert!(!a.is_empty(), "audio arrived");
    assert!((a[0] - 0.5).abs() < 0.001);
    assert!(r.stats.connected.load(Ordering::Relaxed));
    assert_eq!(*r.stats.sender_name.lock(), "Gaming PC");
    assert!(r.stats.rtt_ms.load(Ordering::Relaxed) < 200);
    assert!(r.stats.latency_ms.load(Ordering::Relaxed) < 500, "latency {}", r.stats.latency_ms.load(Ordering::Relaxed));
    assert_eq!(s.stats.clients.load(Ordering::Relaxed), 1);
    assert!(s.stats.kbps.load(Ordering::Relaxed) > 0);
    println!("latency {} ms, rtt {} ms, {} kbps", r.stats.latency_ms.load(Ordering::Relaxed), r.stats.rtt_ms.load(Ordering::Relaxed), r.stats.kbps.load(Ordering::Relaxed));
}

#[test]
fn wrong_or_missing_pin_is_refused() {
    let s = LinkSender::start(cfg("Locked", LinkCodec::Jpeg { quality: 80, full_chroma: false }, "1111")).unwrap();
    let addr = format!("127.0.0.1:{}", s.port);
    let wrong = LinkReceiver::start(&addr, "2222", PathBuf::from("ffmpeg"), None);
    let none = LinkReceiver::start(&addr, "", PathBuf::from("ffmpeg"), None);
    assert!(pump(&s, &wrong, &frame(1), 3).is_none());
    std::thread::sleep(Duration::from_millis(300));
    let e1 = wrong.stats.error.lock().clone().unwrap_or_default();
    let e2 = none.stats.error.lock().clone().unwrap_or_default();
    assert!(!e1.is_empty(), "wrong PIN reported");
    assert!(e2.contains("PIN"), "{e2}");
    assert!(!wrong.stats.connected.load(Ordering::Relaxed));
}

#[test]
fn lossless_link_is_exact_including_alpha() {
    let s = LinkSender::start(cfg("Overlay", LinkCodec::Lossless, "")).unwrap();
    let r = LinkReceiver::start(&format!("127.0.0.1:{}", s.port), "", PathBuf::from("ffmpeg"), None);
    let f = frame(77);
    let got = pump(&s, &r, &f, 10).expect("no frame");
    assert_eq!(got.data, f.data);
}

#[test]
fn hardware_codec_stream_decodes() {
    if std::process::Command::new("ffmpeg").arg("-version").output().is_err() {
        return;
    }
    let s = LinkSender::start(cfg("H264", LinkCodec::Hardware { encoder: "libx264".into(), bitrate_kbps: 2000, preset: 6 }, "")).unwrap();
    let r = LinkReceiver::start(&format!("127.0.0.1:{}", s.port), "", PathBuf::from("ffmpeg"), None);
    let f = frame(150);
    let got = pump(&s, &r, &f, 15).expect("no decoded frame from the H.264 link");
    assert_eq!((got.width, got.height), (320, 180));
    // Lossy, but colours should be close in the opaque area.
    let i = (90 * 320 + 200) * 4;
    assert!((got.data[i + 2] as i32 - 200).abs() < 20, "red ~200, got {}", got.data[i + 2]);
}

#[test]
fn discovered_by_name() {
    let s = LinkSender::start(cfg("Discoverable PC", LinkCodec::Jpeg { quality: 80, full_chroma: false }, "")).unwrap();
    ferrous_link::discovery::start_listening();
    let mut found = false;
    for _ in 0..40 {
        if ferrous_link::discovered().iter().any(|a| a.name == "Discoverable PC" && a.port == s.port) {
            found = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(found, "beacon heard: {:?}", ferrous_link::discovered());
    let r = LinkReceiver::start("Discoverable PC", "", PathBuf::from("ffmpeg"), None);
    assert!(pump(&s, &r, &frame(5), 10).is_some(), "connected by name");
}

#[test]
fn static_scene_repeats_and_late_joiners_get_the_picture() {
    for codec in [LinkCodec::Jpeg { quality: 90, full_chroma: false }, LinkCodec::Lossless] {
        let s = LinkSender::start(cfg("Static", codec.clone(), "")).unwrap();
        let f = frame(42);
        let first = LinkReceiver::start(&format!("127.0.0.1:{}", s.port), "", PathBuf::from("ffmpeg"), None);
        assert!(pump(&s, &first, &f, 5).is_some(), "{codec:?}: first receiver gets a frame");
        // Keep the picture static for a while, then a second receiver joins.
        for _ in 0..30 {
            s.push_frame(&f);
            std::thread::sleep(Duration::from_millis(10));
        }
        let late = LinkReceiver::start(&format!("127.0.0.1:{}", s.port), "", PathBuf::from("ffmpeg"), None);
        let got = pump(&s, &late, &f, 5).unwrap_or_else(|| panic!("{codec:?}: late joiner got no picture from a static scene"));
        if codec == LinkCodec::Lossless {
            assert_eq!(got.data, f.data, "lossless picture is exact");
        }
        let before = s.stats.bytes_sent.load(Ordering::Relaxed);
        for _ in 0..30 {
            s.push_frame(&f);
            std::thread::sleep(Duration::from_millis(10));
        }
        let per_frame = (s.stats.bytes_sent.load(Ordering::Relaxed) - before) / 30;
        assert!(per_frame < 200, "{codec:?}: static frames cost {per_frame} bytes each (should be tiny repeats)");
    }
}

/// Switching the encoder restarts the sender; a receiver that was watching
/// must come back with the new codec, on the same port, and show its picture.
#[test]
fn receiver_follows_encoder_switches() {
    if std::process::Command::new("ffmpeg").arg("-version").output().is_err() {
        return;
    }
    let hw = |bitrate| LinkCodec::Hardware { encoder: "libx264".into(), bitrate_kbps: bitrate, preset: 6 };
    // A port of its own: the other tests start senders on the default range
    // in parallel and would take this one in the gap between two senders.
    let mut c0 = cfg("Switcher", hw(2000), "");
    c0.port = 47_930;
    let first = LinkSender::start(c0).unwrap();
    let port = first.port;
    let r = LinkReceiver::start(&format!("127.0.0.1:{port}"), "", PathBuf::from("ffmpeg"), None);
    assert!(pump(&first, &r, &frame(60), 15).is_some(), "no picture from the first encoder");
    drop(first);

    for (n, codec) in [LinkCodec::Jpeg { quality: 90, full_chroma: false }, hw(3000), LinkCodec::Lossless, hw(2500)].into_iter().enumerate() {
        let mut c = cfg("Switcher", codec, "");
        c.port = port;
        let s = LinkSender::start(c).unwrap();
        assert_eq!(s.port, port, "switch {n}: the sender must take its old port back");
        // Only frames from the new sender count: forget the old picture.
        let marker = 120 + n as u8 * 20;
        let f = frame(marker);
        let end = Instant::now() + Duration::from_secs(20);
        let mut ok = false;
        while Instant::now() < end {
            s.push_frame(&f);
            std::thread::sleep(Duration::from_millis(33));
            if let Some(got) = r.latest_frame() {
                if (got.data[0] as i32 - marker as i32).abs() < 12 {
                    ok = true;
                    break;
                }
            }
        }
        assert!(ok, "switch {n}: the receiver never showed the new sender's picture");
        drop(s);
    }
}

/// A sender that misbehaves (empty message, short audio, a zero-size lossless frame) must not
/// take the receiver down: it keeps running and reconnects.
#[test]
fn receiver_survives_a_hostile_sender() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let hello = br#"{"proto":3,"name":"evil","width":320,"height":180,"fps":30,"codec":"Lossless","audio":true,"encrypted":false,"salt":""}"#;
    let evil = std::thread::spawn(move || {
        let mut sessions = 0;
        listener.set_nonblocking(false).unwrap();
        let end = Instant::now() + Duration::from_secs(12);
        while sessions < 2 && Instant::now() < end {
            listener.set_nonblocking(true).unwrap();
            let Ok((mut s, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(20));
                continue;
            };
            s.set_nonblocking(false).unwrap();
            let send = |s: &mut std::net::TcpStream, body: &[u8]| {
                s.write_all(&(body.len() as u32).to_be_bytes()).unwrap();
                s.write_all(body).unwrap();
            };
            send(&mut s, hello);
            let mut len = [0u8; 4];
            s.read_exact(&mut len).unwrap();
            let mut auth = vec![0u8; u32::from_be_bytes(len) as usize];
            s.read_exact(&mut auth).unwrap();
            send(&mut s, &[]); // zero-length message
            send(&mut s, &[4, 1, 2]); // audio, shorter than its timestamp
            let mut video = vec![2u8]; // VIDEO: header with a 0x0 picture, then junk
            video.extend_from_slice(&[0; 16]);
            video.extend_from_slice(&[1, 0xFF, 0xFF, 0xFF, 0xFF]);
            send(&mut s, &video);
            std::thread::sleep(Duration::from_millis(300));
            drop(s); // hang up: a healthy receiver reconnects
            sessions += 1;
        }
        sessions
    });
    let rx = LinkReceiver::start(&addr, "", PathBuf::from("ffmpeg"), None);
    assert_eq!(evil.join().unwrap(), 2, "the receiver stopped reconnecting after hostile input");
    drop(rx);
}

fn read_hello_salt(port: u16) -> String {
    use std::io::Read;
    let mut c = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut len = [0u8; 4];
    c.read_exact(&mut len).unwrap();
    let mut body = vec![0u8; u32::from_be_bytes(len) as usize];
    c.read_exact(&mut body).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    v["salt"].as_str().unwrap().to_string()
}

/// Every connection must be encrypted under its own key. With one salt per sender, the nonce counters
/// (which restart at 0 on each connection) repeated under the same key.
#[test]
fn each_connection_gets_a_fresh_salt() {
    let s = LinkSender::start(cfg("Salty", LinkCodec::Jpeg { quality: 80, full_chroma: false }, "2468")).unwrap();
    let (a, b) = (read_hello_salt(s.port), read_hello_salt(s.port));
    assert!(!a.is_empty() && !b.is_empty());
    assert_ne!(a, b, "two connections shared a key");
}

/// A peer that connects and never speaks must not stop others joining.
#[test]
fn a_silent_connection_does_not_block_real_receivers() {
    let s = LinkSender::start(cfg("Busy", LinkCodec::Jpeg { quality: 80, full_chroma: false }, "")).unwrap();
    let _silent: Vec<_> = (0..3).map(|_| std::net::TcpStream::connect(("127.0.0.1", s.port)).unwrap()).collect();
    let r = LinkReceiver::start(&format!("127.0.0.1:{}", s.port), "", PathBuf::from("ffmpeg"), None);
    assert!(pump(&s, &r, &frame(9), 4).is_some(), "a real receiver waited behind silent peers");
}

/// A Hardware Link fed by the in-process GPU encoder stops promptly even while that encoder still
/// holds its channel open (restarting such a Link used to hang the settings window for good).
#[test]
fn externally_fed_link_stops_while_its_encoder_still_runs() {
    let mut c = cfg("Restart", LinkCodec::Hardware { encoder: "hevc_nvenc".into(), bitrate_kbps: 20_000, preset: 6 }, "");
    c.external_stream = true;
    let s = LinkSender::start(c).expect("sender");
    let encoder_side = s.stream_input().expect("external input");
    // The encoder delivers a little, then keeps the channel open without sending (a static scene).
    let _ = encoder_side.send(vec![0x47; 188]);
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        drop(s);
        let _ = done_tx.send(());
    });
    assert!(done_rx.recv_timeout(Duration::from_secs(5)).is_ok(), "the Link stopped while its encoder still held the channel");
    // And the encoder now finds the Link gone (it stops itself on that).
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while encoder_side.send(vec![0x47; 188]).is_ok() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(encoder_side.send(vec![0x47; 188]).is_err(), "the encoder's channel is closed");
}

/// The in-process GPU encoder's path: MPEG-TS pushed into the sender (HEVC here, from a file made
/// with libx265 and wrapped by `EsMux`), decoded by a receiver.
#[test]
fn externally_fed_hevc_stream_decodes() {
    let Ok(ts) = std::fs::read("/tmp/tsx/out.ts") else { return };
    let c = SenderConfig { width: 1280, height: 720, fps: 60, external_stream: true, ..cfg("Ext", LinkCodec::Hardware { encoder: "hevc_nvenc".into(), bitrate_kbps: 20_000, preset: 6 }, "") };
    let s = LinkSender::start(c).unwrap();
    let input = s.stream_input().unwrap();
    let r = LinkReceiver::start(&format!("127.0.0.1:{}", s.port), "", PathBuf::from("ffmpeg"), None);
    let end = Instant::now() + Duration::from_secs(20);
    let mut got = 0;
    'outer: while Instant::now() < end {
        for chunk in ts.chunks(188 * 40) {
            input.send(chunk.to_vec()).unwrap();
            std::thread::sleep(Duration::from_millis(16));
            let _ = r.latest_frame();
            got = r.stats.frames.load(std::sync::atomic::Ordering::Relaxed);
            if got > 30 {
                break 'outer;
            }
        }
    }
    eprintln!("frames {got} error {:?}", r.stats.error.lock());
    assert!(got > 30, "decoded {got}; error {:?}", r.stats.error.lock());
}
