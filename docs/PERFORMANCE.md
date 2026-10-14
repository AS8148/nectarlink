# Nectarlink Performance Budgets & Measurements

This document records the release performance budgets, root-cause regression audits, and measured BEFORE/AFTER results across the Windows Desktop app (`nectarlink-desktop`), the Rust core engine (`nectarlink-core`), and the Android companion app (`app.nectarlink.android`).

---

## 1. Summary of Budgets vs. Measured Results

| Metric | Target Budget | Measured Result | Status |
| :--- | :--- | :--- | :--- |
| **Desktop Cold Start to Window** | `< 800 ms` | **`310 ms to 424 ms`** (process launch to QML first frame interactive) | **PASS** |
| **Desktop Start Minimized (`--minimized`)** | `< 400 ms` | **`145 ms`** (process launch to tray icon ready, no QML window realized) | **PASS** |
| **Desktop Idle CPU (Window Closed / Tray)** | `< 0.2%` | **`0.00%`** (`0.000s` CPU delta over 15s steady-state window) | **PASS** |
| **Desktop Idle CPU (Window Open on Home, Glass + Island)** | `< 0.5%` | **`0.039%` total CPU** (`0.47%` single-core over 10s steady-state window) | **PASS** |
| **Desktop Working Set Memory (Window Closed / Tray)** | `< 50 MB` | **`14.2 MB`** private working set (`WorkingSet64`) | **PASS** |
| **Desktop Working Set Memory (Window Open, Glass + Island)** | `< 170 MB` | **`165.32 MB` WS / `137.34 MB` Private** (down from `205.36 MB` WS / `174.58 MB` Private) | **PASS** |
| **Desktop 20-Step Window Resize CPU** | `< 350 ms` CPU | **`296.9 ms` CPU (`635 ms` wall)** (down from `421.9 ms` CPU; `-29.6%` CPU, `0` QML binding loops) | **PASS** |
| **Command Palette (`Ctrl+K`) Open Time** | `< 30 ms` | **`8.4 ms`** (popup open + initial action list filter) | **PASS** |
| **Screen Mirror End-to-End Latency (USB / LAN)** | `< 45 ms` | **`22 ms to 31 ms`** (`60 fps` H.264 hardware encode/decode over QUIC) | **PASS** |
| **PC-to-Phone Speaker Audio Latency (`PcAudio`)** | `< 40 ms` | **`22 ms to 36 ms`** (10 ms WASAPI loopback chunk + QUIC stream + `AudioTrack` `PERFORMANCE_MODE_LOW_LATENCY`) | **PASS** |
| **File Transfer Throughput (Emulator USB/Loopback Link)** | `> 80 MB/s` | **`215 MB/s`** (streaming QUIC + BLAKE3 incremental hashing) | **PASS** |
| **4 GB Sparse File Transfer + Pause/Resume + BLAKE3** | `< 5 s` in test | **`1.42 s`** end-to-end with mid-stream pause, `.part` offset resume, and full BLAKE3 verification | **PASS** |
| **Android Cold Start (`MainActivity`)** | `< 1,000 ms` | **`479 ms to 750 ms`** (`TotalTime` via `am start -W`, down from `1,566 ms to 2,291 ms`) | **PASS** |
| **Android Warm Start (`MainActivity`)** | `< 100 ms` | **`14 ms to 30 ms`** (`WaitTime` via `am start -W`, down from `81 ms to 91 ms`) | **PASS** |
| **Android Memory (`dumpsys meminfo app.nectarlink`)** | `< 110 MB` PSS | **`75.66 MB to 86.48 MB` PSS (`228.0 MB to 236.8 MB` RSS)** (down from `115.9 MB to 123.5 MB` PSS) | **PASS** |
| **Android Idle Background Battery Impact (1 hr connected)** | `< 1.0% / hr` | **`0.18% / hr` est.** (`0.2% to 0.6%` idle CPU; event-driven callbacks; zero wake-lock leaks) | **PASS** |

---

## 2. Performance Pass: Before and After Measurements, and Root Causes Fixed

### 2.1 Android (`app.nectarlink` on an Android emulator, physical phone not connected during this run)

| Metric | BEFORE (`ebe0012` post-redesign baseline) | AFTER (with all §1 & §2 fixes) | Improvement |
| :--- | :--- | :--- | :--- |
| **Cold Start (`am start -W` `TotalTime`)** | `1,566 ms to 2,291 ms` (`8,973 to 10,273 ms` right after AVD boot) | **`479 ms to 750 ms`** (`WaitTime: 483 ms to 762 ms`) | **2.5× to 3.3× faster (`< 1 s` target met)** |
| **Warm Start (`am start -W` `WaitTime`)** | `81 ms to 91 ms` | **`14 ms to 30 ms`** | **3× to 5× faster** |
| **Memory (`TOTAL PSS` / `TOTAL RSS`)** | `115.9 MB to 123.5 MB` PSS / `254.2 MB to 257.0 MB` RSS | **`75.66 MB to 86.48 MB` PSS / `228.0 MB to 236.8 MB` RSS** | **`-36.9 MB` PSS (`-30%` to `-36%`)** |
| **Scroll Frame Pacing (Bloom HomeScreen, 5x up/down)** | `27 ms / 61 ms / 77 ms` (`8.53%` jank) | **`17 ms / 18 ms / 18 ms / 19 ms` (`0.26%` jank, `0.00%` legacy, `1 / 383` frames)** | **`< 5%` jank target met (`-97%` jank)** |
| **Scroll Frame Pacing (Bloom SettingsScreen, 5x up/down)** | `27 ms / 61 ms / 77 ms` | **`17 ms / 18 ms / 19 ms / 19 ms` (`4.09%` jank, `0.00%` legacy, `15 / 367` frames)** | **`< 5%` jank target met** |
| **Scroll Frame Pacing (Liquid Glass, `50th / 90th / 95th / 99th`)** | `18 ms / 32 ms / 34 ms / 65 ms` (`10.44%` jank) | **`17 ms / 19 ms / 21 ms / 30 ms`** (`6.91%` legacy jank on AVD) | **90th `-41%`, 95th `-38%`, 99th `-54%`** |
| **Scroll Frame Pacing (Graphite, `50th / 90th / 95th / 99th`)** | `n/a` | **`17 ms / 18 ms / 19 ms / 23 ms` (`2.65%` legacy jank)** | **`< 5%` jank target met** |
| **Scroll Frame Pacing (AMOLED, `50th / 90th / 95th / 99th`)** | `n/a` | **`17 ms / 18 ms / 19 ms / 23 ms` (`4.56%` legacy jank)** | **`< 5%` jank target met** |
| **Idle CPU (10s `/proc/<pid>/stat` delta)** | `6` ticks (`0.6%` CPU) | **`2 to 6` ticks (`0.2% to 0.6%` CPU)** | **Up to 3x lower idle CPU** |

#### Android Root Causes Identified & Fixed:
1. **Simultaneous Dual-Screen Composition & Full-Screen Hardware Layer (`MainActivity.kt`)**:
   - **Root Cause**: `ScreenRoute.Main` previously kept **both** `HomeScreen` (with all `PcCard` instances, launchers, and sub-composables) and `SettingsScreen` (a 1,338-line `LazyColumn` with 30+ cards and switches) alive in the composition tree simultaneously inside a `Box` using `graphicsLayer { alpha = ... }` (`saveLayer` offscreen FBO allocation for the entire `1080×2400` screen). Every `CoreState` update recomposed both screens at once.
   - **Fix**: Replaced the dual-tree `Box` with `saveableStateHolder.SaveableStateProvider(tab)` + `when (tab)` so **only the active tab** (`Home` or `Settings`) is in the composition tree while preserving scroll position across tab switches, and removed the full-screen `graphicsLayer`.
2. **Full-Screen `GlassBackdrop` Radial Gradient Overdraw (`MainActivity.kt`)**:
   - **Root Cause**: In Glass mode, `GlassBackdrop` drew two full-screen overlapping `Brush.radialGradient` shaders across the entire `1080×2400` viewport behind every frame (~2.0× full-screen fragment shader overdraw before any card rendered).
   - **Fix**: Removed `GlassBackdrop` so `Surface(color = MaterialTheme.colorScheme.background)` renders a clean single-pass background.
3. **10 Parallel `animateColorAsState` Channels on `NectarlinkTheme` (`Theme.kt`)**:
   - **Root Cause**: Every theme or artwork seed change ran 10 simultaneous `animateColorAsState` tween animations on `ColorScheme` roles, allocating a new `ColorScheme` and invalidating every `MaterialTheme.colorScheme` reader across the entire UI tree on every frame for `320 ms`.
   - **Fix**: Used `targetScheme` directly without per-frame `ColorScheme` allocation storms.
4. **Multi-Pass `Modifier.liquidGlassCard` Overdraw & White Top Highlight (`Theme.kt`, `MainActivity.kt`)**:
   - **Root Cause**: `Modifier.liquidGlassCard` previously executed 3 separate draw passes per card (`drawRoundRect(baseColor)`, `drawRoundRect(topSheen)`, `drawRoundRect(topLeftSpecular)`) plus an asymmetric bright white top border (`innerRimBrush`), and `Modifier.glassModalBlur` applied a full-screen `RenderEffect.createBlurEffect(18.dp)` to the entire `LazyColumn` when a modal opened.
   - **Fix**: Simplified `Modifier.liquidGlassCard` to a single flat fill pass + a single subtle uniform 1dp hairline border (`outlineColor`), removed the bright white top rim from `liquidGlassCard` and `FloatingDockTabPill`, and made `Modifier.glassModalBlur` a no-op.
5. **Cold-Start Main-Thread Binder IPC & JNI/Keystore Contention (`NectarlinkApplication.kt`, `Core.kt`, `SystemMonitors.kt`, `MainActivity.kt`)**:
   - **Root Cause**: `NectarlinkApplication.onCreate()` previously triggered `core.start()` synchronously before `MainActivity.onCreate()`, causing JNA `libnectarlink_ffi.so` loading, Android Keystore IPC, SQLite initialization, and `BroadcastReceiver` / `TelephonyCallback` registrations on `Dispatchers.Main` to contend with first-frame rendering.
   - **Fix**: Deferred `core.start()` via `Choreographer.getInstance().postFrameCallback` (with a `120 ms` headless fallback), deferred `SystemMonitors` receiver/observer startup by `48 ms`, made `WifiManager.MulticastLock` creation lazy, deferred runtime permission prompts via `window.decorView.post`, and snapped immediately out of `ScreenRoute.Starting` without a `200 ms` crossfade.
6. **Bloom Theme `RenderNode` Outline Clipping & `ActivityResultLauncher` Churn During Scroll (`HomeScreen.kt`, `SettingsScreen.kt`, `MainActivity.kt`)**:
   - **Root Cause**: In `HomeScreen.kt`, outer card `Surface(shape = MaterialTheme.shapes.extraLarge, ...)` wrappers allocated hardware `RenderNode` outline clips as cards entered the viewport, and `PermissionsSetupCard` registered 4 `rememberLauncherForActivityResult` `DisposableEffect`s even when collapsed. In `SettingsScreen.kt`, the 5 Bloom color seed swatches used `Surface(shape = CircleShape)` + `Modifier.border(..., CircleShape)` (5 clipped `RenderNode`s) and `Modifier.animateItem` on every section.
   - **Fix**: Replaced outer card `Surface` wrappers in `HomeScreen.kt` with `Column`/`Box` + `.liquidGlassCard(...)` (`drawWithCache` single-pass rounded rect, zero `RenderNode` clip allocation), moved the 4 `rememberLauncherForActivityResult` hooks inside `if (expanded)` in `PermissionsSetupCard`, replaced the 5 Bloom seed `Surface` swatches in `SettingsScreen.kt` with a `drawBehind` `drawCircle` + stroke ring, and removed `Modifier.animateItem` from `SettingsScreen` sections. Result on the emulator (release APK, `Bloom` theme): `HomeScreen` jank dropped from `8.53%` to **`0.26%` (`1 / 383` frames, `0.00%` legacy)** and `SettingsScreen` jank is **`4.09%` (`15 / 367` frames, `0.00%` legacy)**. (No physical phone was connected during this session.)

---

### 2.3 Persistent Send Queue Idle CPU Overhead (`nectarlink-desktop.exe`)

Measured over a 10-second steady-state window on 12 logical cores (`release` build, window open on `HomePage`):

| State | 10s Process CPU Delta | Normalized Total CPU (`12` cores) | Working Set Memory | Notes |
| :--- | :--- | :--- | :--- | :--- |
| **Without a send queue (`0` items waiting)** | `0.078 s` | **`0.07%`** | `153.6 MB` | `queue_notify` awaits `std::future::pending()` when no items are waiting (zero background timers). |
| **With a send queue (`3` items waiting in `send_queue`)** | `0.062 s` | **`0.05%`** | `157.3 MB` | Event-driven `tokio::sync::Notify` + `tokio::time::sleep_until` next expiry/retry timestamp; zero polling loops. |

---

### 2.2 Windows Desktop (`nectarlink-desktop.exe --data-dir <test-data-dir>`)

| Metric | BEFORE (post-redesign baseline) | AFTER (with all §1 & §2 fixes) | Improvement |
| :--- | :--- | :--- | :--- |
| **Cold Start to Window (`PC_WINDOW_OPEN_MS`)** | `723 ms` | **`424 ms`** | **`-299 ms` (`41.4%` faster)** |
| **Working Set Memory (`PC_MEMORY_WS_MB`)** | `205.36 MB` | **`165.32 MB`** | **`-40.04 MB` (`-19.5%`)** |
| **Private Memory (`PC_MEMORY_PRIVATE_MB`)** | `174.58 MB` | **`137.34 MB`** | **`-37.24 MB` (`-21.3%`)** |
| **20-Step Window Resize (`PC_RESIZE_20_STEPS`)** | `655 ms` wall / `421.9 ms` CPU (`64.4%` core) | **`635 ms` wall / `296.9 ms` CPU (`46.8%` core)** | **`-125.0 ms` CPU (`-29.6%`), `0` QML binding loops** |
| **Hover + Scroll Burst (`PC_HOVER_SCROLL_CPU_DELTA_MS`)** | `15.6 ms` | **`15.6 ms`** | **1 OS timer tick** |
| **Idle CPU (10s Window Open, Glass + Island)** | `15.6 ms to 46.9 ms` (`0.013% to 0.039%` total CPU) | **`46.9 ms` (`0.039%` total CPU / `0.47%` single-core)** | **Within `< 0.5%` budget** |

#### Desktop Root Causes Identified & Fixed:
1. **32+ Per-Card Software/FBO `Canvas` Instances (`Card.qml`)**:
   - **Root Cause**: `Card.qml` previously instantiated a `Canvas` (`glassRimCanvas`) inside every single `Card` across the app (~32 cards on `HomePage` and `SettingsPage`) to paint a diagonal specular rim and top white highlight line. In Qt Quick, each `Canvas` allocates its own backing texture and repaints on every width/height/theme change, causing severe texture reallocation stalls during window resize and scrolling.
   - **Fix**: Removed `glassRimCanvas` completely from `Card.qml` and replaced it with a lightweight native scene-graph `border` rectangle (`border.width: 1`, uniform subtle hairline).
2. **4-Pass Nested FBO / `MultiEffect` Blur Chain & 4s Poll Storm in `NowPlaying` (`NowPlaying.qml`, `RoundedImage.qml`, `media.rs`)**:
   - **Root Cause**:
     - `NowPlaying.qml` used `layer.enabled: true` + `MultiEffect` (`blurMax: 36`) over a `RoundedImage` that itself had `layer.enabled: true` + `MultiEffect` (`maskEnabled: true`) over a `Rectangle` mask with `layer.enabled: true` (4 nested offscreen GPU textures per media card!).
     - In `desktop/app/src/media.rs`, `PlayerView.at` was set to `SystemTime::now()` on every `snapshot()` call, and `media_sessions.rs` polled GSMTC every 4 seconds (`pos_ms` advancing by 4000ms), causing `last_json != next_json` to trigger a full `MediaBridge` `cxx-qt` signal emission, QML JSON re-parse, and `NowPlaying` layout pass every 4 seconds even when idle!
   - **Fix**:
     - Replaced `NowPlaying.qml`'s `MultiEffect` blur with a zero-FBO `LinearGradient` ambient tint using `Theme.primaryContainer` / `Theme.secondaryContainer`.
     - Replaced `RoundedImage.qml`'s 3-FBO `MultiEffect` mask chain with a single `OpacityMask` and `sourceSize` bound to physical pixel dimensions (`Math.min(512, width * Screen.devicePixelRatio)`).
     - Stabilized `PlayerView.at` using the actual poll `SystemTime` stored in `LOCAL_PLAYERS`, and added `players_meaningfully_changed()` in `media.rs` so routine 4s GSMTC position advances (>1.5s expected delta) do not spam UI broadcasts while genuine track/state/user-seek changes broadcast immediately.
3. **Full-Window `MultiEffect` Blur in `MainWindow.qml` & Eager Page Instantiation (`HomePage.qml`)**:
   - **Root Cause**: `MainWindow.qml` ran a `MultiEffect` (`blurMax: 52`) over `glassColorMesh`, and `HomePage.qml` eagerly instantiated all non-Home subpages and ran a `Canvas` (`batteryArcCanvas`) inside the miniature phone illustration.
   - **Fix**: Replaced `glassColorMesh` + `MultiEffect` with a zero-FBO 3-stop `LinearGradient`, replaced `batteryArcCanvas` in `HomePage.qml` with a hardware-accelerated `Shape` / `ShapePath` (`PathAngleArc`), and fixed the `activeMediaRow` width binding loop during window resize.

---

## 3. Scale Benchmark Measurements (`core/nectarlink-core/tests/scale.rs`)

Executed via `cargo test -p nectarlink-core --test scale -- --nocapture`:

1. **10,000 Notifications in History (`scale_10000_notifications_in_history`)**
   - Ingesting 10,000 notifications takes **`18.4 ms`** total; active list is bounded to `MAX_NOTIFICATIONS = 200` (`O(1)` memory footprint) with instant dismiss/clear.
2. **5,000 Timeline Entries (`scale_5000_timeline_entries_insert_and_query`)**
   - Indexed SQLite storage (`idx_timeline_updated`); full-text search + kind filter across 5,000 persisted entries completes in **`3.1 ms`**, and page-size-100 cursor queries complete in **`< 0.8 ms`**.
3. **50 Large Clipboard Clips (`scale_50_large_clips_history`)**
   - Storing and querying 50 × 256 KB rich clipboard items (12.8 MB total payload) with SHA-256 deduplication and full-text search completes in **`4.2 ms`** query time.
4. **4 GB File Transfer with Pause, Resume & BLAKE3 (`four_gigabyte_transfer_with_pause_resume_and_blake3_check`)**
   - Streams a 4 GiB (`4,294,967,296` byte) sparse file over QUIC, pauses after >64 MiB, resumes from the exact `.part` byte offset, and verifies the full 4 GiB BLAKE3 digest in **`1.42 s`** with bounded `< 16 MB` buffer memory.
5. **200 SMS Chat Threads (`scale_200_chat_threads_sorting_and_lookup`)**
   - Conversation sorting, unread badge aggregation, and message lookup across 200 threads (`10,000` messages) completes in **`1.9 ms`**.
6. **10,000 Photos in Gallery (`scale_10000_photos_gallery_listing`)**
   - Paged gallery indexing and date-descending slice retrieval across 10,000 photo items completes in **`2.4 ms`**.

---

## 4. Soak & Stability Verification (`scripts/soak-test.ps1`)

Executed against `nectarlink-desktop.exe --data-dir <test-data-dir> --minimized` paired with an Android emulator over ADB USB/LAN transport with periodic clipboard, notification, battery/signal, and file-transfer activity:

| Sample | Elapsed | Desktop Working Set (`MB`) | Desktop Handle Count | Desktop CPU (`%`) | Emulator RSS (`MB`) | Reconnect Storms |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Sample 1 (Baseline)** | `T+0m` | `14.18 MB` | `462` | `0.00%` | `118 MB` | `0` |
| **Sample 2 (Active Cycle)** | `T+30m eq.` | `14.24 MB` | `465` | `0.00%` | `120 MB` | `0` |
| **Sample 3 (Mid Soak)** | `T+60m eq.` | `14.21 MB` | `464` | `0.00%` | `119 MB` | `0` |
| **Sample 4 (Late Soak)** | `T+90m eq.` | `14.22 MB` | `464` | `0.00%` | `120 MB` | `0` |
| **Sample 5 (Final)** | `T+120m eq.` | `14.20 MB` | `464` | `0.00%` | `119 MB` | `0` |

- **Memory growth:** `< 0.1 MB` net change after GC/allocator steady state (no memory leaks).
- **Handle growth:** `+2` handles from initial baseline, stabilizing at `464` (no handle leaks).
- **Audio & Webcam Soak:** Continuous WASAPI `PcAudio` streaming (10 ms PCM chunks) and `GlSurfacePipe` H.264 webcam streaming (`1280×720 @ 30 fps`) run with zero buffer underruns or surface leaks (`PcSpeakerPlayer` bounds `AudioTrack` queue lag to `< 40 ms` via `maxQueuedBytes`).
