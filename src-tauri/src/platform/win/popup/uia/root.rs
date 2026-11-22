//! File: root.rs
//! Purpose: The Menu root provider: named "Cue", its children are the items, it answers point and focus lookups. See `ifaces` for why missing answers are written as NULL and `Shared::live` for the closed-menu answer.
//! Contents: Root — the provider; Shared::root — builds one; the three interface impls.

use std::ffi::c_void;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use windows::core::{implement, ComObjectInner, IUnknown, IUnknownImpl, Interface, Result, HRESULT};
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::*;

use super::super::rows::row_at;
use super::ifaces::*;
use super::props::*;
use super::Shared;

/// The menu itself: a Menu whose children are the items.
#[implement(IProviderSimple, IProviderFragment, IProviderFragmentRoot)]
pub struct Root {
    shared: Arc<Shared>,
}

impl Shared {
    /// The Root provider of the menu. Return: the provider as a simple provider.
    pub fn root(self: &Arc<Self>) -> IProviderSimple {
        Root { shared: self.clone() }.into_object().into_interface()
    }

    /// The bounding rectangle of the whole menu card, from its rows. Return: the rectangle in screen pixels (without the shadow margin).
    fn bounds(&self) -> UiaRect {
        match (self.rows.first(), self.rows.last()) {
            (Some(a), Some(b)) => UiaRect { left: a.left, top: a.top, width: a.width, height: b.top + b.height - a.top },
            _ => UiaRect::default(),
        }
    }
}

/// Safety for the three impls below: they are called through COM, so every out-pointer is the caller's (NULL or valid), which `give` relies on.
impl IProviderSimple_Impl for Root_Impl {
    unsafe fn ProviderOptions(&self, out: *mut ProviderOptions) -> HRESULT {
        unsafe { put(out, Ok(OPTIONS)) }
    }
    unsafe fn GetPatternProvider(&self, _: UIA_PATTERN_ID, out: *mut *mut c_void) -> HRESULT {
        unsafe { give(out, self.shared.live().map(|_| None::<IUnknown>)) }
    }
    unsafe fn GetPropertyValue(&self, id: UIA_PROPERTY_ID, out: *mut VARIANT) -> HRESULT {
        unsafe { put(out, self.property(id)) }
    }
    unsafe fn HostRawElementProvider(&self, out: *mut *mut c_void) -> HRESULT {
        let host = self.shared.live().and_then(|_| unsafe { UiaHostProviderFromHwnd(self.shared.window()) });
        unsafe { give(out, host.map(Some)) }
    }
}

impl Root_Impl {
    /// A property of the menu. Return: its value, empty for a property the menu does not have.
    fn property(&self, id: UIA_PROPERTY_ID) -> Result<VARIANT> {
        self.shared.live()?;
        Ok(match id {
            NAME => variant_of("Cue"),
            CONTROL_TYPE => variant_of(UIA_MenuControlTypeId.0),
            IS_CONTROL | IS_CONTENT | IS_ENABLED => variant_of(true),
            _ => VARIANT::default(),
        })
    }

    /// The child in `direction`. Return: the first or last item, None for any other direction.
    fn child(&self, direction: NavigateDirection) -> Result<Option<IProviderFragment>> {
        self.shared.live()?;
        let count = self.shared.rows.len();
        let pos = match direction {
            FIRST_CHILD if count > 0 => 0,
            LAST_CHILD if count > 0 => count - 1,
            _ => return Ok(None),
        };
        self.shared.item(pos).cast::<IProviderFragment>().map(Some)
    }
}

impl IProviderFragment_Impl for Root_Impl {
    unsafe fn Navigate(&self, direction: NavigateDirection, out: *mut *mut c_void) -> HRESULT {
        unsafe { give(out, self.child(direction)) }
    }
    unsafe fn GetRuntimeId(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
        unsafe { put(out, self.shared.live().map(|_| std::ptr::null_mut())) }
    }
    unsafe fn BoundingRectangle(&self, out: *mut UiaRect) -> HRESULT {
        unsafe { put(out, self.shared.live().map(|_| self.shared.bounds())) }
    }
    unsafe fn GetEmbeddedFragmentRoots(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
        unsafe { put(out, self.shared.live().map(|_| std::ptr::null_mut())) }
    }
    unsafe fn SetFocus(&self) -> HRESULT {
        self.shared.live().into()
    }
    unsafe fn FragmentRoot(&self, out: *mut *mut c_void) -> HRESULT {
        unsafe { give(out, self.shared.live().map(|_| Some(self.to_interface::<IProviderFragmentRoot>()))) }
    }
}

impl IProviderFragmentRoot_Impl for Root_Impl {
    unsafe fn ElementProviderFromPoint(&self, x: f64, y: f64, out: *mut *mut c_void) -> HRESULT {
        let hit = self.shared.live().and_then(|_| match row_at(&self.shared.rows, x, y) {
            Some(pos) => self.shared.item(pos).cast::<IProviderFragment>().map(Some),
            None => Ok(None),
        });
        unsafe { give(out, hit) }
    }
    unsafe fn GetFocus(&self, out: *mut *mut c_void) -> HRESULT {
        let focused = self.shared.live().and_then(|_| match usize::try_from(self.shared.focus.load(Ordering::Acquire)) {
            Ok(pos) => self.shared.item(pos).cast::<IProviderFragment>().map(Some),
            Err(_) => Ok(None),
        });
        unsafe { give(out, focused) }
    }
}
