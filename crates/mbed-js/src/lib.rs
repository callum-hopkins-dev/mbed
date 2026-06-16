#[cfg(feature = "bundle")]
mod bundle;

#[proc_macro]
#[inline]
#[cfg(feature = "bundle")]
pub fn bundle(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    crate::bundle::proc_macro(item)
}
