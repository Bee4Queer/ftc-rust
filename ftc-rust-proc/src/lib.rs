//! Rust Project

use std::{collections::HashSet, hash::Hash, path::PathBuf};

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote, quote_spanned};
use syn::{
    Error, Ident, ItemFn, LitStr, Token,
    parse::{Parse, Parser},
    parse_macro_input,
    spanned::Spanned,
};

extern crate proc_macro;

#[derive(Debug, Clone)]
enum FtcArg {
    Name(String, Span),
    Linear(Span),
    Iterative(Span),
    Teleop(Span),
    Autonomous(Span),
    Group(String, Span),
    Disabled(Span),
}

impl FtcArg {
    pub const fn get_span(&self) -> &Span {
        use FtcArg::{Autonomous, Disabled, Group, Iterative, Linear, Name, Teleop};
        match self {
            Name(_, span)
            | Linear(span)
            | Iterative(span)
            | Teleop(span)
            | Autonomous(span)
            | Group(_, span)
            | Disabled(span) => span,
        }
    }
    pub const fn get_name(&self) -> &'static str {
        use FtcArg::{Autonomous, Disabled, Group, Iterative, Linear, Name, Teleop};
        match self {
            Name(_, _) => "name",
            Linear(_) => "linear",
            Iterative(_) => "iterative",
            Teleop(_) => "teleop",
            Autonomous(_) => "auto",
            Group(_, _) => "group",
            Disabled(_) => "disabled",
        }
    }
}

impl PartialEq for FtcArg {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}
impl Eq for FtcArg {}

impl Hash for FtcArg {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
    }
}

impl Parse for FtcArg {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(Ident) {
            let name_ident: Ident = input.parse()?;
            let name = name_ident.to_string();
            Ok(match name.as_str() {
                "linear" => FtcArg::Linear(name_ident.span()),
                "iterative" => FtcArg::Iterative(name_ident.span()),
                "teleop" => FtcArg::Teleop(name_ident.span()),
                "auto" => FtcArg::Autonomous(name_ident.span()),
                "disabled" => FtcArg::Disabled(name_ident.span()),
                "name" | "group" => {
                    let lookahead = input.lookahead1();
                    if lookahead.peek(Token![=]) {
                        let _: Token![=] = input.parse()?;

                        let lookahead = input.lookahead1();
                        if lookahead.peek(LitStr) {
                            let lit: LitStr = input.parse()?;
                            if name.as_str() == "name" {
                                FtcArg::Name(lit.value(), name_ident.span())
                            } else {
                                FtcArg::Group(lit.value(), name_ident.span())
                            }
                        } else {
                            return Err(lookahead.error());
                        }
                    } else {
                        return Err(lookahead.error());
                    }
                }
                _ => {
                    return Err(Error::new(
                        name_ident.span(),
                        "ident should be one of linear, iterative, teleop, auto, disabled, name, \
                         or group",
                    ));
                }
            })
        } else {
            Err(lookahead.error())
        }
    }
}

fn snake_to_camel(s: &str) -> String {
    s.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => unreachable!(),
            }
        })
        .collect()
}

/// The primary attribute used in Rust FTC programming.
///
/// Examples:
///
/// ```no_run
/// use ftc::log::info; // the popular `log` crate is re-exported because versions
/// #[ftc(name = "Example: My Linear Op Mode", linear, teleop, group = "Example", disabled)]
/// fn my_linear_op_mode(ctx: &ftc::FtcContext) {
///     // equivalent to hardwareMap.get(DcMotor.class, "motor") in Java:
///     let motor = ctx.hardware().get::<DcMotor>("motor");
///     motor.set_direction(ftc::hardware::Direction::Forward);
///
///     ctx.telemetry().add_data("Status", "Initalized");
///     ctx.telemetry().update();
///
///     info!("Finished initalizing!");
///
///     ctx.wait_for_start();
///
///     // ctx.running() instead of opModeIsActive()    
///
///     motor.set_power(0.5);
///     ctx.sleep_s(2.0);
///     motor.set_power(0.0);
///
///     info!("Ran for {:?}!", ctx.runtime());
/// }
/// ```
///
/// ```no_run
/// #[ftc(name = "Example: My Iterative Op Mode", iterative, teleop, group = "Example", disabled)]
/// fn my_iterative_op_mode(iterative: &ftc::IterativeContext) {
///     iterative.init(|ctx: &ftc::FtcContext| {
///         // equivalent to hardwareMap.get(DcMotor.class, "motor") in Java:
///         let motor = ctx.hardware().get::<DcMotor>("motor");
///         motor.set_direction(ftc::hardware::Direction::Forward);
///
///         ctx.telemetry().add_data("Status", "Initalized");
///         ctx.telemetry().update();
///     });
///
///     iterative.start(|ctx| { // types can be elided in closures
///         let motor = ctx.hardware().get::<DcMotor>("motor");
///         motor.set_power(0.5);
///         ctx.sleep_s(2.0);
///         motor.set_power(0.0);
///     });
///
///     iterative.stop(|ctx| {
///         info!("Ran for {:?}!", ctx.runtime());
///     });
///
///     // attempting to call wait_for_start with an interative context will immediately return and print a warning
/// }
/// ```
#[proc_macro_attribute]
pub fn ftc(attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);

    let func_name = func.sig.ident.to_string();
    let class_name = snake_to_camel(&func_name);

    let args = match syn::punctuated::Punctuated::<FtcArg, Token![,]>::parse_terminated
        .parse(attr)
        .map_err(syn::Error::into_compile_error)
    {
        Ok(args) => args,
        Err(err) => return err.into(),
    }
    .into_iter()
    .collect::<Vec<_>>();

    let mut set = HashSet::new();
    for arg in &args {
        if !set.insert(arg) {
            return Error::new(
                *arg.get_span(),
                format!("cannot pass {} more than once", arg.get_name()),
            )
            .into_compile_error()
            .into();
        }
    }

    let mut name = None;
    let mut group = None;
    let mut linear = false;
    let mut iterative = false;
    let mut teleop = false;
    let mut autonomous = false;
    let mut disabled = false;

    for arg in args {
        match arg {
            FtcArg::Name(v, _) => name = Some(v),
            FtcArg::Linear(_) => linear = true,
            FtcArg::Iterative(_) => iterative = true,
            FtcArg::Teleop(_) => teleop = true,
            FtcArg::Autonomous(_) => autonomous = true,
            FtcArg::Group(v, _) => group = Some(v),
            FtcArg::Disabled(_) => disabled = true,
        }
    }

    if !(teleop || autonomous) {
        return Error::new(
            func.span(),
            "an op mode must either be teleop or autonomous, not neither",
        )
        .into_compile_error()
        .into();
    }

    if teleop && autonomous {
        return Error::new(
            func.span(),
            "an op mode must either be teleop or autonomous, not both",
        )
        .into_compile_error()
        .into();
    }

    if linear && iterative {
        return Error::new(
            func.span(),
            "an op mode must either be linear or iterative, not both",
        )
        .into_compile_error()
        .into();
    }

    let Some(name) = name else {
        return Error::new(func.span(), "an op mode must have a name")
            .into_compile_error()
            .into();
    };

    if func.sig.inputs.len() != 1 && linear {
        return Error::new(
            func.span(),
            "a linear op mode must take one argument of type &FtcContext",
        )
        .into_compile_error()
        .into();
    }

    if func.sig.inputs.len() != 1 && iterative {
        return Error::new(
            func.span(),
            "an iterative op mode must take one argument of type &IterativeContext",
        )
        .into_compile_error()
        .into();
    }

    let java_bindings_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .join("src/main/java/org/firstinspires/ftc/teamcode");

    let java = format!(
        r#"/* DO NOT EDIT THIS FILE - it is machine generated by ftc-rust v{}.
 * DO NOT touch this comment! It is used to identify classes as machine generated for various purposes.
 * DO NOT touch the code overall either! It will be overwritten if you do, and if you don't remove this comment after changing it this file will probably be removed by a build script.
 */

package org.firstinspires.ftc.teamcode;

import com.qualcomm.robotcore.eventloop.opmode.{};
import com.qualcomm.robotcore.eventloop.opmode.{};
import com.qualcomm.robotcore.eventloop.opmode.Disabled;

@{1}(name = "{}"{})
{}
public class {class_name} extends {2} {{
    private long rust_id;
    {}

    static {{
        System.loadLibrary("team_code_rust");
    }}
}}
"#,
        env!("CARGO_PKG_VERSION"),
        if teleop { "TeleOp" } else { "Autonomous" },
        if iterative { "OpMode" } else { "LinearOpMode" },
        name,
        if let Some(group) = group {
            format!(", group = \"{group}\"")
        } else {
            String::new()
        },
        if disabled { "@Disabled" } else { "" },
        if linear {
            "@Override\n    public native void runOpMode();".to_string()
        } else {
            "@Override
    public native void init();
    @Override
    public native void init_loop();
    @Override
    public native void start();
    @Override
    public native void loop();
    @Override
    public native void stop();"
                .to_string()
        }
    );

    let java_path = java_bindings_dir.join(class_name.clone() + ".java");
    if java_path.exists()
        && !std::fs::read_to_string(&java_path)
            .unwrap()
            .starts_with("/* DO NOT EDIT THIS FILE - it is machine generated by ftc-rust")
    {
        return quote_spanned! {func.sig.ident.span()=>
            compile_error!(concat!("class ", stringify!(#class_name), " already exists; remove file if you want to overwrite it"));
            #func
        }.into();
    }

    std::fs::write(java_path, java).unwrap();

    let func_name = func.sig.ident.clone();

    if linear {
        let exported_func_name =
            format_ident!("Java_org_firstinspires_ftc_teamcode_{class_name}_runOpMode");
        quote! {
            #func

            const _: () = {
                const fn assert_f_ty<R: ::ftc::command::Command>(f: fn(&::ftc::FtcContext) -> R) {}
                assert_f_ty(#func_name);
            };
            #[doc = concat!("DO NOT USE MANUALLY. Autogenerated function for opmode ", stringify!(#class_name))]
            #[unsafe(no_mangle)]
            #[doc(hidden)]
            pub extern "system" fn #exported_func_name<'local>(
                    mut unowned_env: ::ftc::jni::EnvUnowned<'local>,
                    this: ::ftc::jni::objects::JObject<'local>
                ) {
                let outcome = unowned_env.with_env(|env| -> ::ftc::jni::errors::Result<_> {
                    let mut ctx = ::ftc::FtcContext::new(
                        env,
                        this,
                    );

                    let cmd = #func_name (&ctx);

                    ::ftc::command::Command::schedule(cmd);

                    ctx.run_scheduler();
                    ::ftc::command::get_scheduler().wait_until_queue_clear();

                    Ok(())
                });

                outcome.resolve::<::ftc::policy::ThrowRuntimeExAndDefault>()
            }
        }
        .into()
    } else {
        let exported_init_name =
            format_ident!("Java_org_firstinspires_ftc_teamcode_{class_name}_init");

        // JNI name mangling replaces _ in method names with _1
        let exported_init_loop_name =
            format_ident!("Java_org_firstinspires_ftc_teamcode_{class_name}_init_1loop");
        let exported_start_name =
            format_ident!("Java_org_firstinspires_ftc_teamcode_{class_name}_start");
        let exported_loop_name =
            format_ident!("Java_org_firstinspires_ftc_teamcode_{class_name}_loop");
        let exported_stop_name =
            format_ident!("Java_org_firstinspires_ftc_teamcode_{class_name}_stop");
        quote! {
            #func

            const _: () = {
                const fn assert_f_ty(f: fn(&::ftc::IterativeContext) -> ()) {}
                assert_f_ty(#func_name);
            };

            #[doc = concat!("DO NOT USE MANUALLY. Autogenerated function for opmode ", stringify!(#class_name))]
            #[unsafe(no_mangle)]
            #[doc(hidden)]
            pub extern "system" fn #exported_init_name<'local>(
                    mut unowned_env: ::ftc::jni::EnvUnowned<'local>,
                    this: ::ftc::jni::objects::JObject<'local>
                ) {
                let outcome = unowned_env.with_env(|env| -> ::ftc::jni::errors::Result<_> {
                    ::ftc::log::trace!(concat!("initalizing ", stringify!(#class_name)));
                    let mut iterative = ::ftc::IterativeContext::get_for(
                        env,
                        &this,
                    );

                    let mut ctx = ::ftc::FtcContext::new(
                        env,
                        this,
                    );

                    #func_name (&iterative);

                    iterative.call_init(&ctx);

                    ::ftc::log::trace!(concat!("initalized ", stringify!(#class_name), ", beginning scheduler"));

                    ctx.run_scheduler();

                    Ok(())
                });

                outcome.resolve::<::ftc::policy::ThrowRuntimeExAndDefault>()
            }

            #[doc = concat!("DO NOT USE MANUALLY. Autogenerated function for opmode ", stringify!(#class_name))]
            #[unsafe(no_mangle)]
            #[doc(hidden)]
            pub extern "system" fn #exported_init_loop_name<'local>(
                    mut unowned_env: ::ftc::jni::EnvUnowned<'local>,
                    this: ::ftc::jni::objects::JObject<'local>
                ) {
                let outcome = unowned_env.with_env(|env| -> ::ftc::jni::errors::Result<_> {
                    let mut iterative = ::ftc::IterativeContext::get_for(
                        env,
                        &this,
                    );

                    let mut ctx = ::ftc::FtcContext::new_no_log(
                        env,
                        this,
                    );

                    iterative.call_init_loop(&ctx);

                    Ok(())
                });

                outcome.resolve::<::ftc::policy::ThrowRuntimeExAndDefault>()
            }

            #[doc = concat!("DO NOT USE MANUALLY. Autogenerated function for opmode ", stringify!(#class_name))]
            #[unsafe(no_mangle)]
            #[doc(hidden)]
            pub extern "system" fn #exported_start_name<'local>(
                    mut unowned_env: ::ftc::jni::EnvUnowned<'local>,
                    this: ::ftc::jni::objects::JObject<'local>
                ) {
                let outcome = unowned_env.with_env(|env| -> ::ftc::jni::errors::Result<_> {
                    let mut iterative = ::ftc::IterativeContext::get_for(
                        env,
                        &this,
                    );

                    let mut ctx = ::ftc::FtcContext::new_no_log(
                        env,
                        this,
                    );

                    iterative.call_start(&ctx);

                    Ok(())
                });

                outcome.resolve::<::ftc::policy::ThrowRuntimeExAndDefault>()
            }

            #[doc = concat!("DO NOT USE MANUALLY. Autogenerated function for opmode ", stringify!(#class_name))]
            #[unsafe(no_mangle)]
            #[doc(hidden)]
            pub extern "system" fn #exported_loop_name<'local>(
                    mut unowned_env: ::ftc::jni::EnvUnowned<'local>,
                    this: ::ftc::jni::objects::JObject<'local>
                ) {
                let outcome = unowned_env.with_env(|env| -> ::ftc::jni::errors::Result<_> {
                    let mut iterative = ::ftc::IterativeContext::get_for(
                        env,
                        &this,
                    );

                    let mut ctx = ::ftc::FtcContext::new_no_log(
                        env,
                        this,
                    );

                    iterative.call_loop(&ctx);

                    Ok(())
                });

                outcome.resolve::<::ftc::policy::ThrowRuntimeExAndDefault>()
            }

            #[doc = concat!("DO NOT USE MANUALLY. Autogenerated function for opmode ", stringify!(#class_name))]
            #[unsafe(no_mangle)]
            #[doc(hidden)]
            pub extern "system" fn #exported_stop_name<'local>(
                    mut unowned_env: ::ftc::jni::EnvUnowned<'local>,
                    this: ::ftc::jni::objects::JObject<'local>
                ) {
                let outcome = unowned_env.with_env(|env| -> ::ftc::jni::errors::Result<_> {
                    let mut iterative = ::ftc::IterativeContext::get_for(
                        env,
                        &this,
                    );

                    let mut ctx = ::ftc::FtcContext::new_no_log(
                        env,
                        this,
                    );

                    iterative.call_stop(&ctx);

                    Ok(())
                });

                outcome.resolve::<::ftc::policy::ThrowRuntimeExAndDefault>()
            }
        }
        .into()
    }
}
