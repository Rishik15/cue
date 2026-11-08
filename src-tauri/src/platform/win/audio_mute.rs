//! File: audio_mute.rs
//! Purpose: Mute and restore the default speakers (Windows Core Audio), so music and videos stay out of the microphone while dictating.
//! Contents: mute_output — mute the default output; returns its id only when this call did the muting; unmute_output — restore that device.
//! Each call initialises COM on the calling thread, so call from a short-lived thread.

use windows::core::PCWSTR;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eConsole, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL};

use super::com::Com;

/// Wraps a Core Audio error in wording the user can read.
fn err(e: windows::core::Error) -> String {
    format!("Audio device error: {e}")
}

/// Creates the device enumerator; COM must be initialised on this thread. Safety: Creates a COM object with no aliasing; the caller holds a `Com` guard for this thread.
fn enumerator() -> Result<IMMDeviceEnumerator, String> {
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(err)
}

/// Opens the endpoint volume control of `device`. Safety: `device` is a live COM interface; the control is returned owned.
fn volume(device: &IMMDevice) -> Result<IAudioEndpointVolume, String> {
    unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) }.map_err(err)
}

/// The id string of `device`. Safety: `GetId` hands over a CoTaskMem string that is read once and freed here, on every path.
fn device_id(device: &IMMDevice) -> Result<String, String> {
    unsafe {
        let id = device.GetId().map_err(err)?;
        let text = id.to_string().map_err(|e| format!("Audio device error: {e}"));
        CoTaskMemFree(Some(id.0 as _));
        text
    }
}

/// Mutes the default speakers. `Ok(None)` when they were already muted: then there is nothing to restore later. The device id is read before the mute, so a failure to name the device leaves the speakers untouched and a returned id always means "muted by this call"; a mute that then fails is undone before the error is returned. Safety: Plain Core Audio queries on interfaces owned by this call, on a thread with COM initialised.
pub fn mute_output() -> Result<Option<String>, String> {
    let _com = Com::init();
    let device = unsafe { enumerator()?.GetDefaultAudioEndpoint(eRender, eConsole) }.map_err(err)?;
    let volume = volume(&device)?;
    if unsafe { volume.GetMute() }.map_err(err)?.as_bool() {
        return Ok(None);
    }
    let id = device_id(&device)?;
    if let Err(e) = unsafe { volume.SetMute(true, std::ptr::null()) } {
        let _ = unsafe { volume.SetMute(false, std::ptr::null()) };
        return Err(err(e));
    }
    Ok(Some(id))
}

/// Unmutes the device `mute_output` returned, even if the default output changed meanwhile. Safety: `device` is a live COM interface. Safety: `wide` is a NUL-terminated string that outlives the call.
pub fn unmute_output(id: &str) -> Result<(), String> {
    let _com = Com::init();
    let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
    let device = unsafe { enumerator()?.GetDevice(PCWSTR(wide.as_ptr())) }.map_err(err)?;
    unsafe { volume(&device)?.SetMute(false, std::ptr::null()) }.map_err(err)
}
