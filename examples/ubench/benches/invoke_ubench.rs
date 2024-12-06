#![feature(naked_functions)]

use std::time::Instant;

use encapfn::branding::EFID;
use encapfn::rt::EncapfnRt;
use encapfn::types::{AccessScope, AllocScope};

use ef_ubench_lib::libefdemo::LibEFDemo;
use ef_ubench_lib::{with_mockrt_lib, with_mpkrt_lib};

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

fn bench_demo_nop<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibEFDemo<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<'_, RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
    label: &'static str,
    iters: u32,
) -> BenchLoopRes {
    bench_loop(&format!("{} - demo_nop", label), iters, &mut || {
        lib.demo_nop(alloc, access).unwrap();
    })
}

fn bench_demo_3args<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibEFDemo<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<'_, RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
    label: &'static str,
    iters: u32,
) -> BenchLoopRes {
    bench_loop(&format!("{} - demo_3args", label), iters, &mut || {
        lib.demo_3args(0, 1, 2, alloc, access).unwrap();
    })
}

fn bench_demo_4args<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibEFDemo<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<'_, RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
    label: &'static str,
    iters: u32,
) -> BenchLoopRes {
    bench_loop(&format!("{} - demo_4args", label), iters, &mut || {
        lib.demo_4args(0, 1, 2, 3, alloc, access).unwrap();
    })
}

fn bench_demo_5args<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibEFDemo<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<'_, RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
    label: &'static str,
    iters: u32,
) -> BenchLoopRes {
    bench_loop(&format!("{} - demo_5args", label), iters, &mut || {
        lib.demo_5args(0, 1, 2, 3, 4, alloc, access).unwrap();
    })
}

fn bench_demo_7args<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibEFDemo<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<'_, RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
    label: &'static str,
    iters: u32,
) -> BenchLoopRes {
    bench_loop(&format!("{} - demo_7args", label), iters, &mut || {
        lib.demo_7args(0, 1, 2, 3, 4, 5, 6, alloc, access).unwrap();
    })
}

fn bench_demo_10args<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibEFDemo<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<'_, RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
    label: &'static str,
    iters: u32,
) -> BenchLoopRes {
    bench_loop(&format!("{} - demo_10args", label), iters, &mut || {
        lib.demo_10args(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, alloc, access)
            .unwrap();
    })
}

fn bench_demo_25args<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibEFDemo<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<'_, RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
    label: &'static str,
    iters: u32,
) -> BenchLoopRes {
    bench_loop(&format!("{} - demo_25args", label), iters, &mut || {
        lib.demo_25args(
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, alloc, access,
        )
        .unwrap();
    })
}

fn bench_demo_50args<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibEFDemo<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<'_, RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
    label: &'static str,
    iters: u32,
) -> BenchLoopRes {
    bench_loop(&format!("{} - demo_50args", label), iters, &mut || {
        lib.demo_50args(
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            46, 47, 48, 49, alloc, access,
        )
        .unwrap();
    })
}

fn bench_demo_100args<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibEFDemo<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<'_, RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
    label: &'static str,
    iters: u32,
) -> BenchLoopRes {
    bench_loop(&format!("{} - demo_100args", label), iters, &mut || {
        lib.demo_100args(
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
            68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89,
            90, 91, 92, 93, 94, 95, 96, 97, 98, 99, alloc, access,
        )
        .unwrap();
    })
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
    const DEMO_NOP_ITERS: u32 = 1_000_000;

    let mut mpk_nop_bench_res = [None; 5];
    let mut mpk_3args_bench_res = [None; 5];
    let mut mpk_4args_bench_res = [None; 5];
    let mut mpk_5args_bench_res = [None; 5];
    let mut mpk_7args_bench_res = [None; 5];
    let mut mpk_10args_bench_res = [None; 5];
    let mut mpk_25args_bench_res = [None; 5];
    let mut mpk_50args_bench_res = [None; 5];
    let mut mpk_100args_bench_res = [None; 5];

    let mut mock_nop_bench_res = [None; 5];
    let mut mock_3args_bench_res = [None; 5];
    let mut mock_4args_bench_res = [None; 5];
    let mut mock_5args_bench_res = [None; 5];
    let mut mock_7args_bench_res = [None; 5];
    let mut mock_10args_bench_res = [None; 5];
    let mut mock_25args_bench_res = [None; 5];
    let mut mock_50args_bench_res = [None; 5];
    let mut mock_100args_bench_res = [None; 5];

    encapfn::branding::new(|brand| {
        with_mpkrt_lib(brand, |lib, mut alloc, mut access| {
            for i in 0..mock_nop_bench_res.len() {
                bench_demo_nop(
                    &lib,
                    &mut alloc,
                    &mut access,
                    "warmup...mpk",
                    DEMO_NOP_ITERS,
                );
                mpk_nop_bench_res[i] = Some(bench_demo_nop(
                    &lib,
                    &mut alloc,
                    &mut access,
                    "mpk",
                    DEMO_NOP_ITERS,
                ));
                mpk_3args_bench_res[i] = Some(bench_demo_3args(
                    &lib,
                    &mut alloc,
                    &mut access,
                    "mpk",
                    DEMO_NOP_ITERS,
                ));
                mpk_4args_bench_res[i] = Some(bench_demo_4args(
                    &lib,
                    &mut alloc,
                    &mut access,
                    "mpk",
                    DEMO_NOP_ITERS,
                ));
                mpk_5args_bench_res[i] = Some(bench_demo_5args(
                    &lib,
                    &mut alloc,
                    &mut access,
                    "mpk",
                    DEMO_NOP_ITERS,
                ));
                mpk_7args_bench_res[i] = Some(bench_demo_7args(
                    &lib,
                    &mut alloc,
                    &mut access,
                    "mpk",
                    DEMO_NOP_ITERS,
                ));
                mpk_10args_bench_res[i] = Some(bench_demo_10args(
                    &lib,
                    &mut alloc,
                    &mut access,
                    "mpk",
                    DEMO_NOP_ITERS,
                ));
                mpk_25args_bench_res[i] = Some(bench_demo_25args(
                    &lib,
                    &mut alloc,
                    &mut access,
                    "mpk",
                    DEMO_NOP_ITERS,
                ));
                mpk_50args_bench_res[i] = Some(bench_demo_50args(
                    &lib,
                    &mut alloc,
                    &mut access,
                    "mpk",
                    DEMO_NOP_ITERS,
                ));
                mpk_5args_bench_res[i] = Some(bench_demo_5args(
                    &lib,
                    &mut alloc,
                    &mut access,
                    "mpk",
                    DEMO_NOP_ITERS,
                ));
                mpk_100args_bench_res[i] = Some(bench_demo_100args(
                    &lib,
                    &mut alloc,
                    &mut access,
                    "mpk",
                    DEMO_NOP_ITERS,
                ));
            }
        })
    });

    encapfn::branding::new(|brand| {
        with_mockrt_lib(
            brand,
            encapfn::rt::mock::stack_alloc::StackAllocator::<
                encapfn::rt::mock::stack_alloc::StackFrameAllocAMD64,
            >::new(),
            |lib, mut alloc, mut access| {
                for i in 0..mock_nop_bench_res.len() {
                    mock_nop_bench_res[i] = Some(bench_demo_nop(
                        &lib,
                        &mut alloc,
                        &mut access,
                        "mock",
                        DEMO_NOP_ITERS,
                    ));
                    mock_3args_bench_res[i] = Some(bench_demo_3args(
                        &lib,
                        &mut alloc,
                        &mut access,
                        "mock",
                        DEMO_NOP_ITERS,
                    ));
                    mock_4args_bench_res[i] = Some(bench_demo_4args(
                        &lib,
                        &mut alloc,
                        &mut access,
                        "mock",
                        DEMO_NOP_ITERS,
                    ));
                    mock_5args_bench_res[i] = Some(bench_demo_5args(
                        &lib,
                        &mut alloc,
                        &mut access,
                        "mock",
                        DEMO_NOP_ITERS,
                    ));
                    mock_7args_bench_res[i] = Some(bench_demo_7args(
                        &lib,
                        &mut alloc,
                        &mut access,
                        "mock",
                        DEMO_NOP_ITERS,
                    ));
                    mock_10args_bench_res[i] = Some(bench_demo_10args(
                        &lib,
                        &mut alloc,
                        &mut access,
                        "mock",
                        DEMO_NOP_ITERS,
                    ));
                    mock_25args_bench_res[i] = Some(bench_demo_25args(
                        &lib,
                        &mut alloc,
                        &mut access,
                        "mock",
                        DEMO_NOP_ITERS,
                    ));
                    mock_50args_bench_res[i] = Some(bench_demo_50args(
                        &lib,
                        &mut alloc,
                        &mut access,
                        "mock",
                        DEMO_NOP_ITERS,
                    ));
                    mock_100args_bench_res[i] = Some(bench_demo_100args(
                        &lib,
                        &mut alloc,
                        &mut access,
                        "mock",
                        DEMO_NOP_ITERS,
                    ));
                }
            },
        )
    });

    print_avg_result("mpk_nop", &mpk_nop_bench_res);
    print_avg_result("mpk_3args", &mpk_3args_bench_res);
    print_avg_result("mpk_4args", &mpk_4args_bench_res);
    print_avg_result("mpk_5args", &mpk_5args_bench_res);
    print_avg_result("mpk_7args", &mpk_7args_bench_res);
    print_avg_result("mpk_10args", &mpk_10args_bench_res);
    print_avg_result("mpk_25args", &mpk_25args_bench_res);
    print_avg_result("mpk_50args", &mpk_50args_bench_res);
    print_avg_result("mpk_100args", &mpk_100args_bench_res);

    print_avg_result("mock_nop", &mock_nop_bench_res);
    print_avg_result("mock_3args", &mock_3args_bench_res);
    print_avg_result("mock_4args", &mock_4args_bench_res);
    print_avg_result("mock_5args", &mock_5args_bench_res);
    print_avg_result("mock_7args", &mock_7args_bench_res);
    print_avg_result("mock_10args", &mock_10args_bench_res);
    print_avg_result("mock_25args", &mock_25args_bench_res);
    print_avg_result("mock_50args", &mock_50args_bench_res);
    print_avg_result("mock_100args", &mock_100args_bench_res);
}
