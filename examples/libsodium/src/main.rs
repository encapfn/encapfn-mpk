#![feature(naked_functions)]

use std::fs::File;
use std::io::Write;
use std::time::Instant;

use ef_libsodium_lib::{
    calc_hash, calc_hash_unsafe, calc_hash_validate, libsodium_public, libsodium_public_unsafe,
    libsodium_public_validate, with_mockrt_lib, with_mpkrt_lib,
};

fn main() {
    env_logger::init();

    let mut file = File::create("sodium_data.out").unwrap();

    const ITERATIONS: f64 = 100000.0;
    const ITERATIONS_INDEX: usize = 100000;

    let start = Instant::now();
    for _ in 0..ITERATIONS_INDEX {
        libsodium_public_unsafe();
    }
    let end = Instant::now();
    let unsafe_sodium_public = end.duration_since(start).as_nanos() as f64 / ITERATIONS;

    file.write_all(format!("unsafe_sodium_public = {:?}\n", unsafe_sodium_public).as_bytes())
        .unwrap();

    let start = Instant::now();
    for _ in 0..ITERATIONS_INDEX {
        calc_hash_unsafe();
    }
    let end = Instant::now();
    let unsafe_sodium_hash = end.duration_since(start).as_nanos() as f64 / ITERATIONS;

    file.write_all(format!("unsafe_sodium_hash = {:?}\n", unsafe_sodium_hash).as_bytes())
        .unwrap();

    encapfn::branding::new(|brand| {
        with_mockrt_lib(
            brand,
            encapfn::rt::mock::stack_alloc::StackAllocator::<
                encapfn::rt::mock::stack_alloc::StackFrameAllocAMD64,
            >::new(),
            |lib, mut alloc, mut access| {
                let start = Instant::now();
                for _ in 0..ITERATIONS_INDEX {
                    libsodium_public_validate(&lib, &mut alloc, &mut access);
                }
                let end = Instant::now();
                let validation_only_sodium_public =
                    end.duration_since(start).as_nanos() as f64 / ITERATIONS;

                file.write_all(
                    format!(
                        "validation-only_sodium_public = {:?}\n",
                        validation_only_sodium_public
                    )
                    .as_bytes(),
                )
                .unwrap();

                let start = Instant::now();
                for _ in 0..ITERATIONS_INDEX {
                    calc_hash_validate(&lib, &mut alloc, &mut access);
                }
                let end = Instant::now();
                let validation_only_sodium_hash =
                    end.duration_since(start).as_nanos() as f64 / ITERATIONS;

                file.write_all(
                    format!(
                        "validation-only_sodium_hash = {:?}\n",
                        validation_only_sodium_hash
                    )
                    .as_bytes(),
                )
                .unwrap();
            },
        );
    });

    encapfn::branding::new(|brand| {
        with_mpkrt_lib(brand, |lib, mut alloc, mut access| {
            // Warmups

            for _ in 0..ITERATIONS_INDEX {
                libsodium_public(&lib, &mut alloc, &mut access);
            }

            for _ in 0..ITERATIONS_INDEX {
                calc_hash(&lib, &mut alloc, &mut access);
            }

            //

            let start = Instant::now();
            for _ in 0..ITERATIONS_INDEX {
                libsodium_public(&lib, &mut alloc, &mut access);
            }
            let end = Instant::now();
            let protection_only_sodium_public =
                end.duration_since(start).as_nanos() as f64 / ITERATIONS;

            file.write_all(
                format!(
                    "protection-only_sodium_public = {:?}\n",
                    protection_only_sodium_public
                )
                .as_bytes(),
            )
            .unwrap();

            let start = Instant::now();
            for _ in 0..ITERATIONS_INDEX {
                libsodium_public_validate(&lib, &mut alloc, &mut access);
            }
            let end = Instant::now();
            let both_sodium_public = end.duration_since(start).as_nanos() as f64 / ITERATIONS;

            file.write_all(format!("both_sodium_public = {:?}\n", both_sodium_public).as_bytes())
                .unwrap();

            let start = Instant::now();
            for _ in 0..ITERATIONS_INDEX {
                calc_hash(&lib, &mut alloc, &mut access);
            }
            let end = Instant::now();
            let protection_only_sodium_hash =
                end.duration_since(start).as_nanos() as f64 / ITERATIONS;

            file.write_all(
                format!(
                    "protection-only_sodium_hash = {:?}\n",
                    protection_only_sodium_hash
                )
                .as_bytes(),
            )
            .unwrap();

            let start = Instant::now();
            for _ in 0..ITERATIONS_INDEX {
                calc_hash_validate(&lib, &mut alloc, &mut access);
            }
            let end = Instant::now();
            let both_sodium_hash = end.duration_since(start).as_nanos() as f64 / ITERATIONS;

            file.write_all(format!("both_sodium_hash = {:?}\n", both_sodium_hash).as_bytes())
                .unwrap();
        });
    });

    // encapfn::branding::new(|brand| match args.runtime {
    //     EFRuntime::Mock => with_mockrt_lib(brand, |lib, mut alloc, mut access| {
    //         run(args, &lib, &mut alloc, &mut access);
    //     }),
    //     EFRuntime::MPK => with_mpkrt_lib(brand, |lib, mut alloc, mut access| {
    //         run(args, &lib, &mut alloc, &mut access);
    //     }),
    //     EFRuntime::No => with_no_lib(|| {
    //         run_unsafe();
    //     }),
    // });
}
