#![feature(naked_functions)]

use std::ptr::null;

// Necessary evil:
use core::ffi::c_void;
use encapfn::branding::EFID;
use encapfn::rt::EncapfnRt;
use encapfn::types::{AccessScope, AllocScope, EFCopy, EFPtr};

// Auto-generated bindings, so doesn't follow Rust conventions at all:
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
#[allow(improper_ctypes)] // TODO: fix this by wrapping functions with u128s
pub mod libsodium {
    include!(concat!(env!("OUT_DIR"), "/libsodium_bindings.rs"));
}

// These are the Encapsulated Functions wrapper types / traits generated.
use libsodium::{LibSodium, LibSodiumRt};

use libsodium::*;
//#[global_allocator]
//static GLOBAL_PKEY_ALLOC: encapfn_mpk::PkeyAlloc<std::alloc::System> =
//    encapfn_mpk::PkeyAlloc::new(std::alloc::System);

pub fn libsodium_get_key_pair_unsafe(
    seed: &str,
) -> (
    [u8; crypto_box_PUBLICKEYBYTES as usize],
    [u8; crypto_box_SECRETKEYBYTES as usize],
) {
    let mut p1_pub_key = [0 as u8; crypto_box_PUBLICKEYBYTES as usize];
    let mut p1_sec_key = [0 as u8; crypto_box_SECRETKEYBYTES as usize];

    let mut bytes_array = [0; crypto_box_SEEDBYTES as usize];
    let bytes_slice = seed.as_bytes();
    for (i, &byte) in bytes_slice.iter().enumerate() {
        bytes_array[i] = byte;
    }

    unsafe {
        crypto_box_seed_keypair(
            p1_pub_key.as_ptr() as *mut u8,
            p1_sec_key.as_ptr() as *mut u8,
            bytes_array.as_ptr() as *const u8,
        )
    };

    (p1_pub_key, p1_sec_key)
}

pub fn libsodium_get_key_pair<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibSodium<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
    seed: &str,
) -> (
    [u8; crypto_box_PUBLICKEYBYTES as usize],
    [u8; crypto_box_SECRETKEYBYTES as usize],
) {
    let mut p1_pub_key = [0 as u8; crypto_box_PUBLICKEYBYTES as usize];
    let mut p1_sec_key = [0 as u8; crypto_box_SECRETKEYBYTES as usize];

    let mut bytes_array = [0; crypto_box_SEEDBYTES as usize];
    let bytes_slice = seed.as_bytes();
    for (i, &byte) in bytes_slice.iter().enumerate() {
        bytes_array[i] = byte;
    }

    lib.rt()
        .allocate_stacked_t_mut::<[u8; crypto_box_SEEDBYTES as usize], _, _>(alloc, |seed_ref, alloc|{
            seed_ref.write_copy(&EFCopy::new(bytes_array), access);

            lib.rt()
                .allocate_stacked_t_mut::<[u8; crypto_box_SECRETKEYBYTES as usize], _, _>(alloc, |public_key, alloc| {
                    lib.rt()
                        .allocate_stacked_t_mut::<[u8; crypto_box_PUBLICKEYBYTES as usize], _, _>(alloc, |secret_key, _alloc| {
                            lib.crypto_box_seed_keypair(
                public_key.as_ptr().cast::<u8>().into(),
                secret_key.as_ptr().cast::<u8>().into(),
                seed_ref.as_ptr().cast::<u8>().into(),
                access
                ).unwrap();

                            p1_sec_key = secret_key.copy(access).validate().unwrap();
                        }
                    ).unwrap();
                    p1_pub_key = public_key.copy(access).validate().unwrap();
                }
            ).unwrap();
        }).unwrap();

    (p1_pub_key, p1_sec_key)
}

pub fn libsodium_public_unsafe() {
    // println!();
    // println!("Public key encryption test");

    // println!("Generating Person 1's keys (deterministically)");

    // let mut p1_pub_key = [0 as u8; crypto_box_PUBLICKEYBYTES as usize];
    // let mut p1_sec_key = [0 as u8; crypto_box_SECRETKEYBYTES as usize];

    // Get a key pair for both person 1 and person 2
    let (p1_pub_key, p1_sec_key) = libsodium_get_key_pair_unsafe("Person 1 Seed");

    // println!("Person 1 Public key: {:2x?}", p1_pub_key);
    // println!("Person 1 Secret key: {:2x?}", p1_sec_key);

    // println!("Generating Person 2's keys (deterministically)");

    let (p2_pub_key, p2_sec_key) = libsodium_get_key_pair_unsafe("Person 2 Seed");

    // println!("Person 2 Public key: {:2x?}", p2_pub_key);
    // println!("Person 2 Secret key: {:2x?}", p2_sec_key);

    // Get a random nonce

    let rand_seed = "Nonce seed";

    let mut bytes_array = [0; randombytes_SEEDBYTES as usize];
    let bytes_slice = rand_seed.as_bytes();
    for (i, &byte) in bytes_slice.iter().enumerate() {
        bytes_array[i] = byte;
    }

    // // println!("Rand seed bytes {:2x?}", &EFCopy::new(rand_seed_bytes).validate().unwrap());

    let nonce = [0 as u8; crypto_box_NONCEBYTES as usize];
    unsafe {
        randombytes_buf_deterministic(
            nonce.as_ptr() as *mut c_void,
            crypto_box_NONCEBYTES as usize,
            bytes_array.as_ptr() as *const u8,
        )
    };

    // TODO WHY ISN'T THIS DETERMINISTIC
    // println!("Nonce: {:2x?}", nonce);

    // For now to reintroduce deterministicness
    let nonce = [42 as u8; crypto_box_NONCEBYTES as usize];

    // Create encrypted message

    const M_TO_SEND: &str = "Message to encrypt!";
    let mut m_to_send = [0 as u8; M_TO_SEND.len() as usize];
    m_to_send[..M_TO_SEND.len()].copy_from_slice(M_TO_SEND.as_bytes());

    const CIPHERTEXT_LEN: usize = crypto_box_MACBYTES as usize + M_TO_SEND.len() as usize;

    let mut cipher = [0 as u8; CIPHERTEXT_LEN];

    let res = unsafe {
        crypto_box_easy(
            cipher.as_ptr() as *mut u8,
            m_to_send.as_ptr() as *const u8,
            M_TO_SEND.len() as u64,
            nonce.as_ptr() as *const u8,
            p2_pub_key.as_ptr() as *const u8,
            p1_sec_key.as_ptr() as *const u8,
        )
    };

    // println!("Cipher: {:2x?}", cipher);

    // Decrypt
    let decrypted = [0; M_TO_SEND.len()];
    let res = unsafe {
        crypto_box_open_easy(
            decrypted.as_ptr() as *mut u8,
            cipher.as_ptr() as *const u8,
            CIPHERTEXT_LEN as u64,
            nonce.as_ptr() as *const u8,
            p1_pub_key.as_ptr() as *const u8,
            p2_sec_key.as_ptr() as *const u8,
        )
    };

    let s = String::from_utf8((&decrypted).to_vec()).expect("Decrypt");
    // println!("Decrypted: {}", s);
}

pub fn libsodium_public<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibSodium<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
) {
    // println!();
    // println!("Public key encryption test");

    // println!("Generating Person 1's keys (deterministically)");

    // let mut p1_pub_key = [0 as u8; crypto_box_PUBLICKEYBYTES as usize];
    // let mut p1_sec_key = [0 as u8; crypto_box_SECRETKEYBYTES as usize];

    // Get a key pair for both person 1 and person 2
    let (p1_pub_key, p1_sec_key) = libsodium_get_key_pair(lib, alloc, access, "Person 1 Seed");

    // println!("Person 1 Public key: {:2x?}", p1_pub_key);
    // println!("Person 1 Secret key: {:2x?}", p1_sec_key);

    // println!("Generating Person 2's keys (deterministically)");

    let (p2_pub_key, p2_sec_key) = libsodium_get_key_pair(lib, alloc, access, "Person 2 Seed");

    // println!("Person 2 Public key: {:2x?}", p2_pub_key);
    // println!("Person 2 Secret key: {:2x?}", p2_sec_key);

    // Get a random nonce

    let rand_seed = "Nonce seed";

    let mut bytes_array = [0; randombytes_SEEDBYTES as usize];
    let bytes_slice = rand_seed.as_bytes();
    for (i, &byte) in bytes_slice.iter().enumerate() {
        bytes_array[i] = byte;
    }

    // // println!("Rand seed bytes {:2x?}", &EFCopy::new(rand_seed_bytes).validate().unwrap());

    let nonce = lib
        .rt()
        .allocate_stacked_t_mut::<[u8; randombytes_SEEDBYTES as usize], _, _>(
            alloc,
            |seed_ref, alloc| {
                seed_ref.write_copy(&EFCopy::new(bytes_array), access);

                lib.rt()
                    .allocate_stacked_t_mut::<[u8; crypto_box_NONCEBYTES as usize], _, _>(
                        alloc,
                        |nonce_gen, _alloc| {
                            nonce_gen.write([0 as u8; crypto_box_NONCEBYTES as usize], access);

                            lib.randombytes_buf_deterministic(
                                nonce_gen.as_ptr().cast::<c_void>().into(),
                                crypto_box_NONCEBYTES as usize,
                                seed_ref.as_ptr().cast::<u8>().into(),
                                access,
                            )
                            .unwrap();

                            nonce_gen.copy(access).validate().unwrap()
                        },
                    )
                    .unwrap()
            },
        )
        .unwrap();

    // TODO WHY ISN'T THIS DETERMINISTIC
    // println!("Nonce: {:2x?}", nonce);

    // For now to reintroduce deterministicness
    let nonce = [42; crypto_box_NONCEBYTES as usize];

    // Create encrypted message

    const M_TO_SEND: &str = "Message to encrypt!";
    let mut m_to_send = [0 as u8; M_TO_SEND.len() as usize];
    m_to_send[..M_TO_SEND.len()].copy_from_slice(M_TO_SEND.as_bytes());

    const CIPHERTEXT_LEN: usize = crypto_box_MACBYTES as usize + M_TO_SEND.len() as usize;

    let cipher =
    lib.rt().allocate_stacked_t_mut::<[u8; M_TO_SEND.len() as usize], _, _>(alloc, |message_to_send, alloc| {
            message_to_send.write_copy(&EFCopy::new(m_to_send), access);

            lib.rt().allocate_stacked_t_mut::<[u8; crypto_box_NONCEBYTES as usize], _, _>(alloc, |nonce_to_send, alloc| {
        nonce_to_send.write_copy(&EFCopy::new(nonce), access);

            lib.rt().allocate_stacked_t_mut::<[u8; crypto_box_PUBLICKEYBYTES as usize], _, _>(alloc, |pub_key_to_send, alloc| {
                pub_key_to_send.write_copy(&EFCopy::new(p2_pub_key), access);

                lib.rt().allocate_stacked_t_mut::<[u8; crypto_box_SECRETKEYBYTES as usize], _, _>(alloc, |sec_key_to_send, alloc| {
                    sec_key_to_send.write_copy(&EFCopy::new(p1_sec_key), access);

                    lib.rt().allocate_stacked_t_mut::<[u8; CIPHERTEXT_LEN as usize], _, _>(alloc, |cipher, _alloc| {
                        cipher.write([0;CIPHERTEXT_LEN], access);

                        let res = lib.crypto_box_easy(
                cipher.as_ptr().cast::<u8>().into(),
                message_to_send.as_ptr().cast::<u8>().into(),
                            M_TO_SEND.len() as u64,
                nonce_to_send.as_ptr().cast::<u8>().into(),
                pub_key_to_send.as_ptr().cast::<u8>().into(),
                sec_key_to_send.as_ptr().cast::<u8>().into(),
                            access,
                        ).unwrap();

                        assert!(res.validate().unwrap() == 0);


                        cipher.copy(access).validate().unwrap()
                    }).unwrap()
                }).unwrap()
            }).unwrap()
        }).unwrap()
    }).unwrap();

    // println!("Cipher: {:2x?}", cipher);

    // Decrypt

    let decrypted =
    lib.rt().allocate_stacked_t_mut::<[u8; CIPHERTEXT_LEN as usize], _, _>(alloc, |cipher_to_send, alloc| {
        cipher_to_send.write_copy(&EFCopy::new(cipher), access);

            lib.rt().allocate_stacked_t_mut::<[u8; crypto_box_NONCEBYTES as usize], _, _>(alloc, |nonce_to_send, alloc| {
        nonce_to_send.write_copy(&EFCopy::new(nonce), access);

        lib.rt().allocate_stacked_t_mut::<[u8; crypto_box_PUBLICKEYBYTES as usize], _, _>(alloc, |pub_key_to_send, alloc| {
                    pub_key_to_send.write_copy(&EFCopy::new(p1_pub_key), access);

                    lib.rt().allocate_stacked_t_mut::<[u8; crypto_box_SECRETKEYBYTES as usize], _, _>(alloc, |sec_key_to_send, alloc| {
            sec_key_to_send.write_copy(&EFCopy::new(p2_sec_key), access);

            lib.rt().allocate_stacked_t_mut::<[u8; M_TO_SEND.len() as usize], _, _>(alloc, |decrypted, _alloc| {
                            decrypted.write([0;M_TO_SEND.len()], access);

                            let res = lib.crypto_box_open_easy(
                            decrypted.as_ptr().cast::<u8>().into(),
                            cipher_to_send.as_ptr().cast::<u8>().into(),
                            CIPHERTEXT_LEN as u64,
                            nonce_to_send.as_ptr().cast::<u8>().into(),
                            pub_key_to_send.as_ptr().cast::<u8>().into(),
                            sec_key_to_send.as_ptr().cast::<u8>().into(),
                            access
                            ).unwrap();

                            assert!(res.validate().unwrap() == 0);

                            // decrypted.copy(access).validate().unwrap()

                            // core::hint::black_box(
                            //     &*decrypted
                            //         .as_immut()
                            //         .as_slice()
                            //         .validate_as_str(access)
                            //         .unwrap(),
                            // );

            }).unwrap()
                    }).unwrap()
        }).unwrap()
            }).unwrap()
    }).unwrap();

    // let s = String::from_utf8((&decrypted).to_vec()).expect("Decrypt");
    // println!("Decrypted: {}", s);
}

pub fn libsodium_public_validate<
    ID: EFID,
    RT: EncapfnRt<ID = ID>,
    L: LibSodium<ID, RT, RT = RT>,
>(
    lib: &L,
    alloc: &mut AllocScope<RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
) {
    // println!();
    // println!("Public key encryption test");

    // println!("Generating Person 1's keys (deterministically)");

    // let mut p1_pub_key = [0 as u8; crypto_box_PUBLICKEYBYTES as usize];
    // let mut p1_sec_key = [0 as u8; crypto_box_SECRETKEYBYTES as usize];

    // Get a key pair for both person 1 and person 2
    let (p1_pub_key, p1_sec_key) = libsodium_get_key_pair(lib, alloc, access, "Person 1 Seed");

    // println!("Person 1 Public key: {:2x?}", p1_pub_key);
    // println!("Person 1 Secret key: {:2x?}", p1_sec_key);

    // println!("Generating Person 2's keys (deterministically)");

    let (p2_pub_key, p2_sec_key) = libsodium_get_key_pair(lib, alloc, access, "Person 2 Seed");

    // println!("Person 2 Public key: {:2x?}", p2_pub_key);
    // println!("Person 2 Secret key: {:2x?}", p2_sec_key);

    // Get a random nonce

    let rand_seed = "Nonce seed";

    let mut bytes_array = [0; randombytes_SEEDBYTES as usize];
    let bytes_slice = rand_seed.as_bytes();
    for (i, &byte) in bytes_slice.iter().enumerate() {
        bytes_array[i] = byte;
    }

    // // println!("Rand seed bytes {:2x?}", &EFCopy::new(rand_seed_bytes).validate().unwrap());

    let nonce = lib
        .rt()
        .allocate_stacked_t_mut::<[u8; randombytes_SEEDBYTES as usize], _, _>(
            alloc,
            |seed_ref, alloc| {
                seed_ref.write_copy(&EFCopy::new(bytes_array), access);

                lib.rt()
                    .allocate_stacked_t_mut::<[u8; crypto_box_NONCEBYTES as usize], _, _>(
                        alloc,
                        |nonce_gen, _alloc| {
                            nonce_gen.write([0 as u8; crypto_box_NONCEBYTES as usize], access);

                            lib.randombytes_buf_deterministic(
                                nonce_gen.as_ptr().cast::<c_void>().into(),
                                crypto_box_NONCEBYTES as usize,
                                seed_ref.as_ptr().cast::<u8>().into(),
                                access,
                            )
                            .unwrap();

                            nonce_gen.copy(access).validate().unwrap()
                        },
                    )
                    .unwrap()
            },
        )
        .unwrap();

    // TODO WHY ISN'T THIS DETERMINISTIC
    // println!("Nonce: {:2x?}", nonce);

    // For now to reintroduce deterministicness
    let nonce = [42; crypto_box_NONCEBYTES as usize];

    // Create encrypted message

    const M_TO_SEND: &str = "Message to encrypt!";
    let mut m_to_send = [0 as u8; M_TO_SEND.len() as usize];
    m_to_send[..M_TO_SEND.len()].copy_from_slice(M_TO_SEND.as_bytes());

    const CIPHERTEXT_LEN: usize = crypto_box_MACBYTES as usize + M_TO_SEND.len() as usize;

    let cipher =
    lib.rt().allocate_stacked_t_mut::<[u8; M_TO_SEND.len() as usize], _, _>(alloc, |message_to_send, alloc| {
            message_to_send.write_copy(&EFCopy::new(m_to_send), access);

            lib.rt().allocate_stacked_t_mut::<[u8; crypto_box_NONCEBYTES as usize], _, _>(alloc, |nonce_to_send, alloc| {
        nonce_to_send.write_copy(&EFCopy::new(nonce), access);

            lib.rt().allocate_stacked_t_mut::<[u8; crypto_box_PUBLICKEYBYTES as usize], _, _>(alloc, |pub_key_to_send, alloc| {
                pub_key_to_send.write_copy(&EFCopy::new(p2_pub_key), access);

                lib.rt().allocate_stacked_t_mut::<[u8; crypto_box_SECRETKEYBYTES as usize], _, _>(alloc, |sec_key_to_send, alloc| {
                    sec_key_to_send.write_copy(&EFCopy::new(p1_sec_key), access);

                    lib.rt().allocate_stacked_t_mut::<[u8; CIPHERTEXT_LEN as usize], _, _>(alloc, |cipher, _alloc| {
                        cipher.write([0;CIPHERTEXT_LEN], access);

                        let res = lib.crypto_box_easy(
                cipher.as_ptr().cast::<u8>().into(),
                message_to_send.as_ptr().cast::<u8>().into(),
                            M_TO_SEND.len() as u64,
                nonce_to_send.as_ptr().cast::<u8>().into(),
                pub_key_to_send.as_ptr().cast::<u8>().into(),
                sec_key_to_send.as_ptr().cast::<u8>().into(),
                            access,
                        ).unwrap();

                        assert!(res.validate().unwrap() == 0);


                        cipher.copy(access).validate().unwrap()
                    }).unwrap()
                }).unwrap()
            }).unwrap()
        }).unwrap()
    }).unwrap();

    // println!("Cipher: {:2x?}", cipher);

    // Decrypt

    let decrypted =
    lib.rt().allocate_stacked_t_mut::<[u8; CIPHERTEXT_LEN as usize], _, _>(alloc, |cipher_to_send, alloc| {
        cipher_to_send.write_copy(&EFCopy::new(cipher), access);

            lib.rt().allocate_stacked_t_mut::<[u8; crypto_box_NONCEBYTES as usize], _, _>(alloc, |nonce_to_send, alloc| {
        nonce_to_send.write_copy(&EFCopy::new(nonce), access);

        lib.rt().allocate_stacked_t_mut::<[u8; crypto_box_PUBLICKEYBYTES as usize], _, _>(alloc, |pub_key_to_send, alloc| {
                    pub_key_to_send.write_copy(&EFCopy::new(p1_pub_key), access);

                    lib.rt().allocate_stacked_t_mut::<[u8; crypto_box_SECRETKEYBYTES as usize], _, _>(alloc, |sec_key_to_send, alloc| {
            sec_key_to_send.write_copy(&EFCopy::new(p2_sec_key), access);

            lib.rt().allocate_stacked_t_mut::<[u8; M_TO_SEND.len() as usize], _, _>(alloc, |decrypted, _alloc| {
                            decrypted.write([0;M_TO_SEND.len()], access);

                            let res = lib.crypto_box_open_easy(
                decrypted.as_ptr().cast::<u8>().into(),
                cipher_to_send.as_ptr().cast::<u8>().into(),
                CIPHERTEXT_LEN as u64,
                nonce_to_send.as_ptr().cast::<u8>().into(),
                pub_key_to_send.as_ptr().cast::<u8>().into(),
                sec_key_to_send.as_ptr().cast::<u8>().into(),
                access
                            ).unwrap();

                            assert!(res.validate().unwrap() == 0);

                            // decrypted.copy(access).validate().unwrap()

                            core::hint::black_box(
                                &*decrypted
                                    .as_immut()
                                    .as_slice()
                                    .validate_as_str(access)
                                    .unwrap(),
                            );

            }).unwrap()
                    }).unwrap()
        }).unwrap()
            }).unwrap()
    }).unwrap();

    // let s = String::from_utf8((&decrypted).to_vec()).expect("Decrypt");
    // println!("Decrypted: {}", s);
}

pub fn test_libsodium_unsafe() {
    let mut hash: EFCopy<[u8; 32]> = EFCopy::zeroed();

    let ver_major = unsafe { sodium_library_version_major() };
    let ver_minor = unsafe { sodium_library_version_minor() };

    // println!("Libsodium Version: {:?}.{:?}", ver_major, ver_minor);

    let rand_bytes = unsafe { randombytes_random() };
    // println!("random u32: {:?}", rand_bytes);

    let message = "Arbitrary data to hash";

    let mut hash = [0 as u8; 32];
    unsafe {
        crypto_generichash(
            hash.as_ptr() as *mut u8,
            32,
            message.as_ptr() as *const u8,
            message.as_bytes().len() as u64,
            null(),
            0,
        )
    };
}

// The signature of this is quite ugly. Unfortunately I haven't found a way to
// make it nicer without things breaking:
pub fn test_libsodium<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibSodium<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
) {
    let mut hash: EFCopy<[u8; 32]> = EFCopy::zeroed();

    // println!("Runtime pointer: {:p}", lib.rt());

    let ver_major = lib
        .sodium_library_version_major(access)
        .unwrap()
        .validate()
        .unwrap();
    let ver_minor = lib
        .sodium_library_version_minor(access)
        .unwrap()
        .validate()
        .unwrap();
    // println!("Libsodium Version: {:?}.{:?}", ver_major, ver_minor);

    let rand_bytes = lib.randombytes_random(access).unwrap().validate().unwrap();
    // println!("random u32: {:?}", rand_bytes);

    let message = "Arbitrary data to hash";

    lib.rt()
        .allocate_stacked_slice_mut::<u8, _, _>(
            message.as_bytes().len(),
            alloc,
            |message_ref, alloc| {
                // Initialize the EFAllocation into an EFMutVal:
                message_ref.copy_from_slice(message.as_bytes(), access);

                hash = lib
                    .rt()
                    .allocate_stacked_t_mut::<[u8; 32], _, _>(alloc, |hash_ref, _alloc| {
                        let res = lib
                            .crypto_generichash(
                                hash_ref.as_ptr().cast::<u8>().into(),
                                32,
                                message_ref.as_ptr().into(),
                                message.as_bytes().len() as u64,
                                null(),
                                0,
                                access,
                            )
                            .unwrap()
                            .validate()
                            .unwrap();

                        // println!(
                        //     "hash res: {:?}, output (it uses Blake2b-256): {:x?}",
                        //     res,
                        //     hash_ref.validate(access).as_deref(),
                        // );

                        hash_ref.copy(access)
                    })
                    .unwrap();
            },
        )
        .unwrap();

    // println!(
    //     "test output (it uses Blake2b-256): {:x?}",
    //     hash.validate().unwrap(),
    // );

    lib.rt()
        .allocate_stacked_t_mut::<[u8; 4096], _, _>(alloc, |message, alloc| {
            // Initialize the EFAllocation into an EFMutVal:

            message.write([42; 4096], access);

            lib.rt()
                .allocate_stacked_t_mut::<[u8; 32], _, _>(alloc, |hash, _alloc| {
                    let res = lib
                        .crypto_generichash(
                            hash.as_ptr().cast::<u8>().into(),
                            32,
                            message.as_ptr().cast::<u8>().into(),
                            4096,
                            null(),
                            0,
                            access,
                        )
                        .unwrap();

                    // println!(
                    //     "hash res: {:?}, output (it uses Blake2b-256): {:x?}",
                    //     res,
                    //     hash.validate(access).as_deref(),
                    // );
                    assert!(res.validate().unwrap() == 0);
                })
                .unwrap();
        })
        .unwrap();
}

pub fn calc_hash<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibSodium<ID, RT, RT = RT>>(
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
                    let res = lib.crypto_generichash(
                        <EFPtr<[u8; 32]> as Into<*mut [u8; 32]>>::into(hash.as_ptr()) as *mut u8,
                        32,
                        <EFPtr<[u8; 4096]> as Into<*const [u8; 4096]>>::into(message.as_ptr())
                            as *const u8,
                        4096,
                        null(),
                        0,
                        access,
                    );
                    // assert!(res.validate().unwrap() == 0);
                })
                .unwrap();
        })
        .unwrap();
}

pub fn calc_hash_validate<ID: EFID, RT: EncapfnRt<ID = ID>, L: LibSodium<ID, RT, RT = RT>>(
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
                    let res = lib.crypto_generichash(
                        <EFPtr<[u8; 32]> as Into<*mut [u8; 32]>>::into(hash.as_ptr()) as *mut u8,
                        32,
                        <EFPtr<[u8; 4096]> as Into<*const [u8; 4096]>>::into(message.as_ptr())
                            as *const u8,
                        4096,
                        null(),
                        0,
                        access,
                    );
                    core::hint::black_box(&*hash.validate(access).unwrap());
                    // assert!(res.validate().unwrap() == 0);
                })
                .unwrap();
        })
        .unwrap();
}

pub fn calc_hash_unsafe() {
    let message = [42 as u8; 4096];

    let mut hash = [0 as u8; 32];
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

pub fn with_mockrt_lib<'a, ID: EFID + 'a, A: encapfn::rt::mock::MockRtAllocator, R>(
    brand: ID,
    allocator: A,
    f: impl FnOnce(
        LibSodiumRt<ID, encapfn::rt::mock::MockRt<ID, A>>,
        AllocScope<
            <encapfn::rt::mock::MockRt<ID, A> as encapfn::rt::EncapfnRt>::AllocTracker<'a>,
            ID,
        >,
        AccessScope<ID>,
    ) -> R,
) -> R {
    // This is unsafe, as it instantiates a runtime that can be used to run
    // foreign functions without memory protection:
    let (rt, alloc, mut access) =
        unsafe { encapfn::rt::mock::MockRt::new(false, allocator, brand) };

    // Create a "bound" runtime, which implements the LibSodium API:
    let bound_rt = LibSodiumRt::new(&rt).unwrap();

    // All further functions expect libsodium to be initialized:
    // println!("Initializing libsodium...");
    assert!(
        0 == bound_rt
            .sodium_init(&mut access)
            .unwrap()
            .validate()
            .unwrap()
    );
    // println!("Libsodium initialized!");

    // Run the provided closure:
    f(bound_rt, alloc, access)
}

pub fn with_mpkrt_lib<ID: EFID, R>(
    brand: ID,
    f: impl for<'a> FnOnce(
        LibSodiumRt<ID, encapfn_mpk::EncapfnMPKRt<ID>>,
        AllocScope<<encapfn_mpk::EncapfnMPKRt<ID> as encapfn::rt::EncapfnRt>::AllocTracker<'a>, ID>,
        AccessScope<ID>,
    ) -> R,
) -> R {
    let (rt, alloc, mut access) = encapfn_mpk::EncapfnMPKRt::new(
        [c"libsodium.so"].into_iter(),
        brand,
        //Some(GLOBAL_PKEY_ALLOC.get_pkey()),
        None,
        false,
    );

    // Create a "bound" runtime, which implements the LibSodium API:
    let bound_rt = LibSodiumRt::new(&rt).unwrap();

    // All further functions expect libsodium to be initialized:
    // println!("Initializing libsodium:");
    assert!(
        0 == bound_rt
            .sodium_init(&mut access)
            .unwrap()
            .validate()
            .unwrap()
    );
    // println!("Libsodium initialized!");

    // Run the provided closure:
    f(bound_rt, alloc, access)
}

pub fn with_no_lib(f: impl FnOnce()) {
    // println!("Initializing libsodium:");
    unsafe { sodium_init() };
    // println!("Libsodium initialized!");

    f();
}
