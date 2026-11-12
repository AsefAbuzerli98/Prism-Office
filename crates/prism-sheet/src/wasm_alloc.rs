//! Global allocator for the WASM build: dlmalloc on top of a system layer
//! that grows linear memory in large steps. With the default 64 KB steps,
//! every `memory.grow` makes V8 detach and re-wrap the memory buffer, which
//! cost tens of seconds of GC when loading million-cell workbooks.

use core::alloc::{GlobalAlloc, Layout};
use core::arch::wasm32;
use core::cell::UnsafeCell;
use core::ptr;

const PAGE: usize = 64 * 1024;
const MIN_GROW: usize = 32 * 1024 * 1024;

struct Grow;

unsafe impl dlmalloc::Allocator for Grow {
    fn alloc(&self, size: usize) -> (*mut u8, usize, u32) {
        let bytes = size.max(MIN_GROW).div_ceil(PAGE) * PAGE;
        let prev = wasm32::memory_grow(0, bytes / PAGE);
        if prev == usize::MAX {
            // fall back to the exact request when a big step does not fit
            let exact = size.div_ceil(PAGE) * PAGE;
            let prev = wasm32::memory_grow(0, exact / PAGE);
            if prev == usize::MAX {
                return (ptr::null_mut(), 0, 0);
            }
            return ((prev * PAGE) as *mut u8, exact, 0);
        }
        ((prev * PAGE) as *mut u8, bytes, 0)
    }
    fn remap(&self, _ptr: *mut u8, _old: usize, _new: usize, _can_move: bool) -> *mut u8 {
        ptr::null_mut()
    }
    fn free_part(&self, _ptr: *mut u8, _old: usize, _new: usize) -> bool {
        false
    }
    fn free(&self, _ptr: *mut u8, _size: usize) -> bool {
        false
    }
    fn can_release_part(&self, _flags: u32) -> bool {
        false
    }
    fn allocates_zeros(&self) -> bool {
        true
    }
    fn page_size(&self) -> usize {
        PAGE
    }
}

struct Global(UnsafeCell<dlmalloc::Dlmalloc<Grow>>);

// wasm32-unknown-unknown without atomics is single-threaded.
unsafe impl Sync for Global {}

#[global_allocator]
static ALLOC: Global = Global(UnsafeCell::new(dlmalloc::Dlmalloc::new_with_allocator(
    Grow,
)));

unsafe impl GlobalAlloc for Global {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        (*self.0.get()).malloc(layout.size(), layout.align())
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        (*self.0.get()).calloc(layout.size(), layout.align())
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        (*self.0.get()).free(p, layout.size(), layout.align())
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        (*self.0.get()).realloc(p, layout.size(), layout.align(), new_size)
    }
}
