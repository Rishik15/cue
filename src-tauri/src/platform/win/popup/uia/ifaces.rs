//! File: ifaces.rs
//! Purpose: The three UI Automation provider interfaces, declared locally so a missing answer can be NULL. The `windows` crate's own `IRawElementProvider*` adapters write an interface out-pointer only for `Ok`, and an empty error maps to `S_OK`, so a missing pattern, neighbour or host would report success and leave the caller's pointer as it was; UI Automation expects `S_OK` with a NULL pointer. These declarations use the original IIDs and method order, so they are ABI-identical (a client cannot tell them from the originals, and `cast` to the `windows` types works through `QueryInterface`), but every method takes its raw out-pointer and returns an `HRESULT` (the `interface` macro has no other way to declare an out-parameter), so the provider itself writes NULL for the nullable ones. The `implement` macro still builds the object, so it owns the layout, offsets and reference counts; nothing here touches a vtable or the object's memory.
//! Contents: IProviderSimple / IProviderFragment / IProviderFragmentRoot — the interfaces (Microsoft: IRawElementProviderSimple, IRawElementProviderFragment, IRawElementProviderFragmentRoot); put — writes a value into an out-pointer; give — writes an optional interface into one.

// The methods keep Microsoft's names, which are the ABI's documented contract.
#![allow(non_snake_case)]

use std::ffi::c_void;
use std::ptr::null_mut;

use windows::core::{interface, IUnknown, IUnknown_Vtbl, Interface, Result, HRESULT};
use windows::Win32::Foundation::{E_POINTER, S_OK};
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::*;

/// `IRawElementProviderSimple`: options, patterns, properties and the host provider.
#[interface("d6dd68d1-86fd-4332-8666-9abedea2d24c")]
pub unsafe trait IProviderSimple: IUnknown {
    fn ProviderOptions(&self, out: *mut ProviderOptions) -> HRESULT;
    fn GetPatternProvider(&self, pattern: UIA_PATTERN_ID, out: *mut *mut c_void) -> HRESULT;
    fn GetPropertyValue(&self, property: UIA_PROPERTY_ID, out: *mut VARIANT) -> HRESULT;
    fn HostRawElementProvider(&self, out: *mut *mut c_void) -> HRESULT;
}

/// `IRawElementProviderFragment`: navigation, runtime id, rectangle, focus, fragment root.
#[interface("f7063da8-8359-439c-9297-bbc5299a7d87")]
pub unsafe trait IProviderFragment: IUnknown {
    fn Navigate(&self, direction: NavigateDirection, out: *mut *mut c_void) -> HRESULT;
    fn GetRuntimeId(&self, out: *mut *mut SAFEARRAY) -> HRESULT;
    fn BoundingRectangle(&self, out: *mut UiaRect) -> HRESULT;
    fn GetEmbeddedFragmentRoots(&self, out: *mut *mut SAFEARRAY) -> HRESULT;
    fn SetFocus(&self) -> HRESULT;
    fn FragmentRoot(&self, out: *mut *mut c_void) -> HRESULT;
}

/// `IRawElementProviderFragmentRoot`: hit testing and the focused child.
#[interface("620ce2a5-ab8f-40a9-86cb-de3c75599b58")]
pub unsafe trait IProviderFragmentRoot: IUnknown {
    fn ElementProviderFromPoint(&self, x: f64, y: f64, out: *mut *mut c_void) -> HRESULT;
    fn GetFocus(&self, out: *mut *mut c_void) -> HRESULT;
}

/// Answers a method that returns a value. Input: the caller's out-pointer and the outcome. Return: `S_OK` after writing the value, the error's code otherwise, `E_POINTER` when `out` is NULL. Safety: `out` must be NULL or valid for a write, as COM requires of an out-pointer.
pub unsafe fn put<T>(out: *mut T, value: Result<T>) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    match value {
        Ok(value) => {
            // SAFETY: `out` is non-NULL and the caller's valid out-pointer.
            unsafe { out.write(value) };
            S_OK
        }
        Err(error) => error.code(),
    }
}

/// Answers a method that returns an optional interface. Input: the caller's out-pointer and the outcome: `Ok(Some)` hands the interface over, `Ok(None)` is "there is none" (NULL with `S_OK`), `Err` is a failure (NULL with its code). Return: the `HRESULT`; `E_POINTER` when `out` is NULL. Safety: `out` must be NULL or valid for a pointer write, as COM requires of an out-pointer.
pub unsafe fn give<I: Interface>(out: *mut *mut c_void, found: Result<Option<I>>) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    // SAFETY: `out` is non-NULL and the caller's valid out-pointer.
    unsafe { out.write(null_mut()) };
    match found {
        Ok(Some(interface)) => {
            // SAFETY: as above; ownership of one reference passes to the caller.
            unsafe { out.write(interface.into_raw()) };
            S_OK
        }
        Ok(None) => S_OK,
        Err(error) => error.code(),
    }
}
