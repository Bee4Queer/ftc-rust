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
    Group(String, Span),
    Description(String, Span),
    RenameCrate(Ident),
    Linear(Span),
    Iterative(Span),
    Teleop(Span),
    Autonomous(Span),
    Utility(Span),
    Disabled(Span),
}

impl FtcArg {
    pub fn get_span(&self) -> Span {
        use FtcArg::{Autonomous, Description, Disabled, Group, Iterative, Linear, Name, Teleop, Utility, RenameCrate};
        match self {
            Name(_, span)
            | Description(_, span)
            | Linear(span)
            | Iterative(span)
            | Utility(span)
            | Teleop(span)
            | Autonomous(span)
            | Group(_, span)
            | Disabled(span) => *span,
            RenameCrate(ident) => ident.span(),
        }
    }
    pub const fn get_name(&self) -> &'static str {
        use FtcArg::{Autonomous, Description, Disabled, Group, Iterative, Linear, Name, Teleop, Utility, RenameCrate};
        match self {
            Name(_, _) => "name",
            Description(_, _) => "description",
            Linear(_) => "linear",
            Iterative(_) => "iterative",
            Utility(_) => "utility",
            Teleop(_) => "teleop",
            Autonomous(_) => "auto",
            Group(_, _) => "group",
            Disabled(_) => "disabled",
            RenameCrate(_) => "rename_crate",
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
                "utility" => FtcArg::Utility(name_ident.span()),
                "teleop" => FtcArg::Teleop(name_ident.span()),
                "auto" => FtcArg::Autonomous(name_ident.span()),
                "disabled" => FtcArg::Disabled(name_ident.span()),
                "rename_crate" => {
                    let lookahead = input.lookahead1();
                    if lookahead.peek(Token![=]) {
                        let _: Token![=] = input.parse()?;

                        let lookahead = input.lookahead1();
                        if lookahead.peek(Ident) {
                            let crate_name: Ident = input.parse()?;
                            FtcArg::RenameCrate(crate_name)
                        } else {
                            return Err(lookahead.error());
                        }
                    } else {
                        return Err(lookahead.error());
                    }
                }
                "name" | "group" | "description" => {
                    let lookahead = input.lookahead1();
                    if lookahead.peek(Token![=]) {
                        let _: Token![=] = input.parse()?;

                        let lookahead = input.lookahead1();
                        if lookahead.peek(LitStr) {
                            let lit: LitStr = input.parse()?;
                            if name.as_str() == "name" {
                                FtcArg::Name(lit.value(), name_ident.span())
                            } else if name.as_str() == "description" {
                                FtcArg::Description(lit.value(), name_ident.span())
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
                        "ident should be one of linear, iterative, teleop, auto, utility, description, disabled, name, \
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
                None => part.to_string(),
            }
        })
        .collect()
}

/// The core attribute of Rust-on-FTC.
///
/// List of all arguments that can be passed (in any order):
/// 
/// - `name` (required) - The name shown on the driver station for this op mode.
/// - `linear`/`iterative` (exactly one is required) - Whether this is a linear op mode or an iterative op mode.
/// - `teleop`/`auto`/`utility` (exactly one is required) - The type of this op mode.
/// - `disabled` - Set whether this op mode is disabled.
/// - `group` - Not supported for utility op modes. Used for sorting and that's about it.
/// - `description` - Only supported for utility op modes. Provides a description of the op mode.
/// - `rename_crate` - Takes the name of the ftc crate if it has been renamed. If not provided, uses "ftc".
/// 
/// This functions by creating a java file under the new "autogenerated" directory within the teamcode directory that contains 
/// 
/// Examples:
///
/// ```no_run
/// use std::time::Duration;
/// 
/// use ftc::log::info; // the popular `log` crate is re-exported because version hell
/// 
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
///     std::thread::sleep(Duration::from_secs(2));
///     motor.set_power(0.0);
///
///     info!("Ran for {:?}!", ctx.runtime());
/// }
/// ```
///
/// ```no_run
/// use ftc::log::info;
/// use std::time::Duration;
/// 
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
///         std::thread::sleep(Duration::from_secs(2));
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
                arg.get_span(),
                format!("cannot pass {} more than once", arg.get_name()),
            )
            .into_compile_error()
            .into();
        }
    }

    let mut name = None;
    let mut group = None;
    let mut description = None;
    let mut linear = false;
    let mut iterative = false;
    let mut utility = false;
    let mut teleop = false;
    let mut autonomous = false;
    let mut disabled = false;
    let mut ftc = Ident::new("ftc", Span::call_site());

    for arg in args {
        match arg {
            FtcArg::Name(v, _) => name = Some(v),
            FtcArg::Description(v, _) => description = Some(v),
            FtcArg::Linear(_) => linear = true,
            FtcArg::Iterative(_) => iterative = true,
            FtcArg::Utility(_) => utility = true,
            FtcArg::Teleop(_) => teleop = true,
            FtcArg::Autonomous(_) => autonomous = true,
            FtcArg::Group(v, _) => group = Some(v),
            FtcArg::Disabled(_) => disabled = true,
            FtcArg::RenameCrate(name) => ftc = name,
        }
    }

    if !(teleop || autonomous || utility) {
        return Error::new(
            func.span(),
            "an op mode must either be teleop, autonomous, or utility, not none of them",
        )
        .into_compile_error()
        .into();
    }

    if [teleop, autonomous, utility]
        .into_iter()
        .filter(|v| *v)
        .count()
        > 1
    {
        return Error::new(
            func.span(),
            "an op mode must either be teleop, autonomous, or utility, not more than one",
        )
        .into_compile_error()
        .into();
    }

    if group.is_some() && utility {
        return Error::new(
            func.span(),
            "utility op modes cannot be in groups",
        )
        .into_compile_error()
        .into();
    }

    if description.is_some() && !utility {
        return Error::new(
            func.span(),
            "non-utility op modes cannot have a description",
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

    if !(linear || iterative) {
        return Error::new(
            func.span(),
            "an op mode must either be linear or iterative, not neither",
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
            "a linear op mode must take exactly one argument of type &FtcContext",
        )
        .into_compile_error()
        .into();
    }

    if func.sig.inputs.len() != 1 && iterative {
        return Error::new(
            func.span(),
            "an iterative op mode must take exactly one argument of type &IterativeContext",
        )
        .into_compile_error()
        .into();
    }

    let java_bindings_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .join("src/main/java/org/firstinspires/ftc/teamcode/autogenerated");

    let _ = std::fs::create_dir_all(&java_bindings_dir);

    let java = format!(
        r#"/* DO NOT EDIT THIS FILE - it is machine generated by ftc-rust v{}.
DO NOT PUT YOUR FILES IN THIS DIRECTORY, THEY MAY BE DELETED */

package org.firstinspires.ftc.teamcode.autogenerated;

import com.qualcomm.robotcore.eventloop.opmode.{};
import com.qualcomm.robotcore.eventloop.opmode.{};
import com.qualcomm.robotcore.eventloop.opmode.Disabled;

@{1}(name = "{}"{}{})
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
        if teleop { "TeleOp" } else if utility { "Utility" } else { "Autonomous" },
        if iterative { "OpMode" } else { "LinearOpMode" },
        name,
        if let Some(group) = group {
            format!(", group = \"{group}\"")
        } else {
            String::new()
        },
        if let Some(description) = description {
            format!(", description = \"{description}\"")
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
    if java_path.exists() {
        let contents = std::fs::read_to_string(&java_path).unwrap();
        let contents = contents.trim();

        if !contents.starts_with("/* DO NOT EDIT THIS FILE - it is machine generated by ftc-rust")
            && !contents.is_empty()
        {
            return quote_spanned! {func.sig.ident.span()=>
                compile_error!(concat!("class ", stringify!(#class_name), " already exists; remove file if you want to overwrite it or rename your opmode"));
                #func
            }.into();
        }
    }

    std::fs::write(java_path, java).unwrap();

    let func_name = func.sig.ident.clone();

    let kind = Ident::new(if teleop {
        "Teleop"
        } else if autonomous {
            "Auto"
        } else {
            "Utility"
        }, Span::call_site());
    let disabled_code = if disabled {
        quote! {
            ::#ftc::log::warn!("disabled op mode is being ran anyway; continuing");
        }
    } else {
        quote! {}
    };
    let location = quote_spanned!{func.span()=> const { ::core::panic::Location::caller() }};
    if linear {
        let exported_func_name =
            format_ident!("Java_org_firstinspires_ftc_teamcode_autogenerated_{class_name}_runOpMode");
        quote! {
            #func

            const _: () = { // scope hiding the function definitions
                const fn assert_f_ty<R: ::#ftc::command::Command>(f: fn(&::#ftc::FtcContext) -> R) {}
                assert_f_ty(#func_name);
                #[doc = concat!("DO NOT USE MANUALLY (how would you even?). Autogenerated function for opmode ", stringify!(#class_name))]
                #[unsafe(no_mangle)]
                #[doc(hidden)]
                extern "system" fn #exported_func_name<'local>(
                    mut unowned_env: ::#ftc::jni::EnvUnowned<'local>,
                    this: ::#ftc::jni::objects::JObject<'local>
                ) {
                    let outcome = unowned_env.with_env(|env| -> ::#ftc::jni::errors::Result<_> {
                        let mut ctx = ::#ftc::FtcContext::new(
                            env,
                            this,
                            ::#ftc::OpModeType::#kind,
                            stringify!(#class_name),
                            #location,
                        );

                        #disabled_code

                        let cmd = #func_name (&ctx);

                        ::#ftc::command::Command::schedule(cmd);

                        ::#ftc::log::trace!(concat!("finished executing ", stringify!(#class_name), ", beginning scheduler and waiting until queue clear"));

                        ctx.run_scheduler();
                        ::#ftc::command::get_scheduler().wait_until_queue_clear();

                        Ok(())
                    });

                    outcome.resolve::<::#ftc::policy::ThrowRuntimeExAndDefault>()
                }
            };
        }
        .into()
    } else {
        let exported_init_name =
            format_ident!("Java_org_firstinspires_ftc_teamcode_autogenerated_{class_name}_init");
        // JNI name mangling replaces _ in method names with _1
        let exported_init_loop_name =
            format_ident!("Java_org_firstinspires_ftc_teamcode_autogenerated_{class_name}_init_1loop");
        let exported_start_name =
            format_ident!("Java_org_firstinspires_ftc_teamcode_autogenerated_{class_name}_start");
        let exported_loop_name =
            format_ident!("Java_org_firstinspires_ftc_teamcode_autogenerated_{class_name}_loop");
        let exported_stop_name =
            format_ident!("Java_org_firstinspires_ftc_teamcode_autogenerated_{class_name}_stop");
        quote! {
            #func

            const _: () = { // scope hiding the function definitions
                const fn assert_f_ty(f: fn(&::#ftc::IterativeContext) -> ()) {}
                assert_f_ty(#func_name);

                #[doc = concat!("DO NOT USE MANUALLY (how would you even?). Autogenerated function for opmode ", stringify!(#class_name))]
                #[unsafe(no_mangle)]
                #[doc(hidden)]
                extern "system" fn #exported_init_name<'local>(
                    mut unowned_env: ::#ftc::jni::EnvUnowned<'local>,
                    this: ::#ftc::jni::objects::JObject<'local>
                ) {
                    let outcome = unowned_env.with_env(|env| -> ::#ftc::jni::errors::Result<_> {
                        ::#ftc::log::trace!(concat!("initalizing ", stringify!(#class_name)));
                        let mut iterative = ::#ftc::IterativeContext::get_for(
                            env,
                            &this,
                            ::#ftc::OpModeType::#kind,
                            stringify!(#class_name),
                            #location,
                        );

                        let mut ctx = ::#ftc::FtcContext::new(
                            env,
                            this,
                            ::#ftc::OpModeType::#kind,
                            stringify!(#class_name),
                            #location,
                        );

                        #disabled_code

                        #func_name (&iterative);

                        iterative.call_init(&ctx);

                        ::#ftc::log::trace!(concat!("initalized ", stringify!(#class_name), ", beginning scheduler"));

                        ctx.run_scheduler();

                        Ok(())
                    });

                    outcome.resolve::<::#ftc::policy::ThrowRuntimeExAndDefault>()
                }

                #[doc = concat!("DO NOT USE MANUALLY (how would you even?). Autogenerated function for opmode ", stringify!(#class_name))]
                #[unsafe(no_mangle)]
                #[doc(hidden)]
                extern "system" fn #exported_init_loop_name<'local>(
                    mut unowned_env: ::#ftc::jni::EnvUnowned<'local>,
                    this: ::#ftc::jni::objects::JObject<'local>
                ) {
                    let outcome = unowned_env.with_env(|env| -> ::#ftc::jni::errors::Result<_> {
                        let mut iterative = ::#ftc::IterativeContext::get_for(
                            env,
                            &this,
                            ::#ftc::OpModeType::#kind,
                            stringify!(#class_name),
                            #location,
                        );

                        let mut ctx = ::#ftc::FtcContext::new_no_log(
                            env,
                            this,
                            ::#ftc::OpModeType::#kind,
                            stringify!(#class_name),
                            #location,
                        );

                        iterative.call_init_loop(&ctx);

                        Ok(())
                    });

                    outcome.resolve::<::#ftc::policy::ThrowRuntimeExAndDefault>()
                }

                #[doc = concat!("DO NOT USE MANUALLY (how would you even?). Autogenerated function for opmode ", stringify!(#class_name))]
                #[unsafe(no_mangle)]
                #[doc(hidden)]
                extern "system" fn #exported_start_name<'local>(
                    mut unowned_env: ::#ftc::jni::EnvUnowned<'local>,
                    this: ::#ftc::jni::objects::JObject<'local>
                ) {
                    let outcome = unowned_env.with_env(|env| -> ::#ftc::jni::errors::Result<_> {
                        ::#ftc::log::trace!(concat!("opmode ", stringify!(#class_name), " is starting"));
                        let mut iterative = ::#ftc::IterativeContext::get_for(
                            env,
                            &this,
                            ::#ftc::OpModeType::#kind,
                            stringify!(#class_name),
                            #location,
                        );

                        let mut ctx = ::#ftc::FtcContext::new_no_log(
                            env,
                            this,
                            ::#ftc::OpModeType::#kind,
                            stringify!(#class_name),
                            #location,
                        );

                        iterative.call_start(&ctx);

                        Ok(())
                    });

                    outcome.resolve::<::#ftc::policy::ThrowRuntimeExAndDefault>()
                }

                #[doc = concat!("DO NOT USE MANUALLY (how would you even?). Autogenerated function for opmode ", stringify!(#class_name))]
                #[unsafe(no_mangle)]
                #[doc(hidden)]
                extern "system" fn #exported_loop_name<'local>(
                    mut unowned_env: ::#ftc::jni::EnvUnowned<'local>,
                    this: ::#ftc::jni::objects::JObject<'local>
                ) {
                    let outcome = unowned_env.with_env(|env| -> ::#ftc::jni::errors::Result<_> {
                        let mut iterative = ::#ftc::IterativeContext::get_for(
                            env,
                            &this,
                            ::#ftc::OpModeType::#kind,
                            stringify!(#class_name),
                            #location,
                        );

                        let mut ctx = ::#ftc::FtcContext::new_no_log(
                            env,
                            this,
                            ::#ftc::OpModeType::#kind,
                            stringify!(#class_name),
                            #location,
                        );

                        iterative.call_loop(&ctx);

                        Ok(())
                    });

                    outcome.resolve::<::#ftc::policy::ThrowRuntimeExAndDefault>()
                }

                #[doc = concat!("DO NOT USE MANUALLY (how would you even?). Autogenerated function for opmode ", stringify!(#class_name))]
                #[unsafe(no_mangle)]
                #[doc(hidden)]
                extern "system" fn #exported_stop_name<'local>(
                    mut unowned_env: ::#ftc::jni::EnvUnowned<'local>,
                    this: ::#ftc::jni::objects::JObject<'local>
                ) {
                    let outcome = unowned_env.with_env(|env| -> ::#ftc::jni::errors::Result<_> {
                        let mut iterative = ::#ftc::IterativeContext::get_for(
                            env,
                            &this,
                            ::#ftc::OpModeType::#kind,
                            stringify!(#class_name),
                            #location,
                        );

                        let mut ctx = ::#ftc::FtcContext::new_no_log(
                            env,
                            this,
                            ::#ftc::OpModeType::#kind,
                            stringify!(#class_name),
                            #location,
                        );

                        iterative.call_stop(&ctx);

                        ctx.stop_scheduler();

                        Ok(())
                    });

                    outcome.resolve::<::#ftc::policy::ThrowRuntimeExAndDefault>()
                }
            };
        }
        .into()
    }
}
