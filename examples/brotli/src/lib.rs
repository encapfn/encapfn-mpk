#![feature(naked_functions)]

// Necessary evil:
use brotli::*;
use encapfn::branding::EFID;
use encapfn::rt::EncapfnRt;
use encapfn::types::{AccessScope, AllocScope};

// Auto-generated bindings, so doesn't follow Rust conventions at all:
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
pub mod brotli {
    include!(concat!(env!("OUT_DIR"), "/brotli_bindings.rs"));
}

const MESSAGE: &str = "As the manager of the Performance sits before the curtain on the \
     boards and looks into the Fair, a feeling of profound melancholy \
     comes over him in his survey of the bustling place. There is a great \
     quantity of eating and drinking, making love and jilting, laughing \
     and the contrary, smoking, cheating, fighting, dancing and fiddling; \
     there are bullies pushing about, bucks ogling the women, knaves \
     picking pockets, policemen on the look-out, quacks (OTHER quacks, \
     plague take them!) bawling in front of their booths, and yokels \
     looking up at the tinselled dancers and poor old rouged tumblers, \
     while the light-fingered folk are operating upon their pockets \
     behind. Yes, this is VANITY FAIR; not a moral place certainly; nor a \
     merry one, though very noisy.";

pub fn test_brotli<ID: EFID, RT: EncapfnRt<ID = ID>, L: Brotli<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
    message_len: usize,
) {
    // Take a nicer, power-of-two number of the first characters to compress:
    let message_to_compress = MESSAGE.get(..message_len).unwrap();

    // Allocate a compressed buffer with twice the message size. This
    // should hopefully be sufficient even for entirely random messages,
    // with any headers that are attached:
    let encoded_buf_size = message_to_compress.as_bytes().len() * 2;

    lib.rt()
        .allocate_stacked_slice_mut::<u8, _, _>(encoded_buf_size, alloc, |encoded_buf, alloc| {
            let encoded_size = lib
                .rt()
                .allocate_stacked_t_mut::<usize, _, _>(alloc, |encoded_size_ref, alloc| {
                    // Before compression, the encoded size pointer argument
                    // needs to contain the available buffer space:
                    encoded_size_ref.write(encoded_buf_size, access);

                    // Copy the message into foreign memory:
                    lib.rt()
                        .allocate_stacked_slice_mut::<u8, _, _>(
                            message_to_compress.as_bytes().len(),
                            alloc,
                            |source_buf, alloc| {
                                source_buf.copy_from_slice(message_to_compress.as_bytes(), access);

                                // This will make the string invalid UTF-8, causing the below
                                // validation to fail:
                                //message_ref.write_from_iter(core::iter::repeat(0xFF), access);

                                assert_eq!(
                                    1,
                                    lib.BrotliEncoderCompress(
                                        brotli::BROTLI_DEFAULT_QUALITY as i32,
                                        brotli::BROTLI_DEFAULT_WINDOW as i32,
                                        brotli::BrotliEncoderMode_BROTLI_MODE_GENERIC,
                                        message_to_compress.as_bytes().len(),
                                        source_buf.as_ptr().into(),
                                        encoded_size_ref.as_ptr().into(),
                                        encoded_buf.as_ptr().into(),
                                        alloc,
                                        access,
                                    )
                                    .unwrap()
                                    .validate()
                                    .unwrap()
                                );
                            },
                        )
                        .unwrap();

                    // Return the encoded size:
                    *encoded_size_ref.validate(access).unwrap()
                })
                .unwrap();

            // Allocate a buffer for the decoded text, with the same length as the original message.
            lib.rt()
                .allocate_stacked_slice_mut::<u8, _, _>(
                    message_to_compress.as_bytes().len(),
                    alloc,
                    |decoded_buf, alloc| {
                        // Allocate a field to store the decoded size in. It
                        // should be set to the initial available buffer
                        // space:
                        lib.rt()
                            .allocate_stacked_t_mut::<usize, _, _>(
                                alloc,
                                |decoded_size_ref, alloc| {
                                    decoded_size_ref
                                        .write(message_to_compress.as_bytes().len(), access);

                                    assert_eq!(
                                        brotli::BrotliDecoderResult_BROTLI_DECODER_RESULT_SUCCESS,
                                        lib.BrotliDecoderDecompress(
                                            encoded_size,
                                            encoded_buf.as_ptr().into(),
                                            decoded_size_ref.as_ptr().into(),
                                            decoded_buf.as_ptr().into(),
                                            alloc,
                                            access
                                        )
                                        .unwrap()
                                        .validate()
                                        .unwrap(),
                                    );
                                },
                            )
                            .unwrap();

                        // Compare the encoded & decoded message:
                        assert_eq!(
                            message_to_compress,
                            &*decoded_buf.as_immut().validate_as_str(access).unwrap(),
                        );
                    },
                )
                .unwrap();
        })
        .unwrap();
}

pub unsafe fn test_brotli_unsafe(message_len: usize) {
    // Take a nicer, power-of-two number of the first characters to compress:
    let message_to_compress = MESSAGE.get(..message_len).unwrap();

    // Allocate a compressed buffer with twice the maximum message
    // size. This should hopefully be sufficient even for entirely
    // random messages, with any headers that are attached:
    let mut encoded_buf = [0; MESSAGE.len() * 2];

    // Allocate a buffer for the decompressed output:
    let mut decoded_buf = [0; MESSAGE.len()];

    // Before compression, the encoded size pointer argument needs to
    // contain the available buffer space:
    let mut encoded_size: usize = encoded_buf.len();

    assert_eq!(1, unsafe {
        brotli::BrotliEncoderCompress(
            brotli::BROTLI_DEFAULT_QUALITY as i32,
            brotli::BROTLI_DEFAULT_WINDOW as i32,
            brotli::BrotliEncoderMode_BROTLI_MODE_GENERIC,
            message_to_compress.as_bytes().len(),
            message_to_compress.as_bytes().as_ptr(),
            &mut encoded_size as *mut _,
            encoded_buf.as_mut_ptr(),
        )
    },);

    // Before decompression, the decoded size pointer argument needs
    // to contain the available buffer space:
    let mut decoded_size = decoded_buf.len();

    assert_eq!(
        brotli::BrotliDecoderResult_BROTLI_DECODER_RESULT_SUCCESS,
        unsafe {
            brotli::BrotliDecoderDecompress(
                encoded_size,
                encoded_buf.as_ptr(),
                &mut decoded_size as *mut _,
                decoded_buf.as_mut_ptr(),
            )
        },
    );

    // Compare the encoded & decoded message:
    assert_eq!(message_to_compress, unsafe {
        std::str::from_utf8_unchecked(&decoded_buf[..decoded_size])
    },);
}

pub fn with_mockrt_lib<'a, ID: EFID + 'a, A: encapfn::rt::mock::MockRtAllocator, R>(
    brand: ID,
    allocator: A,
    f: impl FnOnce(
        BrotliRt<ID, encapfn::rt::mock::MockRt<ID, A>>,
        AllocScope<
            <encapfn::rt::mock::MockRt<ID, A> as encapfn::rt::EncapfnRt>::AllocTracker<'a>,
            ID,
        >,
        AccessScope<ID>,
    ) -> R,
) -> R {
    // This is unsafe, as it instantiates a runtime that can be used to run
    // foreign functions without memory protection:
    let (rt, alloc, access) = unsafe { encapfn::rt::mock::MockRt::new(false, allocator, brand) };

    // Create a "bound" runtime, which implements the Brotli API:
    let bound_rt = BrotliRt::new(&rt).unwrap();

    // Run the provided closure:
    f(bound_rt, alloc, access)
}

pub fn with_mpkrt_lib<ID: EFID, R>(
    brand: ID,
    f: impl for<'a> FnOnce(
        BrotliRt<ID, encapfn_mpk::EncapfnMPKRt<ID>>,
        AllocScope<<encapfn_mpk::EncapfnMPKRt<ID> as encapfn::rt::EncapfnRt>::AllocTracker<'a>, ID>,
        AccessScope<ID>,
    ) -> R,
) -> R {
    let (rt, alloc, access) = encapfn_mpk::EncapfnMPKRt::new(
        [
            c"libbrotlienc.so",
            c"libbrotlidec.so",
            c"libbrotlicommon.so",
        ]
        .into_iter(),
        brand,
        //Some(GLOBAL_PKEY_ALLOC.get_pkey()),
        None,
        true,
    );

    // Create a "bound" runtime, which implements the Brotli API:
    let bound_rt = BrotliRt::new(&rt).unwrap();

    // Run the provided closure:
    f(bound_rt, alloc, access)
}

pub fn with_no_lib(f: impl FnOnce()) {
    f();
}
