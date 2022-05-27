use std::path::Path;

use proc_macro::TokenStream;
use quote::quote;

#[proc_macro]
pub fn msg_include_all(_input: TokenStream) -> TokenStream {
    let ament_prefix_paths =
        std::env::var("AMENT_PREFIX_PATH").expect("$AMENT_PREFIX_PATH is supposed to be set.");

    let paths = ament_prefix_paths
        .split(':')
        .map(Path::new)
        .collect::<Vec<_>>();

    let packages: Vec<_> = rclrust_msg_parse::get_packages(&paths)
        .unwrap()
        .iter()
        .map(|v| v.token_stream())
        .collect();

    (quote! {
        #(#packages)*
    })
    .into()
}
