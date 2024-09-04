extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Lit};

#[proc_macro_attribute]
pub fn flow_node(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let name = &input.ident;

    // parse node_type
    let lit = parse_macro_input!(attr as Lit);
    let node_type = match lit {
        Lit::Str(lit_str) => lit_str.value(),
        _ => panic!("Expected a string literal for node_type"),
    };

    let expanded = quote! {
        #input

        inventory::submit! {
            BuiltinNodeDescriptor {
                meta: MetaNode {
                    kind: NodeKind::Flow,
                    type_: #node_type,
                    factory: NodeFactory::Flow(#name::create),
                },
            }
        }
    };

    TokenStream::from(expanded)
}
