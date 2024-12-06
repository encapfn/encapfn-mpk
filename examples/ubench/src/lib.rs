#![feature(naked_functions)]

// Necessary evil:
use encapfn::branding::EFID;
use encapfn::types::{AccessScope, AllocScope};

// Auto-generated bindings, so doesn't follow Rust conventions at all:
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
#[allow(improper_ctypes)] // TODO: fix this by wrapping functions with u128s
pub mod libefdemo {
    include!(concat!(env!("OUT_DIR"), "/libefdemo_bindings.rs"));
}

// These are the Encapsulated Functions wrapper types / traits generated.
use libefdemo::LibEFDemoRt;

pub fn with_mockrt_lib<'a, ID: EFID + 'a, A: encapfn::rt::mock::MockRtAllocator, R>(
    brand: ID,
    allocator: A,
    f: impl FnOnce(
        LibEFDemoRt<ID, encapfn::rt::mock::MockRt<ID, A>>,
        AllocScope<
            <encapfn::rt::mock::MockRt<ID, A> as encapfn::rt::EncapfnRt>::AllocTracker<'a>,
            ID,
        >,
        AccessScope<ID>,
    ) -> R,
) -> R {
    // This is unsafe, as it instantiates a runtime that can be used to run
    // foreign functions without memory protection:
    let (rt, alloc, access) = unsafe { encapfn::rt::mock::MockRt::new(false, allocator, brand) };

    // Create a "bound" runtime, which implements the LibEFDemo API:
    let bound_rt = LibEFDemoRt::new(&rt).unwrap();

    // Run the provided closure:
    f(bound_rt, alloc, access)
}

pub fn with_mpkrt_lib<ID: EFID, R>(
    brand: ID,
    f: impl for<'a> FnOnce(
        LibEFDemoRt<ID, encapfn_mpk::EncapfnMPKRt<ID>>,
        AllocScope<<encapfn_mpk::EncapfnMPKRt<ID> as encapfn::rt::EncapfnRt>::AllocTracker<'a>, ID>,
        AccessScope<ID>,
    ) -> R,
) -> R {
    let library_path = std::ffi::CString::new(concat!(env!("OUT_DIR"), "/libefdemo.so")).unwrap();

    let (rt, alloc, access) = encapfn_mpk::EncapfnMPKRt::new(
        [library_path].into_iter(),
        brand,
        //Some(GLOBAL_PKEY_ALLOC.get_pkey()),
        None,
        false,
    );

    // Create a "bound" runtime, which implements the LibEFDemo API:
    let bound_rt = LibEFDemoRt::new(&rt).unwrap();

    // Run the provided closure:
    f(bound_rt, alloc, access)
}
