//! File: platform/typing_policy.rs
//! Purpose: The pure decisions behind typing or pasting a short text: which machines count as a virtual machine (by whole-word BIOS names), and when a burst of typed key events is safe. A burst is one `SendInput` call of key-down and key-up pairs; where a key-up can be lost or reordered the target may repeat or drop a letter, so such a machine pastes instead (one clipboard read the target takes whole).
//! Contents: is_virtual_machine — whether the BIOS names a hypervisor's virtual machine; Env — what is known about the machine; burst_is_safe — whether a typed burst may be used.
//! shortcut: the VM test reads the BIOS manufacturer and product names, so a hypervisor that reports a physical vendor is missed; `weak_machine` still catches the usual small ones, and the upgrade is a hypervisor CPUID check that tells a guest from a Hyper-V host.

/// Names that identify a virtual machine when they appear as whole words in the BIOS manufacturer or product name (case is ignored; `-` and `,` separate words, and so does the step from letters to digits, so `VMware7,1` has the word `vmware`). The list follows the vendor table of systemd's `detect_vm_dmi_vendor` in `src/basic/virt.c` (read on 2026-11-15): KVM, QEMU, VMware, innotek GmbH, VirtualBox, Xen, Bochs, Parallels, BHYVE, Hyper-V, Google Compute Engine and Amazon EC2. systemd tests prefixes of several Linux DMI files, so only the identifiers come from it, not the matching.
const VM_NAMES: [&str; 11] =
    ["vmware", "virtualbox", "innotek", "qemu", "kvm", "xen", "parallels", "bochs", "bhyve", "hyper v", "google compute engine"];

/// What decides whether typing a burst is safe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Env {
    /// Fewer than four logical processors or under 6 GB.
    pub weak: bool,
    /// A Remote Desktop session.
    pub remote: bool,
    /// A virtual machine by its BIOS names.
    pub virtual_machine: bool,
}

/// The lower case words of a name, split at anything that is not a letter or digit and where letters meet digits.
fn words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    for c in name.to_lowercase().chars() {
        let boundary = !c.is_alphanumeric() || word.chars().next().is_some_and(|first| first.is_numeric() != c.is_numeric());
        if boundary && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        if c.is_alphanumeric() {
            word.push(c);
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// Whether `phrase` (words separated by spaces) appears in `words` as whole words, in order.
fn has_phrase(words: &[String], phrase: &str) -> bool {
    let wanted: Vec<&str> = phrase.split(' ').collect();
    words.windows(wanted.len()).any(|w| w.iter().zip(&wanted).all(|(a, b)| a == b))
}

/// Whether the BIOS manufacturer and product names belong to a virtual machine: one of `VM_NAMES`, Microsoft's `Virtual Machine` product, or an Amazon EC2 instance that is not a bare-metal one (its type has the word `metal`; this rule is from AWS's instance naming and was not read from a primary source here). A Hyper-V host or a physical PC with Hyper-V on reports its real vendor, so it is not counted. Input: the two names as Windows gives them. Returns true for a virtual machine.
pub fn is_virtual_machine(manufacturer: &str, product: &str) -> bool {
    let (maker, model) = (words(manufacturer), words(product));
    VM_NAMES.iter().any(|name| has_phrase(&maker, name) || has_phrase(&model, name))
        || (has_phrase(&maker, "microsoft corporation") && has_phrase(&model, "virtual machine"))
        || (has_phrase(&maker, "amazon ec 2") && !has_phrase(&model, "metal"))
}

/// Whether a typed burst may be used: only on a machine that is not weak, not remote and not virtual.
pub fn burst_is_safe(env: Env) -> bool {
    !(env.weak || env.remote || env.virtual_machine)
}
