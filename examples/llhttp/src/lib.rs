#![feature(naked_functions)]

use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::ptr::null;

// Necessary evil:
use encapfn::branding::EFID;
use encapfn::rt::EncapfnRt;
use encapfn::types::{AccessScope, AllocScope, EFCopy, EFPtr};

// Auto-generated bindings, so doesn't follow Rust conventions at all:
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
#[allow(improper_ctypes)] // TODO: fix this by wrapping functions with u128s
pub mod llhttp_bindings {
    include!(concat!(env!("OUT_DIR"), "/llhttp_bindings.rs"));
}

// These are the Encapsulated Functions wrapper types / traits generated.
use llhttp_bindings::{
    llhttp_errno_HPE_OK, llhttp_errno_name, llhttp_execute, llhttp_init, llhttp_settings_init,
    llhttp_settings_t, llhttp_t, llhttp_type_HTTP_BOTH, Llhttp, LlhttpRt,
};

pub fn llhttp_parse_unsafe() {
    let mut parser: MaybeUninit<llhttp_t> = MaybeUninit::uninit();
    let mut settings: MaybeUninit<llhttp_settings_t> = MaybeUninit::uninit();

    // Initialize user callbacks and settings
    unsafe {
        llhttp_settings_init(settings.as_mut_ptr());
    }

    // Initialize the parser in HTTP_BOTH mode, meaning that it will select
    // between HTTP_REQUEST and HTTP_RESPONSE parsing automatically while
    // reading the first input.
    unsafe {
        llhttp_init(
            parser.as_mut_ptr(),
            llhttp_type_HTTP_BOTH,
            settings.as_mut_ptr(),
        );
    }
    let mut parser = unsafe { parser.assume_init() };

    // Parse request!
    let request = b"GET / HTTP/1.1\r\n\r\n";

    let err = unsafe {
        llhttp_execute(
            &mut parser as *mut llhttp_t,
            request.as_ptr() as *const i8,
            request.len(),
        )
    };

    if err == llhttp_errno_HPE_OK {
        println!("Unsafe: Successfully parsed!");
    } else {
        println!(
            "Parse error: {:?} - {:?}",
            unsafe { llhttp_errno_name(err) },
            parser.reason
        );
    }
}

pub fn llhttp_parse<ID: EFID, RT: EncapfnRt<ID = ID>, L: Llhttp<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
) {
    lib.rt()
        .allocate_stacked_t_mut::<llhttp_t, _, _>(alloc, |parser, alloc| {
            // Initialize user callbacks and settings
            lib.rt()
                .allocate_stacked_t_mut::<llhttp_settings_t, _, _>(alloc, |settings, alloc| {
                    lib.llhttp_settings_init(settings.as_ptr().into(), alloc, access)
                        .unwrap();

                    // Initialize the parser in HTTP_BOTH mode, meaning that it will
                    // select between HTTP_REQUEST and HTTP_RESPONSE parsing
                    // automatically while reading the first input.
                    lib.llhttp_init(
                        parser.as_ptr().into(),
                        llhttp_type_HTTP_BOTH,
                        settings.as_ptr().into(),
                        alloc,
                        access,
                    )
                    .unwrap();

                    // Allocate request in foreign memory and copy message into it:
                    let request_bytes = b"GET / HTTP/1.1\r\n\r\n";
                    lib.rt()
                        .write_stacked_slice(
                            request_bytes,
                            alloc,
                            access,
                            |request_alloc, alloc, access| {
                                // Parse the request:
                                let err = lib
                                    .llhttp_execute(
                                        parser.as_ptr().into(),
                                        request_alloc.as_ptr().cast::<i8>().into(),
                                        request_bytes.len(),
                                        alloc,
                                        access,
                                    )
                                    .unwrap()
                                    .validate()
                                    .unwrap();

                                if err == llhttp_errno_HPE_OK {
                                    // println!("Encapsulated: Successfully parsed!");
                                } else {
                                    // let errno_name_ptr: EFPtr<i8> = lib
                                    // 	.llhttp_errno_name(err, access)
                                    // 	.unwrap()
                                    // 	.validate()
                                    // 	.unwrap()
                                    // 	.into();

                                    // let errno_name_cstr = ...

                                    println!(
                                        "Parse error: TODO - TODO",
                                        // parser.reason
                                    );
                                }
                            },
                        )
                        .unwrap();
                })
                .unwrap();
        })
        .unwrap();
}

pub fn with_mockrt_lib<'a, ID: EFID + 'a, A: encapfn::rt::mock::MockRtAllocator, R>(
    brand: ID,
    allocator: A,
    f: impl FnOnce(
        LlhttpRt<ID, encapfn::rt::mock::MockRt<ID, A>>,
        AllocScope<
            <encapfn::rt::mock::MockRt<ID, A> as encapfn::rt::EncapfnRt>::AllocTracker<'a>,
            ID,
        >,
        AccessScope<ID>,
    ) -> R,
) -> R {
    // This is unsafe, as it instantiates a runtime that can be used to run
    // foreign functions without memory protection:
    let (rt, alloc, mut access) =
        unsafe { encapfn::rt::mock::MockRt::new(false, allocator, brand) };

    // Create a "bound" runtime, which implements the llhttp API:
    let bound_rt = LlhttpRt::new(&rt).unwrap();

    // TODO: maybe return an initialized parser here?

    // Run the provided closure:
    f(bound_rt, alloc, access)
}

pub fn with_mpkrt_lib<ID: EFID, R>(
    brand: ID,
    pkey_rust: Option<std::ffi::c_int>,
    f: impl for<'a> FnOnce(
        LlhttpRt<ID, encapfn_mpk::EncapfnMPKRt<ID>>,
        AllocScope<<encapfn_mpk::EncapfnMPKRt<ID> as encapfn::rt::EncapfnRt>::AllocTracker<'a>, ID>,
        AccessScope<ID>,
    ) -> R,
) -> R {
    let libs: [&'static std::ffi::CStr; 0] = [];
    let (rt, alloc, mut access) =
        encapfn_mpk::EncapfnMPKRt::new([c"libllhttp.so"].into_iter(), brand, pkey_rust, true);

    // Create a "bound" runtime, which implements the llhttp API:
    let bound_rt = LlhttpRt::new(&rt).unwrap();

    // TODO: maybe return an initialized parser here?

    // Run the provided closure:
    f(bound_rt, alloc, access)
}
