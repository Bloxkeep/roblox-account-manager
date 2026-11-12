// The app's window has no Windows title bar when this interface is in it (ShellForm drops it): the top bar moves the
// window and has its buttons. This window is a child of the app's (another process), so moving means handing the
// mouse to the app's top-level window as if its title bar were pressed.

/// Starts moving the app's window with the mouse (the left button is down on the top bar).
#[cfg(windows)]
pub fn start_move(hwnd: isize) {
    use windows_sys::Win32::UI::{
        Input::KeyboardAndMouse::ReleaseCapture,
        WindowsAndMessaging::{GA_ROOT, GetAncestor, HTCAPTION, PostMessageW, WM_NCLBUTTONDOWN},
    };
    unsafe {
        let root = GetAncestor(hwnd as _, GA_ROOT);
        if root.is_null() {
            return;
        }
        ReleaseCapture();
        PostMessageW(root, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
    }
}

#[cfg(not(windows))]
pub fn start_move(_: isize) {}
