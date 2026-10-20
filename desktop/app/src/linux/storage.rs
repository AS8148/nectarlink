// SPDX-License-Identifier: GPL-3.0-or-later
//! Linux phone storage synchronization (`~/Nectarlink/<Phone name>` and GTK3 bookmarks).

use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, UNIX_EPOCH},
};

use nectarlink_core::{DeviceId, LinkState, NodeEvent, StorageEntry};

use crate::core_host;
use crate::state::{Changes, CoreStatus};
use crate::storage::{base_sync_dir, relative_path_under_root, sanitize_folder_name};

const LINUX_SYNC_INTERVAL: Duration = Duration::from_secs(4);
const LINUX_MAX_SYNC_DEPTH: usize = 5;
const STREAM_READ_CHUNK_BYTES: usize = 64 * 1024;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Encodes a local directory path as a `file://` URI suitable for `~/.config/gtk-3.0/bookmarks`.
pub fn format_file_uri(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let mut encoded = String::with_capacity(raw.len() + 16);
    encoded.push_str("file://");
    for b in raw.as_bytes() {
        match *b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'/'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b':'
            | b'@'
            | b'('
            | b')' => encoded.push(*b as char),
            other => {
                encoded.push_str(&format!("%{other:02X}"));
            }
        }
    }
    encoded
}

/// Formats a single GTK3 bookmark entry (`file://<encoded_path> <DeviceName> (NectarLink)`).
pub fn format_gtk_bookmark_line(mount_path: &Path, device_name: &str) -> String {
    let uri = format_file_uri(mount_path);
    let label = sanitize_folder_name(device_name);
    format!("{uri} {label} (NectarLink)")
}

/// Upserts a NectarLink device bookmark line in the contents of `~/.config/gtk-3.0/bookmarks`.
pub fn upsert_gtk_bookmarks_content(existing: &str, mount_path: &Path, device_name: &str) -> String {
    let uri = format_file_uri(mount_path);
    let prefix = format!("{uri} ");
    let new_line = format_gtk_bookmark_line(mount_path, device_name);
    let mut lines: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in existing.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == uri || trimmed.starts_with(&prefix) {
            if !replaced {
                lines.push(new_line.clone());
                replaced = true;
            }
        } else {
            lines.push(trimmed.to_owned());
        }
    }
    if !replaced {
        lines.push(new_line);
    }
    if lines.is_empty() { String::new() } else { format!("{}\n", lines.join("\n")) }
}

/// Removes a NectarLink device bookmark line matching `mount_path` from `~/.config/gtk-3.0/bookmarks` content.
pub fn remove_gtk_bookmarks_content(existing: &str, mount_path: &Path) -> String {
    let uri = format_file_uri(mount_path);
    let prefix = format!("{uri} ");
    let lines: Vec<&str> = existing
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty() && *line != uri && !line.starts_with(&prefix))
        .collect();
    if lines.is_empty() { String::new() } else { format!("{}\n", lines.join("\n")) }
}

/// Converts a local path inside `mount_root` into a normalized forward-slash relative remote path
/// (`""` for `mount_root` itself). Returns `None` if `local_path` is outside `mount_root` or
/// contains parent/current directory traversal components.
pub fn to_remote_rel_path(mount_root: &Path, local_path: &Path) -> Option<String> {
    if let Ok(stripped) = local_path.strip_prefix(mount_root) {
        let mut parts: Vec<String> = Vec::new();
        for comp in stripped.components() {
            match comp {
                Component::Normal(os) => {
                    let s = os.to_string_lossy();
                    if !s.is_empty() {
                        parts.push(s.into_owned());
                    }
                }
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
            }
        }
        return Some(parts.join("/"));
    }
    relative_path_under_root(mount_root, local_path)
}

#[derive(Debug)]
struct MountedDevice {
    device: DeviceId,
    device_name: String,
    mount_path: PathBuf,
    stop: Arc<AtomicBool>,
    wake: Arc<(Mutex<bool>, Condvar)>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl MountedDevice {
    fn notify_sync(&self) {
        let (lock_flag, cvar) = &*self.wake;
        let mut pending = lock(lock_flag);
        *pending = true;
        cvar.notify_all();
    }

    fn stop_and_join(&self) {
        self.stop.store(true, Ordering::Release);
        self.notify_sync();
        if let Some(handle) = lock(&self.worker).take() {
            let _ = handle.join();
        }
    }
}

static MOUNTS: OnceLock<Mutex<HashMap<DeviceId, Arc<MountedDevice>>>> = OnceLock::new();
static LINUX_RECONCILING: Mutex<()> = Mutex::new(());

fn mounts() -> MutexGuard<'static, HashMap<DeviceId, Arc<MountedDevice>>> {
    let map = MOUNTS.get_or_init(|| Mutex::new(HashMap::new()));
    lock(map)
}

fn get_mounted_device(device: &DeviceId) -> Option<Arc<MountedDevice>> {
    mounts().get(device).cloned()
}

pub fn mounted_path_for(device: &DeviceId) -> Option<PathBuf> {
    get_mounted_device(device).map(|m| m.mount_path.clone())
}

fn gtk_bookmarks_path() -> Option<PathBuf> {
    dirs::config_dir()
        .filter(|p| p.is_absolute())
        .or_else(|| dirs::home_dir().filter(|p| p.is_absolute()).map(|h| h.join(".config")))
        .map(|cfg| cfg.join("gtk-3.0").join("bookmarks"))
}

fn register_gtk_bookmark(mount_path: &Path, device_name: &str) {
    let Some(bookmarks_file) = gtk_bookmarks_path() else { return };
    if let Some(parent) = bookmarks_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let existing = std::fs::read_to_string(&bookmarks_file).unwrap_or_default();
    let updated = upsert_gtk_bookmarks_content(&existing, mount_path, device_name);
    let _ = std::fs::write(&bookmarks_file, updated);
}

fn unregister_gtk_bookmark(mount_path: &Path) {
    let Some(bookmarks_file) = gtk_bookmarks_path() else { return };
    let Ok(existing) = std::fs::read_to_string(&bookmarks_file) else { return };
    let updated = remove_gtk_bookmarks_content(&existing, mount_path);
    let _ = std::fs::write(&bookmarks_file, updated);
}

pub fn start() {
    drop(mounts());
    core_host::host().hub.subscribe(
        Changes::DEVICES | Changes::STATUS | Changes::CAPABILITIES | Changes::TOGGLES,
        || {
            sync();
            true
        },
    );
    sync();
}

pub fn sync() {
    let host = core_host::host();
    let Some(node) = core_host::node() else { return };
    let ready = host.hub.read(|s| matches!(s.status, Some(CoreStatus::Ready { .. })));
    if !ready {
        return;
    }
    let devices = host.hub.read(|s| {
        s.devices
            .iter()
            .map(|d| (d.id, d.info.name.clone(), matches!(d.link, LinkState::Online { .. })))
            .collect::<Vec<_>>()
    });
    let mut desired: Vec<(DeviceId, String, bool)> = Vec::new();
    let mut disabled: Vec<(DeviceId, String)> = Vec::new();
    for (id, name, online) in devices {
        let enabled = node
            .device_toggles(id)
            .ok()
            .and_then(|t| t.into_iter().find(|(k, _)| *k == "storage").map(|(_, on)| on))
            .unwrap_or(false);
        if enabled {
            desired.push((id, name, online));
        } else {
            disabled.push((id, name));
        }
    }

    let _ = std::thread::Builder::new().name("storage-linux-reconcile".into()).spawn(move || {
        let _guard = lock(&LINUX_RECONCILING);
        reconcile(&desired, &disabled);
    });
}

fn reconcile(desired: &[(DeviceId, String, bool)], disabled: &[(DeviceId, String)]) {
    let Some(base) = base_sync_dir() else { return };

    for (id, name) in disabled {
        unmount_device(*id, Some(name));
    }

    let desired_ids: HashSet<DeviceId> = desired.iter().map(|(id, _, _)| *id).collect();
    let stale: Vec<DeviceId> = mounts().keys().filter(|id| !desired_ids.contains(id)).copied().collect();
    for id in stale {
        unmount_device(id, None);
    }

    for (device, name, online) in desired {
        let folder_name = sanitize_folder_name(name);
        let mount_path = base.join(&folder_name);
        if std::fs::create_dir_all(&mount_path).is_err() {
            continue;
        }
        register_gtk_bookmark(&mount_path, name);

        let existing = get_mounted_device(device);
        if let Some(mounted) = existing {
            if mounted.mount_path == mount_path && mounted.device_name == *name {
                if *online {
                    mounted.notify_sync();
                }
                continue;
            }
            unmount_device(*device, None);
        }

        let stop = Arc::new(AtomicBool::new(false));
        let wake = Arc::new((Mutex::new(*online), Condvar::new()));
        let mounted = Arc::new(MountedDevice {
            device: *device,
            device_name: name.clone(),
            mount_path,
            stop,
            wake,
            worker: Mutex::new(None),
        });
        let worker_mount = Arc::clone(&mounted);
        if let Ok(handle) = std::thread::Builder::new()
            .name(format!("storage-sync-{}", &device.to_string()[..8]))
            .spawn(move || mount_worker(&worker_mount))
        {
            *lock(&mounted.worker) = Some(handle);
            mounts().insert(*device, mounted);
        }
    }
}

fn unmount_device(device: DeviceId, fallback_name: Option<&str>) {
    let removed = mounts().remove(&device);
    if let Some(mounted) = removed {
        mounted.stop_and_join();
        unregister_gtk_bookmark(&mounted.mount_path);
    } else if let Some(name) = fallback_name
        && let Some(base) = base_sync_dir()
    {
        let mount_path = base.join(sanitize_folder_name(name));
        unregister_gtk_bookmark(&mount_path);
    }
}

pub fn on_event(event: &NodeEvent) {
    match event {
        NodeEvent::DeviceRemoved(device) => {
            let device = *device;
            let _ = std::thread::Builder::new().name("storage-linux-unpair".into()).spawn(move || {
                let _guard = lock(&LINUX_RECONCILING);
                unmount_device(device, None);
            });
        }
        NodeEvent::DeviceAdded(_)
        | NodeEvent::PeerInfoChanged { .. }
        | NodeEvent::Capabilities(_)
        | NodeEvent::LinkChanged { .. } => {
            sync();
        }
        NodeEvent::StorageChanged { device, .. } => {
            if let Some(mounted) = get_mounted_device(device) {
                mounted.notify_sync();
            }
        }
        _ => {}
    }
}

pub fn shutdown() {
    let active: Vec<Arc<MountedDevice>> = mounts().drain().map(|(_, v)| v).collect();
    for mounted in active {
        mounted.stop_and_join();
        unregister_gtk_bookmark(&mounted.mount_path);
    }
}

pub fn unregister_all() {
    shutdown();
    if let Some(bookmarks_file) = gtk_bookmarks_path()
        && let Ok(existing) = std::fs::read_to_string(&bookmarks_file)
    {
        let filtered: Vec<&str> = existing
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.is_empty() && !line.ends_with(" (NectarLink)"))
            .collect();
        let updated = if filtered.is_empty() { String::new() } else { format!("{}\n", filtered.join("\n")) };
        let _ = std::fs::write(&bookmarks_file, updated);
    }
    if let Some(base) = base_sync_dir() {
        let _ = std::fs::remove_dir(&base);
    }
}

fn file_mtime_ms(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn set_local_file_mtime_ms(path: &Path, unix_ms: i64) {
    use std::os::unix::fs::OpenOptionsExt;
    if unix_ms <= 0 {
        return;
    }
    let st = UNIX_EPOCH + Duration::from_millis(unix_ms as u64);
    if let Ok(file) = std::fs::OpenOptions::new().write(true).custom_flags(libc::O_NOFOLLOW).open(path) {
        let _ = file.set_modified(st);
    }
}

fn collect_local_files(
    mount_root: &Path,
    dir: &Path,
    depth: usize,
    out: &mut HashMap<String, (PathBuf, u64, i64)>,
) {
    if depth > LINUX_MAX_SYNC_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();
        if name_str.starts_with('.') {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            collect_local_files(mount_root, &path, depth + 1, out);
        } else if meta.is_file()
            && let Some(rel) = to_remote_rel_path(mount_root, &path)
            && !rel.is_empty()
        {
            out.insert(rel, (path, meta.len(), file_mtime_ms(&meta)));
        }
    }
}

fn mount_worker(mounted: &Arc<MountedDevice>) {
    // Snapshot of known synced files: remote_rel_path -> (size, local_mtime_ms, remote_mtime_ms)
    let mut known_files: HashMap<String, (u64, i64, i64)> = HashMap::new();

    while !mounted.stop.load(Ordering::Acquire) {
        let is_online = core_host::host().hub.read(|s| {
            s.devices
                .iter()
                .find(|d| d.id == mounted.device)
                .is_some_and(|d| matches!(d.link, LinkState::Online { .. }))
        });

        if is_online && let Some(node) = core_host::node() {
            core_host::host().runtime.block_on(run_sync_cycle(mounted, &node, &mut known_files));
        }

        if mounted.stop.load(Ordering::Acquire) {
            break;
        }

        let (lock_flag, cvar) = &*mounted.wake;
        let mut pending = lock(lock_flag);
        if !*pending {
            let (guard, _) =
                cvar.wait_timeout(pending, LINUX_SYNC_INTERVAL).unwrap_or_else(|e| e.into_inner());
            pending = guard;
        }
        *pending = false;
    }
}

async fn stream_remote_file_to_local(
    mounted: &MountedDevice,
    node: &nectarlink_core::Node,
    remote_rel: &str,
    local_child: &Path,
) -> Option<u64> {
    use std::os::unix::fs::OpenOptionsExt;

    if let Some(parent) = local_child.parent() {
        std::fs::create_dir_all(parent).ok()?;
        if std::fs::symlink_metadata(parent).is_ok_and(|m| m.file_type().is_symlink()) {
            return None;
        }
    }
    if std::fs::symlink_metadata(local_child).is_ok_and(|m| m.file_type().is_symlink()) {
        return None;
    }
    let file_name = local_child.file_name()?.to_string_lossy();
    let part_path = local_child.with_file_name(format!(".{file_name}.nectarlink-part"));
    let (meta, mut recv) = node.storage_read_stream(mounted.device, remote_rel, 0, None).await.ok()?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .custom_flags(libc::O_NOFOLLOW)
        .mode(0o600)
        .open(&part_path)
        .ok()?;
    let mut written = 0u64;
    loop {
        if mounted.stop.load(Ordering::Acquire) {
            drop(file);
            let _ = std::fs::remove_file(&part_path);
            return None;
        }
        match recv.read_chunk(STREAM_READ_CHUNK_BYTES).await {
            Ok(Some(chunk)) => {
                if written.saturating_add(chunk.len() as u64) > meta.length {
                    drop(file);
                    let _ = std::fs::remove_file(&part_path);
                    return None;
                }
                if file.write_all(&chunk).is_err() {
                    drop(file);
                    let _ = std::fs::remove_file(&part_path);
                    return None;
                }
                written += chunk.len() as u64;
            }
            Ok(None) => break,
            Err(_) => {
                drop(file);
                let _ = std::fs::remove_file(&part_path);
                return None;
            }
        }
    }
    if written != meta.length || file.flush().is_err() {
        drop(file);
        let _ = std::fs::remove_file(&part_path);
        return None;
    }
    drop(file);
    if std::fs::rename(&part_path, local_child).is_err() {
        let _ = std::fs::remove_file(&part_path);
        return None;
    }
    Some(written)
}

async fn run_sync_cycle(
    mounted: &MountedDevice,
    node: &nectarlink_core::Node,
    known_files: &mut HashMap<String, (u64, i64, i64)>,
) {
    // 1. Traverse remote directory tree via BFS and mirror directories/files locally first.
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    queue.push_back((String::new(), 0));
    let mut seen_remote_files: HashSet<String> = HashSet::new();
    let mut traversal_complete = true;

    while let Some((dir_rel, depth)) = queue.pop_front() {
        if mounted.stop.load(Ordering::Acquire) {
            return;
        }
        let Ok(entries): Result<Vec<StorageEntry>, _> =
            node.storage_list(mounted.device, dir_rel.as_str()).await
        else {
            traversal_complete = false;
            continue;
        };

        for entry in entries {
            if mounted.stop.load(Ordering::Acquire) {
                return;
            }
            let name = entry.name.trim_matches('/');
            if name.is_empty()
                || name == "."
                || name == ".."
                || name.starts_with('.')
                || name.contains('/')
                || name.contains('\\')
                || name.chars().any(char::is_control)
            {
                continue;
            }
            let child_rel = if dir_rel.is_empty() { name.to_owned() } else { format!("{dir_rel}/{name}") };
            let local_child = child_rel.split('/').fold(mounted.mount_path.clone(), |acc, seg| acc.join(seg));
            if to_remote_rel_path(&mounted.mount_path, &local_child).is_none() {
                continue;
            }
            if std::fs::symlink_metadata(&local_child).is_ok_and(|m| m.file_type().is_symlink()) {
                continue;
            }

            if entry.is_dir {
                let _ = std::fs::create_dir_all(&local_child);
                if depth + 1 < LINUX_MAX_SYNC_DEPTH {
                    queue.push_back((child_rel, depth + 1));
                }
            } else {
                seen_remote_files.insert(child_rel.clone());
                let needs_download = match known_files.get(&child_rel) {
                    None => !local_child.exists(),
                    Some((known_size, _, known_remote_mtime)) => {
                        !local_child.exists()
                            || entry.size != *known_size
                            || (entry.modified > 0
                                && *known_remote_mtime > 0
                                && (entry.modified - *known_remote_mtime).abs() > 1_000)
                    }
                };
                if needs_download {
                    if let Some(written) =
                        stream_remote_file_to_local(mounted, node, &child_rel, &local_child).await
                    {
                        set_local_file_mtime_ms(&local_child, entry.modified);
                        let local_mtime = std::fs::metadata(&local_child)
                            .map(|m| file_mtime_ms(&m))
                            .unwrap_or(entry.modified);
                        known_files.insert(child_rel, (written, local_mtime, entry.modified));
                    }
                } else if let Ok(meta) = std::fs::metadata(&local_child) {
                    known_files
                        .entry(child_rel)
                        .or_insert_with(|| (meta.len(), file_mtime_ms(&meta), entry.modified));
                }
            }
        }
    }

    // 2. Remove local cache files that were deleted on the phone since the last full pass.
    if traversal_complete {
        let removed_remotely: Vec<String> =
            known_files.keys().filter(|rel| !seen_remote_files.contains(*rel)).cloned().collect();
        for rel in removed_remotely {
            let local_child = rel.split('/').fold(mounted.mount_path.clone(), |acc, seg| acc.join(seg));
            let _ = std::fs::remove_file(&local_child);
            known_files.remove(&rel);
        }
    }

    // 3. Inspect local directory tree for user additions or edits and push them to the phone.
    let mut local_files: HashMap<String, (PathBuf, u64, i64)> = HashMap::new();
    collect_local_files(&mounted.mount_path, &mounted.mount_path, 0, &mut local_files);
    known_files.retain(|rel, _| local_files.contains_key(rel));

    for (rel, (local_path, size, local_mtime)) in &local_files {
        if mounted.stop.load(Ordering::Acquire) {
            return;
        }
        let changed = match known_files.get(rel) {
            None => !seen_remote_files.contains(rel),
            Some((known_size, known_local_mtime, _)) => {
                *size != *known_size || (*local_mtime - *known_local_mtime) > 1_000
            }
        };
        if changed {
            if let Some(parent_rel) = Path::new(rel).parent() {
                let parent_str = parent_rel.to_string_lossy().replace('\\', "/");
                if !parent_str.is_empty() {
                    let _ = node.storage_mkdir(mounted.device, &parent_str).await;
                }
            }
            if node.storage_write(mounted.device, rel.as_str(), local_path).await.is_ok() {
                known_files.insert(rel.clone(), (*size, *local_mtime, *local_mtime));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gtk_bookmark_formatting_upsert_and_removal_preserve_unrelated_entries() {
        let mount = Path::new("/home/alice/Nectarlink/Pixel 9 Pro");
        let line = format_gtk_bookmark_line(mount, "Pixel 9 Pro");
        assert_eq!(line, "file:///home/alice/Nectarlink/Pixel%209%20Pro Pixel 9 Pro (NectarLink)");

        let initial = "file:///home/alice/Documents Documents\nfile:///home/alice/Downloads Downloads\n";
        let with_phone = upsert_gtk_bookmarks_content(initial, mount, "Pixel 9 Pro");
        assert!(with_phone.contains("file:///home/alice/Documents Documents\n"));
        assert!(with_phone.contains("file:///home/alice/Downloads Downloads\n"));
        assert!(with_phone.contains(&line));

        // Upserting again with updated label replaces in-place without duplicating.
        let updated_phone = upsert_gtk_bookmarks_content(&with_phone, mount, "Pixel 9 Pro Renamed");
        assert_eq!(
            updated_phone
                .lines()
                .filter(|l| l.starts_with("file:///home/alice/Nectarlink/Pixel%209%20Pro"))
                .count(),
            1
        );
        assert!(updated_phone.contains("Pixel 9 Pro Renamed (NectarLink)"));

        let cleaned = remove_gtk_bookmarks_content(&updated_phone, mount);
        assert_eq!(cleaned, initial);
    }

    #[test]
    fn to_remote_rel_path_converts_linux_mount_paths_to_slash_relative_paths() {
        let root = Path::new("/home/alice/Nectarlink/Pixel 9");
        assert_eq!(to_remote_rel_path(root, root), Some(String::new()));
        assert_eq!(
            to_remote_rel_path(root, Path::new("/home/alice/Nectarlink/Pixel 9/DCIM/Camera/IMG_001.jpg")),
            Some("DCIM/Camera/IMG_001.jpg".to_owned())
        );
        assert_eq!(to_remote_rel_path(root, Path::new("/home/alice/Nectarlink/OtherPhone/DCIM")), None);
    }
}
