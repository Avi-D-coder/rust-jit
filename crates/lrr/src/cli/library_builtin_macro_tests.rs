//! Library macros the pinned adapter or the ordinary library source expands
//! (`thread_local!`, `offset_of!`, `cfg_select!`, `type_ascribe!`, `pin!`),
//! run through the production CLI path in every official edition. They are
//! LRR-specific controls: the unchanged upstream programs
//! (`threads-sendsync/thread-local-syntax.rs`,
//! `thread-local/thread-local-with-attributes-30756.rs`,
//! `hygiene/thread-local-not-in-prelude.rs`, `offset-of/offset-of-tuple-nested.rs`,
//! `offset-of/offset-of-slice.rs`) are the primary witnesses, and these are
//! not a stock-suite count.
use super::*;

struct OwnedSource(std::path::PathBuf);
impl Drop for OwnedSource {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn source(text: &str) -> OwnedSource {
    use std::io::Write;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "lrr-library-builtin-{}-{}.rs",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.write_all(text.as_bytes()).unwrap();
    OwnedSource(path)
}
/// Every official edition runs the same program to completion.
fn completed(text: &str) {
    for edition in [2015, 2018, 2021, 2024] {
        let owned = source(text);
        match standalone_edition(&owned.0, None, edition) {
            Ok(StandaloneOutcome::Completed(_)) => {}
            Ok(StandaloneOutcome::Panicked(report)) => {
                panic!("edition {edition} panicked: {:?}\n{text}", report.panic)
            }
            Ok(_) => panic!("edition {edition} produced another outcome\n{text}"),
            Err(error) => panic!("edition {edition} faulted: {error:?}\n{text}"),
        }
    }
}


#[test]
fn thread_local_const_and_lazy_initializers_reach_every_key_method() {
    completed(
        "use std::cell::{Cell, RefCell};\n\
         thread_local!{ static C: Cell<u32> = const { Cell::new(7) }; static D: Cell<u32> = Cell::new(8); }\n\
         thread_local!{ static Y: RefCell<Vec<u8>> = RefCell::new(vec![1, 2]); }\n\
         fn main() {\n\
             C.with(|c| assert_eq!(c.get(), 7));\n\
             assert_eq!(D.get(), 8);\n\
             C.set(1); D.set(2);\n\
             assert_eq!(C.replace(3), 1);\n\
             assert_eq!(D.take(), 2);\n\
             assert_eq!(D.get(), 0);\n\
             let r = C.try_with(|c| c.get());\n\
             assert_eq!(r.unwrap(), 3u32);\n\
             Y.with_borrow_mut(|v| v.push(3));\n\
             assert_eq!(Y.with_borrow(|v| v.len()), 3);\n\
             assert_eq!(Y.with(|c| c.borrow().len()), 3);\n\
         }\n",
    );
}

#[test]
fn offset_of_names_the_layout_of_structs_unions_and_nested_paths() {
    completed(
        "use std::mem::offset_of;\n\
         #[repr(C, packed)] struct Pk { a: u8, b: u32 }\n\
         union U { a: u8, b: u64 }\n\
         #[repr(C)] struct In { x: u16, y: u64 }\n\
         #[repr(C)] struct S { a: u8, b: u32, inner: In }\n\
         const OB: usize = offset_of!(S, b);\n\
         const OY: usize = offset_of!(S, inner.y);\n\
         static OP: usize = offset_of!(Pk, b);\n\
         fn main() {\n\
             assert_eq!((OB, OY, OP), (4, 16, 1));\n\
             assert_eq!(offset_of!(U, b), 0);\n\
             let local = offset_of!(S, inner);\n\
             assert_eq!(local, 8);\n\
             let arr = [0u8; offset_of!(S, inner)];\n\
             assert_eq!(arr.len(), 8);\n\
         }\n",
    );
}

#[test]
fn cfg_select_selects_in_item_expression_statement_and_generated_item_position() {
    completed(
        "cfg_select! { windows => { fn which() -> u32 { 1 } } unix => { fn which() -> u32 { 2 } } _ => { fn which() -> u32 { 3 } } }\n\
         macro_rules! pick { ($e:expr, $n:ident) => { cfg_select! { $e => { fn $n() -> u32 { 1 } } _ => { fn $n() -> u32 { 2 } } } } }\n\
         pick!(true, yes);\n\
         pick!(false, no);\n\
         fn main() {\n\
             assert_eq!(which(), 2);\n\
             assert_eq!((yes(), no()), (1, 2));\n\
             let v = cfg_select! { target_pointer_width = \"64\" => { 64 } _ => { 32 } };\n\
             assert_eq!(v, 64);\n\
             let w = cfg_select! { windows => 1, not(unix) => 2, _ => 3 };\n\
             assert_eq!(w, 3);\n\
             cfg_select! { true => { let a = 1; assert_eq!(a, 1); } _ => {} }\n\
             cfg_select! { false => { unreachable!(); } _ => { assert!(true); } }\n\
             ();\n\
         }\n",
    );
}

#[test]
fn type_ascribe_pins_the_type_of_a_value_expression() {
    completed(
        "const C1: u8 = type_ascribe!(10, u8);\n\
         fn main() {\n\
             assert_eq!(C1, 10);\n\
             let x = [1, 2, 3];\n\
             let y: &[i32] = type_ascribe!(&x, &[i32; 3]);\n\
             assert_eq!(y.len(), 3);\n\
             let a = type_ascribe!(1, u8);\n\
             assert_eq!(std::mem::size_of_val(&a), 1);\n\
             let b = type_ascribe!(a.into(), u16);\n\
             assert_eq!(b, type_ascribe!(1, u16));\n\
         }\n",
    );
}
