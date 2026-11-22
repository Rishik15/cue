//! File: abi.rs
//! Purpose: Makes the providers' "no such thing" answers NULL at the COM boundary. The `windows` crate's generated adapters write an interface out-pointer only for `Ok`, and an empty error maps to `S_OK`, so a missing pattern, neighbour or host would report success and leave the caller's pointer as it was. UI Automation expects `S_OK` with a NULL pointer, so these vtables write NULL first and then run the stock adapter unchanged.
//! Contents: simple / fragment / fragment_root — the stock vtable of an interface with its interface-returning methods swapped for NULL-first versions; install — points a live object's interface at such a vtable.

use std::ffi::c_void;
use std::ptr::null_mut;

use windows::core::{Interface, HRESULT};
use windows::Win32::Foundation::E_POINTER;
use windows::Win32::UI::Accessibility::*;

/// Defines a method that nulls its interface out-pointer, then calls the stock adapter for the same method. Input: the new function's name, the vtable type and Impl trait it belongs to, the method and its extra arguments.
macro_rules! null_first {
    ($name:ident, $vtbl:ident, $impl_trait:ident, $method:ident ( $($arg:ident : $ty:ty),* )) => {
        unsafe extern "system" fn $name<T: $impl_trait, const O: isize>(this: *mut c_void, $($arg: $ty,)* out: *mut *mut c_void) -> HRESULT {
            if out.is_null() {
                return E_POINTER;
            }
            // SAFETY: `out` is the caller's valid out-pointer; the stock adapter gets the same arguments.
            unsafe {
                out.write(null_mut());
                ($vtbl::new::<T, O>().$method)(this, $($arg,)* out)
            }
        }
    };
}

null_first!(pattern, IRawElementProviderSimple_Vtbl, IRawElementProviderSimple_Impl, GetPatternProvider(id: UIA_PATTERN_ID));
null_first!(host, IRawElementProviderSimple_Vtbl, IRawElementProviderSimple_Impl, HostRawElementProvider());
null_first!(navigate, IRawElementProviderFragment_Vtbl, IRawElementProviderFragment_Impl, Navigate(direction: NavigateDirection));
null_first!(from_point, IRawElementProviderFragmentRoot_Vtbl, IRawElementProviderFragmentRoot_Impl, ElementProviderFromPoint(x: f64, y: f64));
null_first!(focus, IRawElementProviderFragmentRoot_Vtbl, IRawElementProviderFragmentRoot_Impl, GetFocus());

/// The `IRawElementProviderSimple` vtable of `T` with NULL-first pattern and host answers. `O` is the offset the `implement` macro gives that interface (minus one, minus its position in the interface list).
pub const fn simple<T: IRawElementProviderSimple_Impl, const O: isize>() -> IRawElementProviderSimple_Vtbl {
    let mut v = IRawElementProviderSimple_Vtbl::new::<T, O>();
    v.GetPatternProvider = pattern::<T, O>;
    v.HostRawElementProvider = host::<T, O>;
    v
}

/// The `IRawElementProviderFragment` vtable of `T` with a NULL-first `Navigate`. `O` as in `simple`.
pub const fn fragment<T: IRawElementProviderFragment_Impl, const O: isize>() -> IRawElementProviderFragment_Vtbl {
    let mut v = IRawElementProviderFragment_Vtbl::new::<T, O>();
    v.Navigate = navigate::<T, O>;
    v
}

/// The `IRawElementProviderFragmentRoot` vtable of `T` with NULL-first point and focus lookups. `O` as in `simple`.
pub const fn fragment_root<T: IRawElementProviderFragmentRoot_Impl, const O: isize>() -> IRawElementProviderFragmentRoot_Vtbl {
    let mut v = IRawElementProviderFragmentRoot_Vtbl::new::<T, O>();
    v.ElementProviderFromPoint = from_point::<T, O>;
    v.GetFocus = focus::<T, O>;
    v
}

/// Points an interface of a freshly built `implement` object at `vtbl`. An interface pointer of such an object is the address of its `&'static` vtable field, so this replaces that field; later `QueryInterface` calls hand out the same field. Input: the interface and the replacement vtable, which must be the stock vtable of the same object with only the methods' behaviour changed. Safety: call it before the object is shared with another thread, once per interface.
pub unsafe fn install<I: Interface>(interface: &I, vtbl: &'static I::Vtable) {
    unsafe { (interface.as_raw() as *mut &'static I::Vtable).write(vtbl) }
}
