// A window's live picture, frame by frame, through Windows' own capture (Windows.Graphics.Capture, Windows 10 1903+):
// used for Wallpaper Engine's wallpaper played in a window of its own (wallpaper_engine.rs), shown behind the page as
// a video is (video.rs). Each frame's inside (no title bar or edges) is copied out of the GPU into memory (BGRA), at
// most about 30 a second, and handed on until nobody takes them any more or the window is gone.
#![cfg(all(windows, any(feature = "core", feature = "shot")))]

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use windows::{
    Graphics::{
        Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem},
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
    },
    Win32::{
        Foundation::{HMODULE, HWND},
        Graphics::{
            Direct3D::D3D_DRIVER_TYPE_HARDWARE,
            Direct3D11::{
                D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX,
                D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
            },
            Dxgi::{IDXGIDevice, IDXGIKeyedMutex, IDXGIResource},
        },
        System::WinRT::{
            Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess},
            Graphics::Capture::IGraphicsCaptureItemInterop,
            RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize,
        },
        UI::WindowsAndMessaging::IsWindow,
    },
    core::Interface,
};

/// What each frame becomes: pixels in memory (the still, and the fallback), or a texture GPUI draws straight from.
pub enum Out<'a> {
    /// Its time (100 ns from the start), size and pixels (BGRA, rows packed); false: nobody takes them any more.
    Pixels(&'a mut dyn FnMut(i64, u32, u32, Vec<u8>) -> bool),
    /// The shared texture holding the latest frame (Window::paint_shared_texture: its handle and size), given after
    /// each frame copied in; false: nobody draws it any more. The frame never leaves the GPU: measured on the owner's
    /// wallpaper at 1920×1200 (2026-11-18), ~0.6 % of one core against ~9.4 % through memory.
    Shared(&'a mut dyn FnMut(isize, u32, u32) -> bool),
}

/// A texture another device (GPUI's) reads: made with a keyed mutex, key 0 (see gpui-pre-windows › draw_surfaces).
struct SharedOut {
    texture: ID3D11Texture2D,
    mutex: IDXGIKeyedMutex,
    handle: isize,
    w: u32,
    h: u32,
}

/// Runs until `give` says nobody takes frames any more (false) or the window closes; each frame goes to `give` with its
/// time (100 ns from the start), size and pixels (BGRA, rows packed). Err: why capturing couldn't start or stopped.
///
/// `stop`: set from outside to end it (a window that draws nothing new sends no frame, so `give` alone never gets to say).
pub fn run(hwnd: isize, widest: u32, stop: &AtomicBool, mut give: impl FnMut(i64, u32, u32, Vec<u8>) -> bool) -> Result<(), String> {
    run_to(hwnd, widest, stop, None, Out::Pixels(&mut give))
}

/// As `run`, each frame's inside copied on the GPU into a texture GPUI draws (Out::Shared). Err before any frame was
/// given (a GPU without shared textures: Windows' software one, an old driver): the caller falls back to `run`.
pub fn run_shared(hwnd: isize, stop: &AtomicBool, mut give: impl FnMut(isize, u32, u32) -> bool) -> Result<(), String> {
    run_to(hwnd, u32::MAX, stop, None, Out::Shared(&mut give))
}

/// `until`: ends then even with no frame come (a still window sends none after its first).
fn run_to(hwnd: isize, widest: u32, stop: &AtomicBool, until: Option<Instant>, mut out: Out) -> Result<(), String> {
    fn e(what: &'static str) -> impl Fn(windows::core::Error) -> String {
        move |err| format!("{what}: {err}")
    }
    // SAFETY: plain Windows calls with valid out-pointers; the window handle is checked by Windows itself.
    unsafe {
        // Joined to COM for this run and let go after it (declared first, so dropped after everything made here): it
        // runs on threads that are reused for other work.
        struct Ro(bool);
        impl Drop for Ro {
            fn drop(&mut self) {
                if self.0 {
                    // SAFETY: pairs the RoInitialize that succeeded on this thread.
                    unsafe { RoUninitialize() };
                }
            }
        }
        let _ro = Ro(RoInitialize(RO_INIT_MULTITHREADED).is_ok());
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        D3D11CreateDevice(None, D3D_DRIVER_TYPE_HARDWARE, HMODULE::default(), D3D11_CREATE_DEVICE_BGRA_SUPPORT, None, D3D11_SDK_VERSION, Some(&mut device), None, Some(&mut context)).map_err(e("Direct3D device"))?;
        let (device, context) = (device.ok_or("no Direct3D device")?, context.ok_or("no Direct3D context")?);
        let dxgi: IDXGIDevice = device.cast().map_err(e("DXGI device"))?;
        let d3d: IDirect3DDevice = CreateDirect3D11DeviceFromDXGIDevice(&dxgi).and_then(|i| i.cast()).map_err(e("WinRT Direct3D device"))?;
        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(e("capture interop"))?;
        let item: GraphicsCaptureItem = interop.CreateForWindow(HWND(hwnd as _)).map_err(e("capture item for the window"))?;
        let mut size = item.Size().map_err(e("window size"))?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(&d3d, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, size).map_err(e("frame pool"))?;
        let session = pool.CreateCaptureSession(&item).map_err(e("capture session"))?;
        let _ = session.SetIsCursorCaptureEnabled(false);
        // (Windows 11 draws a yellow edge round a captured window unless told not to; Windows 10 has no such switch.)
        let _ = session.SetIsBorderRequired(false);
        session.StartCapture().map_err(e("starting the capture"))?;
        let started = Instant::now();
        let mut staging: Option<(ID3D11Texture2D, u32, u32)> = None;
        let mut shared: Option<SharedOut> = None;
        let mut result = Ok(());
        loop {
            if stop.load(Ordering::Relaxed) || until.is_some_and(|u| Instant::now() >= u) || !IsWindow(Some(HWND(hwnd as _))).as_bool() {
                break;
            }
            let Ok(frame) = pool.TryGetNextFrame() else {
                std::thread::sleep(Duration::from_millis(8));
                continue;
            };
            let to_gpu = matches!(out, Out::Shared(_));
            let shot = (|| -> Result<Option<(i64, u32, u32, Vec<u8>)>, String> {
                let got = frame.ContentSize().map_err(e("frame size"))?;
                if got.Width != size.Width || got.Height != size.Height {
                    // The window changed size: the pool is made again for it, this frame dropped.
                    size = got;
                    pool.Recreate(&d3d, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, size).map_err(e("frame pool again"))?;
                    return Ok(None);
                }
                let access: IDirect3DDxgiInterfaceAccess = frame.Surface().and_then(|s| s.cast()).map_err(e("frame surface"))?;
                let texture: ID3D11Texture2D = access.GetInterface().map_err(e("frame texture"))?;
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                texture.GetDesc(&mut desc);
                let (w, h) = (desc.Width, desc.Height);
                if to_gpu {
                    // On the GPU: the inside copied into the shared texture under its key (made again at a new size).
                    let (x0, y0, cw, ch) = inside(hwnd, w, h);
                    if shared.as_ref().is_none_or(|s| s.w != cw || s.h != ch) {
                        let mut d = desc;
                        d.Width = cw;
                        d.Height = ch;
                        d.MipLevels = 1;
                        d.ArraySize = 1;
                        d.Usage = D3D11_USAGE_DEFAULT;
                        d.BindFlags = (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32;
                        d.CPUAccessFlags = 0;
                        d.MiscFlags = D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX.0 as u32;
                        let mut made: Option<ID3D11Texture2D> = None;
                        device.CreateTexture2D(&d, None, Some(&mut made)).map_err(e("shared texture"))?;
                        let texture = made.ok_or("no shared texture")?;
                        let mutex: IDXGIKeyedMutex = texture.cast().map_err(e("shared texture's key"))?;
                        let resource: IDXGIResource = texture.cast().map_err(e("shared texture's handle"))?;
                        let handle = resource.GetSharedHandle().map_err(e("shared texture's handle"))?.0 as isize;
                        shared = Some(SharedOut { texture, mutex, handle, w: cw, h: ch });
                    }
                    let Some(s) = shared.as_ref() else { return Ok(None) };
                    // (Waits at most a tenth of a second for GPUI's copy; a frame it can't have is skipped.)
                    // (Exactly S_OK: windows-rs reads WAIT_TIMEOUT, a positive HRESULT, as success.)
                    let held = (windows::core::Interface::vtable(&s.mutex).AcquireSync)(windows::core::Interface::as_raw(&s.mutex), 0, 100) == windows::Win32::Foundation::S_OK;
                    if held {
                        let b = D3D11_BOX { left: x0, top: y0, front: 0, right: x0 + cw, bottom: y0 + ch, back: 1 };
                        context.CopySubresourceRegion(&s.texture, 0, 0, 0, 0, &texture, 0, Some(&b));
                        let _ = s.mutex.ReleaseSync(0);
                        context.Flush();
                    }
                    return Ok(Some((0, cw, ch, Vec::new())));
                }
                if staging.as_ref().is_none_or(|(_, sw, sh)| *sw != w || *sh != h) {
                    let mut copy = desc;
                    copy.Usage = D3D11_USAGE_STAGING;
                    copy.BindFlags = 0;
                    copy.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                    copy.MiscFlags = 0;
                    copy.MipLevels = 1;
                    copy.ArraySize = 1;
                    let mut made: Option<ID3D11Texture2D> = None;
                    device.CreateTexture2D(&copy, None, Some(&mut made)).map_err(e("copy texture"))?;
                    staging = Some((made.ok_or("no copy texture")?, w, h));
                }
                let Some((copy, _, _)) = staging.as_ref() else { return Ok(None) };
                context.CopyResource(copy, &texture);
                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                context.Map(copy, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).map_err(e("reading the frame"))?;
                // Only the window's inside: the capture holds its title bar and edges too (a white "Bloxkeep wallpaper"
                // bar and its – □ × showed across the top of the page, 2026-11-18).
                let (x0, y0, cw, ch) = inside(hwnd, w, h);
                // Every `step`-th pixel and row when the window is wider than the page needs (whole steps keep it cheap).
                let step = cw.div_ceil(widest.max(1)).max(1);
                let (ow, oh) = (cw / step, ch / step);
                let mut bgra = Vec::with_capacity((ow * oh * 4) as usize);
                let base = mapped.pData as *const u8;
                for y in 0..oh {
                    let full = std::slice::from_raw_parts(base.add((y0 + y * step) as usize * mapped.RowPitch as usize), (w * 4) as usize);
                    let row = &full[(x0 * 4) as usize..((x0 + cw) * 4) as usize];
                    if step == 1 {
                        bgra.extend_from_slice(row);
                    } else {
                        for x in 0..ow {
                            let i = (x * step * 4) as usize;
                            bgra.extend_from_slice(&row[i..i + 4]);
                        }
                    }
                }
                context.Unmap(copy, 0);
                // (Wallpaper Engine's window has no see-through parts: every pixel solid, whatever its alpha says.)
                for px in bgra.chunks_exact_mut(4) {
                    px[3] = 255;
                }
                Ok(Some(((started.elapsed().as_nanos() / 100) as i64, ow, oh, bgra)))
            })();
            let _ = frame.Close();
            match shot {
                Ok(Some((at, w, h, bgra))) => {
                    let taken = match &mut out {
                        Out::Pixels(give) => give(at, w, h, bgra),
                        Out::Shared(give) => shared.as_ref().is_some_and(|s| give(s.handle, s.w, s.h)),
                    };
                    if !taken {
                        break;
                    }
                }
                Ok(None) => {}
                Err(why) => {
                    result = Err(why);
                    break;
                }
            }
            // About 30 a second at most.
            std::thread::sleep(Duration::from_millis(30));
        }
        let _ = session.Close();
        let _ = pool.Close();
        result
    }
}

/// One frame of the window's inside, once it has drawn for a moment (a wallpaper's first frames can be blank): None
/// when none came in a few seconds.
pub fn still(hwnd: isize, widest: u32) -> Option<(u32, u32, Vec<u8>)> {
    let mut got = None;
    let begun = Instant::now();
    let never = AtomicBool::new(false);
    // A still wallpaper sends one frame and then none: without the time limit this waited (and held its thread, device
    // and capture) forever. Past it, the last frame seen is taken, however early.
    let mut give = |at: i64, w: u32, h: u32, bgra: Vec<u8>| {
        // (`at` counts 100 ns from the start: 1.2 s in.)
        let early = at < 12_000_000 && begun.elapsed() < Duration::from_secs(5);
        got = Some((w, h, bgra));
        early
    };
    let _ = run_to(hwnd, widest, &never, Some(begun + Duration::from_secs(5)), Out::Pixels(&mut give));
    got
}

/// Where the window's inside (its client area) sits in a `w`×`h` capture of it: (left, top, width, height). A window's
/// capture covers its visible frame (DWM's extended frame bounds: the edges past them are invisible resize borders);
/// the whole capture when the window can't be asked.
fn inside(hwnd: isize, w: u32, h: u32) -> (u32, u32, u32, u32) {
    use windows_sys::Win32::{
        Foundation::{POINT, RECT},
        Graphics::{
            Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
            Gdi::ClientToScreen,
        },
        UI::WindowsAndMessaging::{GetClientRect, GetWindowRect},
    };
    let whole = (0, 0, w, h);
    let mut frame = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    let mut origin = POINT { x: 0, y: 0 };
    // SAFETY: plain queries of a window by its handle into local out-structs of the asked size; a gone window fails them.
    let ok = unsafe {
        (DwmGetWindowAttribute(hwnd as _, DWMWA_EXTENDED_FRAME_BOUNDS as u32, (&mut frame as *mut RECT).cast(), std::mem::size_of::<RECT>() as u32) == 0
            || GetWindowRect(hwnd as _, &mut frame) != 0)
            && GetClientRect(hwnd as _, &mut client) != 0
            && ClientToScreen(hwnd as _, &mut origin) != 0
    };
    let (fw, fh) = (frame.right - frame.left, frame.bottom - frame.top);
    if !ok || fw < 2 || fh < 2 || client.right < 2 || client.bottom < 2 {
        return whole;
    }
    // (The capture is in the window's pixels; the frame bounds too, unless this thread sees scaled coordinates.)
    let (sx, sy) = (w as f64 / fw as f64, h as f64 / fh as f64);
    let x0 = (((origin.x - frame.left) as f64 * sx).round().max(0.) as u32).min(w - 1);
    let y0 = (((origin.y - frame.top) as f64 * sy).round().max(0.) as u32).min(h - 1);
    let cw = ((client.right as f64 * sx).round() as u32).min(w - x0);
    let ch = ((client.bottom as f64 * sy).round() as u32).min(h - y0);
    if cw < 2 || ch < 2 { whole } else { (x0, y0, cw, ch) }
}

/// A decoded video's picture on the GPU (video.rs): one texture GPUI draws straight from (Window::paint_shared_texture),
/// each frame written over the last. A new picture for every frame (15–30 a second) went through GPUI's atlas: a texture
/// made and let go each time, and the RAM of a playing clip kept climbing (the owner, 2026-11-19: "khi chạy video roblox
/// ram tăng lên nhiều"). Made on the device GPUI draws with: one of its own cost some 47 MB and 21 NVIDIA driver
/// threads, kept for good after the first clip (measured with --clip-check, 2026-11-19). Written on the window thread.
pub struct SharedFrame {
    out: SharedOut,
    /// The device it was made on: GPUI makes a new one after a lost device, and the texture is made again then.
    device: ID3D11Device,
    context: ID3D11DeviceContext,
}

impl SharedFrame {
    /// A texture of `w`×`h`; Err: why it couldn't be made (no hardware device, no shared textures): pictures then.
    pub fn new(w: u32, h: u32) -> Result<Self, String> {
        fn e(what: &'static str) -> impl Fn(windows::core::Error) -> String {
            move |err| format!("{what}: {err}")
        }
        let (device, _) = Self::device().ok_or("no Direct3D device")?;
        let d = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX.0 as u32,
        };
        // SAFETY: plain Direct3D calls with valid out-pointers on a device made here.
        unsafe {
            let mut made: Option<ID3D11Texture2D> = None;
            device.CreateTexture2D(&d, None, Some(&mut made)).map_err(e("shared texture"))?;
            let texture = made.ok_or("no shared texture")?;
            let mutex: IDXGIKeyedMutex = texture.cast().map_err(e("shared texture's key"))?;
            let resource: IDXGIResource = texture.cast().map_err(e("shared texture's handle"))?;
            let handle = resource.GetSharedHandle().map_err(e("shared texture's handle"))?.0 as isize;
            let context = Self::device().map(|d| d.1).ok_or("no Direct3D device")?;
            Ok(Self { out: SharedOut { texture, mutex, handle, w, h }, device, context })
        }
    }

    /// GPUI's device and its context (None before GPUI drew anything, or outside its process: --clip-check).
    fn device() -> Option<(ID3D11Device, ID3D11DeviceContext)> {
        let (d, c) = gpui_windows::d3d11_device()?;
        // SAFETY: each a reference handed to us (gpui_windows::d3d11_device), taken over here once.
        unsafe { Some((ID3D11Device::from_raw(d), ID3D11DeviceContext::from_raw(c))) }
    }

    /// Made on a device GPUI no longer draws with (it lost it and made another): to be made again.
    pub fn stale(&self) -> bool {
        Self::device().is_none_or(|(d, _)| d.as_raw() != self.device.as_raw())
    }

    /// Its handle and size, for Window::paint_shared_texture.
    pub fn handle(&self) -> (isize, u32, u32) {
        (self.out.handle, self.out.w, self.out.h)
    }

    pub fn size(&self) -> (u32, u32) {
        (self.out.w, self.out.h)
    }

    /// The frame's pixels (BGRA, rows packed, its size) written in; false when GPUI held the texture past a tenth of a
    /// second (the frame is skipped) or the pixels are short.
    pub fn put(&self, bgra: &[u8]) -> bool {
        let (w, h) = (self.out.w, self.out.h);
        if bgra.len() < w as usize * h as usize * 4 {
            return false;
        }
        let context = &self.context;
        // SAFETY: the texture is ours, written only while its key is held; `bgra` holds w×h×4 bytes (checked above).
        unsafe {
            // (Exactly S_OK: windows-rs reads WAIT_TIMEOUT, a positive HRESULT, as success.)
            let held = (Interface::vtable(&self.out.mutex).AcquireSync)(Interface::as_raw(&self.out.mutex), 0, 100) == windows::Win32::Foundation::S_OK;
            if !held {
                return false;
            }
            context.UpdateSubresource(&self.out.texture, 0, None, bgra.as_ptr().cast(), w * 4, 0);
            let _ = self.out.mutex.ReleaseSync(0);
            context.Flush();
        }
        true
    }
}
