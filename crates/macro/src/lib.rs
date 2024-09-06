extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Lit};

#[proc_macro_attribute]
pub fn flow_node(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let struct_name = &input.ident;
    let meta_node_name_string = format!("__{}_meta_node", struct_name).to_uppercase();
    let meta_node_name = syn::Ident::new(&meta_node_name_string, struct_name.span());

    // parse node_type
    let lit = parse_macro_input!(attr as Lit);
    let node_type = match lit {
        Lit::Str(lit_str) => lit_str.value(),
        _ => panic!("Expected a string literal for node_type"),
    };

    let expanded = quote! {
        #input

        #[linkme::distributed_slice(__META_NODES)]
        static #meta_node_name: MetaNode = MetaNode {
            kind: NodeKind::Flow,
            type_: #node_type,
            factory: NodeFactory::Flow(#struct_name::build),
        };
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn global_node(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let struct_name = &input.ident;
    let meta_node_name_string = format!("__{}_meta_node", struct_name).to_uppercase();
    let meta_node_name = syn::Ident::new(&meta_node_name_string, struct_name.span());

    // parse node_type
    let lit = parse_macro_input!(attr as Lit);
    let node_type = match lit {
        Lit::Str(lit_str) => lit_str.value(),
        _ => panic!("Expected a string literal for node_type"),
    };

    let expanded = quote! {
        #input

        #[linkme::distributed_slice(__META_NODES)]
        static #meta_node_name: MetaNode = MetaNode {
            kind: NodeKind::Global,
            type_: #node_type,
            factory: NodeFactory::Global(#struct_name::build),
        };
    };

    TokenStream::from(expanded)
}
