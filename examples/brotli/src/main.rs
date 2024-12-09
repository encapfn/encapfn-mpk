fn main() {
    encapfn::branding::new(|brand| {
        ef_brotli_lib::with_mpkrt_lib(brand, |lib, mut alloc, mut access| {
            println!("Running with MPKRt");
            ef_brotli_lib::test_brotli(&lib, &mut alloc, &mut access, 512);
        });
    });

    encapfn::branding::new(|brand| {
        ef_brotli_lib::with_mockrt_lib(
            brand,
            encapfn::rt::mock::stack_alloc::StackAllocator::<
                encapfn::rt::mock::stack_alloc::StackFrameAllocAMD64,
            >::new(),
            |lib, mut alloc, mut access| {
                println!("Running with MockRt");
                ef_brotli_lib::test_brotli(&lib, &mut alloc, &mut access, 512);
            },
        )
    });

    println!("Running unsafe");
    unsafe { ef_brotli_lib::test_brotli_unsafe(512) };
}
