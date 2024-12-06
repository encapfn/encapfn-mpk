use sandcrust::*;

sandbox! {
    fn demo_nop_sandcrust() {
        unsafe { ef_ubench_lib::libefdemo::demo_nop() }
    }
}

fn main() {
    // Used to measure startup time against
    // process_startup_demo_nop_unsafe.rs
    demo_nop_sandcrust();
}
