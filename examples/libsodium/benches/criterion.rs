#![feature(naked_functions)]

use criterion::{criterion_group, criterion_main, Criterion};

use ef_libsodium_lib::{
    calc_hash, calc_hash_unsafe, calc_hash_validate, libsodium_public, libsodium_public_unsafe,
    libsodium_public_validate, with_mockrt_lib, with_mpkrt_lib, with_no_lib,
};

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

    with_no_lib(|| {
        c.bench_function("unsafe_sodium_hash", |b| b.iter(|| calc_hash_unsafe()));

        c.bench_function("unsafe_sodium_public", |b| {
            b.iter(|| libsodium_public_unsafe())
        });
    });
}

criterion_group!(benches, bench_libsodium,);

criterion_main!(benches);
