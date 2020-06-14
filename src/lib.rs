#![allow(unused_imports)]

use ::std::{
    env,
    fs,
    io::{self, Write},
    iter,
    process::{Command, Stdio},
    ops::Not as _,
};

extern crate proc_macro;
use ::proc_macro::{
    TokenStream,
};
use ::proc_macro2::{
    Span,
    TokenStream as TokenStream2,
    TokenTree as TT,
};
use ::quote::{
    quote,
    quote_spanned,
    ToTokens,
};
use ::syn::{*,
    parse::{Parse, Parser, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
};
use ::tempdir::{
    TempDir,
};

type Result<Ok, Err = ::syn::Error> = ::core::result::Result<Ok, Err>;

/// Invoke rustc to build a `wasm32-unknown-unknown` crate with dependencies on
/// `unicode_xid`, `proc_macro2`, `syn`, and `quote`.
fn build_code (source: &'_ str)
  -> io::Result<Vec<u8>>
{
    // Build within a tempdir
    let tmp = TempDir::new("ctrs_build")?;

    macro_rules! CRATE_NAME {() => ("ctrs")}
    let wasm_path = tmp.path().join(concat!(CRATE_NAME!(), ".wasm"));

    let mut cmd = Command::new(env!("RUSTC"));
    cmd.args(&[
        "-", // input source code is piped
        "-o", wasm_path.to_str().unwrap(),
        "--target", "wasm32-unknown-unknown",
        "--edition", "2018",
        "--crate-type", "cdylib",
        "--crate-name", CRATE_NAME!(),
        "-L", &format!("dependency={}", tmp.path().to_str().unwrap()),
    ]);
    macro_rules! rlibs {
        () => (rlibs! {
            proc_macro2, quote, syn, unicode_xid,
        });

        (
            $($lib:ident),* $(,)?
        ) => ({
            struct Paths {
                $(
                    $lib: String,
                )*
            }
            let paths = Paths {
                $(
                    $lib:
                        tmp .path()
                            .join(concat!("lib", stringify!($lib), ".rlib"))
                            .to_string_lossy()
                            .into_owned()
                    ,
                )*
            };
            $(
                fs::write(&paths.$lib, &include_bytes! {
                    concat!(
                        env!("OUT_DIR"), "/wasm32-unknown-unknown/release/",
                        "lib", stringify!($lib), ".rlib",
                    )
                }[..])?;
                cmd.arg("--extern");
                cmd.arg(&format!(concat!(stringify!($lib), "={}"), paths.$lib));
            )*
        });
    }
    rlibs!();

    // Spawn the compiler
    let mut child = cmd.stdin(Stdio::piped()).spawn()?;
    // Pipe the source code in (scoped binding to ensure pipe is closed).
    match child.stdin.take().unwrap() { mut stdin => {
        stdin.write_all(source.as_bytes())?;
        stdin.write_all(stringify!(
            extern crate proc_macro2 as proc_macro;

            #[macro_export]
            macro_rules! parse_macro_input {
                (
                    $expr:tt as $T:ty
                ) => (
                    match ::syn::parse2::<$T>($expr) {
                        | Ok(it) => it,
                        | Err(err) => return err.to_compile_error().into(),
                    }
                );

                (
                    $expr:expr
                ) => (
                    parse_macro_input!($expr as _)
                );
            }
        ).as_bytes())?;
    }}
    // Wait for the compiler to succeed.
    let status = child.wait()?;
    if status.success() {
        // Read in the resulting wasm file
        Ok(fs::read(&wasm_path)?)
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("rustc exited with status {}", status),
        ))
    }
}

fn log_stream (ts: &TokenStream)
{
    let in_str = ts.to_string();
    if in_str.len() > 1000 {
        let pre = in_str.chars().take(400).collect::<String>();
        let post = in_str.chars().rev().take(400).collect::<String>().chars().rev().collect::<String>();
        println!("{} [.. {} chars ..] {}", pre, in_str.len() - 800, post)
    } else {
        println!("{}", in_str);
    }
}

#[proc_macro]
#[doc(hidden)] /** Not part of the public API **/ pub
fn __eval_wasm__ (input: TokenStream)
  -> TokenStream
{
    let debug = env::var("DEBUG_INLINE_MACROS").ok().map_or(false, |s| s == "1");
    if debug {
        println!("<<<__eval_wasm__! {{");
        log_stream(&input);
        println!("}}\n>>>");
    }
    let mut tokens = TokenStream2::into_iter(input.into());
    let func =
        tokens
            .next()
            .expect("Missing procmacro name")
            .to_string()
    ;
    let wasm_lit =
        tokens
            .next()
            .expect("Missing WASM-compiled procmacro source code")
            .to_string()
    ;
    assert!(wasm_lit.starts_with('"') && wasm_lit.ends_with('"'));
    let wasm =
        ::base64::decode(&wasm_lit[1 .. wasm_lit.len() - 1])
            .unwrap()
    ;
    ::watt::proc_macro(&func, TokenStream2::into(tokens.collect()), &wasm)
}

#[proc_macro] pub
fn compile (input: TokenStream)
  -> TokenStream
{
    let debug = env::var("DEBUG_INLINE_MACROS").ok().map_or(false, |s| s == "1");
    if debug {
        println!("<<<\ncompile! {{");
        log_stream(&input);
        println!("}}\n>>>");
    }
    let ret = match extract_macro_names(input.into()) {
        | Err(err) => return err.to_compile_error().into(),
        | Ok((items, macro_names)) => {
            let ref src = quote!( #(#items)* ).to_string();
            let compiled_wasm =
                build_code(src)
                    .expect("error building crate")
            ;
            let b64_compiled_wasm = ::base64::encode(&compiled_wasm).to_string();
            build_result(parse_quote!(#b64_compiled_wasm), macro_names)

        },
    };
    if debug { log_stream(&ret); }
    ret
}

fn build_result (
    wasm_code: LitStr,
    macros: Vec<Ident>,
) -> TokenStream
{
    let mut ret = TokenStream2::new();
    macros.into_iter().for_each(|macro_name| {
        ret.extend(quote_spanned! { macro_name.span()=>
            macro_rules! #macro_name {(
                $($proc_macro_input:tt)*
            ) => (
                ::inline_proc_macros::__eval_wasm__! {
                    #macro_name
                    #wasm_code
                    $($proc_macro_input)*
                }
            )}
        });
    });
    ret.into()
}

fn extract_macro_names (ts: TokenStream2)
  -> Result<(Vec<Item>, Vec<Ident>)>
{Ok({
    let parse_with = |ts, parser| Parser::parse2(parser, ts);
    let mut items: Vec<Item> = parse_with(ts, |parse_stream: ParseStream<'_> | {
        let mut ret = vec![];
        while parse_stream.is_empty().not() {
            ret.push(parse_stream.parse()?);
        }
        Ok(ret)
    })?;
    let mut macro_names = Vec::with_capacity(items.len());
    items.iter_mut().try_for_each(|item| Ok(match *item {
        | Item::Fn(ref mut func) => {
            let ref f_name = func.sig.ident;
            if matches!(func.vis, Visibility::Public(_)).not() {
                return Err(Error::new(
                    f_name.span(),
                    "`#[proc_macro]` function must be `pub`",
                ));
            }
            if let Some(ref abi) = func.sig.abi {
                return Err(Error::new(
                    abi.span(),
                    "`#[proc_macro]` function cannot have an `extern` annotation",
                ));
            }
            // Check for the `proc_macro` attribute, and remove it.
            let mut found = false;
            func.attrs.retain(|attr| {
                let is_proc_macro = attr.path.is_ident("proc_macro");
                found |= is_proc_macro;
                is_proc_macro.not()
            });
            if found {
                macro_names.push(f_name.to_owned());
                // Transform the method into a wasm export.
                func.attrs.push(parse_quote!(#[no_mangle]));
                func.vis = parse_quote!(pub);
                func.sig.abi.replace(parse_quote!(extern "C"));
                func.block.stmts.insert(0, parse_quote! {
                    ::proc_macro2::set_wasm_panic_hook();
                });
            }
        },
        | _ => {},
    }))?;
    (items, macro_names)
})}
