//! File: hook_delivery.rs
//! Purpose: Lab measurement (ignored by default) of whether a keyboard hook delivers injected keys: the permanent kind (made once) and the per-recording kind (made fresh each cycle, read after 0, 20 and 100 ms).
//! Contents: inject — key down/up through the OS; delivered — whether a listener reports the key; the ignored test `hook_delivery_rates` that prints the pass rate of each kind over many cycles.
//! shortcut: calls user32 directly, as a lab outside the app and not through `platform/`; move behind `platform` if the lab ever ships.
//! Run on Windows: `cargo test --manifest-path src-tauri/Cargo.toml --test hook_delivery -- --ignored --nocapture`, or `scripts/cue.ps1 lab hooks`.

#![cfg(windows)]

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use handy_keys::{Key, KeyboardListener};

#[link(name = "user32")]
extern "system" {
    fn keybd_event(vk: u8, scan: u8, flags: u32, extra: usize);
}

const VK_F24: u8 = 0x87;
const KEYUP: u32 = 2;
const CYCLES: usize = 200;

/// Presses and releases F24 through the OS input path.
fn inject() {
    unsafe {
        keybd_event(VK_F24, 0, 0, 0);
        keybd_event(VK_F24, 0, KEYUP, 0);
    }
}

/// Whether `listener` reports F24 going down within `wait`. Input: the listener and the time to wait. Returns true when the key was seen.
fn delivered(listener: &KeyboardListener, wait: Duration) -> bool {
    let end = std::time::Instant::now() + wait;
    while let Some(left) = end.checked_duration_since(std::time::Instant::now()) {
        match listener.recv_timeout(left) {
            Ok(ev) if ev.key == Some(Key::F24) && ev.is_key_down => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    false
}

/// Prints how often each kind of hook saw the injected key.
#[test]
#[ignore = "lab: injects keys on the real desktop"]
fn hook_delivery_rates() {
    let permanent = KeyboardListener::new_with_blocking(Arc::new(Mutex::new(HashSet::new()))).expect("permanent hook");
    sleep(Duration::from_millis(100));
    let mut permanent_seen = 0;
    for _ in 0..CYCLES {
        inject();
        if delivered(&permanent, Duration::from_millis(300)) {
            permanent_seen += 1;
        }
        sleep(Duration::from_millis(10));
    }
    let mut fresh_with_permanent = 0;
    for _ in 0..CYCLES {
        let rec = KeyboardListener::new_recording().expect("recording hook");
        sleep(Duration::from_millis(20));
        inject();
        if delivered(&rec, Duration::from_millis(300)) {
            fresh_with_permanent += 1;
        }
        drop(rec);
        while permanent.try_recv().is_some() {}
        sleep(Duration::from_millis(10));
    }
    println!("fresh hook, permanent alive, inject at +20: {fresh_with_permanent}/{CYCLES}");
    drop(permanent);

    let mut fresh = [0usize; 3];
    for (slot, pause_ms) in [0u64, 20, 100].into_iter().enumerate() {
        for _ in 0..CYCLES {
            let rec = KeyboardListener::new_recording().expect("recording hook");
            sleep(Duration::from_millis(pause_ms));
            inject();
            if delivered(&rec, Duration::from_millis(300)) {
                fresh[slot] += 1;
            }
            drop(rec);
            sleep(Duration::from_millis(10));
        }
    }
    println!("permanent hook:            {permanent_seen}/{CYCLES}");
    println!("fresh hook, inject at +0:   {}/{CYCLES}", fresh[0]);
    println!("fresh hook, inject at +20:  {}/{CYCLES}", fresh[1]);
    println!("fresh hook, inject at +100: {}/{CYCLES}", fresh[2]);
}

/// Lab: logs every key a separate process hears for 120 s to `C:\dev\cue\observer.log`, to compare with what Cue's own hooks heard.
#[test]
#[ignore = "lab: watches the real keyboard"]
fn observe_keys() {
    use std::io::Write;
    let listener = KeyboardListener::new_with_blocking(Arc::new(Mutex::new(HashSet::new()))).expect("observer hook");
    let mut out = std::fs::File::create("C:\\dev\\cue\\observer.log").expect("log file");
    let end = std::time::Instant::now() + Duration::from_secs(120);
    while std::time::Instant::now() < end {
        if let Ok(ev) = listener.recv_timeout(Duration::from_millis(200)) {
            let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
            let _ = writeln!(out, "{ms} key={:?} down={} mods={:?}", ev.key, ev.is_key_down, ev.modifiers);
        }
    }
}
