use crate::{DevicePixels, Pixels, Result, SharedString, Size, size};
use smallvec::SmallVec;

use image::{Delay, Frame};
use std::{
    borrow::Cow,
    fmt,
    hash::Hash,
    sync::atomic::{AtomicUsize, Ordering::SeqCst},
};

/// A source of assets for this app to use.
pub trait AssetSource: 'static + Send + Sync {
    /// Load the given asset from the source path.
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>>;

    /// List the assets at the given path.
    fn list(&self, path: &str) -> Result<Vec<SharedString>>;
}

impl AssetSource for () {
    fn load(&self, _path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(None)
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

/// A unique identifier for the image cache
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ImageId(pub usize);

#[derive(PartialEq, Eq, Hash, Clone)]
#[expect(missing_docs)]
pub struct RenderImageParams {
    pub image_id: ImageId,
    pub frame_index: usize,
}

/// A cached and processed image, in BGRA format
pub struct RenderImage {
    /// The ID associated with this image
    pub id: ImageId,
    /// The scale factor of this image on render.
    pub(crate) scale_factor: f32,
    /// (Bloxkeep) Each frame's size and delay, kept when its pixels are given to the GPU (`discard_after_upload`).
    sizes: SmallVec<[(u32, u32); 1]>,
    delays: SmallVec<[Delay; 1]>,
    data: parking_lot::Mutex<SmallVec<[Frame; 1]>>,
    discard: bool,
}

/// (Bloxkeep) Images whose pixels were given to the GPU and let go here, asked for again by an atlas that hasn't got
/// them (another window's, or one made again after the graphics device was lost): the app makes them again.
static LOST: parking_lot::Mutex<Vec<ImageId>> = parking_lot::Mutex::new(Vec::new());

/// (Bloxkeep) The images asked for since the last call whose pixels were let go (see `RenderImage::discard_after_upload`).
pub fn take_lost_images() -> Vec<ImageId> {
    std::mem::take(&mut *LOST.lock())
}

pub(crate) fn note_lost_image(id: ImageId) {
    let mut lost = LOST.lock();
    if !lost.contains(&id) {
        lost.push(id);
    }
}

impl PartialEq for RenderImage {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for RenderImage {}

impl RenderImage {
    /// Create a new image from the given data.
    pub fn new(data: impl Into<SmallVec<[Frame; 1]>>) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let data: SmallVec<[Frame; 1]> = data.into();
        let id = ImageId(NEXT_ID.fetch_add(1, SeqCst));
        let first = data.first().map(|f| f.buffer().dimensions()).unwrap_or_default();
        held_add(id, first, data.len(), data.iter().map(|f| f.buffer().as_raw().len()).sum());
        Self {
            id,
            scale_factor: 1.0,
            sizes: data.iter().map(|f| f.buffer().dimensions()).collect(),
            delays: data.iter().map(|f| f.delay()).collect(),
            data: parking_lot::Mutex::new(data),
            discard: false,
        }
    }

    /// (Bloxkeep) Its pixels are let go here once the first atlas has them on the GPU: a picture is then kept once, on
    /// the graphics card, not twice (a still image only; a moving one's frames are uploaded one by one as they play).
    /// An atlas asking for it again later gets nothing and says so (`take_lost_images`).
    pub fn discard_after_upload(mut self) -> Self {
        self.discard = self.sizes.len() == 1;
        self
    }

    /// The pixels for an atlas: taken out of this image when it lets them go after the upload, else a copy. None when
    /// they were let go already.
    pub(crate) fn bytes_for_upload(&self, frame_index: usize) -> Option<Vec<u8>> {
        let mut data = self.data.lock();
        let frame = data.get_mut(frame_index)?;
        if frame.buffer().as_raw().is_empty() {
            return None;
        }
        if self.discard {
            let empty = Frame::new(image::RgbaImage::new(0, 0));
            let raw = std::mem::replace(frame, empty).into_buffer().into_raw();
            held_less(self.id, raw.len());
            Some(raw)
        } else {
            Some(frame.buffer().as_raw().clone())
        }
    }

    /// A copy of this image's pixels (None once given to the GPU, see `discard_after_upload`).
    pub fn as_bytes(&self, frame_index: usize) -> Option<Vec<u8>> {
        let data = self.data.lock();
        let raw = data.get(frame_index)?.buffer().as_raw();
        (!(self.discard && raw.is_empty())).then(|| raw.clone())
    }

    /// Get the size of this image, in pixels.
    pub fn size(&self, frame_index: usize) -> Size<DevicePixels> {
        self.sizes
            .get(frame_index)
            .map(|&(width, height)| size((width as i32).into(), (height as i32).into()))
            .unwrap_or_default()
    }

    /// Get the size of this image, in pixels for display, adjusted for the scale factor.
    pub(crate) fn render_size(&self, frame_index: usize) -> Size<Pixels> {
        self.size(frame_index)
            .map(|v| (v.0 as f32 / self.scale_factor).into())
    }

    /// Get the delay of this frame from the previous
    pub fn delay(&self, frame_index: usize) -> Delay {
        self.delays
            .get(frame_index)
            .copied()
            .unwrap_or(Delay::from_numer_denom_ms(100, 1))
    }

    /// Get the number of frames for this image.
    pub fn frame_count(&self) -> usize {
        self.sizes.len()
    }
}

impl Drop for RenderImage {
    fn drop(&mut self) {
        if let Some(held) = HELD.lock().as_mut() {
            held.remove(&self.id.0);
        }
    }
}

/// (Bloxkeep) The pixels every live RenderImage holds (decoded pictures, their smaller copies, the theme's pictures,
/// clip frames, the 3D avatar...), by image: its first frame's size, frames, and bytes still held (0 once given to the
/// GPU, see `discard_after_upload`). For error.log's memory line.
static HELD: parking_lot::Mutex<Option<std::collections::HashMap<usize, ((u32, u32), usize, usize)>>> = parking_lot::Mutex::new(None);

fn held_add(id: ImageId, size: (u32, u32), frames: usize, bytes: usize) {
    HELD.lock().get_or_insert_with(Default::default).insert(id.0, (size, frames, bytes));
}

fn held_less(id: ImageId, bytes: usize) {
    if let Some(e) = HELD.lock().as_mut().and_then(|h| h.get_mut(&id.0)) {
        e.2 = e.2.saturating_sub(bytes);
    }
}

/// (Bloxkeep) What the live pictures hold as pixels in this process's memory.
#[derive(Debug, Default, Clone)]
pub struct HeldPictures {
    /// Live images.
    pub images: usize,
    /// Those of them still holding pixels here.
    pub holding: usize,
    /// Those pixels' bytes.
    pub bytes: usize,
    /// The biggest holders: first frame's size, frames, bytes; biggest first.
    pub biggest: Vec<((u32, u32), usize, usize)>,
}

/// (Bloxkeep) The pixels the live pictures hold now, with the `top` biggest.
#[doc(hidden)]
pub fn held_pictures(top: usize) -> HeldPictures {
    let held = HELD.lock();
    let Some(held) = held.as_ref() else { return HeldPictures::default() };
    let mut all: Vec<((u32, u32), usize, usize)> = held.values().copied().filter(|e| e.2 > 0).collect();
    let bytes = all.iter().map(|e| e.2).sum();
    let holding = all.len();
    all.sort_unstable_by(|a, b| b.2.cmp(&a.2));
    all.truncate(top);
    HeldPictures { images: held.len(), holding, bytes, biggest: all }
}

/// (Bloxkeep) The graphics card's picture atlas: textures made and their bytes, kept by the platform's atlas.
static ATLAS: parking_lot::Mutex<(usize, usize)> = parking_lot::Mutex::new((0, 0));

/// (Bloxkeep) An atlas texture of `bytes` made (`made`) or let go.
#[doc(hidden)]
pub fn note_atlas_texture(bytes: usize, made: bool) {
    let mut a = ATLAS.lock();
    if made {
        a.0 += 1;
        a.1 += bytes;
    } else {
        a.0 = a.0.saturating_sub(1);
        a.1 = a.1.saturating_sub(bytes);
    }
}

/// (Bloxkeep) The atlas textures alive now and their bytes (on the graphics card, every window's).
#[doc(hidden)]
pub fn atlas_textures() -> (usize, usize) {
    *ATLAS.lock()
}

impl fmt::Debug for RenderImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageData")
            .field("id", &self.id)
            .field("size", &self.sizes.first())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::SmallVec;

    #[test]
    fn empty_render_image_does_not_panic() {
        let image = RenderImage::new(SmallVec::new());
        assert_eq!(image.frame_count(), 0);
        assert_eq!(image.size(0), Size::default());
        assert_eq!(image.as_bytes(0), None);
        assert_eq!(image.render_size(0), Size::default());
        assert_eq!(image.delay(0), Delay::from_numer_denom_ms(100, 1));
        let _ = format!("{image:?}");
    }
}
