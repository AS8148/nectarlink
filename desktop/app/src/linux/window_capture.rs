// SPDX-License-Identifier: GPL-3.0-or-later
//! Capture a single window on Linux via in-process X11 `XGetImage`,
//! matching the public API of `desktop/app/src/win/window_capture.rs`.

use std::{
    cell::RefCell,
    ffi::{CStr, c_char, c_int, c_long, c_uint, c_ulong, c_void},
    sync::OnceLock,
};

use super::browser::{HWND, RECT, frame_of};

pub struct WindowCapture {
    hwnd: HWND,
    rgba: Vec<u8>,
}

// SAFETY: WindowCapture only holds a window XID and an owned byte buffer.
unsafe impl Send for WindowCapture {}

impl WindowCapture {
    pub fn new(hwnd: HWND) -> Result<WindowCapture, String> {
        if hwnd.0.is_null() {
            return Err("invalid window handle".to_owned());
        }
        Ok(WindowCapture { hwnd, rgba: Vec::new() })
    }

    pub fn window(&self) -> HWND {
        self.hwnd
    }

    /// Capture `self.hwnd`, optionally cropped to `crop` (in screen coordinates),
    /// and scale the result into `bgra` (`dst_w` x `dst_h`, tightly packed BGRA).
    pub fn capture(&mut self, crop: RECT, dst_w: u32, dst_h: u32, bgra: &mut Vec<u8>) -> bool {
        if dst_w == 0 || dst_h == 0 {
            return false;
        }
        let Some(frame) = frame_of(self.hwnd) else {
            return false;
        };
        let win_w = (frame.right - frame.left).max(0) as u32;
        let win_h = (frame.bottom - frame.top).max(0) as u32;
        if win_w < 32 || win_h < 32 {
            return false;
        }

        let xid = self.hwnd.0 as usize;
        if let Some((src_w, src_h)) = capture_x11_window_bgra(xid, win_w, win_h, &mut self.rgba) {
            let (cx0, cy0, cw, ch) = compute_window_crop(frame, crop, src_w, src_h);
            scale_bgra(&self.rgba, src_w, src_w * 4, cx0, cy0, cw, ch, dst_w, dst_h, bgra);
            return true;
        }

        false
    }
}

fn compute_window_crop(frame: RECT, crop: RECT, src_w: u32, src_h: u32) -> (u32, u32, u32, u32) {
    let cw = crop.right - crop.left;
    let ch = crop.bottom - crop.top;
    if cw >= 64 && ch >= 64 {
        let x0 = (crop.left - frame.left).clamp(0, src_w as i32) as u32;
        let y0 = (crop.top - frame.top).clamp(0, src_h as i32) as u32;
        let x1 = (crop.right - frame.left).clamp(x0 as i32, src_w as i32) as u32;
        let y1 = (crop.bottom - frame.top).clamp(y0 as i32, src_h as i32) as u32;
        if x1 - x0 >= 64 && y1 - y0 >= 64 {
            return (x0, y0, x1 - x0, y1 - y0);
        }
    }
    (0, 0, src_w, src_h)
}

#[repr(C)]
struct XWindowAttributes {
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    border_width: c_int,
    depth: c_int,
    visual: *mut c_void,
    root: c_ulong,
    class: c_int,
    bit_gravity: c_int,
    win_gravity: c_int,
    backing_store: c_int,
    backing_planes: c_ulong,
    backing_pixel: c_ulong,
    save_under: c_int,
    colormap: c_ulong,
    map_installed: c_int,
    map_state: c_int,
    all_event_masks: c_long,
    your_event_mask: c_long,
    do_not_propagate_mask: c_long,
    override_redirect: c_int,
    screen: *mut c_void,
}

#[repr(C)]
struct XImageFuncs {
    create_image: *mut c_void,
    destroy_image: Option<unsafe extern "C" fn(*mut XImage) -> c_int>,
    get_pixel: *mut c_void,
    put_pixel: *mut c_void,
    sub_image: *mut c_void,
    add_pixel: *mut c_void,
}

#[repr(C)]
struct XImage {
    width: c_int,
    height: c_int,
    xoffset: c_int,
    format: c_int,
    data: *mut u8,
    byte_order: c_int,
    bitmap_unit: c_int,
    bitmap_bit_order: c_int,
    bitmap_pad: c_int,
    depth: c_int,
    bytes_per_line: c_int,
    bits_per_pixel: c_int,
    red_mask: c_ulong,
    green_mask: c_ulong,
    blue_mask: c_ulong,
    obdata: *mut c_void,
    f: XImageFuncs,
}

struct X11WinCapApi {
    open_display: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    get_window_attributes: unsafe extern "C" fn(*mut c_void, c_ulong, *mut XWindowAttributes) -> c_int,
    get_image: unsafe extern "C" fn(
        *mut c_void,
        c_ulong,
        c_int,
        c_int,
        c_uint,
        c_uint,
        c_ulong,
        c_int,
    ) -> *mut XImage,
    close_display: unsafe extern "C" fn(*mut c_void) -> c_int,
    free: unsafe extern "C" fn(*mut c_void) -> c_int,
}

unsafe fn dl_sym<T>(handle: *mut c_void, name: &CStr) -> Option<T> {
    let raw = unsafe { libc::dlsym(handle, name.as_ptr()) };
    if raw.is_null() {
        return None;
    }
    Some(unsafe { std::mem::transmute_copy(&raw) })
}

fn x11_win_cap_api() -> Option<&'static X11WinCapApi> {
    static API: OnceLock<Option<X11WinCapApi>> = OnceLock::new();
    API.get_or_init(|| unsafe {
        let mut lib = std::ptr::null_mut();
        for name in [c"libX11.so.6", c"libX11.so"] {
            lib = libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if !lib.is_null() {
                break;
            }
        }
        if lib.is_null() {
            return None;
        }
        Some(X11WinCapApi {
            open_display: dl_sym(lib, c"XOpenDisplay")?,
            get_window_attributes: dl_sym(lib, c"XGetWindowAttributes")?,
            get_image: dl_sym(lib, c"XGetImage")?,
            close_display: dl_sym(lib, c"XCloseDisplay")?,
            free: dl_sym(lib, c"XFree")?,
        })
    })
    .as_ref()
}

struct ThreadDisplay(*mut c_void);

impl Drop for ThreadDisplay {
    fn drop(&mut self) {
        if !self.0.is_null()
            && let Some(api) = x11_win_cap_api()
        {
            unsafe {
                let _ = (api.close_display)(self.0);
            }
        }
    }
}

fn capture_x11_window_bgra(xid: usize, _w: u32, _h: u32, out: &mut Vec<u8>) -> Option<(u32, u32)> {
    if std::env::var_os("DISPLAY").is_none() || xid == 0 {
        return None;
    }
    std::thread_local! {
        static DISPLAY: RefCell<Option<ThreadDisplay>> = const { RefCell::new(None) };
    }
    let api = x11_win_cap_api()?;
    DISPLAY.with(|cell| unsafe {
        let mut borrow = cell.borrow_mut();
        if borrow.is_none() {
            let dpy = (api.open_display)(std::ptr::null());
            if !dpy.is_null() {
                *borrow = Some(ThreadDisplay(dpy));
            }
        }
        let dpy = borrow.as_ref()?.0;
        let mut attrs: XWindowAttributes = std::mem::zeroed();
        // map_state == 2 is IsViewable in X11.
        if (api.get_window_attributes)(dpy, xid as c_ulong, &mut attrs) == 0
            || attrs.map_state != 2
            || attrs.width < 32
            || attrs.height < 32
        {
            return None;
        }
        let src_w = attrs.width as u32;
        let src_h = attrs.height as u32;
        let img = (api.get_image)(
            dpy,
            xid as c_ulong,
            0,
            0,
            src_w,
            src_h,
            !0 as c_ulong, // AllPlanes
            2,             // ZPixmap
        );
        if img.is_null() {
            return None;
        }
        if (*img).data.is_null() {
            if let Some(destroy) = (*img).f.destroy_image {
                let _ = destroy(img);
            } else {
                let _ = (api.free)(img.cast());
            }
            return None;
        }
        let pitch = (*img).bytes_per_line.max(0) as usize;
        let bpp = ((*img).bits_per_pixel.max(24) as usize) / 8;
        let needed = (src_w as usize).checked_mul(src_h as usize)?.checked_mul(4)?;
        out.resize(needed, 0);
        let raw_len = pitch.saturating_mul(src_h as usize);
        let src_bytes = std::slice::from_raw_parts((*img).data, raw_len);
        for y in 0..src_h as usize {
            let s_row = y * pitch;
            let d_row = y * (src_w as usize) * 4;
            for x in 0..src_w as usize {
                let s_idx = s_row + x * bpp;
                let d_idx = d_row + x * 4;
                if s_idx + 3 <= src_bytes.len() && d_idx + 4 <= out.len() {
                    out[d_idx] = src_bytes[s_idx];
                    out[d_idx + 1] = src_bytes[s_idx + 1];
                    out[d_idx + 2] = src_bytes[s_idx + 2];
                    out[d_idx + 3] = 0xFF;
                }
            }
        }
        if let Some(destroy) = (*img).f.destroy_image {
            let _ = destroy(img);
        } else {
            let _ = (api.free)(img.cast());
        }
        Some((src_w, src_h))
    })
}

/// Area-averaging box filter from a cropped BGRA region `(crop_x, crop_y, crop_w, crop_h)`
/// of `src` into a tightly packed `dst_w` x `dst_h` BGRA buffer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn scale_bgra(
    src: &[u8],
    src_w: u32,
    src_pitch: u32,
    crop_x: u32,
    crop_y: u32,
    crop_w: u32,
    crop_h: u32,
    dst_w: u32,
    dst_h: u32,
    dst: &mut Vec<u8>,
) {
    let dst_len = (dst_w as usize) * (dst_h as usize) * 4;
    dst.resize(dst_len, 0);
    if crop_w == 0 || crop_h == 0 || dst_w == 0 || dst_h == 0 || src_w == 0 {
        return;
    }
    let pitch = src_pitch as usize;

    if crop_w == dst_w && crop_h == dst_h {
        let row_bytes = (dst_w as usize) * 4;
        let x_off = (crop_x as usize) * 4;
        for y in 0..dst_h as usize {
            let s_off = (crop_y as usize + y) * pitch + x_off;
            let d_off = y * row_bytes;
            if s_off + row_bytes <= src.len() {
                dst[d_off..d_off + row_bytes].copy_from_slice(&src[s_off..s_off + row_bytes]);
            }
        }
        return;
    }

    for dy in 0..dst_h {
        let sy0 = crop_y + ((dy as u64 * crop_h as u64) / dst_h as u64) as u32;
        let sy1 = (crop_y + (((dy + 1) as u64 * crop_h as u64) / dst_h as u64) as u32).max(sy0 + 1);
        for dx in 0..dst_w {
            let sx0 = crop_x + ((dx as u64 * crop_w as u64) / dst_w as u64) as u32;
            let sx1 = (crop_x + (((dx + 1) as u64 * crop_w as u64) / dst_w as u64) as u32).max(sx0 + 1);
            let mut b_sum = 0u32;
            let mut g_sum = 0u32;
            let mut r_sum = 0u32;
            let mut count = 0u32;
            for sy in sy0..sy1 {
                let row = (sy as usize) * pitch;
                for sx in sx0..sx1 {
                    let idx = row + (sx as usize) * 4;
                    if idx + 4 <= src.len() {
                        b_sum += src[idx] as u32;
                        g_sum += src[idx + 1] as u32;
                        r_sum += src[idx + 2] as u32;
                        count += 1;
                    }
                }
            }
            let out_idx = ((dy * dst_w + dx) as usize) * 4;
            if let (Some(b), Some(g), Some(r)) =
                (b_sum.checked_div(count), g_sum.checked_div(count), r_sum.checked_div(count))
            {
                dst[out_idx] = b as u8;
                dst[out_idx + 1] = g as u8;
                dst[out_idx + 2] = r as u8;
                dst[out_idx + 3] = 0xFF;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_size_is_a_copy() {
        let src = vec![
            10, 20, 30, 255, 40, 50, 60, 255, //
            70, 80, 90, 255, 100, 110, 120, 255,
        ];
        let mut dst = Vec::new();
        scale_bgra(&src, 2, 8, 0, 0, 2, 2, 2, 2, &mut dst);
        assert_eq!(dst, src);
    }

    #[test]
    fn halving_averages_and_a_flat_picture_stays_flat() {
        let mut src = Vec::with_capacity(4 * 4 * 4);
        for _ in 0..16 {
            src.extend_from_slice(&[50, 100, 150, 255]);
        }
        let mut dst = Vec::new();
        scale_bgra(&src, 4, 16, 0, 0, 4, 4, 2, 2, &mut dst);
        assert_eq!(dst.len(), 2 * 2 * 4);
        for px in dst.as_chunks::<4>().0 {
            assert_eq!(px, &[50, 100, 150, 255]);
        }
    }
}
