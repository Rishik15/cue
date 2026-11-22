//! File: props.rs
//! Purpose: What both providers share: the C-style Windows constants under upper-case names, the provider options, VARIANT and runtime-id helpers.
//! Contents: property and direction constants; OPTIONS; variant_of; runtime_id.

use windows::core::{Error, Result};
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Ole::{SafeArrayCreateVector, SafeArrayPutElement};
use windows::Win32::System::Variant::{VARIANT, VT_I4};
use windows::Win32::UI::Accessibility::*;

// The Windows crate names its constants in the C style; patterns need upper-case names, so the ones used here are renamed once.
pub const NAME: UIA_PROPERTY_ID = UIA_NamePropertyId;
pub const CONTROL_TYPE: UIA_PROPERTY_ID = UIA_ControlTypePropertyId;
pub const IS_CONTROL: UIA_PROPERTY_ID = UIA_IsControlElementPropertyId;
pub const IS_CONTENT: UIA_PROPERTY_ID = UIA_IsContentElementPropertyId;
pub const IS_ENABLED: UIA_PROPERTY_ID = UIA_IsEnabledPropertyId;
pub const IS_FOCUSABLE: UIA_PROPERTY_ID = UIA_IsKeyboardFocusablePropertyId;
pub const HAS_FOCUS: UIA_PROPERTY_ID = UIA_HasKeyboardFocusPropertyId;
pub const POSITION_IN_SET: UIA_PROPERTY_ID = UIA_PositionInSetPropertyId;
pub const SIZE_OF_SET: UIA_PROPERTY_ID = UIA_SizeOfSetPropertyId;
pub const FIRST_CHILD: NavigateDirection = NavigateDirection_FirstChild;
pub const LAST_CHILD: NavigateDirection = NavigateDirection_LastChild;
pub const PARENT: NavigateDirection = NavigateDirection_Parent;
pub const NEXT_SIBLING: NavigateDirection = NavigateDirection_NextSibling;
pub const PREVIOUS_SIBLING: NavigateDirection = NavigateDirection_PreviousSibling;

/// What both providers declare: server-side, and they set focus themselves. The menu window never takes focus (that would close the tray overflow behind it), so UI Automation must not move native focus to it for a client's `SetFocus`.
pub const OPTIONS: ProviderOptions = ProviderOptions(ProviderOptions_ServerSideProvider.0 | ProviderOptions_ProviderOwnsSetFocus.0);

/// A VARIANT for property answers. Input: the value. Return: the variant.
pub fn variant_of(v: impl Into<VARIANT>) -> VARIANT {
    v.into()
}

/// The runtime id of an element: `UiaAppendRuntimeId` then `ids`, as a SAFEARRAY of 32-bit integers. Return: the array, or an error when memory ran out. Safety: creates the array and fills indexes inside its bounds.
pub fn runtime_id(ids: &[i32]) -> Result<*mut SAFEARRAY> {
    unsafe {
        let all: Vec<i32> = std::iter::once(UiaAppendRuntimeId as i32).chain(ids.iter().copied()).collect();
        let array = SafeArrayCreateVector(VT_I4, 0, all.len() as u32);
        if array.is_null() {
            return Err(Error::from(windows::Win32::Foundation::E_OUTOFMEMORY));
        }
        for (i, id) in all.iter().enumerate() {
            SafeArrayPutElement(array, &(i as i32), id as *const i32 as *const _)?;
        }
        Ok(array)
    }
}
