use proc_macro::TokenStream;
use quote::quote;

// #[proc_macro]
// pub fn characterize(input: TokenStream) -> TokenStream {
//     let input = parse_macro_input!(input as Ident);
//     let char_lit = input.to_string().chars().next().unwrap();
//     let expanded = quote! { #char_lit };
//     TokenStream::from(expanded)
// }

#[proc_macro]
pub fn characterize(input: TokenStream) -> TokenStream {
    let mut iter = input.into_iter();
    match iter.next() {
        None => {
            let ch = 'a';
            let expanded = quote! { #ch };
            TokenStream::from(expanded)
        }
        Some(next) => {
            let ch = next.to_string().chars().next().unwrap_or('a');
            let expanded = quote! { #ch };
            TokenStream::from(expanded)
        }
    }
}
