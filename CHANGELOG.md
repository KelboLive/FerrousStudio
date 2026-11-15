# Changelog

Newest first. Each entry lists what changed for users; the detail and test evidence live in
[`docs/PROGRESS.md`](docs/PROGRESS.md).

## v1.0.0 — Ferrous Studio

The first release under the new name, with an installer. Highlights since v0.4:

### New
- **Renamed to Ferrous Studio.** Settings, profiles, scenes and plugins are copied from `%APPDATA%\Ferrous` to `%APPDATA%\Ferrous Studio` on first launch (the old folder is left as it was). The virtual camera is now "Ferrous Studio Virtual Camera".
- **Windows installer** with everything included: FFmpeg (GPL build with every encoder), the virtual camera, the OBS plugin host, the example plugin, the C runtime built into the exe, WebView2 installed where it's missing, and an optional Windows Firewall rule for Ferrous Link. A portable zip is published alongside it.
- **Preview straight from the GPU** (Settings → Video → *GPU preview*): the compositor and the window share one GPU device, so the preview is drawn from the canvas texture with no copy back to the CPU, as OBS does.
- **Vertical-only mode**: hide the main canvas and keep just the vertical one when you only stream vertically; the main picture isn't composited while nothing uses it.
- **A monitoring button on every mixer channel**, like OBS: one click to hear a channel through your monitoring device, one click to stop.
- **Ferrous Link by computer name**: type the sending PC's Windows computer name (or the Link's name, in any case) instead of its IP address; the list of senders shows which PC each one is on, with a firewall hint when nothing appears.
- **Ctrl+Shift+F12 saves a screenshot** of the Ferrous Studio window as a full-resolution PNG (in `%APPDATA%\Ferrous Studio\screenshots`), handy for bug reports.
- **Encoder preset for Hardware Links** (Settings → Ferrous Link): P1 (fastest) to P7 (best quality), P6 by default: cleaner detail at the same bitrate, on the GPU's own video engine so games keep their frame rate.

### Better
- **Cleaner docking**: tab bars appear only where a dock holds more than one tab; single docks get a small corner title that stays readable on every theme. Docks still snap together exactly as before.
- **Performance Monitor and Stats** scale to any dock size: steady graph cards, summary rows that don't jump, multi-column stats in wide docks, no wasted space.
- **Controls** fit narrow docks without scrolling; the settings cog and Recordings button always stay visible.
- **Hardware Links**: a keyframe every 2 seconds instead of every second (fewer quality pulses).

### Fixed
- **Changing a Hardware (NVENC) Link setting froze Ferrous until Windows closed it**: the old Link and its GPU encoder waited on each other. A restart now takes about a tenth of a second.
- **Saved replays hitched every 2 seconds**: the replay segments were joined with a gap of 2–4 frames at each join. Replays are now continuous in picture and sound.
- **"The system cannot find the path specified (os error 3)" on a Hardware Link receiver**: a saved FFmpeg path that no longer exists now falls back to the bundled FFmpeg, and a decoder that can't start says which path it tried.
- **The darker GPU preview** and a stale preview after a scene switch.
- **Add Source menu text overlapping**, and the Window Capture icon missing from it.
- **A "frame size mismatch" error** logged when the vertical replay buffer started (harmless, now silent).
- The preview reopens by itself if its dock is closed.

### Removed (simpler, nothing lost)
- **HDR10 output**: SDR sources mapped to HDR gave nothing over SDR. HDR *capture* (tone-mapping an HDR display to SDR) stays.
- **The GPU capture, pipelined read-back and adaptive-JPEG switches**: GPU capture and pipelined read-back are now always on where they work and fall back by themselves; the JPEG Link keeps a fixed quality.
- **Snap guides and the dock compass**, which duplicated the live snap preview.

## v0.4
- **Start never freezes the window**: Start Recording, Start Streaming, Go Live and the replay buffer start in the background (FFmpeg and the GPU encoder can take a few seconds), with "Starting…" on the button until they are up.
- **Sources start off the render thread**: capture, media, devices, browser, images, plugins and Link receivers start on a helper thread and appear when ready, so a slow driver, a hung window or a big file can't hold up rendering (and every recording, stream and Link with it).
- **Reconnects off the render thread, and zero-copy again**: a dropped stream reconnects on a helper thread without holding up rendering, and the reconnected stream uses zero-copy GPU encoding again (it used to fall back to FFmpeg encoding).
- **Hardware Links at start-up** get their GPU encoder (they were made before the render thread existed, waited 3 s and fell back to FFmpeg).
- **CI**: a muxer test passed Windows paths through FFmpeg's tee list, where `:` and `\` mean something else; it now uses plain names.
- **Lighter preview**: the GPU draws the preview at the window's size, in the window's colour order, and only that small copy comes back to the CPU (about 3.7 MB a frame at 1440p instead of 14.7 MB, with no CPU scaling). Recordings, streams and Links are unaffected.
- **Game Capture can't stall rendering any more**: looking for the game ran on the render thread and reads window titles, which waits on each window; a game still loading (or hung) froze rendering for many seconds, so a recording started before the game failed with "FFmpeg audio input never opened". The search and the capture start now run on their own thread.
- **One graphics API fewer**: the Ferrous window draws with DirectX 12 like the compositor (Vulkan was eframe's default), falling back to Vulkan by itself; `WGPU_BACKEND` still overrides.
- **Capture hands frames over without waiting**: the capture signals a shared fence after each GPU copy and the compositor's GPU waits on it, instead of the capture thread polling for the copy to finish. Devices without shared fences keep the old wait.
- **Quieter log**: the game-search and UI-rate lines appear every 5 minutes instead of every 15–30 seconds (and losing the game is logged at once).
- **CI fixed**: the Windows build's only failing test assumed the system monitor sampled within 4 s, but on Windows it waits 5 s after start on purpose; the test now waits for the sample.
- **Video filters on the GPU**: chroma key, colour correction, scroll, LUTs and the background-removal mask now run as GPU passes in the compositor instead of per-pixel CPU loops, with CPU twins kept as the fallback (tests prove both give the same picture).
- **GPU-resident capture (experimental, off by default; Settings → Video → GPU capture)**: Windows display, window and game capture can hand each frame to the GPU renderer as a shared texture, skipping the copy to system memory. It falls back to the normal CPU copy by itself for plugin filters, background removal, nested scenes, a different GPU, or any error.
- **Sharper downscaling**: layers shrunk more than ~1.25:1 (a 4K display capture on a 1080p canvas, a big camera in a corner) are now area-averaged instead of bilinear-sampled, on both the GPU and CPU compositors, so fine lines and text no longer shimmer or vanish. 1:1 and enlarged layers are unchanged and cost nothing extra.
- **A GPU figure you can rely on**: the Performance Monitor's system GPU percentage now comes from the card's own utilisation counter (the one RTSS and the NVIDIA overlay read), with the Windows counters as a fallback. Windows' per-engine counters can read far too high, and they no longer drive the headline figure. CPU totals say whether they are "of all cores" or "of one core".
- **Window frame rate** (Settings → General): Ferrous's own window redraws at most 30, 60 (default), 120, 144 or 240 times a second, or unlimited. It never changes your video's frame rate; lower values use less GPU.
- **Startup stability**: the GPU compositor only considers Vulkan and DirectX 12 (the OpenGL probe could crash on some PCs), GPU filter shaders compile the first time a filter is used, and a native crash writes `native-crash.txt` (exception code and module) next to your settings.
- **NVIDIA encoding inside Ferrous, straight from the GPU (as OBS does)**: recording, streaming, the replay buffer and Hardware Links of the program picture encode with NVENC in-process from the canvas texture; FFmpeg only writes the file or sends the stream. B-frames, your rate control and preset carry over; frames reach FFmpeg in MPEG-TS/IVF with exact timestamps (a bare stream starved recent FFmpeg's audio input). Settings → Video → *Zero-copy encoding* (on); anything else, or any failure starting it, uses the FFmpeg path.
- **Only the read-back a frame needs**: the GPU path no longer copies the full BGRA picture back every frame. Outputs that take the GPU's NV12 get only that, GPU-encoded outputs get nothing, and BGRA comes back only when the preview is due, a transition runs or something else on the CPU looks at the picture.
- **JPEG Link the obs-teleport way**: the GPU converts the program picture to JPEG's YCbCr (BT.601 full range) and the Link compresses those planes directly: 1.5 bytes a pixel read back instead of 4, and no colour conversion on the CPU.
- **HDR displays**: captures are taken in 16-bit and tone-mapped on the GPU to standard range, with Windows' SDR content brightness as white, so highlights roll off instead of clipping to flat white.
- **GPU capture really on**: captures follow the setting live (game capture used to start before it was applied and stayed on the CPU path), and the log says when it is in use.
- **Fixes**: a recording with a static scene no longer waits forever for its first frame; audio from the seconds FFmpeg takes to open its inputs is kept, so sound starts with the picture; a window saved at a tiny size (minimised, remote desktop) reopens at a normal size.
- **Ferrous Link, tested gaming PC to streaming PC**: JPEG now works like obs-teleport (quarter-resolution colour, every core but one, short bursts queue instead of dropping), with a *Full colour detail* option for sharper HUD text; the Hardware codec no longer bursts frames (no lookahead, frames released as encoded, "hq" tuning) and the receiver paces them evenly; a link keeps its port when its settings change.
- **GPU priority on by default**, as in OBS: with a game using the whole GPU, this is what keeps Ferrous at its frame rate.
- **Game Capture finds anti-cheat and Game Pass games** (Call of Duty and similar didn't appear at all), tolerates borderless windows a few pixels short of the monitor, and logs why when it can't find a game.
- **Vertical canvas** reads back a frame late like the main canvas, instead of stalling every changed frame.
- **Works with newer FFmpeg builds**: recent FFmpeg refuses `-thread_queue_size` in front of an input, which stopped the replay buffer (and would stop recording and streaming) with "Error opening input files: Invalid argument". Ferrous now tests your FFmpeg once and leaves the option out when it isn't accepted.
- **Efficiency and robustness pass**: one persistent GPU buffer for layer parameters instead of a new buffer per layer per frame; the capture thread sleeps instead of spinning while the GPU finishes a copy; audio buffers are sized up front; event, remote-control and client queues are bounded so a stalled reader can't grow memory; a frame of the wrong size is skipped instead of panicking; a poisoned lock no longer stops transitions.
- **Security and robustness pass**: the remote-control server no longer lets a client skip the password with Reidentify; Ferrous Link survives malformed or hostile senders (empty/short messages, absurd picture sizes), encrypts each connection under its own key, and no longer lets a silent peer block new receivers; Stop no longer hangs when FFmpeg stops reading (it is killed after 15 s).
- **Debug pass**: damaged settings, scene and library files are repaired or kept as `.bad` instead of failing; 14 corrupt-profile launches (truncated, empty, binary, zero/huge fps, bad UI scale or layout) start cleanly; Settings tabs and the scene/source button rows now wrap in narrow windows so nothing is unreachable.
- **Recording presets and a speed/quality benchmark** (Settings → Recording).
- **HDR10 output** for HEVC and AV1 (SDR sources mapped to BT.2020/PQ, adjustable white level).
- **Encoder quality pass**: NVENC two-pass and lookahead, H.264 High profile, AV1 10-bit internals, Quick Sync
  adaptive I/B, AMF VBAQ, SVT-AV1 visual-quality tune.
- **New icon** (Fe 26 glass) in the exe, taskbar and top bar.
- Safe Game Capture, Source Clone, Track Matte transitions, OBS-style Add Source dialog, undo/redo,
  audio sync offset, per-scene transitions, projector and multiview windows.
- Renamed from Ember Studio to Ferrous (settings migrate automatically).
- Leaner build: trimmed eframe and env_logger default features.
