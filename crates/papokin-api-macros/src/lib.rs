#![allow(clippy::unwrap_used, clippy::expect_used)]

use proc_macro::TokenStream;
use proc_macro_error2::{abort, proc_macro_error};
use proc_macro2::Ident;
use quote::quote;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;
use syn::{ImplItem, ItemFn, ItemImpl, ItemStruct, parse_macro_input, parse_quote};

/// 存储所有标注了 `#[plugin_method]` 的函数，供后续在 impl 中使用。
static PLUGIN_METHODS: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 将函数标记为插件方法，将其包装为返回 `PluginFuture` 的形式。
///
/// 函数体将通过全局运行时异步执行。
#[proc_macro_error]
#[proc_macro_attribute]
pub fn plugin_method(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_inputs = &input_fn.sig.inputs;
    let fn_output = &input_fn.sig.output;
    let fn_body = &input_fn.block;

    let output_type = match fn_output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };

    let method = quote! {
        #[expect(unused_mut)]
        fn #fn_name(#fn_inputs) -> PluginFuture<'_, #output_type> {
            crate::GLOBAL_RUNTIME.block_on(async move {
                Box::pin(async move {
                    #fn_body
                })
            })
        }
    }
    .to_string();

    PLUGIN_METHODS
        .lock()
        .unwrap()
        .insert(fn_name.to_string(), method);

    TokenStream::new()
}

/// 将结构体包装为插件实现。
///
/// 这会为该结构体生成 `Plugin` impl，包括所有带有 `#[plugin_method]` 标注的函数，
/// 并提供 `plugin()` 函数，将其以装箱 trait 对象的形式返回。
#[proc_macro_error]
#[proc_macro_attribute]
pub fn plugin_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // 解析输入结构体
    let input_struct = parse_macro_input!(item as ItemStruct);
    let struct_ident = &input_struct.ident;

    let methods: Vec<proc_macro2::TokenStream> = {
        let guard = PLUGIN_METHODS.lock().unwrap();
        guard
            .values()
            .map(|s| {
                s.parse::<proc_macro2::TokenStream>().unwrap_or_else(|e| {
                    abort!(
                        struct_ident,
                        format!("re-parsing cached method failed: {e}")
                    )
                })
            })
            .collect()
    };

    // 将原始结构体定义与 impl 块及 plugin() 函数合并
    let expanded = quote! {
        use papokin::plugin::PluginFuture;

        pub static GLOBAL_RUNTIME: std::sync::LazyLock<std::sync::Arc<tokio::runtime::Runtime>> =
            std::sync::LazyLock::new(|| std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap()));

        #[unsafe(no_mangle)]
        pub static METADATA: std::sync::LazyLock<papokin::plugin::PluginMetadata> = std::sync::LazyLock::new(|| {
            papokin::plugin::PluginMetadata {
                name: env!("CARGO_PKG_NAME").to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                authors: env!("CARGO_PKG_AUTHORS").split(',').map(String::from).collect(),
                description: env!("CARGO_PKG_DESCRIPTION").to_string(),
                dependencies: Vec::new(),
                permissions: Vec::new(),
                load_after: Vec::new(),
                load_before: Vec::new(),
                provides: Vec::new(),
                load_order: papokin::plugin::LoadOrder::PostWorld,
            }
        });

        #[unsafe(no_mangle)]
        pub static PUMPKIN_API_VERSION: u32 = papokin::plugin::PLUGIN_API_VERSION;

        #input_struct

        impl papokin::plugin::Plugin for #struct_ident {
            #(#methods)*
        }

        #[unsafe(no_mangle)]
        pub fn plugin() -> Box<dyn papokin::plugin::Plugin> {
            Box::new(#struct_ident::new())
        }
    };

    TokenStream::from(expanded)
}

/// 包装 `impl` 块中的所有函数，使其在运行时中执行。
#[proc_macro_error]
#[proc_macro_attribute]
pub fn with_runtime(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemImpl);

    let mode: Ident = parse_macro_input!(attr as Ident);
    let use_global = match mode.to_string().as_str() {
        "global" => true,
        "local" => false,
        other => abort!(mode, format!("expected `global` or `local`, got `{other}`")),
    };

    for item in &mut input.items {
        if let ImplItem::Fn(method) = item {
            let original_body = &method.block;

            method.block = if use_global {
                parse_quote!({
                    crate::GLOBAL_RUNTIME.block_on(async move {
                        #original_body
                    })
                })
            } else {
                parse_quote!({
                    tokio::runtime::Runtime::new()
                        .unwrap()
                        .block_on(async move {
                            #original_body
                        })
                })
            };
        }
    }

    TokenStream::from(quote!(#input))
}
