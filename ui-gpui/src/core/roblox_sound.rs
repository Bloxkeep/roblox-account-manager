//! The sound of each Roblox window, for "Mute background windows": only the window played makes sound.
//!
//! Windows keeps a sound session per program that plays sound, on each speaker it plays on; a Roblox window's sessions
//! are muted or unmuted through their own volume (ISimpleAudioVolume), the switch the Volume mixer shows. A Roblox that
//! hasn't played a sound yet has no session, so nothing is found for it and it is asked again later.

/// Mutes (`true`) or unmutes (`false`) every sound session of each process, on every speaker. Answers the processes at
/// least one session was found and set for. Blocking: COM on the calling thread (run it off the core's thread).
#[cfg(windows)]
pub fn set_muted(wants: &[(i32, bool)]) -> Vec<i32> {
    use windows::Win32::Media::Audio::{
        DEVICE_STATE_ACTIVE, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, eRender,
    };
    use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
    use windows::core::Interface;

    if wants.is_empty() {
        return Vec::new();
    }
    let mut found = Vec::new();
    // SAFETY: plain COM calls on interfaces this function holds; nothing is passed that outlives them. The thread joins
    // the shared apartment (a thread already in one keeps it, and the calls work there too).
    let result: windows::core::Result<()> = unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        (|| {
            let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let speakers = devices.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            for i in 0..speakers.GetCount()? {
                let Ok(speaker) = speakers.Item(i) else { continue };
                let Ok(manager) = speaker.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { continue };
                let Ok(sessions) = manager.GetSessionEnumerator() else { continue };
                for j in 0..sessions.GetCount().unwrap_or(0) {
                    let Ok(session) = sessions.GetSession(j) else { continue };
                    let Ok(control) = session.cast::<IAudioSessionControl2>() else { continue };
                    let Ok(pid) = control.GetProcessId() else { continue };
                    let Some(&(_, mute)) = wants.iter().find(|(p, _)| *p as u32 == pid && pid != 0) else { continue };
                    let Ok(volume) = session.cast::<ISimpleAudioVolume>() else { continue };
                    if volume.SetMute(mute, std::ptr::null()).is_ok() && !found.contains(&(pid as i32)) {
                        found.push(pid as i32);
                    }
                }
            }
            Ok(())
        })()
    };
    if let Err(e) = result {
        crate::core::log::note(&format!("Mute background windows: Windows' sound sessions couldn't be read ({:#x}).", e.code().0));
    }
    found
}

#[cfg(not(windows))]
pub fn set_muted(_wants: &[(i32, bool)]) -> Vec<i32> {
    Vec::new()
}

/// Whether the process's first sound session is muted (None: it has none). For the checks.
#[cfg(windows)]
pub fn is_muted(pid: i32) -> Option<bool> {
    use windows::Win32::Media::Audio::{
        DEVICE_STATE_ACTIVE, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, eRender,
    };
    use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
    use windows::core::Interface;

    // SAFETY: as in set_muted.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let speakers = devices.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE).ok()?;
        for i in 0..speakers.GetCount().ok()? {
            let Ok(speaker) = speakers.Item(i) else { continue };
            let Ok(manager) = speaker.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { continue };
            let Ok(sessions) = manager.GetSessionEnumerator() else { continue };
            for j in 0..sessions.GetCount().unwrap_or(0) {
                let Ok(session) = sessions.GetSession(j) else { continue };
                let Ok(control) = session.cast::<IAudioSessionControl2>() else { continue };
                if control.GetProcessId().ok() != Some(pid as u32) {
                    continue;
                }
                let volume = session.cast::<ISimpleAudioVolume>().ok()?;
                return volume.GetMute().ok().map(|b| b.as_bool());
            }
        }
        None
    }
}

#[cfg(not(windows))]
pub fn is_muted(_pid: i32) -> Option<bool> {
    None
}
