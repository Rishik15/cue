//! File: root.rs
//! Purpose: The Menu root provider: named "Cue", its children are the items, it answers point and focus lookups. See `abi` for why missing answers are NULL and `Shared::live` for the closed-menu answer.
//! Contents: Root — the provider; Shared::root — builds one with its NULL-first vtables installed; the three interface impls.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use windows::core::{implement, ComObjectInner, IUnknown, IUnknownImpl, Interface, Result};
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::*;

use super::super::rows::row_at;
use super::abi::{fragment, fragment_root, install, simple};
use super::props::*;
use super::Shared;

/// The menu itself: a Menu whose children are the items.
#[implement(IRawElementProviderSimple, IRawElementProviderFragment, IRawElementProviderFragmentRoot)]
pub struct Root {
    shared: Arc<Shared>,
}

// The `implement` macro places the interfaces in the order listed, so interface `n` has offset `-1 - n`.
static ROOT_SIMPLE: IRawElementProviderSimple_Vtbl = simple::<Root_Impl, -1>();
static ROOT_FRAGMENT: IRawElementProviderFragment_Vtbl = fragment::<Root_Impl, -2>();
static ROOT_FRAGMENT_ROOT: IRawElementProviderFragmentRoot_Vtbl = fragment_root::<Root_Impl, -3>();

impl Shared {
    /// The Root provider of the menu. Return: the provider as a simple provider.
    pub fn root(self: &Arc<Self>) -> IRawElementProviderSimple {
        let object = Root { shared: self.clone() }.into_object();
        let (simple, fragment) = (object.to_interface::<IRawElementProviderSimple>(), object.to_interface::<IRawElementProviderFragment>());
        let top = object.to_interface::<IRawElementProviderFragmentRoot>();
        // SAFETY: as in `item`.
        unsafe {
            install(&simple, &ROOT_SIMPLE);
            install(&fragment, &ROOT_FRAGMENT);
            install(&top, &ROOT_FRAGMENT_ROOT);
        }
        simple
    }

    /// The bounding rectangle of the whole menu card, from its rows. Return: the rectangle in screen pixels (without the shadow margin).
    fn bounds(&self) -> UiaRect {
        match (self.rows.first(), self.rows.last()) {
            (Some(a), Some(b)) => UiaRect { left: a.left, top: a.top, width: a.width, height: b.top + b.height - a.top },
            _ => UiaRect::default(),
        }
    }
}

impl IRawElementProviderSimple_Impl for Root_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(OPTIONS)
    }
    fn GetPatternProvider(&self, _: UIA_PATTERN_ID) -> Result<IUnknown> {
        self.shared.live()?;
        none()
    }
    fn GetPropertyValue(&self, id: UIA_PROPERTY_ID) -> Result<VARIANT> {
        self.shared.live()?;
        Ok(match id {
            NAME => variant_of("Cue"),
            CONTROL_TYPE => variant_of(UIA_MenuControlTypeId.0),
            IS_CONTROL | IS_CONTENT | IS_ENABLED => variant_of(true),
            _ => VARIANT::default(),
        })
    }
    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        self.shared.live()?;
        unsafe { UiaHostProviderFromHwnd(self.shared.window()) }
    }
}

impl IRawElementProviderFragment_Impl for Root_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        self.shared.live()?;
        let count = self.shared.rows.len();
        let pos = match direction {
            FIRST_CHILD if count > 0 => 0,
            LAST_CHILD if count > 0 => count - 1,
            _ => return none(),
        };
        self.shared.item(pos).cast()
    }
    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        self.shared.live()?;
        Ok(std::ptr::null_mut())
    }
    fn BoundingRectangle(&self) -> Result<UiaRect> {
        self.shared.live()?;
        Ok(self.shared.bounds())
    }
    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        self.shared.live()?;
        Ok(std::ptr::null_mut())
    }
    fn SetFocus(&self) -> Result<()> {
        self.shared.live()
    }
    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        self.shared.live()?;
        Ok(self.to_interface())
    }
}

impl IRawElementProviderFragmentRoot_Impl for Root_Impl {
    fn ElementProviderFromPoint(&self, x: f64, y: f64) -> Result<IRawElementProviderFragment> {
        self.shared.live()?;
        match row_at(&self.shared.rows, x, y) {
            Some(pos) => self.shared.item(pos).cast(),
            None => none(),
        }
    }
    fn GetFocus(&self) -> Result<IRawElementProviderFragment> {
        self.shared.live()?;
        match usize::try_from(self.shared.focus.load(Ordering::Acquire)) {
            Ok(pos) => self.shared.item(pos).cast(),
            Err(_) => none(),
        }
    }
}
