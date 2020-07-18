> Procedural macros without an extra crate.

# `::inline_proc_macros`

Hack to implement inline procedural macros, forked off
[https://github.com/mystor/ctrs](https://github.com/mystor/ctrs)
(credits for the idea and the first draft implementation go to them, _c.f._,
the LICENSE).

  - Note: this only works for function-like procedural macros, not for derive
    or attribute macros. That being said, the [`#[macro_rules_attribute]`](
    https://docs.rs/macro_rules_attribute) crate lets you solve that part.

## Requirements

  - The `wasm32-unknown-unknown` target must be installed (or `rustup` must be
    available, so as to automatically install it when missing).

  - **Minimum Supported Rust Version**: `1.42.0`

      - This crate follows the MSRV policy, _i.e._, that breaking MSRV will be
        considered a breaking change.

  - **Recommended Rust Version**: `>= 1.45.0`

      - Otherwise you will not be able to use procedural macros in expression
        position.

        <details><summary>Workaround</summary>

        Instead of having the macro expand to `some expression ...`, have it
        expand to:

        ```rust
        macro_rules! expansion { () => (
            some expression ...
        )}
        ```

        And change its name to something like `__some_macro_name__`.

        Then, wrap the generated macro `__some_macro_name__!`, as follows:

        ```rust
        #[inline_proc_macros::macro_use]
        mod some_module_name {}

        macro_rules! some_macro_name {
            (@as_item $it:item) => ($it);
            (
                $($input:tt)*
            ) => ({
                some_macro_name! {
                    @as_item
                    __some_macro_name__! { $($input)* }
                }
                expansion!()
            });
        }
        ```

        </details>

## Examples

  - See the [`downstream/` directory](https://github.com/danielhenrymantilla/rust-inline_proc_macros/tree/proc-macro-approach/downstream).

  - Or see the documentation of [`#[inline_proc_macros::macro_use]`][1].

## Notes

  - This is currently at a slightly experimental stage; moreover, it relies
    on procedural macros being able to tinker with the filesystem like
    `build.rs` scripts do, which is not a feature Rust guarantees (more info
    about this in the remarks for the documentation of the
    [`#[inline_proc_macros::macro_use]`][1] attribute).
    
    It is thus ill-advised to use this crate in an environment where the
    compilation needs to be robust and long-term future-proof.

  - That being said, this crate can be great to quickly prototype 
    procedural macros, so as to better evaluate how useful they can be for your
    crate, before fully committing to [the cumbersome but more reliable
    double-crate pattern](
    https://users.rust-lang.org/t/proc-macros-using-third-party-crate/42465/4).


## Debugging

An optional compilation feature, "trace-macros" can be used to print at
compile-time the exact macro call:

```toml
[dependencies]
inline_proc_macros = { ..., features = ["trace-macros"] }
```

[1]: https://docs.rs/inline_proc_macros/0.0.1/inline_proc_macros/attr.macro_use.html
