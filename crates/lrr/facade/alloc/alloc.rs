// `alloc::alloc`: memory layouts and the allocator interfaces. The guest
// store owns every allocation (a leaked zero-filled byte vector), so
// deallocation is the end of the run rather than an operation, and fresh
// memory is always zeroed (which every `alloc_zeroed` contract permits).
use crate::boxed::Box;
use crate::vec::Vec;
use core::ptr::NonNull;

pub struct Layout {
    size: usize,
    align: usize,
}
pub struct LayoutError;
pub struct AllocError;
pub struct Global;
pub struct System;

impl Layout {
    pub const fn from_size_align(size: usize, align: usize) -> Result<Layout, LayoutError> {
        if align == 0 || (align & (align - 1)) != 0 {
            return Result::Err(LayoutError);
        }
        // `size`, rounded up to `align`, must not overflow `isize`.
        let max: usize = (isize::MAX as usize) - (align - 1);
        if size > max {
            return Result::Err(LayoutError);
        }
        Result::Ok(Layout { size: size, align: align })
    }
    pub const unsafe fn from_size_align_unchecked(size: usize, align: usize) -> Layout {
        Layout { size: size, align: align }
    }
    pub const fn size(&self) -> usize {
        self.size
    }
    pub const fn align(&self) -> usize {
        self.align
    }
    pub const fn new<T>() -> Layout {
        Layout {
            size: core::mem::size_of::<T>(),
            align: core::mem::align_of::<T>(),
        }
    }
    pub fn for_value<T>(t: &T) -> Layout {
        Layout {
            size: core::mem::size_of_val(t),
            align: core::mem::align_of_val(t),
        }
    }
    pub fn array<T>(n: usize) -> Result<Layout, LayoutError> {
        let element: usize = core::mem::size_of::<T>();
        if element != 0 && n > (isize::MAX as usize) / element {
            return Result::Err(LayoutError);
        }
        Layout::from_size_align(element * n, core::mem::align_of::<T>())
    }
    pub const fn pad_to_align(&self) -> Layout {
        let rounded: usize = (self.size + self.align - 1) & !(self.align - 1);
        Layout { size: rounded, align: self.align }
    }
}
impl Clone for Layout {
    fn clone(&self) -> Layout {
        Layout { size: self.size, align: self.align }
    }
}
impl Copy for Layout {}
impl PartialEq for Layout {
    fn eq(&self, other: &Layout) -> bool {
        self.size == other.size && self.align == other.align
    }
}
impl Eq for Layout {}
impl core::fmt::Debug for Layout {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Layout")
            .field("size", &self.size)
            .field("align", &self.align)
            .finish()
    }
}

impl Clone for LayoutError {
    fn clone(&self) -> LayoutError {
        LayoutError
    }
}
impl Copy for LayoutError {}
impl PartialEq for LayoutError {
    fn eq(&self, other: &LayoutError) -> bool {
        true
    }
}
impl Eq for LayoutError {}
impl core::fmt::Debug for LayoutError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("LayoutError")
    }
}
impl core::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("invalid parameters to Layout::from_size_align")
    }
}

impl Clone for AllocError {
    fn clone(&self) -> AllocError {
        AllocError
    }
}
impl Copy for AllocError {}
impl PartialEq for AllocError {
    fn eq(&self, other: &AllocError) -> bool {
        true
    }
}
impl Eq for AllocError {}
impl core::fmt::Debug for AllocError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("AllocError")
    }
}
impl core::fmt::Display for AllocError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("memory allocation failed")
    }
}

// A fresh zero-filled, never-reclaimed block of `size` bytes.
fn lrr_block(size: usize) -> &'static mut [u8] {
    let mut bytes: Vec<u8> = Vec::new();
    let mut i: usize = 0;
    while i < size {
        bytes.push(0u8);
        i += 1;
    }
    let kept = core::intrinsics::lrr_shared_to_mut(core::intrinsics::lrr_box_leak(Box::new(bytes)));
    kept.as_mut_slice()
}

pub unsafe trait Allocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError>;
    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.allocate(layout)
    }
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout);
}

pub unsafe trait GlobalAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8;
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout);
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        self.alloc(layout)
    }
}

unsafe impl Allocator for Global {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let block = lrr_block(layout.size());
        Result::Ok(unsafe { NonNull::new_unchecked(block as *mut [u8]) })
    }
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {}
}
unsafe impl Allocator for System {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        Global.allocate(layout)
    }
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {}
}
unsafe impl GlobalAlloc for System {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {}
}
impl Clone for Global {
    fn clone(&self) -> Global {
        Global
    }
}
impl Copy for Global {}
impl Default for Global {
    fn default() -> Global {
        Global
    }
}
impl core::fmt::Debug for Global {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Global")
    }
}
impl Clone for System {
    fn clone(&self) -> System {
        System
    }
}
impl Copy for System {}
impl core::fmt::Debug for System {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("System")
    }
}

pub unsafe fn alloc(layout: Layout) -> *mut u8 {
    lrr_block(layout.size()).as_mut_ptr()
}
pub unsafe fn alloc_zeroed(layout: Layout) -> *mut u8 {
    alloc(layout)
}
pub unsafe fn dealloc(ptr: *mut u8, layout: Layout) {}
pub unsafe fn realloc(ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    let fresh: *mut u8 = alloc(Layout::from_size_align_unchecked(new_size, layout.align()));
    let keep: usize = if layout.size() < new_size { layout.size() } else { new_size };
    let mut i: usize = 0;
    while i < keep {
        core::ptr::write(fresh.add(i), core::ptr::read(ptr.add(i)));
        i += 1;
    }
    fresh
}
pub fn handle_alloc_error(layout: Layout) -> ! {
    core::panicking::panic("memory allocation failed")
}
