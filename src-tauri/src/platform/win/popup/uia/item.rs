//! File: item.rs
//! Purpose: The MenuItem provider, one per row: named like its row, with position in set, size of set, a rectangle and the Invoke pattern. Invoke and SetFocus post a request (with the menu's token) back to the menu window.
//! Contents: Item — the provider; Shared::item — builds one; the interface impls.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use std::ffi::c_void;

use windows::core::{implement, ComObjectInner, Error, IUnknownImpl, Interface, Result, HRESULT};
use windows::Win32::Foundation::{E_FAIL, E_POINTER, LPARAM, WPARAM};
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use super::super::rows::Row;
use super::ifaces::*;
use super::props::*;
use super::{Shared, FOCUS_MESSAGE, INVOKE_MESSAGE};

/// One menu item: a MenuItem named like its row, invokable.
#[implement(IProviderSimple, IProviderFragment, IInvokeProvider)]
pub struct Item {
    shared: Arc<Shared>,
    pos: usize,
}

impl Shared {
    /// The Item provider of the row at `pos`. Input: the position in `rows`. Return: the provider as a simple provider.
    pub fn item(self: &Arc<Self>, pos: usize) -> IProviderSimple {
        Item { shared: self.clone(), pos }.into_object().into_interface()
    }
}

impl Item_Impl {
    /// A property of the item. Return: its value, empty for a property the item does not have.
    fn property(&self, id: UIA_PROPERTY_ID) -> Result<VARIANT> {
        self.shared.live()?;
        let focused = self.shared.focus.load(Ordering::Acquire) == self.pos as isize;
        Ok(match id {
            NAME => variant_of(self.row()?.name.as_str()),
            CONTROL_TYPE => variant_of(UIA_MenuItemControlTypeId.0),
            POSITION_IN_SET => variant_of(self.pos as i32 + 1),
            SIZE_OF_SET => variant_of(self.shared.rows.len() as i32),
            HAS_FOCUS => variant_of(focused),
            IS_FOCUSABLE | IS_CONTROL | IS_CONTENT | IS_ENABLED => variant_of(true),
            _ => VARIANT::default(),
        })
    }

    /// The rectangle of the item's row. Return: the rectangle in screen pixels.
    fn rect(&self) -> Result<UiaRect> {
        self.shared.live()?;
        let r = self.row()?;
        Ok(UiaRect { left: r.left, top: r.top, width: r.width, height: r.height })
    }

    /// The row this item stands for. Return: the row, or an error when the menu has fewer rows (it cannot, rows never change).
    fn row(&self) -> Result<&Row> {
        self.shared.rows.get(self.pos).ok_or_else(|| Error::from(E_FAIL))
    }

    /// Posts `message` with this item's entry index and the menu's token to the menu window, which handles it on its own thread and drops it when the token is not the open menu's. Return: an error when the menu is gone.
    fn post(&self, message: u32) -> Result<()> {
        self.shared.live()?;
        let entry = self.row()?.entry;
        unsafe { PostMessageW(Some(self.shared.window()), message, WPARAM(entry), LPARAM(self.shared.token as isize)) }
    }
}

impl IProviderSimple_Impl for Item_Impl {
    unsafe fn ProviderOptions(&self, out: *mut ProviderOptions) -> HRESULT {
        unsafe { put(out, Ok(OPTIONS)) }
    }
    unsafe fn GetPatternProvider(&self, pattern: UIA_PATTERN_ID, out: *mut *mut c_void) -> HRESULT {
        let found = self.shared.live().map(|_| (pattern == UIA_InvokePatternId).then(|| self.to_interface::<IInvokeProvider>()));
        unsafe { give(out, found) }
    }
    unsafe fn GetPropertyValue(&self, id: UIA_PROPERTY_ID, out: *mut VARIANT) -> HRESULT {
        unsafe { put(out, self.property(id)) }
    }
    unsafe fn HostRawElementProvider(&self, out: *mut *mut c_void) -> HRESULT {
        unsafe { give(out, self.shared.live().map(|_| None::<IRawElementProviderSimple>)) }
    }
}

impl Item_Impl {
    /// The neighbour in `direction`. Return: the parent, the next or previous item, None when there is none.
    fn neighbour(&self, direction: NavigateDirection) -> Result<Option<IProviderFragment>> {
        self.shared.live()?;
        let count = self.shared.rows.len();
        match direction {
            PARENT => self.shared.root().cast().map(Some),
            NEXT_SIBLING if self.pos + 1 < count => self.shared.item(self.pos + 1).cast().map(Some),
            PREVIOUS_SIBLING if self.pos > 0 => self.shared.item(self.pos - 1).cast().map(Some),
            _ => Ok(None),
        }
    }
}

impl IProviderFragment_Impl for Item_Impl {
    unsafe fn Navigate(&self, direction: NavigateDirection, out: *mut *mut c_void) -> HRESULT {
        unsafe { give(out, self.neighbour(direction)) }
    }
    unsafe fn GetRuntimeId(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
        if out.is_null() {
            // Refused before `runtime_id` allocates an array nobody would free.
            return E_POINTER;
        }
        unsafe { put(out, self.shared.live().and_then(|_| runtime_id(&[self.pos as i32 + 1]))) }
    }
    unsafe fn BoundingRectangle(&self, out: *mut UiaRect) -> HRESULT {
        unsafe { put(out, self.rect()) }
    }
    unsafe fn GetEmbeddedFragmentRoots(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
        unsafe { put(out, self.shared.live().map(|_| std::ptr::null_mut())) }
    }
    unsafe fn SetFocus(&self) -> HRESULT {
        self.post(FOCUS_MESSAGE).into()
    }
    unsafe fn FragmentRoot(&self, out: *mut *mut c_void) -> HRESULT {
        unsafe { give(out, self.shared.live().and_then(|_| self.shared.root().cast::<IProviderFragmentRoot>()).map(Some)) }
    }
}

impl IInvokeProvider_Impl for Item_Impl {
    fn Invoke(&self) -> Result<()> {
        self.post(INVOKE_MESSAGE)
    }
}
