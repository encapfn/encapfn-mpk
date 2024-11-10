#![feature(
    naked_functions,
    linked_list_retain,
    asm_const,
    thread_local,
    maybe_uninit_as_bytes,
    maybe_uninit_write_slice
)]
#![allow(named_asm_labels)]

use std::borrow::Cow;
use std::cell::{Cell, UnsafeCell};
use std::collections::HashMap;
use std::collections::LinkedList;
use std::ffi::{CStr, CString};
use std::io::{Seek, SeekFrom, Write};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::Range;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use log::{debug, info};

use encapfn::abi::calling_convention::{Stacked, AREG0, AREG1, AREG2, AREG3, AREG4, AREG5};
use encapfn::abi::sysv_amd64::SysVAMD64ABI;
use encapfn::branding::EFID;
use encapfn::rt::sysv_amd64::{SysVAMD64BaseRt, SysVAMD64InvokeRes, SysVAMD64Rt};
use encapfn::rt::EncapfnRt;
use encapfn::types::{AccessScope, AllocScope, AllocTracker};
use encapfn::{EFError, EFResult};

const ENABLE_DEBUG: bool = true;

// 8MB virtual memory stack by default:
const STACK_SIZE: usize = 8 * 1024 * 1024;

// x86-64 Linux always uses 4k pages:
const PAGE_SIZE: usize = 4096;

// For now, we publically re-export the dlfcn and link bindings:
pub mod libc_bindings;

// We further provide a simple, stupid heap allocator, based on GlobalAlloc,
// that returns pages protected with a pkey. This is mostly useful for
// debugging, when one still wants to protect Rust memory but not disable PKEY 0
// fully. This is generally a less secure cnfiguration, because new Rust
// mappings might not be assigned the right pkey, instead be assigned to key 0.
pub mod pkey_alloc;

// For some reason, MAP_FAILED is not included in the above bindings. It has a
// fixed definition and is part of the Linux userspace ABI, hence we define it
// as a constant here:
const MAP_FAILED: *mut std::ffi::c_void = !0 as *mut std::ffi::c_void;

// Include the C runtime shared object, to be loaded as the first
// library in our new dlmopen link namespace:
static ENCAPFN_MPK_C_RT: &'static [u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/libencapfn_mpk_rt.so"));

static ENCAPFN_MPK_LOADER_STUB: &'static [u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/libencapfn_mpk_loader_stub.so"));

// TODO: update docs
// // Expected WRPKRU value to be set when transitioning into the foreign library.
// // This is a thread-local global static to be able to get a symbol to it, and
// // compare it after executing a WRPKRU instruction, without relying on any
// // state in the register file (that could be forged by untrusted code).
// //
// // This static will reside in Rust-memory that is inaccessible to foreign code.
// // It will occupy at least a full page, given its alignment constraints and
// // size. This allows us to assign it a PKEY that is readable while in foreign
// // code.
// //
// // Only the first array value (at a 0-byte offset) is used:
#[repr(C, align(4096))]
struct RustThreadState {
    runtime: *const (),
    pkru_shadow: u32,
}

const _: () = assert!(std::mem::align_of::<RustThreadState>() == PAGE_SIZE);
const _: () = assert!(std::mem::size_of::<RustThreadState>() == PAGE_SIZE);

impl RustThreadState {
    const fn new() -> Self {
        RustThreadState {
            runtime: std::ptr::null(),
            pkru_shadow: 0,
        }
    }
}

// TODO: figure out how to make this actually thread-local, in a way that
// foreign code cannot break
//#[thread_local]
static mut RUST_THREAD_STATE: RustThreadState = RustThreadState::new();

fn get_dlerror() -> Option<&'static std::ffi::CStr> {
    // Try to retrieve an error description from dlerror(), if one is available:
    let error_msg: *const i8 = unsafe { libc_bindings::dlfcn::dlerror() };

    if error_msg == std::ptr::null_mut() {
        None
    } else {
        Some(unsafe { std::ffi::CStr::from_ptr(error_msg) })
    }
}

static ENCAPFN_MPK_RT_COUNT: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug)]
pub struct LinkMapEntry {
    pub addr: *const (),
    pub name: CString,
}

#[derive(Clone, Debug)]
pub struct MemMapEntry {
    pub start: *const (),
    pub end: *const (),
    pub read: bool,
    pub write: bool,
    pub exec: bool,
    pub shared: bool,
    pub private: bool,
    pub offset: usize,
    pub major: usize,
    pub minor: usize,
    pub inode: usize,
    pub pathname: Option<String>, // Also contains other attributes like "(deleted)"
}

impl MemMapEntry {
    fn as_mprotect_prot(&self) -> std::ffi::c_int {
        let mut prot = libc_bindings::sys_mman::PROT_NONE;

        if self.read {
            prot |= libc_bindings::sys_mman::PROT_READ;
        }

        if self.write {
            prot |= libc_bindings::sys_mman::PROT_WRITE;
        }

        if self.exec {
            prot |= libc_bindings::sys_mman::PROT_EXEC;
        }

        prot as std::ffi::c_int
    }
}

fn parse_proc_self_maps() -> Vec<MemMapEntry> {
    std::fs::read_to_string("/proc/self/maps")
        .unwrap()
        .trim()
        .lines()
        .map(|line| {
            fn perm_set(perm: &str, expected: &str, permissible: &[&str]) -> bool {
                if perm == expected {
                    true
                } else if !permissible.contains(&perm) {
                    panic!(
                        "Unexpected permissions string \"{}\", expected \"{}\" or \"-\"",
                        perm, expected
                    );
                } else {
                    false
                }
            }

            let mut split = line.trim().splitn(6, " ");

            // start-end
            let start_end = split.next().unwrap().trim().split("-").collect::<Vec<_>>();
            assert!(start_end.len() == 2);
            let start = usize::from_str_radix(start_end[0], 16).unwrap() as *const ();
            let end = usize::from_str_radix(start_end[1], 16).unwrap() as *const ();

            // rwxp, split as ["", "r", "w", "x", "p", ""]
            let perms = split.next().unwrap().trim().split("").collect::<Vec<_>>();
            assert!(perms.len() == 6);
            let read = perm_set(perms[1], "r", &["-"]);
            let write = perm_set(perms[2], "w", &["-"]);
            let exec = perm_set(perms[3], "x", &["-"]);
            let shared = perm_set(perms[4], "s", &["-", "p"]);
            let private = perm_set(perms[4], "p", &["-", "s"]);

            // offset
            let offset = usize::from_str_radix(split.next().unwrap(), 16).unwrap();

            // major:minor
            let major_minor = split.next().unwrap().trim().split(":").collect::<Vec<_>>();
            assert!(major_minor.len() == 2);
            let major = usize::from_str_radix(major_minor[0], 16).unwrap();
            let minor = usize::from_str_radix(major_minor[1], 16).unwrap();

            // inode
            let inode = usize::from_str_radix(split.next().unwrap(), 10).unwrap();

            // inode
            let pathname = split.next().map(|s| s.trim().to_string());

            // We want the pathname (potentially containing spaces) to be fully
            // captured in the last yield of the split iterator, hence ensure
            // that we don't have any remaining strings:
            assert!(split.next().is_none());

            MemMapEntry {
                start,
                end,
                read,
                write,
                exec,
                shared,
                private,
                offset,
                major,
                minor,
                inode,
                pathname,
            }
        })
        .collect()
}

unsafe fn get_link_map_entries(library_handle: *mut std::ffi::c_void) -> Vec<LinkMapEntry> {
    let mut lm_entries = Vec::new();

    {
        let mut link_map_ptr: *const libc_bindings::link::link_map = std::ptr::null();

        // Retrieve the start of the link map:
        assert!(
            0 == unsafe {
                libc_bindings::dlfcn::dlinfo(
                    library_handle,
                    libc_bindings::dlfcn::RTLD_DI_LINKMAP as ::std::os::raw::c_int,
                    &mut link_map_ptr as *mut *const _ as *mut ::std::os::raw::c_void,
                )
            },
            "Retreiving link map failed!",
        );

        // Ensure that we actually hold a valid link map entry:
        assert!(link_map_ptr != std::ptr::null());

        // Ensure that the first element of the link map doesn't have
        // predecessor (such that we aren't skipping over any element):
        assert!(unsafe { (*link_map_ptr).l_prev } == std::ptr::null_mut());

        // Now, iterate through the complete list:
        while link_map_ptr != std::ptr::null() {
            let entry: &libc_bindings::link::link_map = unsafe { &*link_map_ptr };
            lm_entries.push(LinkMapEntry {
                addr: entry.l_addr as *const (),
                name: (unsafe { std::ffi::CStr::from_ptr(entry.l_name) }).into(),
            });

            // Advance to next link map entry:
            link_map_ptr = unsafe { (&*link_map_ptr).l_next };
        }
    }

    lm_entries
}

#[derive(Debug, Clone)]
struct LinkedLibraryMappings {
    link_map_entry: LinkMapEntry,
    mem_map_entries: Vec<MemMapEntry>,
}

fn match_link_map_mem_map_regions(
    link_map_entries: &mut [LinkMapEntry],
    mem_map_entries: &mut [MemMapEntry],
) -> Vec<LinkedLibraryMappings> {
    let mut library_mappings = Vec::with_capacity(link_map_entries.len());

    // Iterate over both lists, sorted by the mapping start address:
    link_map_entries.sort_by_key(|lm| lm.addr as usize);
    mem_map_entries.sort_by_key(|lm| lm.start as usize);

    // Now, step through all link_map_entries, while also iterating over
    // mem_map_entries in parallel. For each link map entry, iterate through the
    // memory maps for as long as the memory mapping's start address is lower
    // than the link mapping's entry.
    let mut mem_map_iter = mem_map_entries.iter().peekable();

    'outer: for link_map_entry in link_map_entries.iter() {
        // Initialize a new LinkedLibraryMappings object:
        library_mappings.push(LinkedLibraryMappings {
            link_map_entry: link_map_entry.clone(),
            mem_map_entries: vec![],
        });
        let llm = library_mappings.last_mut().unwrap();

        // Find the next mem_map_entry whose start address matches that of this
        // current link_map_entry:
        let mut matching_mem_map_entry = loop {
            match mem_map_iter.peek() {
                // We've reached the end of the iterator, return:
                None => break 'outer,
                Some(mem_map_entry) => {
                    if (mem_map_entry.start as usize) < (link_map_entry.addr as usize) {
                        // The next memory map entry is before the current
                        // library's start address, discard it and continue
                        // looking forward:
                        mem_map_iter.next();
                    } else if (mem_map_entry.start as usize) > (link_map_entry.addr as usize) {
                        // The next memory map entry is beyond the current
                        // library's start address. Avoid popping it and
                        // continue with the next library:
                        continue 'outer;
                    } else {
                        // This memory map entry matches our library's start
                        // address:
                        break mem_map_iter.next();
                    }
                }
            }
        };

        // Now, iterate through the mem_map_iter for as long as we have matching
        // memory map entries:
        while let Some(entry) = matching_mem_map_entry {
            llm.mem_map_entries.push(entry.clone());
            matching_mem_map_entry = None;

            if let Some(next_mem_map_entry) = mem_map_iter.peek() {
                // Also tolerate zero-mappings for .bss
                if next_mem_map_entry.pathname == entry.pathname
                    || (next_mem_map_entry.inode == 0 && next_mem_map_entry.pathname.is_none())
                {
                    matching_mem_map_entry = mem_map_iter.next();
                }
            }
        }
    }

    library_mappings
}

#[repr(C)]
pub struct EncapfnMPKRtAsmState {
    // Foreign stack pointer, read by the protection-domain switch assembly
    // and used as a base to copy stacked arguments & continue execution from:
    foreign_stack_ptr: Cell<*mut ()>,

    // Foreign stack top (exclusive). Stack grows downward from here:
    foreign_stack_top: *mut (),

    // Foreign stack bottom (inclusive). Last usable stack address:
    foreign_stack_bottom: *mut (),

    // PKRU value while foreign code is running:
    foreign_code_pkru: u32,

    // Scratch-space for the protection-domain switch assembly to store the
    // Rust stack pointer while executing foreign code.
    rust_stack_ptr: UnsafeCell<*mut ()>,

    // Scratch-space to store the InvokeRes pointer for encoding the function's
    // return value while executing foreign code:
    invoke_res_ptr: UnsafeCell<*mut EncapfnMPKInvokeResInner>,

    // Log-prefix String. Contained in asm state, as it should be accessible to
    // callbacks running before the runtime struct is fully built:
    log_prefix: String,
}

#[repr(C)]
pub struct EncapfnMPKRt<ID: EFID> {
    // This struct is used both in the protection-domain switch assembly,
    // and in regular Rust code. However, we want to avoid hard-coding offsets
    // into this struct in assembly, but instead use ::core::ptr::offset_of!
    // to resolve offsets of relevant fields at compile. Unfortunately, that is
    // not possible, in general, for a generic type without knowing the generic
    // argument. Instead, we move all assembly-relevant state into a separate
    // struct `EncapfnMPKRtAsmState`, which does not have generic parameters.
    // We ensure that this struct is placed at the very beginning of the
    // `EncapfnMPKRt` type, for every possible combination of generic
    // parameters, through an assertion in its constructor.
    asm_state: EncapfnMPKRtAsmState,

    id: usize,

    // Handle of the Encapfn MPK runtime library, loaded via dlmopen into a
    // fresh link namespace.
    rt_lib_handle: *mut std::ffi::c_void,

    // The namespace (link-map list) ID of the runtime library, and thus
    // including all of its dependencies:
    rt_lmid: libc_bindings::dlfcn::Lmid_t,

    // Handles of all user-supplied libraries, in the order they were supplied:
    lib_handles: Vec<*mut std::ffi::c_void>,

    // Pkey assigned to this library's mutable state:
    pkey_library: std::ffi::c_int,

    // Pkey assigned to pages that are exposed read-only to this library:
    pkey_library_ro: std::ffi::c_int,

    // If we have one, pkey assigned to pages allocated to Rust:
    pkey_rust: Option<std::ffi::c_int>,

    _id: PhantomData<ID>,

    // Ensure that the runtime is !Sync. Currently the runtime cannot be shared
    // across threads!
    //
    // For this we'd need to support multiple threads, think about concurrent
    // accesses to foreign memory, etc.
    //
    // We cannot directly impl !Sync, as that is still unstable. Instead, we
    // use a !Send and !Sync member type to enforce these negative trait
    // bounds, as proposed here:
    // https://users.rust-lang.org/t/negative-trait-bounds-are-not-yet-fully-implemented-use-marker-types-for-now/64495/2
    //
    //impl<ID: EFID> !Sync for EncapfnMPKRt<ID> {}
    _not_sync: PhantomData<*const ()>,
}

impl<ID: EFID> EncapfnMPKRt<ID> {
    // TODO: mark unsafe
    pub fn new<N: AsRef<CStr>>(
        libraries: impl Iterator<Item = N>,
        _id: ID,
        pkey_rust: Option<std::ffi::c_int>,
        allow_global_read: bool,
    ) -> (
        Self,
        AllocScope<'static, EncapfnMPKRtAllocTracker, ID>,
        AccessScope<ID>,
    ) {
        // See the EncapfnMPKRt type definition for an explanation of this
        // const assertion. It is required to allow us to index into fields
        // of the nested `EncapfnMPKRtAsmState` struct from within assembly.
        //
        // Unfortunately, we cannot make this into a const assertion, as
        // constants are instantiated outside of the `impl` block.
        let _: () = assert!(std::mem::offset_of!(Self, asm_state) == 0);

        // Obtain a new ID, for globally addressing this runtime instance:
        let id = ENCAPFN_MPK_RT_COUNT.fetch_add(1, Ordering::Relaxed);
        info!("Initializing new EncapfnMPKRt instance, id {}", id);
        let log_prefix = format!("EncapfnMPKRt[#{}]:", id);

        // Create a map to track assigned memory regions protection keys for
        // debug output:
        let mut pkey_regions: HashMap<std::ffi::c_int, Vec<(Range<*mut ()>, Cow<'static, str>)>> =
            HashMap::new();

        // Acquire a PKEY for this library. If this system call does not work,
        // it either means that we have exhausted the PKEYs for this process
        // (shared across multiple threads) or that the system does not support
        // MPK. Report an error accordingly:
        let pkey_library = unsafe {
            libc_bindings::sys_mman::pkey_alloc(
                // Reserved flags argument, must be zero:
                0,
                // Default permissions set into PKRU for this pkey. Allow all
                // accesses while in Rust:
                0,
            )
        };
        if pkey_library <= 0 {
            panic!("Failed to allocate a pkey: {}", pkey_library);
        }
        pkey_regions.insert(pkey_library, vec![]);
        debug!(
            "{} Allocated MPK PKEY for R/W foreign library memory regions: {}",
            log_prefix, pkey_library
        );

        // Acquire another PKEY, for state that should be exposed read-only to
        // the foreign library.
        let pkey_library_ro = unsafe {
            libc_bindings::sys_mman::pkey_alloc(
                // Reserved flags argument, must be zero:
                0,
                // Default permissions set into PKRU for this pkey. Allow all
                // accesses while in Rust:
                0,
            )
        };
        if pkey_library_ro <= 0 {
            panic!("Failed to allocate a pkey: {}", pkey_library_ro);
        }
        pkey_regions.insert(pkey_library_ro, vec![]);
        debug!(
            "{} Allocated MPK PKEY for R-O foreign library memory regions: {}",
            log_prefix, pkey_library_ro
        );

        // Acquire another PKEY, for state that should be exposed read-only to
        // all foreign libraries (TODO: share this with other instances)
        let pkey_global_ro = unsafe {
            libc_bindings::sys_mman::pkey_alloc(
                // Reserved flags argument, must be zero:
                0,
                // Default permissions set into PKRU for this pkey. Allow all
                // accesses while in Rust:
                0,
            )
        };
        if pkey_global_ro <= 0 {
            panic!("Failed to allocate a pkey: {}", pkey_global_ro);
        }
        pkey_regions.insert(pkey_global_ro, vec![]);
        debug!(
            "{} Allocated MPK PKEY for R-O global memory regions: {}",
            log_prefix, pkey_global_ro
        );

        // Calculate the PKRU value that we should load while this library is
        // executing. We should allow all accesses to the pkey_library, and
        // only reads to the pkey_library_ro key:
        const ALLOW_ALL: u32 = 0b11;
        const WD: u32 = 0b01;
        const AD_WD: u32 = 0b00;

        let default_key_perm = match (pkey_rust.is_some(), allow_global_read) {
            (true, _) => !ALLOW_ALL,
            (false, true) => !WD,
            (false, false) => !AD_WD,
        };

        let foreign_code_pkru: u32 = default_key_perm
            & !(WD << (pkey_library_ro * 2))
            & !(WD << (pkey_global_ro * 2))
            & !(ALLOW_ALL << (pkey_library * 2));

        debug!(
            "{} Calculated foreign code PKRU register as: {:08x}",
            log_prefix, foreign_code_pkru
        );

        enum MemfdOrTempfile {
            Memfd(memfd::Memfd),
            PersistTempfile(std::fs::File, std::path::PathBuf),
        }

        impl MemfdOrTempfile {
            fn as_file(&self) -> &std::fs::File {
                match self {
                    MemfdOrTempfile::Memfd(mfd) => mfd.as_file(),
                    MemfdOrTempfile::PersistTempfile(f, _) => &f,
                }
            }

            fn path(&self) -> std::borrow::Cow<'_, std::path::Path> {
                match self {
                    MemfdOrTempfile::Memfd(mfd) => {
                        let mut pb = PathBuf::from("/proc/self/fd");
                        pb.push(format!("{}", mfd.as_file().as_raw_fd() as std::ffi::c_int));
                        std::borrow::Cow::Owned(pb)
                    }
                    MemfdOrTempfile::PersistTempfile(_, p) => std::borrow::Cow::Borrowed(&p),
                }
            }

            fn for_mfd<R>(&self, f: impl FnOnce(&memfd::Memfd) -> R) -> Option<R> {
                match self {
                    MemfdOrTempfile::Memfd(mfd) => Some(f(&mfd)),
                    MemfdOrTempfile::PersistTempfile(_, _) => None,
                }
            }
        }

        let rt = if !ENABLE_DEBUG {
            // We create an in-memory file descriptor to the Encapfn MPK runtime,
            // included as static bytes. This is slightly wasteful (loads `n + 1`
            // copies of this library in memory for `n` runtime instances), but
            // should be fine given the small size of this library.
            //
            // Once all references to the file are dropped (we let the `File` go out
            // of scope, and the shared library is no longer referenced) the
            // allocated memory will automatically be released;
            let rt_mfd = memfd::MemfdOptions::default()
                .allow_sealing(true)
                .create(&format!("libencapfn_mpk_rt_{}.so", id))
                .expect(
                    "Unable to create in-memory file descriptor for Encapfn MPK C shared library",
                );
            MemfdOrTempfile::Memfd(rt_mfd)
        } else {
            // Debug mode, use a named tempfile instead. This file can then
            // be inspected by a debugger, such as gdb:
            let (file, pathbuf) = tempfile::NamedTempFile::new().unwrap().keep().unwrap();
            MemfdOrTempfile::PersistTempfile(file, pathbuf)
        };

        let mut rt_file = rt.as_file();
        rt_file
            .write_all(ENCAPFN_MPK_C_RT)
            .expect("Failed writing the Encapfn MPK C shared library to the memfd");
        rt_file
            .seek(SeekFrom::Start(0))
            .expect("Failed seek in the Encapfn MPK C shared library memfd");
        rt_file
            .flush()
            .expect("Failed to flush the Encapfn MPK C shared library memfd");

        // Add seals to prevent further changes.
        rt.for_mfd(|mfd| {
            mfd.add_seals(&[
                memfd::FileSeal::SealShrink,
                memfd::FileSeal::SealGrow,
                memfd::FileSeal::SealWrite,
                memfd::FileSeal::SealSeal,
            ])
            .expect("Failed to seal Encapfn MPK C shared library memfd")
        });
        debug!(
            "{} Loaded EncapfnMPK Rt runtime at path: {:?}",
            log_prefix,
            rt.path()
        );

        // Create a new link namespace, with only the Encapfn MPK C runtime
        // loaded for now. This ensures that the symbols defined in that library
        // will take precendence over all symbols loaded in subsequent libraries
        // (provided the right flags given to dlmopen).
        let rt_cstr_path = CString::new(rt.path().as_os_str().as_encoded_bytes()).unwrap();
        let rt_lib_handle = unsafe {
            libc_bindings::dlfcn::dlmopen(
                libc_bindings::dlfcn::LM_ID_NEWLM as std::os::raw::c_long,
                rt_cstr_path.as_ptr(),
                libc_bindings::dlfcn::RTLD_NOW as std::os::raw::c_int
                    | libc_bindings::dlfcn::RTLD_LOCAL as std::os::raw::c_int
                    | libc_bindings::dlfcn::RTLD_DEEPBIND as std::os::raw::c_int,
            )
        };
        std::mem::drop(rt_cstr_path);

        // Check whether the library was correctly loaded, otherwise print error
        // and exit:
        if rt_lib_handle == std::ptr::null_mut() {
            panic!(
                "Failed to load base Encapfn MPK C runtime shared library: {:?}",
                get_dlerror()
            );
        }
        debug!(
            "{} Loaded EncapfnMPK Rt with dlmopen into new namespace, handle: {:?}",
            log_prefix, rt_lib_handle
        );

        // Allocate a new stack for the library to execute from:
        let foreign_stack_bottom = unsafe {
            libc_bindings::sys_mman::mmap(
                // We don't care about the mapping address, as long as its page aligned, which the
                // kernel guarantees us:
                std::ptr::null_mut(),
                STACK_SIZE,
                (libc_bindings::sys_mman::PROT_READ | libc_bindings::sys_mman::PROT_WRITE)
                    as std::ffi::c_int,
                (libc_bindings::sys_mman::MAP_PRIVATE
                    | libc_bindings::sys_mman::MAP_ANONYMOUS
                    | libc_bindings::sys_mman::MAP_STACK) as std::ffi::c_int,
                -1, // don't map any fd, required by MAP_ANONYMOUS
                0,  // no fd, no offset
            )
        } as *mut ();
        if foreign_stack_bottom == MAP_FAILED as *mut () {
            panic!("Failed mmapping stack memory! {:p}", foreign_stack_bottom);
        }

        // Calculate the stack top as well:
        let foreign_stack_top = unsafe { foreign_stack_bottom.byte_add(STACK_SIZE) };

        // Make the stack accessible to pkey_library:
        assert!(
            0 == unsafe {
                libc_bindings::sys_mman::pkey_mprotect(
                    foreign_stack_bottom as *mut std::ffi::c_void,
                    STACK_SIZE,
                    (libc_bindings::sys_mman::PROT_READ | libc_bindings::sys_mman::PROT_WRITE)
                        as std::ffi::c_int,
                    pkey_library,
                )
            },
            "Failed to pkey_mprotect foreign stack at {:p} for {:x?} bytes",
            foreign_stack_bottom as *mut std::ffi::c_void,
            STACK_SIZE,
        );
        pkey_regions.get_mut(&pkey_library).unwrap().push((
            Range {
                start: foreign_stack_bottom,
                end: foreign_stack_top,
            },
            Cow::Borrowed("Foreign Stack"),
        ));
        debug!(
            "{} Allocated foreign stack memory from {:p} down to  {:p}, protected with PKEY {}",
            log_prefix, foreign_stack_top, foreign_stack_bottom as *const u8, pkey_library
        );

        // Allocate new heap pages for the library's malloc to use:
        //
        // 32GB, should be good for now :)
        let foreign_heap_size = 128 * 1024 * 1024 * 1024;
        debug!(
            "{} Allocating heap pages for foreign code, {} MB",
            log_prefix,
            foreign_heap_size / 1024 / 1024
        );

        let foreign_heap_start = unsafe {
            libc_bindings::sys_mman::mmap(
                // We don't care about the mapping address, as long as its page aligned, which the
                // kernel guarantees us:
                std::ptr::null_mut(),
                foreign_heap_size,
                (libc_bindings::sys_mman::PROT_READ | libc_bindings::sys_mman::PROT_WRITE)
                    as std::ffi::c_int,
                (libc_bindings::sys_mman::MAP_PRIVATE | libc_bindings::sys_mman::MAP_ANONYMOUS)
                    as std::ffi::c_int,
                // TODO: make hugepages optional
                // // It seems safer to assume that we have 2MB hugepages than
                // // 1GB hugepages. We should autodetect / configure this
                // // though. 2MB is the default, smallest pagesize.
                // | libc_bindings::sys_mman::MAP_HUGETLB) as std::ffi::c_int,
                -1, // don't map any fd, required by MAP_ANONYMOUS
                0,  // no fd, no offset
            )
        } as *mut ();

        if foreign_heap_start == MAP_FAILED as *mut () {
            panic!("Failed mmapping heap memory! {:p}", foreign_heap_start);
        }
        let foreign_heap_end = unsafe { foreign_heap_start.byte_add(foreign_heap_size) };

        let res = unsafe {
            libc_bindings::sys_mman::pkey_mprotect(
                foreign_heap_start as *mut _,
                foreign_heap_size,
                (libc_bindings::sys_mman::PROT_READ | libc_bindings::sys_mman::PROT_WRITE) as i32,
                pkey_library,
            )
        };
        if res != 0 {
            panic!(
                "Failed performing pkey_mprotect for allocated memory at {:p} for {:x?} bytes",
                foreign_heap_start as *mut _, foreign_heap_size,
            );
        }
        pkey_regions.get_mut(&pkey_library).unwrap().push((
            Range {
                start: foreign_heap_start,
                end: foreign_heap_end,
            },
            Cow::Borrowed("Foreign Heap"),
        ));
        debug!(
            "{} Allocated heap pages at {:p} -- {:p}, protected with library PKEY {}",
            log_prefix, foreign_heap_start, foreign_heap_end, pkey_library
        );

        let mut asm_state = EncapfnMPKRtAsmState {
            foreign_stack_ptr: Cell::new(foreign_stack_top),
            foreign_stack_bottom,
            foreign_stack_top,
            foreign_code_pkru: 0, // run cb init without memory protection for now

            // Scratch-space, initialize with dummy value:
            rust_stack_ptr: UnsafeCell::new(std::ptr::null_mut()),

            // Scratch-space, initialize with dummy value:
            invoke_res_ptr: UnsafeCell::new(std::ptr::null_mut()),

            log_prefix,
        };

        let runtime_init_addr =
            unsafe { libc_bindings::dlfcn::dlsym(rt_lib_handle, c"ef_runtime_init".as_ptr()) };

        if runtime_init_addr == std::ptr::null_mut() {
            panic!("Cannot initialize runtime, symbol ef_runtime_init not found");
        }

        debug!(
            "{} Initializing runtime by calling \"ef_runtime_init\" at {:p}",
            asm_state.log_prefix, runtime_init_addr
        );

        extern "C" {
            static environ: *const *const std::ffi::c_char;
        }

        let mut res = <Self as SysVAMD64BaseRt>::InvokeRes::<()>::new();
        unsafe {
            Self::rt_init(
                &asm_state as *const _ as *const Self,
                runtime_init_addr as *const u8 as *const (),
                &mut res as *mut _,
                foreign_heap_end,
                foreign_heap_start,
                environ,
            )
        };
        res.encode_eferror().unwrap();
        debug!(
            "{} Callback infrastructure initialized!",
            asm_state.log_prefix
        );

        // Acquire the link namespace (link-map list) ID of the above library:
        let mut rt_lmid: libc_bindings::dlfcn::Lmid_t = 0;
        assert!(
            0 == unsafe {
                libc_bindings::dlfcn::dlinfo(
                    rt_lib_handle,
                    libc_bindings::dlfcn::RTLD_DI_LMID as std::os::raw::c_int,
                    &mut rt_lmid as *mut _ as *mut ::std::os::raw::c_void,
                )
            },
            "Failed to acquire the LMID of the Encapfn MPK runtime shared library",
        );
        debug!(
            "{} Resolved link namespace ID for runtime library: {:p}",
            asm_state.log_prefix, rt_lmid as *const ()
        );

        // Normally, we'd simply load the Encapfn MPK runtime into its own link
        // namespace (link-map list), and add all of its symbols to the global
        // symbol table (RTLD_GLOBAL). However, this is currently unsupported in
        // glibc, see [1].
        //
        //  We need to have the symbols in the Encapfn MPK runtime take
        // precedence over any symbols of the actual library loaded, or its
        // dependencies. Without `RTLD_GLOBAL`, the next best method is to load
        // both the runtime and target library in the same `dlmopen` call, using
        // a third library that declares both those libraries as `NEEDED`. Thus
        // we include a stub "loader" library, that is virtually empty (contains
        // a dummy symbol to please the dynamic linker). This library is then
        // patched using `patchelf`, to include both the Encapfn MPK runtime and
        // the target library as its dependencies, in this order. This library,
        // alongside its dependencies, will then be loaded into a new link
        // namespace.
        //
        // [1]: https://patchwork.ozlabs.org/project/glibc/patch/55A73673.3060104@redhat.com/
        let loader = if !ENABLE_DEBUG {
            let loader_mfd = memfd::MemfdOptions::default()
                .allow_sealing(true)
                .create(&format!("libencapfn_mpk_c_loader_patched_{}.so", id))
                .expect("Unable to create in-memory file descriptor for Encapfn MPK loader stub");
            MemfdOrTempfile::Memfd(loader_mfd)
        } else {
            let (file, pathbuf) = tempfile::NamedTempFile::new().unwrap().keep().unwrap();
            MemfdOrTempfile::PersistTempfile(file, pathbuf)
        };

        let mut loader_file = loader.as_file();
        loader_file
            .write_all(ENCAPFN_MPK_LOADER_STUB)
            .expect("Failed writing the Encapfn MPK loader stub to the memfd");
        loader_file
            .flush()
            .expect("Failed to flush the Encapfn MPK loader stub memfd");

        // Add the runtime and actual library to the loader stub as NEEDED.
        //
        // This should really take OsStrings or CStrings, but alas.
        //
        // The path to the rt memfd that we add as a `NEEDED` library to the
        // loader ELF below is only valid for as long as the file is in
        // scope. We ensure that the file lives sufficiently long by dropping it
        // at the end of the function:
        let loader_file_path = loader.path();
        assert!(patchelf::PatchElf::config()
            .input(&loader_file_path.to_str().unwrap())
            .output(&loader_file_path.to_str().unwrap())
            .set_add_needed(rt.path().to_str().unwrap())
            .patch());

        for library in libraries {
            debug!(
                "{} Adding library {:?} to dummy loader ELF file",
                asm_state.log_prefix,
                library.as_ref()
            );
            assert!(patchelf::PatchElf::config()
                .input(&loader_file_path.to_str().unwrap())
                .output(&loader_file_path.to_str().unwrap())
                .set_add_needed(library.as_ref().to_str().unwrap())
                .patch());
        }

        // Seek to start again, in case patchelf reused our fd:
        loader_file
            .seek(SeekFrom::Start(0))
            .expect("Failed seek in the Encapfn MPK loader stub memfd");

        // Add seals to prevent changes.
        loader.for_mfd(|mfd| {
            mfd.add_seals(&[
                memfd::FileSeal::SealShrink,
                memfd::FileSeal::SealGrow,
                memfd::FileSeal::SealWrite,
                memfd::FileSeal::SealSeal,
            ])
            .expect("Failed to seal Encapfn MPK C shared library memfd")
        });

        debug!(
            "{} Loading dummy loader ELF file from {:?} into runtime's link namespace ({:p})",
            asm_state.log_prefix,
            loader.path(),
            rt_lmid as *const ()
        );

        // Now, convert the fd path into a CString and open it with dlmopen:
        let loader_fd_cpath = CString::new(loader_file_path.as_os_str().as_encoded_bytes())
            .expect("Unexpected null-terminator in memfd path");

        // TODO: document!
        let loader_lib_handle = unsafe {
            libc_bindings::dlfcn::dlmopen(
                rt_lmid,
                loader_fd_cpath.as_ptr(),
                libc_bindings::dlfcn::RTLD_NOW as std::os::raw::c_int
                    | libc_bindings::dlfcn::RTLD_LOCAL as std::os::raw::c_int
                    | libc_bindings::dlfcn::RTLD_DEEPBIND as std::os::raw::c_int,
            )
        };

        // Check whether the library was correctly loaded, otherwise print error
        // and exit:
        if loader_lib_handle == std::ptr::null_mut() {
            panic!(
                "Failed to load base Encapfn MPK C loader shared library: {:?}",
                get_dlerror()
            );
        }

        debug!("{} Loaded dummy loader ELF file", asm_state.log_prefix);

        // TODO!
        let lib_handles = vec![loader_lib_handle];

        // Use memory protection for all subsequent calls into the library:
        asm_state.foreign_code_pkru = foreign_code_pkru;
        debug!("{} Engaging memory protection for all subsequent foreign library calls with PKRU {:08x}", asm_state.log_prefix, asm_state.foreign_code_pkru);

        // Now, iterate through all loaded objects in the new link namespace and
        // collect their start addresses:
        let rt_lm_entries = unsafe { get_link_map_entries(rt_lib_handle) };
        debug!(
            "{} Queried runtime link map: {:#?}",
            asm_state.log_prefix, rt_lm_entries
        );

        // Acquire the dlinfo handle for the main program, such that we can
        // filter any overlapping regions (such as the dynamic linker itself)
        // from the link-map entries returned for the new namespace:
        let main_program_handle = unsafe {
            libc_bindings::dlfcn::dlopen(
                std::ptr::null(),
                libc_bindings::dlfcn::RTLD_NOW as std::os::raw::c_int
                    | libc_bindings::dlfcn::RTLD_NOLOAD as std::os::raw::c_int,
            )
        };
        assert!(main_program_handle != std::ptr::null_mut());
        let main_program_lm_entries = unsafe { get_link_map_entries(main_program_handle) };
        debug!(
            "{} Queried main program link map: {:#?}",
            asm_state.log_prefix, main_program_lm_entries
        );

        // Now, remove shared entries from the `rt_lm_entries` list:
        let mut rt_lm_entries_exclusive = rt_lm_entries
            .clone() // TODO: remove when we remove the hack for main_program_lm_entries_exclusive
            .into_iter()
            .filter(|entry| {
                !main_program_lm_entries
                    .iter()
                    .find(|mpe| mpe.addr == entry.addr)
                    .is_some()
            })
            .collect::<Vec<_>>();

        let mut mmaps = parse_proc_self_maps();

        // Assign all pages of the loaded library and its dependencies to the
        // pkey as allocated above:
        for library in match_link_map_mem_map_regions(&mut rt_lm_entries_exclusive, &mut mmaps) {
            for region in library.mem_map_entries {
                assert!(
                    0 == unsafe {
                        libc_bindings::sys_mman::pkey_mprotect(
                            region.start as *mut _,
                            region.end as usize - region.start as usize,
                            region.as_mprotect_prot(),
                            pkey_library,
                        )
                    },
                    "Failed performing pkey_mprotect for library {:?}, region {:?}",
                    &library.link_map_entry,
                    region,
                );
                pkey_regions.get_mut(&pkey_library).unwrap().push((
                    Range {
                        start: region.start as *mut _,
                        end: region.end as *mut _,
                    },
                    Cow::Owned(format!("Foreign Library {:?}", library.link_map_entry.name)),
                ));
            }
        }

        // Okay, this is real bad, and a hack that should be removed. Despite
        // loading things into a new link namespace, the new library can still
        // depend on the single shared libc, and resolve some global state
        // through it. To avoid rigging every single libc call for now, we leave
        // this security hole open and provide access to the ld library for
        // foreign code:
        let mut main_program_lm_entries_exclusive = main_program_lm_entries
            .clone() // TODO: remove clone when pkey_global_ro is removed
            .into_iter()
            .filter(|entry| {
                !rt_lm_entries
                    .iter()
                    .find(|le| le.addr == entry.addr)
                    .is_some()
            })
            .collect::<Vec<_>>();

        // Assign all main-program libraries to the Rust pkey, if we have one:
        if let Some(pkey_main_program) = pkey_rust {
            for library in
                match_link_map_mem_map_regions(&mut main_program_lm_entries_exclusive, &mut mmaps)
            {
                for region in library.mem_map_entries {
                    assert!(
                        0 == unsafe {
                            libc_bindings::sys_mman::pkey_mprotect(
                                region.start as *mut _,
                                region.end as usize - region.start as usize,
                                region.as_mprotect_prot(),
                                pkey_main_program,
                            )
                        },
                        "Failed performing pkey_mprotect for library {:?}, region {:?}",
                        &library.link_map_entry,
                        region
                    );
                    unimplemented!("Add memory region to pkey_regions");
                }
            }
        }

        // Test: does libc in the foreign library work with read-only access to the linker?
        //
        // Answer: it does (at least many functions do)! Thus incorporate this proper!
        //
        // For this, determine all shared libraries between the two namespaces
        // and assign them to the pkey_global_ro:
        let mut lm_entries_shared = main_program_lm_entries
            .into_iter()
            .filter(|entry| {
                rt_lm_entries
                    .iter()
                    .find(|le| le.addr == entry.addr)
                    .is_some()
            })
            .collect::<Vec<_>>();

        for library in match_link_map_mem_map_regions(&mut lm_entries_shared, &mut mmaps) {
            for region in library.mem_map_entries {
                assert!(
                    0 == unsafe {
                        libc_bindings::sys_mman::pkey_mprotect(
                            region.start as *mut _,
                            region.end as usize - region.start as usize,
                            region.as_mprotect_prot(),
                            pkey_global_ro,
                        )
                    },
                    "Failed performing pkey_mprotect for library {:?}, region {:?}",
                    &library.link_map_entry,
                    region
                );
                pkey_regions.get_mut(&pkey_global_ro).unwrap().push((
                    Range {
                        start: region.start as *mut _,
                        end: region.end as *mut _,
                    },
                    Cow::Owned(format!("Common Library {:?}", library.link_map_entry.name)),
                ));
            }
        }

        // Make sure that the library_ro pkey is assigned to all required
        // read-only pages. (Rust will still have read/write access!)
        assert!(unsafe { std::ptr::addr_of_mut!(RUST_THREAD_STATE) } as usize % PAGE_SIZE == 0);
        assert!(std::mem::size_of::<RustThreadState>() == 4096);
        assert!(
            0 == unsafe {
                libc_bindings::sys_mman::pkey_mprotect(
                    std::ptr::addr_of_mut!(RUST_THREAD_STATE) as *mut std::ffi::c_void,
                    std::mem::size_of::<RustThreadState>(),
                    (libc_bindings::sys_mman::PROT_READ | libc_bindings::sys_mman::PROT_WRITE)
                        as std::ffi::c_int,
                    pkey_library_ro,
                )
            },
            "Failed to pkey_mprotect read-only pages at {:p} for {:x?} bytes",
            unsafe { std::ptr::addr_of_mut!(RUST_THREAD_STATE) } as *mut std::ffi::c_void,
            std::mem::size_of::<RustThreadState>(),
        );
        pkey_regions.get_mut(&pkey_library_ro).unwrap().push((
            Range {
                start: unsafe { std::ptr::addr_of_mut!(RUST_THREAD_STATE) } as *mut (),
                end: unsafe {
                    (std::ptr::addr_of_mut!(RUST_THREAD_STATE) as *mut ())
                        .byte_add(std::mem::size_of::<RustThreadState>())
                },
            },
            Cow::Borrowed("Rust Thread State"),
        ));

        // We need to support vDSOs. Enable R/W on that memory region. TODO: this is problematic!
        if let Some(vdso_region) = mmaps
            .iter()
            .find(|m| m.pathname.as_ref().map(|p| p == "[vdso]").unwrap_or(false))
        {
            assert!(
                0 == unsafe {
                    libc_bindings::sys_mman::pkey_mprotect(
                        vdso_region.start as *mut _,
                        vdso_region.end as usize - vdso_region.start as usize,
                        vdso_region.as_mprotect_prot(),
                        pkey_library,
                    )
                },
                "Failed to pkey_mprotect read-write vDSO pages at {:p} for {:x?} bytes",
                vdso_region.start,
                vdso_region.end as usize - vdso_region.start as usize,
            );
            pkey_regions.get_mut(&pkey_library).unwrap().push((
                Range {
                    start: vdso_region.start as *mut (),
                    end: vdso_region.end as *mut (),
                },
                Cow::Borrowed("vDSO"),
            ));
        }

        debug!("Assigned PKEYs to memory regions: {:#?}", pkey_regions);

        let rt = EncapfnMPKRt {
            asm_state,
            id,
            rt_lib_handle,
            rt_lmid,
            lib_handles,
            pkey_library,
            pkey_library_ro,
            pkey_rust,
            _id: PhantomData,
            _not_sync: PhantomData,
        };

        debug!(
            "Maps:\n{}",
            std::fs::read_to_string("/proc/self/maps").unwrap()
        );

        // Drop file descriptors to memfd's after the library is fully
        // loaded. Otherwise it may be possible that the memfd gets reclaimed
        // before the dynamic linker had a chance to acquire a new file
        // descriptor to it:
        #[allow(dropping_references)]
        {
            std::mem::drop(rt_file);
        }

        (
            rt,
            unsafe {
                AllocScope::new(EncapfnMPKRtAllocTracker {
                    allocations: LinkedList::new(),
                })
            },
            unsafe { AccessScope::new() },
        )
    }

    #[naked]
    unsafe extern "C" fn rt_init(
        rt: *const Self,
        runtime_init_addr: *const (),
        res: *mut EncapfnMPKInvokeRes<Self, ()>,
        top: *const (),
        bottom: *const (),
        environ: *const *const std::ffi::c_char,
    ) {
        core::arch::naked_asm!(
            "
            // We don't rely on the foreign function to retain our
            // callee-saved registers, hence stack them. This is written
            // to match the assumptions in generic_invoke:
            push rbx
            push rbp
            push r12
            push r13
            push r14
            push r15

            // Load required parameters for generic_invoke into
            // non-argument registers and continue execution in the
            // generic protection-domain switch routine:
            mov r10, rdi                   // Load runtime pointer into r10
            mov r11, rsi                   // Load function pointer into r11
            mov r12, rdx                   // Load invoke res pointer into r12
            mov r13, 0                     // Copy the stack-spill immediate into r12

            // Load the function arguments:
            // - rdi: callback_addr
            // - rsi: heap_top
            // - rdx: heap_bottom
            // - rcx: environ
            lea rdi, [rip - {raw_callback_handler_sym}]
            mov rsi, rcx
            mov rdx, r8
            mov rcx, r9

            // Continue execution at generic_invoke, which will return from
            // this function for us:
            lea r14, [rip - {generic_invoke_sym}]
            jmp r14
            ",
            generic_invoke_sym = sym Self::generic_invoke,
            raw_callback_handler_sym = sym Self::raw_callback_handler,
        );
    }

    unsafe extern "C" fn callback_handler(
        asm_state: &EncapfnMPKRtAsmState,
        id: usize,
        arg0: usize,
        arg1: usize,
        arg2: usize,
        arg3: usize,
    ) {
        // It is not always legal to upgrade our asm_state pointer to a runtime
        // pointer. Some initial entries into the foreign library (and
        // subsequent callbacks) are made without the fully constructed
        // Runtime). Hence, check whether it's constructed before casting
        // `asm_state` to an `rt: &Self`!

        // TODO: debug segfaults here, not good. Why?
        eprintln!(
            "{} Got callback with ID {}, args: {:016x}, {:016x}, {:016x}, {:016x}",
            asm_state.log_prefix, id, arg0, arg1, arg2, arg3
        );
        std::io::stdout().flush().unwrap();
    }

    #[naked]
    unsafe extern "C" fn raw_callback_handler() {
        core::arch::naked_asm!(
            "
                // We arrive here with the MPK protection mechanism still
                // engaged. Thus, disable those first, and then restore the
                // necessary Rust environment:

                // Foreign code may have passed arguments in rcx and rdx,
                // however we do need to clobber them. Thus we temporarily
                // move those registers into other callee-saved registers:
                mov r10, rcx
                mov r11, rdx

                // Restore access to all PKEYs. All of rax, rcx and rdx are
                // caller-saved, so we can clobber them here:
                xor rax, rax           // Clear rax, used to write PKRU
                xor rcx, rcx           // Clear rcx, required for WRPKRU
                xor rdx, rdx           // Clear rdx, required for WRPKRU
                wrpkru

                // Restore the argument registers:
                mov rdx, r11
                mov rcx, r10

                // We're back in 'trusted code' here. To avoid any spurious SIGSEGV's
                // later on, make sure that untrusted code has indeed cleared PKRU
                // correctly:
                test eax, eax
                jz 200f                // If zero, PKRU cleared correctly.
                ud2                    // If not zero, crash with an illegal insn

              200: // _pkru_cleared
                // Now, load the runtime pointer again and restore the Rust stack.
                // We load the runtime pointer into a callee-saved register that,
                // by convention, is reserved by all callback invocations:
                mov rdi, qword ptr [rip - {rust_thread_state_static} + {rts_runtime_offset}]

                // Update the foreign stack pointer in our runtime struct, such
                // that the callback handler can access it and we use it to
                // restore the stack pointer after the callback has been run:
                mov qword ptr [rdi + {rtas_foreign_stack_ptr_offset}], rsp

                // Now, restore the Rust stack. We did not use the red-zone in
                // the invoke functions, and hence can just align the stack
                // down to 16 bytes to call the function:
                mov rsp, qword ptr [rdi + {rtas_rust_stack_ptr_offset}]
                and rsp, -16

                // Push all values that we intend to retain on the Rust stack.
                // The Rust function follows the C ABI, so we don't need to
                // worry about this introducing any additional clobbers.
                //
                // For now, we only need to save the runtime pointer:
                push rdi               // Save rt pointer on Rust stack

                // Finally, invoke the callback handler:
                call {callback_handler_sym}

                // Restore the runtime pointer from the Rust stack:
                pop rdi

                // Restore the userspace stack pointer:
                mov rsp, qword ptr [rdi + {rtas_foreign_stack_ptr_offset}]

                // Now, switch back the PKEYs. For this, we need to preserve
                // the return value registers rax and rdx. This may overflow
                // the stack. TODO: should we handle this?
                push rax               // Save rax to the foreign stack
                push rdx               // Save rdx to the foreign stack

                // Move the intended PKRU value into the thread-local static, such
                // that we can compare it after we run the WRPKRU instruction.
                // This prevents it from being used as a gadget by untrusted code.
                mov eax, dword ptr [rdi + {rtas_foreign_code_pkru_offset}]
                mov dword ptr [rip - {rust_thread_state_static} + {rts_pkru_shadow_offset}], eax

                xor rcx, rcx           // Clear rcx, required for WRPKRU
                xor rdx, rdx           // Clear rdx, required for WRPKRU
                wrpkru

                // It is important that we now check that we have actually loaded the
                // intended value into the PKRU register. The RUST_THREAD_STATE static
                // is accessible read-only to foreign code, so read its PKRU shadow
                // copy and make sure that its value matches rax.
                //
                // We clobber the r13 scratch register for this, which we don't
                // need to save:
                push r13 // TODO!
                mov r13d, dword ptr [rip - {rust_thread_state_static} + {rts_pkru_shadow_offset}]
                cmp eax, r13d
                je 500f
                ud2                    // Crash with an illegal instruction

             500: // _pkru_loaded_verified
                pop r13

                // Restore the callback return values:
                pop rdx                // Pop rdx from foreign stack, still accessible
                pop rax                // Pop rax from foreign stack, still accessible

                // Now it is safe to return to the calling function on the
                // foreign stack:
                ret
            ",
            // Rust callback handler:
            callback_handler_sym = sym Self::callback_handler,
            // Rust thread-local state and offsets:
            rust_thread_state_static = sym RUST_THREAD_STATE,
            rts_runtime_offset = const std::mem::offset_of!(RustThreadState, runtime),
            rts_pkru_shadow_offset = const std::mem::offset_of!(RustThreadState, pkru_shadow),
            // Runtime ASM state offsets:
            rtas_rust_stack_ptr_offset = const std::mem::offset_of!(EncapfnMPKRtAsmState, rust_stack_ptr),
            rtas_foreign_stack_ptr_offset = const std::mem::offset_of!(EncapfnMPKRtAsmState, foreign_stack_ptr),
            rtas_foreign_code_pkru_offset = const std::mem::offset_of!(EncapfnMPKRtAsmState, foreign_code_pkru),
        )
    }

    #[naked]
    unsafe extern "C" fn generic_invoke() {
        core::arch::naked_asm!(
            "
                // When entering this symbol, we are supposed to invoke a foreign
                // function in an isolated protection domain (changing PKRU to revoke
                // permissions on regular Rust memory).
                //
                // At this stage, we have the all function arguments loaded into
                // registers and spilled on the stack as defined by the SysV AMD64
                // calling convention. We saved all callee-saved registers into the
                // SystemV AMD64 ABI's red-zone (128 bytes below rsp), so our stack
                // pointer is still set the beginning of the spilled arguments list.
                //
                // We have occupied the first 56 bytes of the red-zone at this point.
                //
                // The RT::invoke #[naked] functions also loaded some const-generic
                // data, and some information loaded on the stack / in
                // function-signature dependent registers into a set of well-defined
                // saved registers; specifically
                // - r10: &EncapfnMPKRtAsmState
                // - r11: function pointer to execute
                // - r12: &mut EncapfnMPKInvokeResInner
                // - r13: amount of bytes spilled on the stack
                // - r14: encapfn_mpk_sysv_amd64_invoke (this symbol)
                //
                // We need to copy the stacked arguments, set the PKRU register, and
                // finally jump to the function to execute. Upon return, we need to
                // re-enable access to Rust memory and encode the return value in the
                // wrapper type (TODO!).

                // First, save the original Rust stack pointer (including the function
                // call arguments). Also, save the runtime pointer into the Rust
                // thread-local state, and the InvokeRes pointer for encoding the
                // function's return value and any errrors:
                mov qword ptr [r10 + {rtas_rust_stack_ptr_offset}], rsp
                mov qword ptr [r10 + {rtas_invoke_res_ptr_offset}], r12
                mov qword ptr [rip - {rust_thread_state_static} + {rts_runtime_offset}], r10

                // First, copy the stacked arguments. For this we need to:
                //
                // 1. load the current foreign stack pointer,
                // 2. subtract the amount of bytes occupied by stacked arguments,
                // 3. align the new stack pointer downward to a 16-byte boundary,
                // 4. check whether the new stack pointer has overflowed the stack,
                // 5. copy `r13` bytes from our current stack pointer to the foreign
                //    stack.
                //
                // Load the foreign stack pointer from our runtime:
                mov r14, qword ptr [r10 + {rtas_foreign_stack_ptr_offset}]
                sub r14, r13           // Subtract stack_spill
                setc r15b              // If overflow, set r15b to 1, else 0
                and r14, -16           // Align downward to a 16 byte boundary

                // Now, check whether we overflowed our stack. This has happened when
                // the subtraction underflowed (wrapping), OR when we're below our
                // stack bottom now:
                test r15b, r15b        // Check if r15b is 0 (no stack underflow)
                jnz 200f               // If not zero, underflow occurred!

                // Also check if we're lower than stack_bottom:
                mov r15, qword ptr [r10 + {rtas_foreign_stack_bottom_offset}]
                cmp r14, r15           // Compare the new stack pointer against bottom
                jge 300f               // New sp is greater, no underflow!

                // Stack exceeded, fall through:
              200: // _stack_sub_underflow
                ud2                    // Crash with an illegal instruction
                // TODO! handle this error gracefully!

              300: // _no_stack_underflow
                // We have calculated our new stack pointer in r14. We now need to copy
                // r13 bytes from rsp upward to r14. We stored rsp above, and can thus
                // increment it. We still want to keep our new sp (r14) and thus modify
                // a copy in r15:
                mov r15, r14

                // Now, copy as long as we still have bytes to copy. The new and old
                // stacks are guaranteed to be 16-byte aligned, and arguments should be
                // padded to the pointer width, so use word-copies here.
                //
                // To make sure that we don't overshoot our loop, we ensure that r13
                // is always a multiple of 8 by rounding up (which, in the worst case,
                // would copy a couple of extra bytes):
                add r13, 7             // Should not overflow, can't copy ~2**64 bytes
                and r13, -8            // Ensure r13 is a multiple of 8, rounded up

              400: // _stack_copy
                test r13, r13
                jz 500f                // if r13 == 0, goto _stack_copied

                // Copy a qword from [rsp + {stack_spill} + 8] to [r15], using
                // rax as a scratch register. We add an 8-byte offset to rsp to
                // account for the return address that was stacked by the `call`
                // instruction that was used to run the ::invoke function in the
                // first place.
                mov rax, qword ptr [rsp + {stack_spill} + 8]
                mov qword ptr [r15], rax
                add rsp, 8
                add r15, 8
                sub r13, 8
                jmp 400b

              500: // _stack_copied
                // We copied our stack. Now, we use our new stack to push the registers
                // we need to clobber to execute the WRPKRU instruction:
                mov rsp, r14           // Switch to the foreign stack
                push rcx               // Save rcx to the foreign stack
                push rdx               // Save rdx to the foreign stack

                // Move the intended PKRU value into the thread-local static, such
                // that we can compare it after we run the WRPKRU instruction.
                // This prevents it from being used as a gadget by untrusted code.
                mov eax, dword ptr [r10 + {rtas_foreign_code_pkru_offset}]
                mov dword ptr [rip - {rust_thread_state_static} + {rts_pkru_shadow_offset}], eax

                xor rcx, rcx           // Clear rcx, required for WRPKRU
                xor rdx, rdx           // Clear rdx, required for WRPKRU
                wrpkru

                // It is important that we now check that we have actually loaded the
                // intended value into the PKRU register. The RUST_THREAD_STATE static
                // is accessible read-only to foreign code, so read its PKRU shadow
                // copy and make sure that its value matches rax.
                mov r14d, dword ptr [rip - {rust_thread_state_static} + {rts_pkru_shadow_offset}]
                cmp eax, r14d
                je 600f
                ud2                    // Crash with an illegal instruction

             600: // _pkru_loaded_verified
                pop rdx                // Pop rdx from foreign stack, still accessible
                pop rcx                // Pop rcx from foreign stack, still accessible

                // Finally, invoke our function on the new stack:
                call r11

                // Upon return we need to restore permissions for the Rust code. For
                // this, it is sufficient to clear the PKRU register. However, this
                // may overwrite the function's return value. Thus, save those values
                // to the foreign stack.
                //
                // We may be operating on an invalid / overflowing stack, in which case
                // this may fault. That is fine, though, as we will catch this using
                // our SIGSEGV handler.
                push rax               // Save rax to the foreign stack (retval)
                push rdx               // Save rdx to the foreign stack (retval)
                xor rax, rax           // Clear rax, used to write PKRU
                xor rcx, rcx           // Clear rcx, required for WRPKRU
                xor rdx, rdx           // Clear rdx, required for WRPKRU
                wrpkru

                // We're back in 'trusted code' here. To avoid any spurious SIGSEGV's
                // later on, make sure that untrusted code has indeed cleared PKRU
                // correctly:
                test eax, eax
                jz 700f                // If zero, PKRU cleared correctly.
                ud2                    // If not zero, crash with an illegal insn

              700: // _pkru_cleared
                // Now, load the runtime pointer again and restore the Rust stack,
                // leaving the return values rax and rdx (currently both pushed to
                // foreign stack) intact.
                mov r10, qword ptr [rip - {rust_thread_state_static} + {rts_runtime_offset}]

                // TODO: check whether the foreign stack is actually large enough to
                // hold these 16 bytes:
                pop rdx
                pop rax

                // Restore the Rust stack pointer:
                mov rsp, qword ptr [r10 + {rtas_rust_stack_ptr_offset}]

                // Encode the return value. We recover the InvokeRes pointer
                // from our scratch space and write rax, rdx, and whether an error
                // occurred.
                mov r12, qword ptr [r10 + {rtas_invoke_res_ptr_offset}]
                mov qword ptr [r12 + {ivr_error_offset}], {ive_no_error_const}
                mov qword ptr [r12 + {ivr_rax_offset}], rax     // rax return value
                mov qword ptr [r12 + {ivr_rdx_offset}], rdx     // rdx return value

                // Restore all other saved registers:
                pop r15
                pop r14
                pop r13
                pop r12
                pop rbp
                pop rbx

                // Return to the calling function.
                ret
            ",
            stack_spill = const 48,
            // Rust thread-local state and offsets:
            rust_thread_state_static = sym RUST_THREAD_STATE,
            rts_runtime_offset = const std::mem::offset_of!(RustThreadState, runtime),
            rts_pkru_shadow_offset = const std::mem::offset_of!(RustThreadState, pkru_shadow),
            // Runtime ASM state offsets:
            rtas_rust_stack_ptr_offset = const std::mem::offset_of!(EncapfnMPKRtAsmState, rust_stack_ptr),
            rtas_foreign_stack_ptr_offset = const std::mem::offset_of!(EncapfnMPKRtAsmState, foreign_stack_ptr),
            rtas_foreign_stack_bottom_offset = const std::mem::offset_of!(EncapfnMPKRtAsmState, foreign_stack_bottom),
            rtas_foreign_code_pkru_offset = const std::mem::offset_of!(EncapfnMPKRtAsmState, foreign_code_pkru),
            rtas_invoke_res_ptr_offset = const std::mem::offset_of!(EncapfnMPKRtAsmState, invoke_res_ptr),
            // InvokeResInner offsets:
            ivr_error_offset = const std::mem::offset_of!(EncapfnMPKInvokeResInner, error),
            ivr_rax_offset = const std::mem::offset_of!(EncapfnMPKInvokeResInner, rax),
            ivr_rdx_offset = const std::mem::offset_of!(EncapfnMPKInvokeResInner, rdx),
            // InvokeResError constants:
            ive_no_error_const = const EncapfnMPKInvokeErr::NoError as usize,
        );
    }
}

#[derive(Clone, Debug)]
pub struct EncapfnMPKRtAllocTracker {
    allocations: LinkedList<(*mut (), usize)>,
}

unsafe impl AllocTracker for EncapfnMPKRtAllocTracker {
    fn is_valid(&self, ptr: *const (), len: usize) -> bool {
        // TODO: check all other mutable/immutable pages as well!
        self.is_valid_mut(ptr as *mut (), len)
    }

    fn is_valid_mut(&self, ptr: *mut (), len: usize) -> bool {
        self.allocations
            .iter()
            .find(|(aptr, alen)| {
                (ptr as usize) >= (*aptr as usize)
                    && ((ptr as usize)
                        .checked_add(len)
                        .map(|end| end <= (*aptr as usize) + alen)
                        .unwrap_or(false))
            })
            .is_some()
    }
}

pub struct EncapfnMPKSymbolTable<const SYMTAB_SIZE: usize> {
    symbols: [*const (); SYMTAB_SIZE],
}

unsafe impl<ID: EFID> EncapfnRt for EncapfnMPKRt<ID> {
    type ID = ID;
    type AllocTracker<'a> = EncapfnMPKRtAllocTracker;
    type ABI = SysVAMD64ABI;
    type SymbolTableState<const SYMTAB_SIZE: usize, const FIXED_OFFSET_SYMTAB_SIZE: usize> =
        EncapfnMPKSymbolTable<SYMTAB_SIZE>;

    fn resolve_symbols<const SYMTAB_SIZE: usize, const FIXED_OFFSET_SYMTAB_SIZE: usize>(
        &self,
        symbol_table: &'static [&'static CStr; SYMTAB_SIZE],
        _fixed_offset_symbol_table: &'static [Option<&'static CStr>; FIXED_OFFSET_SYMTAB_SIZE],
    ) -> Option<Self::SymbolTableState<SYMTAB_SIZE, FIXED_OFFSET_SYMTAB_SIZE>> {
        // TODO: this might use an excessive amount of stack space:
        let mut err: bool = false;

        let symbols = symbol_table.clone().map(|symbol_name| {
            if err {
                // If we error on one symbol, don't need to loop up others.
                std::ptr::null()
            } else {
                // Try all libraries in order:
                for lib_handle in self.lib_handles.iter() {
                    let res =
                        unsafe { libc_bindings::dlfcn::dlsym(*lib_handle, symbol_name.as_ptr()) };

                    if res == std::ptr::null_mut() {
                        // Try the next library:
                        continue;
                    } else {
                        // Success!
                        return res as *const _;
                    }
                }

                // Did not find a library that exposes this symbol:
                err = true;
                std::ptr::null_mut()
            }
        });

        if err {
            None
        } else {
            Some(EncapfnMPKSymbolTable { symbols })
        }
    }

    fn lookup_symbol<const SYMTAB_SIZE: usize, const FIXED_OFFSET_SYMTAB_SIZE: usize>(
        &self,
        index: usize,
        symtabstate: &Self::SymbolTableState<SYMTAB_SIZE, FIXED_OFFSET_SYMTAB_SIZE>,
    ) -> Option<*const ()> {
        symtabstate.symbols.get(index).copied()
    }

    // We provide only the required implementations and rely on default
    // implementations for all "convenience" allocation methods. These are as
    // efficient as it gets in our case anyways.
    #[cfg(feature = "mpkrt_foreign_stack_alloc")]
    fn allocate_stacked_untracked_mut<F, R>(
        &self,
        requested_layout: core::alloc::Layout,
        fun: F,
    ) -> Result<R, EFError>
    where
        F: FnOnce(*mut ()) -> R,
    {
        if requested_layout.size() == 0 {
            return Err(EFError::AllocInvalidLayout);
        }

        let mut fsp = self.asm_state.foreign_stack_ptr.get() as usize;
        let original_fsp = fsp;

        // Move the stack pointer downward by the requested size. We always use
        // saturating_sub() to avoid underflows:
        fsp = fsp.saturating_sub(requested_layout.size());

        // Now, adjust the foreign stack pointer downward to the required
        // alignment. The saturating_sub should be optimized away here:
        fsp = fsp.saturating_sub(original_fsp % requested_layout.align());

        // Check that we did not produce a stack overflow. If that happened, we
        // must return before saving this stack pointer, or writing to the
        // pointer.
        if fsp < self.asm_state.foreign_stack_bottom as usize {
            return Err(EFError::AllocNoMem);
        }

        // Save the new stack pointer:
        self.asm_state.foreign_stack_ptr.set(fsp as *mut ());

        // Call the closure with our pointer:
        let res = fun(fsp as *mut ());

        // Finally, restore the previous stack pointer:
        self.asm_state
            .foreign_stack_ptr
            .set(original_fsp as *mut ());

        // Fin:
        Ok(res)
    }

    #[cfg(feature = "mpkrt_heap_alloc_mprotect")]
    fn allocate_stacked_untracked_mut<F, R>(
        &self,
        requested_layout: core::alloc::Layout,
        fun: F,
    ) -> Result<R, EFError>
    where
        F: FnOnce(*mut ()) -> R,
    {
        if requested_layout.size() == 0 {
            return Err(EFError::AllocInvalidLayout);
        }

        // We round up the layout to have page-alignment and at least page
        // size, such that we can use pkey_mprotect without fear of also
        // changing the pkeys to adjacent Rust memory. We need to be sure to
        // remove those protections afterwards:
        let page_layout = requested_layout.align_to(PAGE_SIZE).unwrap().pad_to_align();

        // We're not actually allocating on the stack here, but still providing
        // similar semantics by freeing allocations once we pop the current
        // stack frame:
        let ptr = unsafe { std::alloc::alloc(page_layout) };

        // Assign these pages the appropriate pkey:
        assert!(
            0 == unsafe {
                libc_bindings::sys_mman::pkey_mprotect(
                    ptr as *mut std::ffi::c_void,
                    page_layout.size(),
                    (libc_bindings::sys_mman::PROT_READ | libc_bindings::sys_mman::PROT_WRITE)
                        as std::ffi::c_int,
                    self.pkey_library,
                )
            },
            "Failed to pkey_mprotect memory allocated using allocate_stacked_mut, layout {:?}",
            page_layout,
        );

        // Execute the function:
        let ret = fun(ptr);

        // Revert the pages back to the Rust pkey, or the default pkey:
        assert!(
            0 == unsafe {
                libc_bindings::sys_mman::pkey_mprotect(
                    ptr as *mut std::ffi::c_void,
                    page_layout.size(),
                    (libc_bindings::sys_mman::PROT_READ | libc_bindings::sys_mman::PROT_WRITE)
                        as std::ffi::c_int,
                    self.pkey_rust.unwrap_or(0),
                )
            },
            "Failed to pkey_mprotect memory allocated using allocate_stacked_mut, layout {:?}",
            page_layout,
        );

        // We free the pointer again. There should not be any valid Rust
        // references to this memory in scope any longer, as they must have been
        // bound to the AllocScope with the anonymous lifetime as passed
        // (reborrowed) into the closure:
        unsafe {
            std::alloc::dealloc(ptr, page_layout);
        }

        Ok(ret)
    }

    fn allocate_stacked_mut<'a, F, R>(
        &self,
        layout: core::alloc::Layout,
        alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, ID>,
        fun: F,
    ) -> Result<R, EFError>
    where
        F: for<'b> FnOnce(*mut (), &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>) -> R,
    {
        self.allocate_stacked_untracked_mut(layout, move |ptr| {
            // Add allocation to the tracker, to allow creation of references
            // pointing into this memory:
            alloc_scope
                .tracker_mut()
                .allocations
                // Use the requested layout here, we don't give access to padding
                // that may be added by `alloc_stacked_untracked`.
                .push_back((ptr, layout.size()));

            let ret = fun(ptr, alloc_scope);

            // Remove this allocation from the tracker. Allocations are made by the
            // global heap allocator, which will never alias allocations, so we can
            // uniquely identify ours by its pointer:
            alloc_scope
                .tracker_mut()
                .allocations
                .retain(|(alloc_ptr, _)| *alloc_ptr != ptr);

            ret
        })
    }
}

#[repr(usize)]
enum EncapfnMPKInvokeErr {
    NoError,
    NotCalled,
}

// Depending on the size of the return value, it will be either passed
// as a pointer on the stack as the first argument, or be written to
// %rax and %rdx. In either case, this InvokeRes type is passed by
// reference (potentially on the stack), such that we can even encode
// values that exceed the available two return registers. If a return
// value was passed by invisible reference, we will be passed a
// pointer to that:
#[repr(C)]
pub struct EncapfnMPKInvokeResInner {
    error: EncapfnMPKInvokeErr,
    rax: usize,
    rdx: usize,
}

#[repr(C)]
pub struct EncapfnMPKInvokeRes<RT: SysVAMD64BaseRt, T> {
    inner: EncapfnMPKInvokeResInner,
    _t: PhantomData<T>,
    _rt: PhantomData<RT>,
}

impl<RT: SysVAMD64BaseRt, T> EncapfnMPKInvokeRes<RT, T> {
    fn encode_eferror(&self) -> Result<(), EFError> {
        match self.inner.error {
            EncapfnMPKInvokeErr::NotCalled => panic!(
                "Attempted to use / query {} without it being used by an invoke call!",
                std::any::type_name::<Self>()
            ),

            EncapfnMPKInvokeErr::NoError => Ok(()),
        }
    }
}

unsafe impl<RT: SysVAMD64BaseRt, T> SysVAMD64InvokeRes<RT, T> for EncapfnMPKInvokeRes<RT, T> {
    fn new() -> Self {
        // Required invariant by our assembly:
        let _: () = assert!(std::mem::offset_of!(Self, inner) == 0);

        EncapfnMPKInvokeRes {
            inner: EncapfnMPKInvokeResInner {
                error: EncapfnMPKInvokeErr::NotCalled,
                rax: 0,
                rdx: 0,
            },
            _t: PhantomData,
            _rt: PhantomData,
        }
    }

    fn into_result_registers(self, _rt: &RT) -> EFResult<T> {
        self.encode_eferror()?;

        // Basic assumptions in this method:
        // - sizeof(usize) == sizeof(u64)
        // - little endian
        assert!(std::mem::size_of::<usize>() == std::mem::size_of::<u64>());
        assert!(cfg!(target_endian = "little"));

        // This function must not be called on types larger than two
        // pointers (128 bit), as those cannot possibly be encoded in the
        // two available 64-bit return registers:
        assert!(std::mem::size_of::<T>() <= 2 * std::mem::size_of::<*const ()>());

        // Allocate space to construct the final (unvalidated) T from
        // the register values. During copy, we treat the memory of T
        // as integers:
        let mut ret_uninit: MaybeUninit<T> = MaybeUninit::uninit();

        // TODO: currently, we only support power-of-two return values.
        // It is not immediately obvious how values that are, e.g.,
        // 9 byte in size would be encoded into registers.
        let rax_bytes = u64::to_le_bytes(self.inner.rax as u64);
        let rdx_bytes = u64::to_le_bytes(self.inner.rdx as u64);
        let ret_bytes = [
            rax_bytes[0],
            rax_bytes[1],
            rax_bytes[2],
            rax_bytes[3],
            rax_bytes[4],
            rax_bytes[5],
            rax_bytes[6],
            rax_bytes[7],
            rdx_bytes[0],
            rdx_bytes[1],
            rdx_bytes[2],
            rdx_bytes[3],
            rdx_bytes[4],
            rdx_bytes[5],
            rdx_bytes[6],
            rdx_bytes[7],
        ];

        MaybeUninit::copy_from_slice(
            ret_uninit.as_bytes_mut(),
            &ret_bytes[..std::mem::size_of::<T>()],
        );

        EFResult::Ok(ret_uninit.into())
    }

    unsafe fn into_result_stacked(self, _rt: &RT, stacked_res: *mut T) -> EFResult<T> {
        self.encode_eferror()?;

        // Allocate space to construct the final (unvalidated) T from
        // the register values. During copy, we treat the memory of T
        // as integers:
        let mut ret_uninit: MaybeUninit<T> = MaybeUninit::uninit();

        // Now, we simply to a memcpy from our pointer. We trust the caller
        // that is allocated, non-aliased over any Rust struct, not being
        // mutated and accessible to us. We cast it into a layout-compatible
        // MaybeUninit pointer:
        unsafe {
            std::ptr::copy_nonoverlapping(stacked_res as *const T, ret_uninit.as_mut_ptr(), 1)
        };

        EFResult::Ok(ret_uninit.into())
    }
}

macro_rules! invoke_impl_rtloc_register {
    ($regtype:ident, $rtloc:expr, $fnptrloc:expr, $resptrloc:expr) => {
        impl<const STACK_SPILL: usize, ID: EFID>
            SysVAMD64Rt<STACK_SPILL, $regtype<SysVAMD64ABI>>
            for EncapfnMPKRt<ID>
        {
            #[naked]
            unsafe extern "C" fn invoke() {
                core::arch::naked_asm!(
                    concat!("
                    // This pushes the stack down by {pushed} bytes. We rely on this
                    // offset below. ALWAYS UPDATE THEM IN TANDEM.
                    push rbx
                    push rbp
                    push r12
                    push r13
                    push r14
                    push r15
                    // BEFORE CHANGING THE ABOVE, DID YOU READ THE COMMENT?

                    // Load required parameters in non-argument registers and
                    // continue execution in the generic protection-domain
                    // switch routine:
                    mov r10, ", $rtloc, "          // Load runtime pointer into r10
                    mov r11, ", $fnptrloc, "       // Load function pointer into r11
                    mov r12, ", $resptrloc, "      // Load the InvokeRes pointer into r12
                    mov r13, ${stack_spill}        // Copy the stack-spill immediate into r12
                    lea r14, [rip - {invoke_fn}]
                    jmp r14
                    "),
                    stack_spill = const STACK_SPILL,
                    invoke_fn = sym Self::generic_invoke,
                    // How many bytes we pushed onto the stack above:
                    pushed = const 48,
               );
            }
        }
    };
}

invoke_impl_rtloc_register!(AREG0, "rdi", "rsi", "rdx");
invoke_impl_rtloc_register!(AREG1, "rsi", "rdx", "rcx");
invoke_impl_rtloc_register!(AREG2, "rdx", "rcx", "r8");
invoke_impl_rtloc_register!(AREG3, "rcx", "r8", "r9");
invoke_impl_rtloc_register!(AREG4, "r8", "r9", "[rsp + {pushed} + 8]");
invoke_impl_rtloc_register!(AREG5, "r9", "[rsp + {pushed} + 8]", "[rsp + {pushed} + 16]");

impl<const STACK_SPILL: usize, const RT_STACK_OFFSET: usize, ID: EFID>
    SysVAMD64Rt<STACK_SPILL, Stacked<RT_STACK_OFFSET, SysVAMD64ABI>> for EncapfnMPKRt<ID>
{
    #[naked]
    unsafe extern "C" fn invoke() {
        core::arch::naked_asm!(
            "
            // This pushes the stack down by {pushed} bytes. We rely on this
            // offset below. ALWAYS UPDATE THEM IN TANDEM.
            push rbx
            push rbp
            push r12
            push r13
            push r14
            push r15
            // BEFORE CHANGING THE ABOVE, DID YOU READ THE COMMENT?

            // Load required parameters in non-argument registers and
            // continue execution in the generic protection-domain
            // switch routine:
            mov r10, [rsp + {pushed} + {rt_stack_offset} + 8]  // Load runtime pointer into r10 from stack offset + 8
            mov r11, [rsp + {pushed} + {rt_stack_offset} + 16] // Load function pointer into r11 from stack offset + 16
            mov r12, [rsp + {pushed} + {rt_stack_offset} + 24] // Load the InvokeRes pointer into r12 from stack offset + 24
            mov r13, ${stack_spill}                            // Copy the stack-spill immediate into r13
            lea r14, [rip - {invoke_fn}]
            jmp r14
            ",
            stack_spill = const STACK_SPILL,
            rt_stack_offset = const RT_STACK_OFFSET,
            invoke_fn = sym Self::generic_invoke,
            // How many bytes we pushed onto the stack above. This value is also used in
            // generic_invoke. When updating this value, ALSO UPDATE IT IN GENERIC INVOKE.
            pushed = const 48,
        );
    }
}

impl<ID: EFID> SysVAMD64BaseRt for EncapfnMPKRt<ID> {
    type InvokeRes<T> = EncapfnMPKInvokeRes<Self, T>;
}