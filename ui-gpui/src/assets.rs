// Pictures and icons from the assets folder beside the exe (Bloxkeep's own icon set), then GPUI Kit's built-in ones.
use std::{borrow::Cow, path::PathBuf};

use gpui_kit::{AssetSource, SharedString};

pub struct FsAssets(pub PathBuf);

impl AssetSource for FsAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        match std::fs::read(self.0.join(path)) {
            Ok(b) => Ok(Some(Cow::Owned(b))),
            Err(_) => gpui_kit::assets::Assets.load(path),
        }
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(path)
    }
}
