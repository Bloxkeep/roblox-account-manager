// WebView2Setup.cs: installs the WebView2 Runtime when missing with Microsoft's bootstrapper, run only after Windows
// confirms a valid signature from Microsoft Corporation; and the switches every browser the app opens gets.
use std::{io::Read, path::Path, time::Duration};

use webview2_com::CoreWebView2EnvironmentOptions;
use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND},
    Security::{
        Cryptography::{
            CERT_FIND_SUBJECT_CERT, CERT_INFO, CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED, CERT_QUERY_FORMAT_FLAG_BINARY, CERT_QUERY_OBJECT_FILE, CERT_X500_NAME_STR,
            CMSG_SIGNER_INFO, CMSG_SIGNER_INFO_PARAM, CertCloseStore, CertFindCertificateInStore, CertFreeCertificateContext, CertNameToStrW, CryptMsgClose,
            CryptMsgGetParam, CryptQueryObject, PKCS_7_ASN_ENCODING, X509_ASN_ENCODING,
        },
        WinTrust::{WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_FILE_INFO, WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_IGNORE, WTD_UI_NONE, WinVerifyTrust},
    },
    System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject},
    UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
};

use super::{
    base::new_guid,
    forms::dialog,
    l, prefs, ui,
    win_form,
    win_kit::{from_wide, wide},
};

const BOOTSTRAPPER_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";
const SAFE_RENDERING_ARGS: &str = "--disable-gpu --disable-gpu-compositing";

fn options(args: &str) -> CoreWebView2EnvironmentOptions {
    let options = CoreWebView2EnvironmentOptions::default();
    // SAFETY: sets a string on our own options object before any browser reads it.
    unsafe { options.set_additional_browser_arguments(args.to_string()) };
    options
}

/// Switches for every browser the app opens: CPU drawing when the graphics driver proved broken
/// (prefs' safe_rendering), else None.
pub fn environment_options() -> Option<CoreWebView2EnvironmentOptions> {
    if prefs::lock().safe_rendering { Some(options(SAFE_RENDERING_ARGS)) } else { None }
}

/// The main window shows only Bloxkeep's page, so the browser's own background fetching and component updates
/// are left out there; the sign-in, captcha and roblox.com windows keep a full browser.
/// Its disk cache is capped at 32 MB: the pages come from the app itself, so a bigger one only fills the disk.
const MAIN_WINDOW_ARGS: &str = "--disable-background-networking --disable-component-update --disk-cache-size=33554432";

/// Even on the GPU, the page is painted by the CPU and only composited on the card: GPU painting held about
/// 100 MB more for no smoother result (2026-10-18, RTX 4070 laptop, 80 accounts: 360 → 265 MB opened, 398 → 265 MB
/// after scrolling every tab, 254 → 225 MB minimised).
const GPU_ARGS: &str = "--disable-gpu-rasterization";

/// For measuring only, off unless set: switches in BLOXKEEP_WEBVIEW_ARGS are added to Bloxkeep's own pages, for
/// example "--process-per-site" (one renderer for the main page and every panel). Nothing ships on until measured.
const EXTRA_ARGS_VARIABLE: &str = "BLOXKEEP_WEBVIEW_ARGS";

/// The switches of the app's own pages' browser (main_window_options' text).
pub fn main_window_args() -> String {
    let safe = prefs::lock().safe_rendering;
    let args = if safe { format!("{MAIN_WINDOW_ARGS} {SAFE_RENDERING_ARGS}") } else { format!("{MAIN_WINDOW_ARGS} {GPU_ARGS}") };
    let extra = std::env::var(EXTRA_ARGS_VARIABLE).ok().map(|e| e.trim().to_string()).unwrap_or_default();
    if extra.is_empty() { args } else { format!("{args} {extra}") }
}

pub fn main_window_options() -> CoreWebView2EnvironmentOptions {
    options(&main_window_args())
}

/// Asks, downloads Microsoft's bootstrapper, checks its signature and runs it; true when it installed. On the windows
/// thread (the questions are windows); the download and the wait run off it.
pub async fn try_install(owner: Option<isize>) -> bool {
    let yes = ui::modal(|| {
        dialog::confirm(owner, &l::t("Bloxkeep needs Microsoft Edge WebView2, which isn't installed. Install it now? (about 2 MB)"), &l::t("Install"), &l::t("Not now"), false)
    });
    if !yes {
        return false;
    }
    let dir = std::env::temp_dir().join(format!("Bloxkeep-webview2-{}", new_guid().simple()));
    let path = dir.join("MicrosoftEdgeWebview2Setup.exe");
    enum Step {
        Done(bool),
        NotSigned,
        Failed(String),
    }
    let work_dir = dir.clone();
    let step = win_form::blocking(move || -> Step {
        let download = || -> Result<(), String> {
            std::fs::create_dir_all(&work_dir).map_err(|e| e.to_string())?;
            let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(120)).build();
            let response = agent.get(BOOTSTRAPPER_URL).call().map_err(|e| e.to_string())?;
            let mut bytes = Vec::new();
            response.into_reader().read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            std::fs::write(&path, &bytes).map_err(|e| e.to_string())
        };
        if let Err(e) = download() {
            return Step::Failed(e);
        }
        if !is_signed_by_microsoft(&path) {
            return Step::NotSigned;
        }
        match run_and_wait(&path, "/silent /install") {
            Ok(Some(code)) => Step::Done(code == 0),
            Ok(None) => Step::Done(false),
            Err(e) => Step::Failed(e),
        }
    })
    .await
    .unwrap_or(Step::Done(false));
    let result = match step {
        Step::Done(ok) => ok,
        Step::NotSigned => {
            ui::error(owner, &l::t("The downloaded WebView2 installer is not signed by Microsoft, so it was not run."));
            false
        }
        Step::Failed(message) => {
            ui::error(owner, &l::f("Couldn't install WebView2: {0}", &[&message]));
            false
        }
    };
    // An IOException or UnauthorizedAccessException here is left be.
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Process.Start with UseShellExecute, then WaitForExitAsync: its exit code (None: nothing was started).
fn run_and_wait(path: &Path, args: &str) -> Result<Option<u32>, String> {
    let file = wide(&path.to_string_lossy());
    let params = wide(args);
    // SAFETY: the structure's size is set; strings live for the call; the process handle is closed below.
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = 1; // SW_SHOWNORMAL
        if ShellExecuteExW(&mut info) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if info.hProcess.is_null() {
            return Ok(None);
        }
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 0u32;
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
        Ok(Some(code))
    }
}

pub fn is_signed_by_microsoft(path: &Path) -> bool {
    if !has_valid_signature(path) {
        return false;
    }
    // A CryptographicException (no signer to read): not Microsoft's.
    signer_subject(path).is_some_and(|s| s.contains("O=Microsoft Corporation"))
}

/// The subject of the certificate that signed the file (X509Certificate.CreateFromSignedFile(path).Subject).
fn signer_subject(path: &Path) -> Option<String> {
    let p = wide(&path.to_string_lossy());
    // SAFETY: CryptoAPI calls with out-parameters on the stack; every handle opened is closed before returning.
    unsafe {
        let (mut encoding, mut content, mut format) = (0u32, 0u32, 0u32);
        let mut store = std::ptr::null_mut();
        let mut msg = std::ptr::null_mut();
        if CryptQueryObject(
            CERT_QUERY_OBJECT_FILE,
            p.as_ptr() as *const _,
            CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED,
            CERT_QUERY_FORMAT_FLAG_BINARY,
            0,
            &mut encoding as *mut u32 as *mut _,
            &mut content as *mut u32 as *mut _,
            &mut format as *mut u32 as *mut _,
            &mut store,
            &mut msg,
            std::ptr::null_mut(),
        ) == 0
        {
            return None;
        }
        let mut out = None;
        let mut len = 0u32;
        if CryptMsgGetParam(msg, CMSG_SIGNER_INFO_PARAM, 0, std::ptr::null_mut(), &mut len) != 0 && len > 0 {
            // u64s, so the signer info read from its start is aligned as the struct needs (its blobs point into it).
            let mut buf = vec![0u64; (len as usize).div_ceil(8)];
            if CryptMsgGetParam(msg, CMSG_SIGNER_INFO_PARAM, 0, buf.as_mut_ptr() as *mut _, &mut len) != 0 {
                let signer = &*(buf.as_ptr() as *const CMSG_SIGNER_INFO);
                let mut info: CERT_INFO = std::mem::zeroed();
                info.Issuer = signer.Issuer;
                info.SerialNumber = signer.SerialNumber;
                let cert = CertFindCertificateInStore(store, X509_ASN_ENCODING | PKCS_7_ASN_ENCODING, 0, CERT_FIND_SUBJECT_CERT, &info as *const CERT_INFO as *const _, std::ptr::null());
                if !cert.is_null() {
                    let subject = &(*(*cert).pCertInfo).Subject;
                    let mut name = vec![0u16; 1024];
                    let n = CertNameToStrW(X509_ASN_ENCODING, subject, CERT_X500_NAME_STR, name.as_mut_ptr(), name.len() as u32);
                    if n > 1 {
                        out = Some(from_wide(&name));
                    }
                    CertFreeCertificateContext(cert);
                }
            }
        }
        CryptMsgClose(msg);
        CertCloseStore(store, 0);
        out
    }
}

/// Authenticode check of the file's embedded signature (chain and integrity), without any UI.
fn has_valid_signature(path: &Path) -> bool {
    let p = wide(&path.to_string_lossy());
    // SAFETY: the structures are filled as WinVerifyTrust wants and live for the call.
    unsafe {
        let mut file: WINTRUST_FILE_INFO = std::mem::zeroed();
        file.cbStruct = std::mem::size_of::<WINTRUST_FILE_INFO>() as u32;
        file.pcwszFilePath = p.as_ptr();
        let mut data: WINTRUST_DATA = std::mem::zeroed();
        data.cbStruct = std::mem::size_of::<WINTRUST_DATA>() as u32;
        data.dwUIChoice = WTD_UI_NONE;
        data.fdwRevocationChecks = WTD_REVOKE_NONE;
        data.dwUnionChoice = WTD_CHOICE_FILE;
        data.Anonymous.pFile = &mut file;
        data.dwStateAction = WTD_STATEACTION_IGNORE;
        // GenericVerifyV2: 00AAC56B-CD44-11d0-8CC2-00C04FC295EE.
        let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        WinVerifyTrust(std::ptr::null_mut::<std::ffi::c_void>() as HWND, &mut action, &mut data as *mut WINTRUST_DATA as *mut _) == 0
    }
}
