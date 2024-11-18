#![feature(naked_functions)]

use std::ptr::null;

use criterion::{criterion_group, criterion_main, Criterion};

use ef_libsodium_lib::libsodium_public_validate;
use encapfn::branding::EFID;
use encapfn::rt::EncapfnRt;
use encapfn::types::{AccessScope, AllocScope, EFPtr};

use ef_libsodium_lib::libsodium::LibSodium;
use ef_libsodium_lib::{
    libsodium::crypto_generichash, libsodium_public, libsodium_public_unsafe, with_mockrt_lib,
    with_mpkrt_lib,
};

fn calc_hash<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibSodium<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<'_, RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
) {
    lib.rt()
        .allocate_stacked_t_mut::<[u8; 4096], _, _>(alloc, |message, alloc| {
            // Initialize the EFAllocation into an EFMutVal:
            message.write([42; 4096], access);

            lib.rt()
                .allocate_stacked_t_mut::<[u8; 32], _, _>(alloc, |hash, _alloc| {
                    let res = lib
                        .crypto_generichash(
                            <EFPtr<[u8; 32]> as Into<*mut [u8; 32]>>::into(hash.as_ptr())
                                as *mut u8,
                            32,
                            <EFPtr<[u8; 4096]> as Into<*const [u8; 4096]>>::into(message.as_ptr())
                                as *const u8,
                            4096,
                            null(),
                            0,
                            access,
                        )
                        .unwrap();
                    assert!(res.validate().unwrap() == 0);
                })
                .unwrap();
        })
        .unwrap();
}

fn calc_hash_validate<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibSodium<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<'_, RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
) {
    lib.rt()
        .allocate_stacked_t_mut::<[u8; 4096], _, _>(alloc, |message, alloc| {
            // Initialize the EFAllocation into an EFMutVal:
            message.write([42; 4096], access);

            lib.rt()
                .allocate_stacked_t_mut::<[u8; 32], _, _>(alloc, |hash, _alloc| {
                    let res = lib
                        .crypto_generichash(
                            <EFPtr<[u8; 32]> as Into<*mut [u8; 32]>>::into(hash.as_ptr())
                                as *mut u8,
                            32,
                            <EFPtr<[u8; 4096]> as Into<*const [u8; 4096]>>::into(message.as_ptr())
                                as *const u8,
                            4096,
                            null(),
                            0,
                            access,
                        )
                        .unwrap();
                    core::hint::black_box(&*hash.validate(access).unwrap());
                    assert!(res.validate().unwrap() == 0);
                })
                .unwrap();
        })
        .unwrap();
}

fn calc_hash_unsafe() {
    let message = [42 as u8; 4096];

    let hash = [0 as u8; 32];
    unsafe {
        crypto_generichash(
            hash.as_ptr() as *mut u8,
            32,
            message.as_ptr() as *const u8,
            message.len() as u64,
            null(),
            0,
        )
    };
}

fn bench_libsodium(c: &mut Criterion) {
    encapfn::branding::new(|brand| {
        with_mpkrt_lib(brand, |lib, mut alloc, mut access| {
            c.bench_function("protection-only_sodium_hash", |b| {
                b.iter(|| calc_hash(&lib, &mut alloc, &mut access))
            });

            c.bench_function("both_sodium_hash", |b| {
                b.iter(|| calc_hash_validate(&lib, &mut alloc, &mut access))
            });

            c.bench_function("protection-only_sodium_public", |b| {
                b.iter(|| libsodium_public(&lib, &mut alloc, &mut access))
            });

            c.bench_function("both_sodium_public", |b| {
                b.iter(|| libsodium_public_validate(&lib, &mut alloc, &mut access))
            });
        });
    });
    encapfn::branding::new(|brand| {
        with_mockrt_lib(
            brand,
            encapfn::rt::mock::stack_alloc::StackAllocator::<
                encapfn::rt::mock::stack_alloc::StackFrameAllocAMD64,
            >::new(),
            |lib, mut alloc, mut access| {
                c.bench_function("validation-only_sodium_hash", |b| {
                    b.iter(|| calc_hash_validate(&lib, &mut alloc, &mut access))
                });

                c.bench_function("validation-only_sodium_public", |b| {
                    b.iter(|| libsodium_public_validate(&lib, &mut alloc, &mut access))
                });
            },
        );
    });

    c.bench_function("unsafe_sodium_hash", |b| b.iter(|| calc_hash_unsafe()));

    c.bench_function("unsafe_sodium_public", |b| {
        b.iter(|| libsodium_public_unsafe())
    });
}

criterion_group!(benches, bench_libsodium,);

criterion_main!(benches);
