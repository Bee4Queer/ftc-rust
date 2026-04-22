//! Rust Project

use std::{collections::HashSet, hash::Hash, path::PathBuf};

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, Parser},
    parse_macro_input,
    spanned::Spanned,
    Error, Ident, ItemFn, LitStr, Token,
};

extern crate proc_macro;

#[derive(Debug, Clone)]
enum FtcArg {
    Name(String, Span),
    Linear(Span),
    /// Not currently parsed, need to implement
    #[allow(dead_code)]
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
                // "iterative" => FtcArg::Iterative(name_ident.span()),
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
/// ```ignore
/// #[ftc(name = "Example: My Linear Op Mode", linear, teleop, group = "Example", disabled)]
/// fn my_linear_op_mode(ctx: &ftc::FtcContext) {
///     // equivalent to hardwareMap.get(DcMotor.class, "motor") in Java:
///     let mut motor = ctx.hardware().dc_motor("motor");
///     motor.setDirection(ftc::Direction::Forward);
///
///     ctx.telemetry().add_data("Status", "Initalized");
///     ctx.telemetry().update();
///
///     ctx.wait_for_start();
///
///     // ctx.running() instead of opModeIsActive()    
///
///     motor.set_power(0.5);
///     ctx.sleep_s(2.0);
///     motor.set_power(0.0);
/// }
/// ```
#[proc_macro_attribute]
pub fn ftc(attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);

    if func.sig.inputs.len() != 1 {
        return Error::new(
            func.span(),
            "an op mode must take one argument of type &FtcContext",
        )
        .into_compile_error()
        .into();
    }

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

    if !linear {
        return Error::new(
            func.span(),
            "an op mode must specify linear for forward compatibility when I add iterative support",
        )
        .into_compile_error()
        .into();
    }

    if iterative {
        return Error::new(
            func.span(),
            "iterative opmode support is not implemented",
        )
        .into_compile_error()
        .into();
    }

    let Some(name) = name else {
        return Error::new(func.span(), "an op mode must have a name")
            .into_compile_error()
            .into();
    };

    let java_bindings_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .join("src/main/java/org/firstinspires/ftc/teamcode");

    let java = format!(
        r#"/* DO NOT EDIT THIS FILE - it is machine generated by the FTC rust proc macro */

package org.firstinspires.ftc.teamcode;

import com.qualcomm.robotcore.eventloop.opmode.{};
import com.qualcomm.robotcore.eventloop.opmode.{};
import com.qualcomm.robotcore.eventloop.opmode.Disabled;

@{0}(name = "{}"{})
{}
public class {class_name} extends {1} {{
    {}

    static {{
        System.loadLibrary("team_code_rust");
    }}
}}
"#,
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
            todo!("implement iterative op modes")
        }
    );

    std::fs::write(java_bindings_dir.join(class_name.clone() + ".java"), java).unwrap();

    let func_name = func.sig.ident.clone();

    if linear {
        let exported_func_name =
            format_ident!("Java_org_firstinspires_ftc_teamcode_{class_name}_runOpMode");
        quote! {
            #func

            /// DO NOT USE MANUALLY. Autogenerated function for opmode
            #[doc = stringify!(#class_name)]
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

                    Ok(())
                });

                outcome.resolve::<::ftc::jni::errors::ThrowRuntimeExAndDefault>()
            }
        }
        .into()
    } else {
        todo!("implement iterative op modes")
    }
}
