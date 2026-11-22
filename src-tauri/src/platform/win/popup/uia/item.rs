//! File: item.rs
//! Purpose: The MenuItem provider, one per row: named like its row, with position in set, size of set, a rectangle and the Invoke pattern. Invoke and SetFocus post a request (with the menu's token) back to the menu window.
//! Contents: Item — the provider; Shared::item — builds one with its NULL-first vtables installed; the interface impls.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use windows::core::{implement, ComObjectInner, Error, IUnknown, IUnknownImpl, Interface, Result};
use windows::Win32::Foundation::{E_FAIL, LPARAM, WPARAM};
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use super::super::rows::Row;
use super::abi::{fragment, install, simple};
use super::props::*;
use super::{Shared, FOCUS_MESSAGE, INVOKE_MESSAGE};

/// One menu item: a MenuItem named like its row, invokable.
#[implement(IRawElementProviderSimple, IRawElementProviderFragment, IInvokeProvider)]
pub struct Item {
    shared: Arc<Shared>,
    pos: usize,
}

// As in `root`: interface `n` has offset `-1 - n`.
static ITEM_SIMPLE: IRawElementProviderSimple_Vtbl = simple::<Item_Impl, -1>();
static ITEM_FRAGMENT: IRawElementProviderFragment_Vtbl = fragment::<Item_Impl, -2>();

impl Shared {
    /// The Item provider of the row at `pos`. Input: the position in `rows`. Return: the provider as a simple provider.
    pub fn item(self: &Arc<Self>, pos: usize) -> IRawElementProviderSimple {
        let object = Item { shared: self.clone(), pos }.into_object();
        let (simple, fragment) = (object.to_interface::<IRawElementProviderSimple>(), object.to_interface::<IRawElementProviderFragment>());
        // SAFETY: both vtables are the objects' own stock ones with NULL-first answers; the object is not shared yet.
        unsafe {
            install(&simple, &ITEM_SIMPLE);
            install(&fragment, &ITEM_FRAGMENT);
        }
        simple
    }
}

impl Item_Impl {
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

impl IRawElementProviderSimple_Impl for Item_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(OPTIONS)
    }
    fn GetPatternProvider(&self, pattern: UIA_PATTERN_ID) -> Result<IUnknown> {
        self.shared.live()?;
        if pattern == UIA_InvokePatternId {
            self.to_interface::<IInvokeProvider>().cast()
        } else {
            none()
        }
    }
    fn GetPropertyValue(&self, id: UIA_PROPERTY_ID) -> Result<VARIANT> {
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
    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        self.shared.live()?;
        none()
    }
}

impl IRawElementProviderFragment_Impl for Item_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        self.shared.live()?;
        let count = self.shared.rows.len();
        match direction {
            PARENT => self.shared.root().cast(),
            NEXT_SIBLING if self.pos + 1 < count => self.shared.item(self.pos + 1).cast(),
            PREVIOUS_SIBLING if self.pos > 0 => self.shared.item(self.pos - 1).cast(),
            _ => none(),
        }
    }
    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        self.shared.live()?;
        runtime_id(&[self.pos as i32 + 1])
    }
    fn BoundingRectangle(&self) -> Result<UiaRect> {
        self.shared.live()?;
        let r = self.row()?;
        Ok(UiaRect { left: r.left, top: r.top, width: r.width, height: r.height })
    }
    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        self.shared.live()?;
        Ok(std::ptr::null_mut())
    }
    fn SetFocus(&self) -> Result<()> {
        self.post(FOCUS_MESSAGE)
    }
    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        self.shared.live()?;
        self.shared.root().cast()
    }
}

impl IInvokeProvider_Impl for Item_Impl {
    fn Invoke(&self) -> Result<()> {
        self.post(INVOKE_MESSAGE)
    }
}
