//! File: tests.rs
//! Purpose: Checks the providers at the COM boundary the way UI Automation calls them: through the raw vtable with a garbage-filled out-pointer, so a missing answer must be written as NULL, and with providers kept past the menu's life.
//! Contents: sentinel-output tests (Root and Item), provider options, retained-provider tests (destroy, reopen, other thread).

use std::ffi::c_void;
use std::sync::Arc;

use windows::core::{Interface, HRESULT};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::*;

use super::*;
use crate::platform::win::popup::rows::Row;

const GARBAGE: *mut c_void = 0xDEAD_0000usize as *mut c_void;
const UNAVAILABLE: HRESULT = HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32);

/// Two rows at entry indexes 0 and 2 (a separator between them).
fn two_rows() -> Vec<Row> {
    (0..2).map(|i| Row { entry: i * 2, name: format!("Row {i}"), left: 0.0, top: i as f64 * 20.0, width: 100.0, height: 20.0 }).collect()
}

/// A fresh open menu's state. Return: the state for a window that does not exist (providers only read it).
fn menu() -> Arc<Shared> {
    Arc::new(Shared::new(HWND(std::ptr::null_mut()), two_rows()))
}

/// Calls a method that fills an interface out-pointer that starts as garbage. Input: the call, given the out-pointer. Return: its HRESULT and what the pointer holds afterwards.
fn call(f: impl FnOnce(*mut *mut c_void) -> HRESULT) -> (HRESULT, *mut c_void) {
    let mut out = GARBAGE;
    let hr = f(&mut out);
    (hr, out)
}

/// Asserts a call answered `S_OK` with a NULL pointer. Input: the call's result.
fn assert_null(result: (HRESULT, *mut c_void)) {
    assert_eq!(result.0, HRESULT(0));
    assert!(result.1.is_null(), "the out-pointer kept {:?}", result.1);
}

#[test]
fn item_answers_null_for_a_missing_pattern_host_and_neighbour() {
    let item = menu().item(1);
    let fragment = item.cast::<IRawElementProviderFragment>().unwrap();
    assert_null(call(|o| unsafe { (item.vtable().GetPatternProvider)(item.as_raw(), UIA_TextPatternId, o) }));
    assert_null(call(|o| unsafe { (item.vtable().HostRawElementProvider)(item.as_raw(), o) }));
    assert_null(call(|o| unsafe { (fragment.vtable().Navigate)(fragment.as_raw(), NavigateDirection_NextSibling, o) }));
    assert_null(call(|o| unsafe { (fragment.vtable().Navigate)(fragment.as_raw(), NavigateDirection_FirstChild, o) }));
}

#[test]
fn item_still_hands_out_what_exists() {
    let shared = menu();
    let item = shared.item(1);
    let fragment = item.cast::<IRawElementProviderFragment>().unwrap();
    let (hr, invoke) = call(|o| unsafe { (item.vtable().GetPatternProvider)(item.as_raw(), UIA_InvokePatternId, o) });
    assert_eq!((hr, invoke.is_null()), (HRESULT(0), false));
    let (hr, previous) = call(|o| unsafe { (fragment.vtable().Navigate)(fragment.as_raw(), NavigateDirection_PreviousSibling, o) });
    assert_eq!((hr, previous.is_null()), (HRESULT(0), false));
}

#[test]
fn root_answers_null_for_missing_pattern_neighbour_point_and_focus() {
    let root = menu().root();
    let fragment = root.cast::<IRawElementProviderFragment>().unwrap();
    let top = root.cast::<IRawElementProviderFragmentRoot>().unwrap();
    assert_null(call(|o| unsafe { (root.vtable().GetPatternProvider)(root.as_raw(), UIA_InvokePatternId, o) }));
    assert_null(call(|o| unsafe { (fragment.vtable().Navigate)(fragment.as_raw(), NavigateDirection_Parent, o) }));
    assert_null(call(|o| unsafe { (top.vtable().ElementProviderFromPoint)(top.as_raw(), 500.0, 500.0, o) }));
    assert_null(call(|o| unsafe { (top.vtable().GetFocus)(top.as_raw(), o) }));
}

#[test]
fn null_out_pointer_is_refused_not_written() {
    let item = menu().item(0);
    let hr = unsafe { (item.vtable().GetPatternProvider)(item.as_raw(), UIA_InvokePatternId, std::ptr::null_mut()) };
    assert!(hr.is_err());
}

#[test]
fn both_providers_own_set_focus() {
    let shared = menu();
    for provider in [shared.root(), shared.item(0)] {
        let options = unsafe { provider.ProviderOptions() }.unwrap();
        assert_ne!(options.0 & ProviderOptions_ProviderOwnsSetFocus.0, 0);
        assert_ne!(options.0 & ProviderOptions_ServerSideProvider.0, 0);
    }
}

#[test]
fn a_provider_kept_past_the_menu_answers_unavailable_and_nulls_its_outputs() {
    let shared = session(HWND(std::ptr::null_mut()), two_rows);
    let root = shared.root();
    let item = shared.item(0);
    let fragment = item.cast::<IRawElementProviderFragment>().unwrap();
    assert!(unsafe { fragment.BoundingRectangle() }.is_ok());
    assert!(retire());
    assert_eq!(unsafe { fragment.BoundingRectangle() }.unwrap_err().code(), UNAVAILABLE);
    assert_eq!(unsafe { root.GetPropertyValue(UIA_NamePropertyId) }.unwrap_err().code(), UNAVAILABLE);
    let (hr, out) = call(|o| unsafe { (item.vtable().GetPatternProvider)(item.as_raw(), UIA_InvokePatternId, o) });
    assert!((hr, out) == (UNAVAILABLE, std::ptr::null_mut()));
    let invoke = unsafe { item.GetPatternProvider(UIA_InvokePatternId) };
    assert_eq!(invoke.unwrap_err().code(), UNAVAILABLE);
}

#[test]
fn a_retired_item_does_not_post_and_a_reopened_menu_ignores_its_token() {
    let first = session(HWND(std::ptr::null_mut()), two_rows);
    let stale = first.token;
    let item = first.item(0).cast::<IInvokeProvider>().unwrap();
    assert!(is_current(LPARAM(stale as isize)));
    retire();
    assert_eq!(unsafe { item.Invoke() }.unwrap_err().code(), UNAVAILABLE);
    let second = session(HWND(std::ptr::null_mut()), two_rows);
    assert_ne!(second.token, stale);
    assert!(is_current(LPARAM(second.token as isize)));
    assert!(!is_current(LPARAM(stale as isize)));
    assert_eq!(unsafe { item.Invoke() }.unwrap_err().code(), UNAVAILABLE);
    retire();
}

#[test]
fn closing_the_menu_is_seen_from_another_thread() {
    let shared = menu();
    let root = shared.clone();
    shared.open.store(false, Ordering::Release);
    let seen = std::thread::spawn(move || root.live().unwrap_err().code()).join().unwrap();
    assert_eq!(seen, UNAVAILABLE);
}
