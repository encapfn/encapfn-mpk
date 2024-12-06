fn main() {
    // Used to measure startup time against
    // process_startup_demo_nop_ef_mpk.rs
    unsafe { ef_ubench_lib::libefdemo::demo_nop() }
}
