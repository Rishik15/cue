//! File: platform/tests/typing_policy.rs
//! Purpose: Protect the choice between typing a burst and pasting: virtual machines are found by whole-word BIOS names, physical PCs and lookalike names are not, and any fragile signal sends the text through the paste path.
//! Contents: tests of `is_virtual_machine` and `burst_is_safe`.

use crate::platform::typing_policy::{burst_is_safe, is_virtual_machine, Env};

/// Every identifier in the policy is recognised, as a manufacturer or a product name as the real guests report it.
#[test]
fn hypervisor_names_are_virtual_machines() {
    let guests = [
        ("VMware, Inc.", "VMware7,1"),
        ("VMware, Inc.", "VMware Virtual Platform"),
        ("innotek GmbH", "VirtualBox"),
        ("Oracle", "VirtualBox"),
        ("QEMU", "Standard PC (Q35 + ICH9, 2009)"),
        ("Red Hat", "KVM"),
        ("Microsoft Corporation", "Virtual Machine"),
        ("Microsoft Corporation", "Hyper-V UEFI Release v4.1"),
        ("Xen", "HVM domU"),
        ("Parallels Software International Inc.", "Parallels Virtual Platform"),
        ("Bochs", "Bochs"),
        ("FreeBSD", "BHYVE"),
        ("Google", "Google Compute Engine"),
        ("Amazon EC2", "t3.micro"),
        ("Amazon EC2", "m7i.large"),
    ];
    for (maker, model) in guests {
        assert!(is_virtual_machine(maker, model), "{maker} / {model}");
    }
}

/// Physical vendors are not, including names that merely contain a hypervisor's letters, Surface products, Hyper-V hosts and Amazon bare-metal instances.
#[test]
fn physical_pcs_are_not_virtual_machines() {
    let machines = [
        ("Dell Inc.", "XPS 15 9520"),
        ("LENOVO", "21HD0012US"),
        ("Microsoft Corporation", "Surface Laptop 5"),
        ("XPG", "XENIA 15"),
        ("XPG XENIA", "XENIA"),
        ("ASUSTeK COMPUTER INC.", "ROG Zephyrus G14 GA401"),
        ("Micro-Star International Co., Ltd.", "MS-7C94"),
        ("Gigabyte Technology Co., Ltd.", "B550 AORUS ELITE"),
        ("HP", "HP EliteBook 840 G8"),
        ("Oracle Corporation", "SPARC T7-1"),
        ("Amazon EC2", "m5.metal"),
        ("Amazon EC2", "m7i.metal-24xl"),
        ("", ""),
    ];
    for (maker, model) in machines {
        assert!(!is_virtual_machine(maker, model), "{maker} / {model}");
    }
}

/// A burst needs a machine that is none of weak, remote or virtual.
#[test]
fn a_burst_needs_a_machine_with_no_fragile_signal() {
    assert!(burst_is_safe(Env::default()));
    assert!(!burst_is_safe(Env { weak: true, ..Env::default() }));
    assert!(!burst_is_safe(Env { remote: true, ..Env::default() }));
    assert!(!burst_is_safe(Env { virtual_machine: true, ..Env::default() }));
}
