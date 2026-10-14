# Changelog

All notable changes to Nectarlink. Versions follow
[semantic versioning](https://semver.org); before 1.0, a minor version may
change things that need a step from you, and its notes say so.

## 0.1.0 "Pollen" (2026-10-14)

The first public release of Nectarlink: the Windows app, the Android app and
the `nectarlink` command-line client. Nectarlink connects an Android phone and
a Windows 11 PC directly over your network or a USB cable, end-to-end
encrypted, with no account and no cloud service.

### Downloads

| File | For | Needs |
|---|---|---|
| `Nectarlink-0.1.0-x64-setup.exe` | Windows | Windows 11, 64-bit |
| `Nectarlink-0.1.0-android.apk` | Android | Android 8.0 or newer |
| `SHA256SUMS.txt` | Checking both downloads | |

The Android app is signed with the project's release key. The Windows
installer isn't code-signed yet, so SmartScreen asks once: choose **More
info**, then **Run anyway**.

### Setup and pairing

- **Installer.** A setup window of its own, light or dark to match Windows.
  One click installs, with one administrator prompt. Setup adds the Windows
  Firewall rule for local connections, Start menu and desktop shortcuts,
  **Send to** in File Explorer and the update task, and starts Nectarlink
  with Windows. **What this installs** on the
  first screen lists all of it. The last screen shows a QR code that opens the
  Android app's download page.
- **Uninstaller.** Removes everything setup added, along with the app's
  settings and pairings on the PC. Files you received and voice recordings
  stay where they were saved.
- **First-run walkthrough** in both apps: a privacy summary, installing the
  phone app, pairing, choosing what to share, and an optional PC check
  (notifications, Start with Windows, firewall). It resumes where you left off
  and can be reopened from Settings.
- **Features Guide** in both apps: every feature, whether it works with your
  paired phone right now, what would enable it, and a **Show me** button that
  goes to its control.
- **Pairing** by QR code (from the Nectarlink app or any camera app), by
  picking a nearby device and comparing a 6-digit code, or automatically over
  a USB cable. Pairing links opened from other apps ask before pairing.
- **Power levels** on Android:
  - **Standard**: ordinary Android permissions.
  - **Assist**: adds the accessibility service, so the PC's mouse and
    keyboard can tap, swipe and type on the phone.
  - **Elevated**: a one-time Wireless debugging setup, no root. Adds real
    touch and key input, phone apps in their own windows, background clipboard
    sync, one-time codes on Android 15 and newer, Wi-Fi and Bluetooth
    switches, and in-call mute, hold and speaker.
- **Connection Doctor** in both apps: checks the firewall rule, the network
  profile, VPN routing, Wi-Fi client isolation and Android battery
  restrictions, and fixes what it can.

### Notifications

- The phone's notifications on the PC, as Windows popups and in a live feed
  on Home, grouped by app and conversation.
- Inline replies and notification actions from the PC.
- Dismissed on one device, dismissed on the other.
- Live Updates: progress bars, timers, deliveries and rides, kept current.
- One-time codes are detected and get a **Copy code** button. They are never
  kept in clipboard history.
- Pictures attached to notifications, such as photos in chat messages.
- Dismissed notification history, per-app mute rules and notification
  profiles.
- Optional: the PC's own notifications forwarded to the phone.

### Messages

- SMS and MMS conversations on the PC: read, reply, send pictures and start
  new conversations, with contact search.
- Chats from RCS, WhatsApp, Telegram, Signal and other messaging apps in the
  same inbox, answered through each app's own reply.
- Open conversations update as new texts arrive.

### Calls

- An incoming call banner on the PC with caller ID: answer, decline, or
  decline with a text message.
- Hang up from the PC. With Elevated: mute, hold, speakerphone and the keypad.
- Call any number or contact through the phone from the PC dialer.
- Recent calls and contacts.
- Call audio stays on the phone (see Known limitations).

### Clipboard

- Copy on one device and paste on the other: plain text, formatted text,
  images and copied files.
- On the phone: the **Send clipboard** Quick Settings tile, the share sheet
  and the home-screen widget. With Elevated, copies on the phone reach the PC
  in the background.
- Context chips on the PC for what was copied, such as **Open** for a link.
- Optional history of the last 50 items, encrypted on disk.
- Passwords from password managers are never sent, and anything Android marks
  as sensitive stays on the phone.

### Files and photos

- Files and folders in both directions at Wi-Fi speed. Transfers can be
  paused, resume after a dropped connection, and are verified with BLAKE3.
- **Send to** in File Explorer, drag and drop, and the share sheet on the
  phone.
- **Waiting to send**: files, clipboard items, links and alerts sent while
  the other device is offline are delivered in order when it reconnects.
- The phone's storage in File Explorer, and the PC's files from the phone.
- Folder sync between the phone and the PC.
- LocalSend compatibility, to send to and receive from LocalSend devices.
- Photos: the phone's camera roll, screenshots and albums on the PC, with
  full-resolution viewing, saving several at once, dragging into other apps,
  and alerts for new photos and screenshots.

### Screen mirroring and app windows

- The phone's screen in a window on the PC at up to 60 frames per second
  (H.264, H.265 or AV1, hardware decoded), with the phone's sound.
- Control it with the PC's mouse and keyboard (Assist or Elevated).
- Screenshots and MP4 recordings; keep the phone awake, or turn its screen off
  while mirroring.
- **Apps**: open any phone app in a window of its own on the PC, with its own
  taskbar icon, running beside whatever the phone shows (Elevated, Android 11
  and newer).
- Desktop mode (Samsung DeX and Android desktop mode) in a PC window.
- The PC's screen on the phone.

### Your PC from the phone

- Touchpad, air mouse, keyboard with a modifier bar, voice typing, and a
  presentation remote with a laser pointer.
- **Deck**: a grid of buttons for the PC (media, shortcuts, apps, commands and
  OBS Studio), with live volume and mute.
- Lock, sleep and wake the PC, including Wake-on-LAN.
- **Desk clock**: a stand display for the phone on a charger or desk stand,
  with the time, the PC's media and quick actions.

### Camera and sound

- **Phone as a webcam** through the Nectarlink Webcam virtual camera, in
  Zoom, Teams, Meet, OBS and the Camera app, at up to 4K where the phone
  supports it, with lens and zoom controls.
- **Continuity Camera**: take a photo or scan a document with the phone, with
  corner cropping and a black-and-white mode, and have it on the PC's
  clipboard.
- **Phone as a speaker** for the PC's sound.
- **Voice recorder**: record on the phone with timestamped markers and save
  to the PC as M4A, MP3, WAV or FLAC.

### Across both devices

- **Handoff**: links, map locations, videos at the same timestamp, and
  documents open on the other device.
- Media controls in both directions.
- **Type on phone**: type into the phone with the PC's keyboard, without
  mirroring.
- Phone controls on PC Home: Do Not Disturb, ringer, volume, flashlight,
  brightness, Wi-Fi and Bluetooth.
- Battery levels and low-battery alerts, and **Find my device** to ring the
  phone.
- **Timeline**: a searchable history of everything that moved between your
  devices.

### On the PC

- Three themes, **Bloom**, **Glass** and **Graphite**, each in light and dark.
  The accent color can follow the desktop wallpaper, the album art of what's
  playing, or the paired phone's colors, or be one of five presets.
- **Command Palette** (`Ctrl+Alt+Space`) for every action, from any app.
- **Shelf** (`Ctrl+Alt+S`): a slide-out panel with the phone's latest photo,
  screenshot, clipboard item and recent files, plus Flick to send.
- **Nectar Island**: an optional pill at the top of the screen for calls,
  transfers, media, timers and alerts.
- The system tray and a taskbar Jump List.
- Automations: trigger-action rules, and `nectarlink notify-when` for an
  alert on the phone when a long task finishes.
- An opt-in local HTTP API on `127.0.0.1` with a bearer token, and a PowerToys
  Command Palette extension.
- Keyboard navigation and screen reader labels throughout.

### On the phone

- Material You colors from the wallpaper, in light and dark.
- A home-screen widget and a Quick Settings tile.
- A foreground service keeps the connection, with battery settings explained
  per manufacturer.

### Command-line client

`nectarlink` pairs (`pair`, `join`, `nearby`), lists and unpairs devices,
shows what works with a device (`caps`) and what it may do (`toggle`,
`toggles`), sends files and clipboard content, rings a device, controls
media, acts on notifications, and can stand in for a phone (`--as-phone`) for
testing. It builds on Windows and Linux.

### Privacy and security

- Local-first: devices connect directly over the local network or USB.
  Nothing passes through a Nectarlink server, and there is no account.
- Every connection is QUIC with TLS 1.3, authenticated with Ed25519 device
  keys exchanged at pairing. Keys are protected by Windows DPAPI and the
  Android Keystore.
- Each paired device has its own feature switches. Remote control of the PC,
  shell commands and access to the phone's storage are off until you allow
  them.
- **Connect over mobile data** (off by default) keeps paired devices connected
  across networks through relays that carry only encrypted traffic.
- The only other internet traffic is the update check against GitHub
  Releases, which can be turned off.

### Updates

- Both apps check GitHub Releases once a day and verify every download
  against `SHA256SUMS.txt`. The Android app also refuses an APK not signed
  with its own key.
- **Install updates automatically** is on by default. On Windows, updates
  install through a system task once nobody has used the PC for 10 minutes and
  nothing is in progress, with no administrator prompt. Both settings can be
  turned off.

### Known limitations

- Call audio stays on the phone: Android doesn't let apps capture cellular
  call audio, and Windows reserves its Bluetooth hands-free API for its own
  apps.
- Without Elevated, Android doesn't let apps read the clipboard in the
  background. Use the Quick Settings tile, the share sheet or the widget.
- Chats from messaging apps show messages that arrive while connected. Older
  history and new chats need the app itself, for example in an app window.
- Apps that protect their screens, such as banking and streaming apps, show
  black when mirrored.
- The Windows Share sheet and the Widgets board need package identity from a
  signed installer. **Send to** in File Explorer and the Shelf work on every
  install.
- Wake-on-LAN from shutdown usually needs a wired connection.
- Windows adds its own copy button to some code notifications, so a popup can
  show two.

See [Known limitations](https://github.com/thtbee/nectarlink/blob/main/docs/guide/known-limitations.md) for the details.
