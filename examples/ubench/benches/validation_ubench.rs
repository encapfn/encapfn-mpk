#![feature(naked_functions)]

use std::time::Instant;

use encapfn::branding::EFID;
use encapfn::rt::EncapfnRt;
use encapfn::types::{AccessScope, AllocScope};

use ef_ubench_lib::libefdemo::LibEFDemo;
use ef_ubench_lib::with_mpkrt_lib;

#[derive(Copy, Clone, Debug)]
struct BenchLoopRes {
    start: Instant,
    end: Instant,
    iters: u32,
}

const PRINT_BENCH: bool = false;

#[inline(always)]
fn bench_loop(label: &str, iters: u32, f: &mut impl FnMut()) -> BenchLoopRes {
    if PRINT_BENCH {
        println!("{}: running {} iterations...", label, iters);
    }

    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let end = Instant::now();

    if PRINT_BENCH {
        println!(
            "{}: took {:?} for {} iterations ({:?} per iteration)",
            label,
            end.duration_since(start),
            iters,
            end.duration_since(start) / iters,
        );
    }

    BenchLoopRes { start, end, iters }
}

fn bench_validation<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibEFDemo<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<'_, RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
    label: &str,
    iters: u32,
    size: usize,
) -> BenchLoopRes {
    lib.rt()
        .allocate_stacked_slice_mut::<u8, _, _>(size, alloc, |slice, _alloc| {
            slice.write_from_iter(core::iter::repeat(b'a'), access);

            let slice_ref = &slice;
            bench_loop(
                &format!("{} - validate, {} bytes", label, size),
                iters,
                &mut || {
                    core::hint::black_box(
                        core::hint::black_box(slice_ref)
                            .as_immut()
                            .validate_as_str(access)
                            .unwrap(),
                    );
                },
            )
        })
        .unwrap()
}

fn print_avg_result(label: &str, results: &[Option<BenchLoopRes>]) {
    let mut sum = std::time::Duration::from_nanos(0);

    for res in results {
        let res = res.unwrap();
        sum = sum
            .checked_add(res.end.duration_since(res.start) / res.iters)
            .unwrap();
    }

    println!(
        "{},{:?},{},{}",
        label,
        sum / (results.len() as u32),
        results.len(),
        results[0].unwrap().iters
    );
}

fn main() {
    let mut validate_bench_res = [(0, None); 1000];
    for (i, (size, _res)) in validate_bench_res.iter_mut().enumerate() {
        *size = core::cmp::max(1, i * 1000);
    }

    encapfn::branding::new(|brand| {
        with_mpkrt_lib(brand, |lib, mut alloc, mut access| {
            for (size, res) in validate_bench_res.iter_mut() {
                *res = Some(bench_validation(
                    &lib,
                    &mut alloc,
                    &mut access,
                    "",
                    1_000,
                    *size,
                ));
            }
        })
    });

    for (size, res) in validate_bench_res {
        print_avg_result(&format!("{}", size), &[res.clone()]);
    }
}
