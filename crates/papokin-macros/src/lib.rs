#![allow(clippy::unwrap_used, clippy::expect_used)]

use heck::ToShoutySnakeCase;
use proc_macro::TokenStream;
use proc_macro_error2::{abort, abort_call_site};
use quote::{format_ident, quote, quote_spanned};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{self, Attribute, DeriveInput, Token, Type, parse_quote};
use syn::{Block as SynBlock, Expr, Field, Fields, ItemStruct, Stmt, parse_macro_input};

/// 为事件结构体派生 `Payload` trait，使其可在插件系统中使用。
///
/// # Arguments
/// - `item` – 表示要为其派生 `Event` 的结构体的输入 `TokenStream`。
#[proc_macro_derive(Event)]
pub fn event(item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as DeriveInput);
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    // 携带 `cancelled` 标志的结构体（例如由 `#[cancellable]` 注入）
    // 通过 `Payload::cancelled_state` 暴露其状态，让调度器
    // 无需专门化即可强制实现 `ignoreCancelled` 语义。
    let is_cancellable = match &ast.data {
        syn::Data::Struct(data) => data
            .fields
            .iter()
            .any(|f| f.ident.as_ref().is_some_and(|i| i == "cancelled")),
        _ => false,
    };
    let cancelled_state_impl = is_cancellable.then(|| {
        quote! {
            fn cancelled_state(&self) -> Option<bool> {
                Some(self.cancelled)
            }
        }
    });

    quote! {
        impl #impl_generics crate::plugin::Payload for #name #ty_generics #where_clause {
            fn get_name_static() -> &'static str {
                // 完全限定路径：两个共享裸结构体名称的事件
                // 不得在处理函数映射中重名，否则
                // `EventRegistry` 中按名称防护的向下转型将会
                // 不健全（unsound）。
                std::any::type_name::<Self>()
            }

            fn get_name(&self) -> &'static str {
                std::any::type_name::<Self>()
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            #cancelled_state_impl
        }
    }
    .into()
}

/// 通过添加 `cancelled: bool` 字段将结构体标记为可取消，并生成相应实现
/// 实现 `Cancellable` trait。
///
/// # Arguments
/// - `_args` – 传递给该属性的参数 `TokenStream`（未使用）。
/// - `input` – 表示要修改的结构体的输入 `TokenStream`。
#[proc_macro_attribute]
pub fn cancellable(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut item_struct = parse_macro_input!(input as ItemStruct);
    let name = &item_struct.ident;
    let (impl_generics, ty_generics, where_clause) = item_struct.generics.split_for_impl();

    match &mut item_struct.fields {
        Fields::Named(fields) => {
            if fields
                .named
                .iter()
                .any(|f| f.ident.as_ref().is_some_and(|i| i == "cancelled"))
            {
                abort!(fields.span(), "Struct already has a `cancelled` field");
            }

            let field: Field = parse_quote! {
                pub cancelled: bool
            };
            fields.named.push(field);
        }
        _ => abort!(
            item_struct.span(),
            "#[cancellable] can only be used on structs with named fields"
        ),
    }

    quote! {
        #item_struct

        impl #impl_generics crate::plugin::Cancellable for #name #ty_generics #where_clause {
            fn cancelled(&self) -> bool {
                self.cancelled
            }

            fn set_cancelled(&mut self, cancelled: bool) {
                self.cancelled = cancelled;
            }
        }
    }
    .into()
}

/// 通过插件管理器发送一个可取消的事件。
///
/// # Syntax
/// ```ignore
/// send_cancellable! {{
///     <server_expr>;
///     <event_expr>;
///     'after: { <after_stmts> }
///     'cancelled: { <cancelled_stmts> }
/// }}
/// ```
#[proc_macro]
pub fn send_cancellable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as SynBlock);

    let mut stmts_iter = input.stmts.into_iter();

    let Some(Stmt::Expr(server_stmt, _)) = stmts_iter.next() else {
        abort_call_site!("expected server expression as first statement")
    };

    let Some(Stmt::Expr(event_stmt, _)) = stmts_iter.next() else {
        abort_call_site!("expected event expression as second statement")
    };

    let event_expr = if let Expr::Reference(syn::ExprReference {
        expr,
        mutability: Some(_),
        ..
    }) = event_stmt
    {
        *expr
    } else {
        event_stmt
    };

    let mut after_block = None;
    let mut cancelled_block = None;

    for stmt in stmts_iter {
        if let Stmt::Expr(Expr::Block(b), _) = stmt
            && let Some(ref label) = b.label
        {
            if label.name.ident == "after" {
                after_block = Some(b.block);
            } else if label.name.ident == "cancelled" {
                cancelled_block = Some(b.block);
            }
        }
    }

    let execution = match (after_block, cancelled_block) {
        (Some(after), Some(cancelled)) => quote! {
            if !is_cancelled {
                #after
            } else {
                #cancelled
            }
        },
        (Some(after), None) => quote! {
            if !is_cancelled {
                #after
            }
        },
        (None, Some(cancelled)) => quote! {
            if is_cancelled {
                #cancelled
            }
        },
        (None, None) => quote! {},
    };

    let expanded = quote! {
        {
            let mut event = #event_expr;
            let server_ref: &std::sync::Arc<crate::server::Server> = {
                use std::borrow::Borrow;
                (#server_stmt).borrow()
            };
            server_ref.plugin_manager.fire(server_ref, &mut event).await;

            let is_cancelled = {
                use crate::plugin::Cancellable;
                event.cancelled()
            };

            #execution
        }
    };

    expanded.into()
}

#[proc_macro]
pub fn send_cancellable_blocking(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as SynBlock);

    let mut stmts_iter = input.stmts.into_iter();

    let Some(Stmt::Expr(server_stmt, _)) = stmts_iter.next() else {
        abort_call_site!("expected server expression as first statement")
    };

    let Some(Stmt::Expr(event_stmt, _)) = stmts_iter.next() else {
        abort_call_site!("expected event expression as second statement")
    };

    let event_expr = if let Expr::Reference(syn::ExprReference {
        expr,
        mutability: Some(_),
        ..
    }) = event_stmt
    {
        *expr
    } else {
        event_stmt
    };

    let mut after_block = None;
    let mut cancelled_block = None;

    for stmt in stmts_iter {
        if let Stmt::Expr(Expr::Block(b), _) = stmt
            && let Some(ref label) = b.label
        {
            if label.name.ident == "after" {
                after_block = Some(b.block);
            } else if label.name.ident == "cancelled" {
                cancelled_block = Some(b.block);
            }
        }
    }

    let execution = match (after_block, cancelled_block) {
        (Some(after), Some(cancelled)) => quote! {
            if !is_cancelled {
                #after
            } else {
                #cancelled
            }
        },
        (Some(after), None) => quote! {
            if !is_cancelled {
                #after
            }
        },
        (None, Some(cancelled)) => quote! {
            if is_cancelled {
                #cancelled
            }
        },
        (None, None) => quote! {},
    };

    let expanded = quote! {
        {
            let mut event = #event_expr;
            let server_ref: &std::sync::Arc<crate::server::Server> = {
                use std::borrow::Borrow;
                (#server_stmt).borrow()
            };
            server_ref.plugin_manager.fire_blocking(server_ref, &mut event);

            let is_cancelled = {
                use crate::plugin::Cancellable;
                event.cancelled()
            };

            #execution
        }
    };

    expanded.into()
}

/// 为实现 `Packet` 的结构体附加固定的数据包 ID。
///
/// # Arguments
/// - `args` – 表示数据包 ID 表达式的 `TokenStream`。
/// - `item` – 表示要为其实现 `Packet` 的结构体的输入 `TokenStream`。
#[proc_macro_attribute]
pub fn packet(args: TokenStream, item: TokenStream) -> TokenStream {
    let packet_id_expr = parse_macro_input!(args as Expr);
    let ast = parse_macro_input!(item as DeriveInput);

    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    quote! {
        #ast
        impl #impl_generics crate::packet::Packet for #name #ty_generics #where_clause {
            const PACKET_ID: i32 = #packet_id_expr;
        }
    }
    .into()
}

/// 为实现 `MultiVersionJavaPacket` 的结构体附加多版本数据包 ID。
///
/// # Arguments
/// - `args` – 表示数据包 ID 表达式的 `TokenStream`。
/// - `item` – 表示要为其实现该 trait 的结构体的输入 `TokenStream`。
#[proc_macro_attribute]
pub fn java_packet(args: TokenStream, item: TokenStream) -> TokenStream {
    let packet_id_expr = parse_macro_input!(args as Expr);
    let ast = parse_macro_input!(item as DeriveInput);

    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    quote! {
        #ast
        impl #impl_generics crate::packet::MultiVersionJavaPacket for #name #ty_generics #where_clause {
            #[must_use]
            #[inline]
            fn to_id(version: papokin_util::version::JavaMinecraftVersion) -> i32 {
                #packet_id_expr.to_id(version)
            }
        }
    }
    .into()
}

fn block_expr_to_id(expr: &Expr) -> proc_macro2::TokenStream {
    match expr {
        Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(lit_str),
            ..
        }) => {
            let val = lit_str.value();
            let name = val.strip_prefix("minecraft:").unwrap_or(&val);
            let const_ident = format_ident!("{}", name.to_shouty_snake_case());
            quote_spanned! { lit_str.span() => papokin_data::BlockId::#const_ident }
        }
        Expr::Path(syn::ExprPath { path, .. }) => {
            if path.segments.len() == 1 {
                let ident = &path.segments[0].ident;
                quote_spanned! { expr.span() => papokin_data::BlockId::#ident }
            } else if path.segments.len() == 2
                && (path.segments[0].ident == "Block" || path.segments[0].ident == "BlockId")
            {
                let ident = &path.segments[1].ident;
                quote_spanned! { expr.span() => papokin_data::BlockId::#ident }
            } else {
                quote_spanned! { expr.span() => papokin_data::BlockId::from(#expr) }
            }
        }
        _ => {
            quote_spanned! { expr.span() => papokin_data::BlockId::from(#expr) }
        }
    }
}

/// 将结构体标记为表示按名称、表达式或常量给定的特定方块（或多个方块）。
///
/// # Arguments
/// - `args` – 一个或多个方块名（例如 `"stone"`、`"minecraft:stone"`）、常量（例如 `Block::STONE`、`BlockId::STONE`、`STONE`）或表达式。
/// - `item` – 表示要为其实现 `BlockMetadata` 的结构体的输入 `TokenStream`。
#[proc_macro_attribute]
pub fn pumpkin_block(args: TokenStream, item: TokenStream) -> TokenStream {
    let input_item = item.clone();

    let args = parse_macro_input!(args with Punctuated<Expr, Token![,]>::parse_terminated);
    if args.is_empty() {
        abort_call_site!("expected at least one block argument");
    }

    let id_tokens: Vec<_> = args.iter().map(block_expr_to_id).collect();

    let ast = parse_macro_input!(item as DeriveInput);
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let generated = quote! {
        impl #impl_generics crate::block::BlockMetadata for #name #ty_generics #where_clause {
            fn ids() -> Box<[papokin_data::BlockId]> {
                Box::new([ #(#id_tokens),* ])
            }
        }
    };

    // 合并原始条目和新的 impl。
    let mut output = input_item;
    output.extend(TokenStream::from(generated));
    output
}

/// 将结构体标记为表示来自给定标签的一组方块。
///
/// # Arguments
/// - `args` – 表示方块标签字面量或表达式的 `TokenStream`。
/// - `item` – 表示要为其实现 `BlockMetadata` 的结构体的输入 `TokenStream`。
#[proc_macro_attribute]
pub fn pumpkin_block_from_tag(args: TokenStream, item: TokenStream) -> TokenStream {
    let original_item = item.clone();

    let arg_expr = parse_macro_input!(args as Expr);
    let ast = parse_macro_input!(item as DeriveInput);

    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics crate::block::BlockMetadata for #name #ty_generics #where_clause {
            fn ids() -> Box<[papokin_data::BlockId]> {
                papokin_data::tag::get_tag_ids(papokin_data::tag::RegistryKey::Block, #arg_expr)
                    .unwrap_or_else(|| panic!("获取标签 ID 失败：{}", #arg_expr))
                    .iter()
                    .copied()
                    .map(papokin_data::BlockId::new_or_air)
                    .collect()
            }
        }
    };

    let mut output = original_item;
    output.extend(TokenStream::from(expanded));
    output
}

// #[proc_macro_error]
// #[proc_macro_attribute]
// pub fn block_property(input: TokenStream, item: TokenStream) -> TokenStream {
//     let ast: syn::DeriveInput = syn::parse(item.clone()).unwrap();
//     let name = &ast.ident;
//     let (impl_generics, ty_generics, _) = ast.generics.split_for_impl();

//     let input_string = input.to_string();
//     let input_parts: Vec<&str> = input_string.split("[").collect();
//     let property_name = input_parts[0].trim_ascii().trim_matches(&['"', ','][..]);
//     let mut property_values: Vec<&str> = Vec::new();
//     if input_parts.len() > 1 {
//         property_values = input_parts[1]
//             .trim_matches(']')
//             .split(", ")
//             .map(|p| p.trim_ascii().trim_matches(&['"', ','][..]))
//             .collect::<Vec<&str>>();
//     }

//     let item: proc_macro2::TokenStream = item.into();

//     let (variants, is_enum): (Vec<proc_macro2::Ident>, bool) = match ast.data {
//         syn::Data::Enum(enum_item) => (
//             enum_item.variants.into_iter().map(|v| v.ident).collect(),
//             true,
//         ),
//         syn::Data::Struct(s) => {
//             let fields = match s.fields {
//                 Fields::Named(f) => abort!(f.span(), "Block 属性不能有命名字段"),
//                 Fields::Unnamed(fields) => fields.unnamed,
//                 Fields::Unit => abort!(s.fields.span(), "Block 属性必须有字段"),
//             };
//             if fields.len() != 1 {
//                 abort!(
//                     fields.span(),
//                     "Block 属性的结构体必须恰好有一个字段"
//                 );
//             }
//             let field = fields.first().unwrap();
//             let ty = &field.ty;
//             let struct_type = match field.ty {
//                 syn::Type::Path(ref type_path) => {
//                     type_path.path.segments.first().unwrap().ident.to_string()
//                 }
//                 ref other => abort!(
//                     other.span(),
//                     "Block 属性只能使用原始类型"
//                 ),
//             };
//             match struct_type.as_str() {
//                 "bool" => (
//                     vec![
//                         proc_macro2::Ident::new("true", proc_macro2::Span::call_site()),
//                         proc_macro2::Ident::new("false", proc_macro2::Span::call_site()),
//                     ],
//                     false,
//                 ),
//                 other => abort!(
//                     ty.span(),
//                     format!("`{other}` 不受支持（何不自己实现它？）")
//                 ),
//             }
//         }
//         _ => abort_call_site!("Block properties can only be `enum`s or `struct's"),
//     };

//     let values = variants.iter().enumerate().map(|(i, v)| match is_enum {
//         true => {
//             let mut value = v.to_string().to_snake_case();
//             if !property_values.is_empty() && i < property_values.len() {
//                 value = property_values[i].to_string();
//             }
//             quote! {
//                 Self::#v => #value.to_string(),
//             }
//         }
//         false => {
//             let value = v.to_string();
//             quote! {
//                 Self(#v) => #value.to_string(),
//             }
//         }
//     });

//     let from_values = variants.iter().enumerate().map(|(i, v)| match is_enum {
//         true => {
//             let mut value = v.to_string().to_snake_case();
//             if !property_values.is_empty() && i < property_values.len() {
//                 value = property_values[i].to_string();
//             }
//             quote! {
//                 #value => Self::#v,
//             }
//         }
//         false => {
//             let value = v.to_string();
//             quote! {
//                 #value => Self(#v),
//             }
//         }
//     });

//     let extra_fns = variants.iter().map(|v| {
//         let title = proc_macro2::Ident::new(
//             &v.to_string().to_pascal_case(),
//             proc_macro2::Span::call_site(),
//         );
//         quote! {
//             pub fn #title() -> Self {
//                 Self(#v)
//             }
//         }
//     });

//     let extra = if is_enum {
//         quote! {}
//     } else {
//         quote! {
//             impl #name {
//                 #(#extra_fns)*
//             }
//         }
//     };

//     let code = quote! {
//         #item
//         impl #impl_generics papokin_world::block::properties::BlockPropertyMetadata for #name #ty_generics {
//             fn name(&self) -> &'static str {
//                 #property_name
//             }
//             fn value(&self) -> String {
//                 match self {
//                     #(#values)*
//                 }
//             }
//             fn from_value(value: String) -> Self {
//                 match value.as_str() {
//                     #(#from_values)*
//                     _ => panic!("Block 属性的值无效"),
//                 }
//             }
//         }
//         #extra
//     };

//     code.into()
// }

/// 为结构体派生 `PacketWrite` trait，使其支持序列化。
///
/// # Arguments
/// - `input` – 表示要为其派生 `PacketWrite` 的结构体的输入 `TokenStream`。
#[rustfmt::skip]
#[proc_macro_derive(PacketWrite, attributes(serial))]
pub fn derive_serialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = if let syn::Data::Struct(data) = &input.data {
        data.fields.iter().map(|f| {
            let ident = f.ident.as_ref().unwrap();
            let (is_big_endian, no_prefix) = check_serial_attributes(&f.attrs);
            let is_vec = is_vec(&f.ty);

            if is_vec && !no_prefix {
                // 带前缀的 Vec：先写 VarUInt 长度，再写数据
                if is_big_endian {
                    quote! {
                        crate::codec::var_uint::VarUInt(self.#ident.len() as u32).write(writer)?;
                        self.#ident.write_be(writer)?;
                    }
                } else {
                    quote! {
                        crate::codec::var_uint::VarUInt(self.#ident.len() as u32).write(writer)?;
                        self.#ident.write(writer)?;
                    }
                }
            } else {
                // 非 Vec 或带 no_prefix 的 Vec：直接写入
                if is_big_endian {
                    quote! {
                        self.#ident.write_be(writer)?;
                    }
                } else {
                    quote! {
                        self.#ident.write(writer)?;
                    }
                }
            }
        })
    } else {
        return syn::Error::new(name.span(), "Only structs are supported")
            .to_compile_error()
            .into();
    };

    let type_generic = match input.generics.params.len() {
        0 => quote! {},
        1 => quote! { <'_> },
        _ => {
            return syn::Error::new(name.span(), "Only up to one lifetime parameter is supported.")
            .to_compile_error()
            .into();
        }
    };

    let expanded = quote! {
        impl PacketWrite for #name #type_generic {
            fn write<W: std::io::Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
                #(#fields)*
                Ok(())
            }
        }
    };

    expanded.into()
}

/// 为结构体派生 `PacketRead` trait，使其支持反序列化。
///
/// # Arguments
/// - `input` – 表示要为其派生 `PacketRead` 的结构体的输入 `TokenStream`。
#[rustfmt::skip]
#[proc_macro_derive(PacketRead, attributes(serial))]
pub fn derive_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = if let syn::Data::Struct(data) = &input.data {
        data.fields.iter().map(|f| {
            let ident = f.ident.as_ref().unwrap();
            let (is_big_endian, no_prefix) = check_serial_attributes(&f.attrs);
            let is_vec = is_vec(&f.ty);

            if is_vec && no_prefix {
                return syn::Error::new(name.span(), "Cannot handle non-prefixed vecs")
                    .to_compile_error();
            }

            // 非 Vec 或不带 no_prefix 的 Vec：直接读取
            if is_big_endian {
                quote! {
                    #ident: PacketRead::read_be(reader)?
                }
            } else {
                quote! {
                    #ident: PacketRead::read(reader)?
                }
            }
        })
    } else {
        return syn::Error::new(name.span(), "Only structs are supported")
            .to_compile_error()
            .into();
    };


    let type_generic = match input.generics.params.len() {
        0 => quote! {},
        1 => quote! { <'static> },
        _ => {
            return syn::Error::new(name.span(), "Only up to one lifetime parameter is supported.")
            .to_compile_error()
            .into();
        }
    };

    let expanded = quote! {
        impl PacketRead for #name #type_generic {
            fn read<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
                Ok(Self {
                    #(#fields),*
                })
            }
        }
    };

    expanded.into()
}

/// 为结构体派生 `PacketReadSlice` trait，使其能从切片反序列化。
///
/// # Arguments
/// - `input` – 表示要为其派生 `PacketReadSlice` 的结构体的输入 `TokenStream`。
#[rustfmt::skip]
#[proc_macro_derive(PacketReadSlice, attributes(serial))]
pub fn derive_deserialize_from_slice(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = if let syn::Data::Struct(data) = &input.data {
        data.fields.iter().map(|f| {
            let ident = f.ident.as_ref().unwrap();
            let (is_big_endian, no_prefix) = check_serial_attributes(&f.attrs);
            let is_vec = is_vec(&f.ty);

            if is_vec && no_prefix {
                return syn::Error::new(name.span(), "Cannot handle non-prefixed vecs.")
                    .to_compile_error();
            }

            // 非 Vec 或不带 no_prefix 的 Vec：直接读取
            if is_big_endian {
                return syn::Error::new(name.span(), "Cannot handle big-endian encoded fields")
                    .to_compile_error();
            }

            quote! {
                #ident: PacketReadSlice::read_slice(buf)?
            }
        })
    } else {
        return syn::Error::new(name.span(), "Only structs are supported")
            .to_compile_error()
            .into();
    };

    let expanded = quote! {
        impl<'a> PacketReadSlice<'a> for #name<'a> {
            fn read_slice(buf: &mut &'a [u8]) -> std::io::Result<Self> {
                Ok(Self {
                    #(#fields),*
                })
            }
        }
    };

    expanded.into()
}

/// 检查字段的 `#[serial(...)]` 属性。
///
/// # Arguments
/// - `attrs` – 要检查序列化相关元数据的 `Attribute` 切片。
///
/// # Returns
/// 元组 `(is_big_endian, no_prefix)`，指示该字段是否为大端序
/// 和/或没有长度前缀。
fn check_serial_attributes(attrs: &[Attribute]) -> (bool, bool) {
    let mut is_big_endian = false;
    let mut no_prefix = false;

    for attr in attrs {
        if attr.path().is_ident("serial") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("big_endian") {
                    is_big_endian = true;
                } else if meta.path.is_ident("no_prefix") {
                    no_prefix = true;
                }
                Ok(())
            });
        }
    }

    (is_big_endian, no_prefix)
}

/// 若该类型是 `Vec<_>`，则返回 true。
///
/// # Arguments
/// - `ty` – 要检查的 `Type`。
///
/// # Returns
/// 如果该类型是 `Vec` 则为 `true`，否则为 `false`。
fn is_vec(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        type_path
            .path
            .segments
            .iter()
            .next_back()
            .is_some_and(|segment| segment.ident == "Vec")
    } else {
        false
    }
}

struct TranslateInput {
    java_expr: syn::Expr,
    args: Vec<syn::Expr>,
}

impl syn::parse::Parse for TranslateInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let java_expr: syn::Expr = input.parse()?;

        let mut args = Vec::new();
        while !input.is_empty() {
            let _ = input.parse::<syn::Token![,]>()?;
            if input.is_empty() {
                break;
            }
            args.push(input.parse()?);
        }

        Ok(Self { java_expr, args })
    }
}

fn eval_translation_key_expr(expr: &syn::Expr) -> Option<(&'static str, proc_macro2::Span)> {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(lit_str),
            ..
        }) => {
            // 此处泄漏字符串切片没有问题，因为这运行在编译期的过程宏内部
            let s = Box::leak(lit_str.value().into_boxed_str());
            Some((s, lit_str.span()))
        }
        _ => None,
    }
}

const fn count_placeholders(format_str: &str) -> usize {
    let mut count = 0;
    let bytes = format_str.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'%' {
                i += 2;
                continue;
            }
            // 处理 %1$s、%2$d 等位置说明符
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'$' {
                count += 1;
                i = j + 2; // 跳过 $ 之后的说明符字符（例如 $s）
                continue;
            }
            // 处理 %s、%d、%f、%1s 等说明符类型之前的可选宽度/标志
            count += 1;
            i = j + 1; // 跳过说明符字符
            continue;
        }
        i += 1;
    }
    count
}

/// 在编译期验证翻译键与参数，并返回 `TextComponent`。
#[proc_macro]
pub fn translate(input: TokenStream) -> TokenStream {
    let TranslateInput { java_expr, args } = parse_macro_input!(input as TranslateInput);

    if let Some((java_str, span)) = eval_translation_key_expr(&java_expr) {
        let expected = count_placeholders(java_str);
        if expected != args.len() && matches!(java_expr, syn::Expr::Lit(_)) {
            return syn::Error::new(
                span,
                format!(
                    "Translation key `{}` expects {} argument(s), but {} were provided",
                    java_str,
                    expected,
                    args.len()
                ),
            )
            .to_compile_error()
            .into();
        }
    }

    let expanded = quote! {
        {
            papokin_util::text::TextComponent::translate(#java_expr, vec![#(#args),*])
        }
    };

    expanded.into()
}
