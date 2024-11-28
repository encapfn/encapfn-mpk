#![feature(naked_functions)]

use std::fs::File;
use std::io::prelude::*;

use std::time::Instant;

use env_logger::init;

// use clap::{Parser, ValueEnum};
// use encapfn::branding::EFID;
use encapfn::rt::EncapfnRt;
// use encapfn::types::{AccessScope, AllocScope};

// use ef_llhttp_lib::llhttp::LibSodium;
use ef_llhttp_lib::{
    llhttp_bindings::Llhttp, llhttp_parse, llhttp_parse_unsafe, with_mockrt_lib, with_mpkrt_lib,
};

// #[global_allocator]
// static GLOBAL_PKEY_ALLOC: encapfn_mpk::pkey_alloc::PkeyAlloc<std::alloc::System> =
//     encapfn_mpk::pkey_alloc::PkeyAlloc::new(std::alloc::System);

// #[derive(ValueEnum, Debug, Clone)]
// #[clap(rename_all = "snake_case")]
// enum EFRuntime {
//     Mock,
//     MPK,
//     No,
// }

// #[derive(Parser, Debug, Clone)]
// #[command(version, about, long_about = None)]
// struct Args {
//     #[arg(short, long)]
//     runtime: EFRuntime,
// }

// #[allow(unused)]
// fn run<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibSodium<ID, RT, RT = RT>>(
//     _args: Args,
//     lib: &L,
//     alloc: &mut AllocScope<RT::AllocTracker<'_>, RT::ID>,
//     access: &mut AccessScope<RT::ID>,
// ) {
//     test_libsodium(lib, alloc, access);
//     libsodium_public(lib, alloc, access);
//     println!("Success!")
// }

// #[allow(unused)]
// fn run_unsafe() {
//     test_libsodium_unsafe();
//     libsodium_public_unsafe();
//     println!("Success!");
// }

fn run_mpk(idx: usize) {
    encapfn::branding::new(|brand| {
        with_mpkrt_lib(
            brand,
            // Some(GLOBAL_PKEY_ALLOC.get_pkey()),
            None,
            |lib, mut alloc, mut access| {
                // std::thread::sleep(std::time::Duration::from_secs(1));
                for _ in 0..10_000_000 {
                    // println!("Iter!");
                    llhttp_parse(&lib, &mut alloc, &mut access);
                }
            },
        );
    });
}

fn main() {
    env_logger::init();

    // unsafe {
    //     llhttp_parse_unsafe();
    // }

    // struct LlhttpID;
    // unsafe impl encapfn::branding::EFID for LlhttpID {}

    // let (rt, mut alloc, mut access) =
    //     encapfn_mpk::EncapfnMPKRt::new([c"libllhttp.so"].into_iter(), LlhttpID, None, true);

    // let hdl = std::thread::spawn(move || {

    // 	// Create a "bound" runtime, which implements the llhttp API:
    // 	let bound_rt = ef_llhttp_lib::llhttp_bindings::LlhttpRt::<LlhttpID, encapfn_mpk::EncapfnMPKRt<LlhttpID>>::new(&rt).unwrap();

    // 	for _ in 0..100_000_000 {
    // 	    llhttp_parse(&bound_rt, &mut alloc, &mut access);
    // 	}
    // });

    // run_mpk(0xDEAD);

    // hdl.join().unwrap();

    // encapfn::branding::new(|brand| {
    //     with_mpkrt_lib(brand, |lib, mut alloc, mut access| {
    // 	    let _: () = lib;
    // 	    std::thread::spawn(move || {
    // 		std::mem::drop((lib, alloc, access));
    // 		// llhttp_parse(&lib, &mut alloc, &mut access);
    // 	    }).join().unwrap();
    //     });
    // });

    // let handle = std::thread::spawn(|| run_mpk(0));
    // // run_mpk(1);
    // handle.join().unwrap();

    // encapfn::branding::new(|brand| {
    //     with_mpkrt_lib(brand, |lib, mut alloc, mut access| {
    // 	    println!("Lib 1 intialized!");
    // 	    encapfn::branding::new(|brand2| {
    // 		with_mpkrt_lib(brand2, |lib2, mut alloc2, mut access2| {
    // 		    println!("Lib 2 intialized!");
    // 		    llhttp_parse(&lib, &mut alloc, &mut access);
    // 		    println!("Ran lib 1 function!");
    // 		    llhttp_parse(&lib2, &mut alloc2, &mut access2);
    // 		    println!("Ran lib 2 function!");
    // 		});
    // 	    });
    //     });
    // });

    // encapfn::branding::new(|brand| { with_mockrt_lib(
    // 	brand,
    //     encapfn::rt::mock::stack_alloc::StackAllocator::<encapfn::rt::mock::stack_alloc::StackFrameAllocAMD64>::new(),
    //     |lib, mut alloc, mut access| {
    // 	    let mut counter = 1;
    // 	    lib.rt().setup_callback(&mut || {
    // 		// Callback code
    // 		println!("Hello World, this is run when calling into the callback pointer, called {} times!", counter);
    // 		counter += 1;
    // 	    }, &mut alloc, |callback_trampoline, alloc| {
    // 		lib.rt().setup_callback(&mut || {
    // 		    // Callback code
    // 		    println!("This is a second callback!");
    // 		}, alloc, |callback_trampoline2, alloc| {
    // 		    unsafe { (*callback_trampoline2)() }
    // 		    unsafe { (*callback_trampoline)() }
    // 		    unsafe { (*callback_trampoline)() }
    // 		    unsafe { (*callback_trampoline)() }
    // 		    unsafe { (*callback_trampoline)() }
    // 		    unsafe { (*callback_trampoline2)() }
    // 		    unsafe { (*callback_trampoline)() }
    // 		});
    // 	    });
    // 	});
    // });

    // Instantiate two runtimes after each other:
    // encapfn::branding::new(|brand| {
    //     with_mpkrt_lib(brand, |lib, mut alloc, mut access| {
    // 	    println!("Lib 1 intialized!");
    // 	    llhttp_parse(&lib, &mut alloc, &mut access);
    // 	    println!("Ran lib 1 function!");
    //     });
    // });

    // encapfn::branding::new(|brand2| {
    // 	with_mpkrt_lib(brand2, |lib2, mut alloc2, mut access2| {
    // 	    println!("Lib 2 intialized!");
    // 	    llhttp_parse(&lib2, &mut alloc2, &mut access2);
    // 	    println!("Ran lib 2 function!");
    // 	});
    // });

    // Instantiate two runtimes nested:
    // encapfn::branding::new(|brand| {
    //     with_mpkrt_lib(brand, |lib, mut alloc, mut access| {
    // 	    println!("Lib 1 intialized!");
    // 	    encapfn::branding::new(|brand2| {
    // 		with_mpkrt_lib(brand2, |lib2, mut alloc2, mut access2| {
    // 		    println!("Lib 2 intialized!");
    // 		    llhttp_parse(&lib, &mut alloc, &mut access);
    // 		    println!("Ran lib 1 function!");
    // 		    llhttp_parse(&lib2, &mut alloc2, &mut access2);
    // 		    println!("Ran lib 2 function!");
    // 		});
    // 	    });
    //     });
    // });

    // Move runtime reference to other thread:
    struct LlhttpID;
    unsafe impl encapfn::branding::EFID for LlhttpID {}
    let (rt, mut alloc, mut access) = encapfn_mpk::EncapfnMPKRt::new(
        [c"libllhttp.so"].into_iter(),
        LlhttpID,
        // Some(GLOBAL_PKEY_ALLOC.get_pkey()),
        None,
        true,
    );
    println!("Created runtime!");
    let hdl = std::thread::spawn(move || {
        // Create a "bound" runtime, which implements the llhttp API:
        let bound_rt = ef_llhttp_lib::llhttp_bindings::LlhttpRt::<
            LlhttpID,
            encapfn_mpk::EncapfnMPKRt<LlhttpID>,
        >::new(&rt)
        .unwrap();
        println!("Created bound runtime!");
        for _ in 0..10_000_000 {
            // println!("Iter!");
            llhttp_parse(&bound_rt, &mut alloc, &mut access);
        }
        println!("Ran function on other thread!");
    }); //.join().unwrap();

    encapfn::branding::new(|brand| {
        with_mpkrt_lib(
            brand,
            // Some(GLOBAL_PKEY_ALLOC.get_pkey()),
            None,
            |lib, mut alloc, mut access| {
                for _ in 0..10_000_000 {
                    llhttp_parse(&lib, &mut alloc, &mut access);
                }
                println!("Ran function on mainthread!");
            },
        );
    });

    hdl.join().unwrap();
}
