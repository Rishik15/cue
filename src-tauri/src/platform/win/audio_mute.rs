//! Purpose: Mute and restore the default speakers (Windows Core Audio), so music and videos stay out of the microphone while dictating.
//! Contents: mute_output — mute the default output; returns its id only when this call did the muting; unmute_output — restore that device.
//! Each call initialises COM on the calling thread, so call from a short-lived thread.

use windows::core::PCWSTR;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eConsole, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};

/// COM is initialised for as long as this lives.
struct Com;

impl Com {
    fn init() -> Com {
        // An error means the thread already has a COM apartment, which is fine to use; the balancing uninitialise below is still correct.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Com
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn err(e: windows::core::Error) -> String {
    format!("Audio device error: {e}")
}

fn enumerator() -> Result<IMMDeviceEnumerator, String> {
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(err)
}

fn volume(device: &IMMDevice) -> Result<IAudioEndpointVolume, String> {
    unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) }.map_err(err)
}

/// `Ok(None)` when the speakers were already muted: then there is nothing to restore later.
pub fn mute_output() -> Result<Option<String>, String> {
    let _com = Com::init();
    let device = unsafe { enumerator()?.GetDefaultAudioEndpoint(eRender, eConsole) }.map_err(err)?;
    let volume = volume(&device)?;
    unsafe {
        if volume.GetMute().map_err(err)?.as_bool() {
            return Ok(None);
        }
        volume.SetMute(true, std::ptr::null()).map_err(err)?;
        let id = device.GetId().map_err(err)?;
        let text = id.to_string().map_err(|e| format!("Audio device error: {e}"));
        CoTaskMemFree(Some(id.0 as _));
        text.map(Some)
    }
}

/// Unmutes the device `mute_output` returned, even if the default output changed meanwhile.
pub fn unmute_output(id: &str) -> Result<(), String> {
    let _com = Com::init();
    let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
    let device = unsafe { enumerator()?.GetDevice(PCWSTR(wide.as_ptr())) }.map_err(err)?;
    unsafe { volume(&device)?.SetMute(false, std::ptr::null()) }.map_err(err)
}
