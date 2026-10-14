# Performance Budgets

Nectarlink treats startup latency, idle resource consumption, UI frame pacing, and bulk transfer throughput as regression-tested engineering budgets. Every metric below is taken from [`docs/PERFORMANCE.md`](../PERFORMANCE.md) and verified on hardware (`Windows 11 desktop + Samsung SM-S908E Android 14`).

## Target Budgets vs Measured Results

### Windows Desktop (`nectarlink-desktop`)

| Metric | Budget | Measured | Verification Method |
|---|---|---|---|
| Cold start to first visible frame | `< 800 ms` | **`310 ms - 424 ms`** (median `310 ms`) | `[PERF] cold_start_first_frame_ms` instrumentation in `desktop/app/src/main.rs` |
| Cold start (`--minimized` to system tray) | `< 400 ms` | **`145 ms`** | Process start to event-loop ready in tray mode |
| Idle CPU (minimized to tray, 60 s window) | `< 0.2%` | **`0.00%`** (`0 ms` CPU delta over 60 s) | Windows process CPU time counter (`Get-Process nectarlink-desktop`) |
| Idle CPU (window open, connected, 30 s) | `< 0.5%` | **`0.039%`** (`140.6 ms` CPU over 30 s) | Windows process CPU time counter |
| Idle working set (minimized to tray) | `< 50 MB` | **`14.2 MB`** (`54.0 MB` private bytes) | `EmptyWorkingSet` on `hide_to_tray()` + D3D11 resource release |
| Idle working set (window open, all pages visited) | `< 170 MB` | **`165.32 MB`** | Working set after cycling through all 14 navigation pages |
| Window resize responsiveness (20 steps) | `< 350 ms` CPU | **`296.9 ms`** CPU (`0 MB` working set leak) | Win32 `MoveWindow` 20-step sweep from `1040x680` to `1520x960` |
| Command Palette (`Ctrl+Alt+Space`) open + filter | `< 30 ms` | **`8.4 ms`** | Key press to filtered QML list layout completion |
| Phone-to-PC screen mirror glass-to-glass latency | `< 45 ms` | **`22 ms - 31 ms`** (`60 fps` 1080p H.264) | High-speed camera timestamp delta over LAN/USB |
| PC-to-phone audio streaming latency | `< 40 ms` | **`22 ms - 36 ms`** (`48 kHz` stereo) | WASAPI loopback capture → `NLPA` QUIC datagrams → Android `AudioTrack` |

### Transport and Scale Benchmarks (`nectarlink-core`)

| Metric | Budget | Measured | Verification Method |
|---|---|---|---|
| Bulk file transfer throughput (100 MiB) | `> 80 MB/s` | **`215 MB/s`** (`0.46 s` for 100 MiB) | `cargo test -p nectarlink-core --release --test two_devices file_transfer_100mb_throughput` |
| 4 GiB transfer + mid-stream pause/resume + BLAKE3 | `< 5.0 s` | **`1.42 s`** | `cargo test -p nectarlink-core --release --test scale_tests scale_4gb_transfer_pause_resume` |
| 10,000-file directory batch transfer | `< 30 s` | **`2.19 s`** | `cargo test -p nectarlink-core --release --test scale_tests scale_10k_small_files_batch` |
| 50,000-entry SQLite timeline insert + query + search | `< 500 ms` query | **`817 ms`** insert / **`0.46 ms`** page query / **`15.42 ms`** substring search | `cargo test -p nectarlink-core --release --test scale_tests scale_50k_timeline_entries` |
| 10,000-contact synchronization + lookup | `< 5.0 s` | **`0.55 s`** | `cargo test -p nectarlink-core --release --test scale_tests scale_10k_contacts_sync` |
| 5,000-photo gallery index + pagination | `< 2.0 s` | **`0.18 s`** | `cargo test -p nectarlink-core --release --test scale_tests scale_5k_photos_pagination` |
| 2-hour continuous multi-feature soak test | `0` disconnects, `< 5 MB` RSS delta | **`0` disconnects**, **`+1.8 MB`** RSS delta | Continuous notifications, clipboard, media state, and periodic file transfers |

### Android Client (`app.nectarlink.android`)

| Metric | Budget | Measured | Verification Method |
|---|---|---|---|
| Cold start (`TotalTime` via `am start -W`) | `< 1,000 ms` | **`479 ms - 750 ms`** | `adb shell am start -W -S app.nectarlink.android/.MainActivity` |
| Warm start (`TotalTime` via `am start -W`) | `< 100 ms` | **`14 ms - 30 ms`** | `adb shell am start -W app.nectarlink.android/.MainActivity` |
| Memory footprint (`TOTAL PSS`) | `< 110 MB` | **`75.66 MB - 86.48 MB`** | `adb shell dumpsys meminfo app.nectarlink.android` |
| Home tab scroll jank (`gfxinfo`) | `< 5.0%` | **`0.26%`** (`1 / 382` frames) | `adb shell dumpsys gfxinfo app.nectarlink.android` |
| Settings tab scroll jank (`gfxinfo`) | `< 5.0%` | **`4.09%`** (`11 / 269` frames, `p95 = 11 ms`) | `adb shell dumpsys gfxinfo app.nectarlink.android` |
| Idle background CPU (screen off, connected) | `< 0.5%` | **`0.0% - 0.1%`** | `adb shell top -n 1 -b` |

## Key Engineering Optimizations

The budgets above are enforced through architectural choices across the Rust core, Qt desktop client, and Android app:

### 1. Asynchronous Runtime Bootstrap (`desktop/app/src/main.rs`)

The Qt `QGuiApplication` and QML engine initialize on the main thread while `nectarlink_core::Node::start` (SQLite migration, DPAPI unsealing, `iroh` QUIC socket binding, mDNS discovery, and ADB watcher startup) runs concurrently on the background Tokio runtime. The first QML frame paints in `~310 ms` before network sockets finish binding, and the `NodeBridge` attaches via an asynchronous channel once ready.

### 2. Lazy QML Page Loading (`Loader` Pattern in `Main.qml`)

Rather than instantiating all 14 feature views at startup, `desktop/app/qml/Main.qml` keeps only the primary **Overview** surface resident and loads secondary pages (`MessagesPage`, `CallsPage`, `PhotosPage`, `FilesPage`, `StoragePage`, `MirrorPage`, `WebcamPage`, `ClipboardPage`, `DeckPage`, `NotificationsPage`, `SettingsPage`, `PairWizardModal`) on demand through QML `Loader` components.

### 3. Tray Working-Set Trimming (`hide_to_tray`)

When the user closes the window to the system tray (`desktop/app/src/bridge/node_bridge.rs`):

- QML releases cached GPU textures and offscreen buffers (`releaseResources()`).
- The process invokes Win32 `K32EmptyWorkingSet(GetCurrentProcess())`, trimming the resident working set from `~165 MB` down to **`14.2 MB`** while keeping the background Tokio QUIC node responsive.

### 4. Event-Driven Zero-Polling Subsystems

To keep tray idle CPU at **`0.00%`**:

- **Send Queue Coordinator (`core/nectarlink-core/src/node.rs`)**: Uses `tokio::sync::Notify` (`queue_notify`) plus calculated `tokio::time::sleep_until` deadlines (`Store::next_queue_wake_ms`); when no items are waiting, the task parks indefinitely on `std::future::pending()`.
- **ADB USB Watcher (`core/nectarlink-core/src/adb.rs` & `src/usb.rs`)**: Maintains a single long-lived TCP connection to `127.0.0.1:5037` using the event-driven `host:track-devices` stream instead of spawning `adb.exe` on a timer.
- **QML Animations**: Indeterminate spinners and pulse animations in QML bind `running` to item visibility and active window state so Qt's scene graph ticks `0` frames per second when minimized or on another tab.

### 5. Off-UI-Thread Android Service Startup (`MainActivity.kt` & `ConnectionService.kt`)

- `NectarlinkNode` creation, Android Keystore AES-GCM unsealing, and SQLite initialization run on `Dispatchers.IO` inside `ConnectionService`.
- Compose lists (`HomeTab`, `SettingsTab`, `SendQueueSection`) pre-compute formatted timestamps and strings outside `LazyColumn` item scopes and supply stable `key` lambdas to avoid layout recomposition jank during flings.
