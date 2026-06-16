#[cfg(feature = "tailwindcss")]
mod tailwindcss;

#[proc_macro]
#[inline]
#[cfg(feature = "tailwindcss")]
pub fn tailwindcss(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    crate::tailwindcss::proc_macro(item)
}
