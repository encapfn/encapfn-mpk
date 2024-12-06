fn main() {
    // Used to measure startup time against
    // process_startup_demo_nop_unsafe.rs
    encapfn::branding::new(|brand| {
        ef_ubench_lib::with_mpkrt_lib(brand, |lib, mut alloc, mut access| {
            use ef_ubench_lib::libefdemo::LibEFDemo;
            lib.demo_nop(&mut alloc, &mut access).unwrap();
        });
    });
}
