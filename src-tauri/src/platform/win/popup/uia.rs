//! File: uia.rs
//! Purpose: Lets a screen reader read the tray menu. The menu is one painted window, so without this Narrator sees an unnamed picture. On `WM_GETOBJECT` (Microsoft: "Handling the WM_GETOBJECT Message", `UiaReturnRawElementProvider`) the window answers with a UI Automation fragment: a Menu root whose children are MenuItems with a name, position in set and size of set, a rectangle, the Invoke pattern, and a focus-changed event when the highlighted row moves. Nothing is built until a client asks, so an idle menu costs nothing.
//! Contents: Shared — the rows and the focused row, read from any thread; Root / Item — the providers; provider_for — the answer to `WM_GETOBJECT`; focus_moved — announce the highlighted row; release — tell UI Automation the window is gone; INVOKE_MESSAGE / FOCUS_MESSAGE — what an item posts back to the menu window.

use std::cell::RefCell;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;

use windows::core::{implement, Error, IUnknown, IUnknownImpl, Interface, Result};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Ole::{SafeArrayCreateVector, SafeArrayPutElement};
use windows::Win32::System::Variant::{VARIANT, VT_I4};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

use super::rows::{row_at, Row};

/// Posted to the menu window when a client invokes an item: `wparam` is the item's entry index.
pub const INVOKE_MESSAGE: u32 = WM_APP + 2;
/// Posted when a client moves focus to an item: `wparam` is the item's entry index.
pub const FOCUS_MESSAGE: u32 = WM_APP + 3;

/// What the providers read. Built once on the menu thread and then only read, except `focus`, so UI Automation may call from any thread. `focus` is the highlighted row's position in `rows`, or -1.
struct Shared {
    hwnd: isize,
    rows: Vec<Row>,
    focus: AtomicIsize,
}

thread_local! {
    static PROVIDER: RefCell<Option<Arc<Shared>>> = const { RefCell::new(None) };
}

// The Windows crate names its constants in the C style; patterns need upper-case names, so the ones used here are renamed once.
const NAME: UIA_PROPERTY_ID = UIA_NamePropertyId;
const CONTROL_TYPE: UIA_PROPERTY_ID = UIA_ControlTypePropertyId;
const IS_CONTROL: UIA_PROPERTY_ID = UIA_IsControlElementPropertyId;
const IS_CONTENT: UIA_PROPERTY_ID = UIA_IsContentElementPropertyId;
const IS_ENABLED: UIA_PROPERTY_ID = UIA_IsEnabledPropertyId;
const IS_FOCUSABLE: UIA_PROPERTY_ID = UIA_IsKeyboardFocusablePropertyId;
const HAS_FOCUS: UIA_PROPERTY_ID = UIA_HasKeyboardFocusPropertyId;
const POSITION_IN_SET: UIA_PROPERTY_ID = UIA_PositionInSetPropertyId;
const SIZE_OF_SET: UIA_PROPERTY_ID = UIA_SizeOfSetPropertyId;
const FIRST_CHILD: NavigateDirection = NavigateDirection_FirstChild;
const LAST_CHILD: NavigateDirection = NavigateDirection_LastChild;
const PARENT: NavigateDirection = NavigateDirection_Parent;
const NEXT_SIBLING: NavigateDirection = NavigateDirection_NextSibling;
const PREVIOUS_SIBLING: NavigateDirection = NavigateDirection_PreviousSibling;

/// A COM error that carries no failure code: the interface method then answers "no such element" with a null result, which is how UI Automation expects a missing neighbour or child.
fn none<T>() -> Result<T> {
    Err(Error::empty())
}

/// A VARIANT for property answers. Input: the value. Return: the variant.
fn variant_of(v: impl Into<VARIANT>) -> VARIANT {
    v.into()
}

/// The runtime id of an element: `UiaAppendRuntimeId` then `ids`, as a SAFEARRAY of 32-bit integers. Return: the array, or an error when memory ran out. Safety: creates the array and fills indexes inside its bounds.
fn runtime_id(ids: &[i32]) -> Result<*mut SAFEARRAY> {
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

impl Shared {
    /// The menu window. Return: its handle.
    fn window(&self) -> HWND {
        HWND(self.hwnd as *mut _)
    }

    /// The Item provider of the row at `pos`. Input: the position in `rows`. Return: the provider as a simple provider.
    fn item(self: &Arc<Self>, pos: usize) -> IRawElementProviderSimple {
        Item { shared: self.clone(), pos }.into()
    }

    /// The Root provider of the menu. Return: the provider as a simple provider.
    fn root(self: &Arc<Self>) -> IRawElementProviderSimple {
        Root { shared: self.clone() }.into()
    }

    /// The bounding rectangle of the whole menu, from its window. Return: the rectangle in screen pixels (the card without its shadow margin).
    fn bounds(&self) -> UiaRect {
        let (first, last) = (self.rows.first(), self.rows.last());
        match (first, last) {
            (Some(a), Some(b)) => UiaRect { left: a.left, top: a.top, width: a.width, height: b.top + b.height - a.top },
            _ => UiaRect::default(),
        }
    }
}

/// The menu itself: a Menu whose children are the items.
#[implement(IRawElementProviderSimple, IRawElementProviderFragment, IRawElementProviderFragmentRoot)]
struct Root {
    shared: Arc<Shared>,
}

/// One menu item: a MenuItem named like its row, invokable.
#[implement(IRawElementProviderSimple, IRawElementProviderFragment, IInvokeProvider)]
struct Item {
    shared: Arc<Shared>,
    pos: usize,
}

impl IRawElementProviderSimple_Impl for Root_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }
    fn GetPatternProvider(&self, _: UIA_PATTERN_ID) -> Result<IUnknown> {
        none()
    }
    fn GetPropertyValue(&self, id: UIA_PROPERTY_ID) -> Result<VARIANT> {
        Ok(match id {
            NAME => variant_of("Cue"),
            CONTROL_TYPE => variant_of(UIA_MenuControlTypeId.0),
            IS_CONTROL | IS_CONTENT | IS_ENABLED => variant_of(true),
            _ => VARIANT::default(),
        })
    }
    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        unsafe { UiaHostProviderFromHwnd(self.shared.window()) }
    }
}

impl IRawElementProviderFragment_Impl for Root_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        let count = self.shared.rows.len();
        let pos = match direction {
            FIRST_CHILD if count > 0 => 0,
            LAST_CHILD if count > 0 => count - 1,
            _ => return none(),
        };
        self.shared.item(pos).cast()
    }
    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
    fn BoundingRectangle(&self) -> Result<UiaRect> {
        Ok(self.shared.bounds())
    }
    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
    fn SetFocus(&self) -> Result<()> {
        Ok(())
    }
    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        Ok(self.to_interface())
    }
}

impl IRawElementProviderFragmentRoot_Impl for Root_Impl {
    fn ElementProviderFromPoint(&self, x: f64, y: f64) -> Result<IRawElementProviderFragment> {
        match row_at(&self.shared.rows, x, y) {
            Some(pos) => self.shared.item(pos).cast(),
            None => none(),
        }
    }
    fn GetFocus(&self) -> Result<IRawElementProviderFragment> {
        match usize::try_from(self.shared.focus.load(Ordering::Acquire)) {
            Ok(pos) => self.shared.item(pos).cast(),
            Err(_) => none(),
        }
    }
}

impl Item_Impl {
    /// The row this item stands for. Return: the row, or an error when the menu has fewer rows (it cannot, rows never change).
    fn row(&self) -> Result<&Row> {
        self.shared.rows.get(self.pos).ok_or_else(|| Error::from(windows::Win32::Foundation::E_FAIL))
    }
}

impl IRawElementProviderSimple_Impl for Item_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }
    fn GetPatternProvider(&self, pattern: UIA_PATTERN_ID) -> Result<IUnknown> {
        if pattern == UIA_InvokePatternId {
            self.to_interface::<IInvokeProvider>().cast()
        } else {
            none()
        }
    }
    fn GetPropertyValue(&self, id: UIA_PROPERTY_ID) -> Result<VARIANT> {
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
        none()
    }
}

impl IRawElementProviderFragment_Impl for Item_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        let count = self.shared.rows.len();
        match direction {
            PARENT => self.shared.root().cast(),
            NEXT_SIBLING if self.pos + 1 < count => self.shared.item(self.pos + 1).cast(),
            PREVIOUS_SIBLING if self.pos > 0 => self.shared.item(self.pos - 1).cast(),
            _ => none(),
        }
    }
    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        runtime_id(&[self.pos as i32 + 1])
    }
    fn BoundingRectangle(&self) -> Result<UiaRect> {
        let r = self.row()?;
        Ok(UiaRect { left: r.left, top: r.top, width: r.width, height: r.height })
    }
    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
    fn SetFocus(&self) -> Result<()> {
        self.post(FOCUS_MESSAGE)
    }
    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        self.shared.root().cast()
    }
}

impl IInvokeProvider_Impl for Item_Impl {
    fn Invoke(&self) -> Result<()> {
        self.post(INVOKE_MESSAGE)
    }
}

impl Item_Impl {
    /// Posts `message` with this item's entry index to the menu window, which handles it on its own thread. Return: an error when the window is gone.
    fn post(&self, message: u32) -> Result<()> {
        let entry = self.row()?.entry;
        unsafe { PostMessageW(Some(self.shared.window()), message, WPARAM(entry), LPARAM(0)) }
    }
}

/// The answer to `WM_GETOBJECT`. The message is passed on unfiltered, as Microsoft requires (UI Automation and Active Accessibility clients are told apart inside the call). The provider is built here, on the first ask. Input: the window, the message parameters, the rows of the menu. Return: the message result to return from the window procedure. Safety: `UiaReturnRawElementProvider` is given the message parameters unfiltered, as Microsoft requires.
pub fn provider_for(hwnd: HWND, wp: WPARAM, lp: LPARAM, rows: impl FnOnce() -> Vec<Row>) -> LRESULT {
    let shared = PROVIDER.with(|p| {
        p.borrow_mut().get_or_insert_with(|| Arc::new(Shared { hwnd: hwnd.0 as isize, rows: rows(), focus: AtomicIsize::new(-1) })).clone()
    });
    let root = shared.root();
    unsafe { UiaReturnRawElementProvider(hwnd, wp, lp, &root) }
}

/// Announces the newly highlighted row (a focus-changed event) when a client is listening and a provider was ever asked for. Input: the entry index now highlighted, or None. Costs one thread-local read otherwise. Safety: raises an event for a provider this thread made.
pub fn focus_moved(entry: Option<usize>) {
    let Some(shared) = PROVIDER.with(|p| p.borrow().clone()) else { return };
    let pos = entry.and_then(|e| shared.rows.iter().position(|r| r.entry == e));
    shared.focus.store(pos.map_or(-1, |p| p as isize), Ordering::Release);
    if let Some(pos) = pos {
        if unsafe { UiaClientsAreListening() }.as_bool() {
            let _ = unsafe { UiaRaiseAutomationEvent(&shared.item(pos), UIA_AutomationFocusChangedEventId) };
        }
    }
}

/// Tells UI Automation the window is gone so it drops what it kept for it (Microsoft's recommended call from `WM_DESTROY`), and forgets the provider. Safety: the documented `UiaReturnRawElementProvider(hwnd, 0, 0, NULL)` call.
pub fn release(hwnd: HWND) {
    if PROVIDER.with(|p| p.borrow_mut().take()).is_some() {
        unsafe { UiaReturnRawElementProvider(hwnd, WPARAM(0), LPARAM(0), None::<&IRawElementProviderSimple>) };
    }
}
