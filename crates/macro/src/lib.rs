extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Lit};

#[proc_macro_attribute]
pub fn flow_node(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let struct_name = &input.ident;
    // let meta_node_name_string = format!("__{}_meta_node", struct_name).to_uppercase();
    // let meta_node_name = syn::Ident::new(&meta_node_name_string, struct_name.span());

    // parse node_type
    let lit = parse_macro_input!(attr as Lit);
    let node_type = match lit {
        Lit::Str(lit_str) => lit_str.value(),
        _ => panic!("Expected a string literal for node_type"),
    };

    let expanded = quote! {
        #input

        impl FlowsElement for #struct_name {
            fn id(&self) -> ElementId {
                self.get_node().id
            }

            fn name(&self) -> &str {
                &self.get_node().name
            }

            fn type_str(&self) -> &'static str {
                self.get_node().type_str
            }

            fn ordering(&self) -> usize {
                self.get_node().ordering
            }

            fn parent_element(&self) -> Option<std::sync::Arc<dyn FlowsElement>> {
                self.get_node().flow.upgrade().and_then(|arc| Some(arc as std::sync::Arc<dyn FlowsElement>))
            }

            fn context(&self) -> Arc<Context> {
                self.get_node().context.clone()
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
        }

        ::inventory::submit! {
            MetaNode {
                kind: NodeKind::Flow,
                type_: #node_type,
                factory: NodeFactory::Flow(#struct_name::build),
            }
        }
    }; // quote!

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn global_node(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let struct_name = &input.ident;
    // let meta_node_name_string = format!("__{}_meta_node", struct_name).to_uppercase();
    // let meta_node_name = syn::Ident::new(&meta_node_name_string, struct_name.span());

    // parse node_type
    let lit = parse_macro_input!(attr as Lit);
    let node_type = match lit {
        Lit::Str(lit_str) => lit_str.value(),
        _ => panic!("Expected a string literal for node_type"),
    };

    let expanded = quote! {
        #input

        impl FlowsElement for #struct_name {
            fn id(&self) -> ElementId {
                self.get_node().id
            }

            fn name(&self) -> &str {
                &self.get_node().name
            }

            fn type_str(&self) -> &'static str {
                self.get_node().type_str
            }

            fn ordering(&self) -> usize {
                self.get_node().ordering
            }

            fn parent_element(&self) -> Option<std::sync::Arc<dyn FlowsElement>> {
                // TODO change it to engine
                log::warn!("Cannot get the parent element in global node");
                None
            }

            fn context(&self) -> Arc<Context> {
                self.get_node().context.clone()
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
        }

        ::inventory::submit! {
            MetaNode {
                kind: NodeKind::Global,
                type_: #node_type,
                factory: NodeFactory::Global(#struct_name::build),
            }
        }

    }; // quote!
    TokenStream::from(expanded)
}
