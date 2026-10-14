extern crate proc_macro;

#[proc_macro_attribute]
pub fn echo(_: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    item
}
#[proc_macro]
pub fn identity(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    input
}
