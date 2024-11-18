#![feature(naked_functions)]

use clap::{Parser, ValueEnum};
use encapfn::branding::EFID;
use encapfn::rt::EncapfnRt;
use encapfn::types::{AccessScope, AllocScope};

use ef_libsodium_lib::libsodium::LibSodium;
use ef_libsodium_lib::{
    calc_hash, calc_hash_unsafe, calc_hash_validate, libsodium_public, libsodium_public_unsafe,
    libsodium_public_validate, test_libsodium, test_libsodium_unsafe, with_mockrt_lib,
    with_mpkrt_lib,
};

use std::fs::File;
use std::io::prelude::*;

use std::time::Instant;

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
enum EFRuntime {
    Mock,
    MPK,
    No,
}

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    runtime: EFRuntime,
}

#[allow(unused)]
fn run<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibSodium<ID, RT, RT = RT>>(
    _args: Args,
    lib: &L,
    alloc: &mut AllocScope<RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
) {
    test_libsodium(lib, alloc, access);
    libsodium_public(lib, alloc, access);
    println!("Success!")
}

#[allow(unused)]
fn run_unsafe() {
    test_libsodium_unsafe();
    libsodium_public_unsafe();
    println!("Success!");
}

fn main() {
    // env_logger::init();
    // let args = Args::parse();

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
