// --shot <png>: draws the window without ever showing it, saves the picture and quits, for checking screens on a PC
// where nothing may pop up (built with --features shot, which brings GPUI's render_to_image).
use std::{path::PathBuf, time::Duration};

use gpui_kit::*;

/// After `wait` (the app's state and the pictures arriving), pictures the window at `size` and quits.
pub fn schedule<V: 'static>(view: WeakEntity<V>, out: PathBuf, w: f32, h: f32, wait: Duration, cx: &mut Context<V>) -> Task<()> {
    cx.spawn(async move |_, cx| {
        // A window never shown never gets a size, so its renderer stays 1×1: set one (it stays hidden).
        let handle = view.update_in(cx, |_, window, _| {
            window.resize(size(px(w), px(h + 1.)));
            Window::window_handle(window)
        });
        cx.background_executor().timer(Duration::from_millis(150)).await;
        let _ = view.update_in(cx, |_, window, _| window.resize(size(px(w), px(h))));
        // A hidden window is never asked to draw, and what a page reads (and each picture) is asked for as it is
        // drawn: drawn through the wait, so the wait is time for those to come, one read after another.
        #[cfg(feature = "shot")]
        {
            let until = std::time::Instant::now() + wait;
            while std::time::Instant::now() < until {
                if let Ok(h) = handle {
                    let _ = cx.update_window(h, |_, window, cx| {
                        away(window, cx);
                        window.draw(cx).clear(cx)
                    });
                }
                cx.background_executor().timer(Duration::from_millis(500)).await;
            }
        }
        #[cfg(not(feature = "shot"))]
        cx.background_executor().timer(wait).await;
        #[cfg(feature = "shot")]
        {
            // A hidden window is never asked to draw: draw here.
            // A picture is decoded on the draw after it arrives, and some arrive only after others (a game's cover
            // after its look): a few draws, a moment apart.
            for _ in 0..5 {
                if let Ok(h) = handle {
                    let _ = cx.update_window(h, |_, window, cx| {
                        away(window, cx);
                        window.draw(cx).clear(cx)
                    });
                }
                cx.background_executor().timer(Duration::from_millis(700)).await;
            }
            // BLOXKEEP_SHOT_KEYS="tab tab ctrl-k": keys handed to this hidden window itself (never to the desktop), to
            // see focus rings and shortcuts in the picture.
            if let (Ok(&h), Ok(keys)) = (handle.as_ref(), std::env::var("BLOXKEEP_SHOT_KEYS")) {
                for k in keys.split_whitespace() {
                    let k: &'static str = Box::leak(k.to_string().into_boxed_str());
                    if let Ok(stroke) = Keystroke::parse(k) {
                        let _ = cx.update_window(h, |_, window, cx| {
                            // As a real key press arrives (down, then up), so the focus ring shows as it would.
                            let down = KeyDownEvent { keystroke: stroke.clone(), is_held: false, prefer_character_input: false };
                            window.dispatch_event(PlatformInput::KeyDown(down), cx);
                            window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke: stroke }), cx);
                            window.draw(cx).clear(cx);
                        });
                    }
                    cx.background_executor().timer(Duration::from_millis(150)).await;
                }
            }
            let pic = handle.and_then(|h| {
                cx.update_window(h, |_, window, cx| {
                    window.draw(cx).clear(cx);
                    window.render_to_image()
                })
            });
            match pic {
                Ok(Ok(pic)) => match pic.save(&out) {
                    Ok(()) => eprintln!("shot: {}", out.display()),
                    Err(e) => eprintln!("shot failed: {e}"),
                },
                Ok(Err(e)) | Err(e) => eprintln!("shot failed: {e}"),
            }
        }
        #[cfg(not(feature = "shot"))]
        {
            let _ = handle;
            eprintln!("--shot needs a build with --features shot ({})", out.display());
        }
        let _ = cx.update(|cx| cx.quit());
    })
}

/// The hidden window takes the desktop's real mouse as its own (where the cursor happens to be on the screen), so a
/// tile under it showed its hover in the picture: the mouse is told it is far outside the window, so nothing is hovered.
#[cfg(feature = "shot")]
fn away(window: &mut Window, cx: &mut App) {
    let far = MouseMoveEvent { position: point(px(-4000.), px(-4000.)), pressed_button: None, modifiers: Modifiers::default() };
    window.dispatch_event(PlatformInput::MouseMove(far), cx);
}
