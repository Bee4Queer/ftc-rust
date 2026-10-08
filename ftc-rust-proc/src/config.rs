use std::{collections::HashMap, fmt::Write, path::PathBuf};

use heck::ToShoutySnekCase;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote, quote_spanned};
use syn::{
    Attribute, Error, Ident, LitBool, LitInt, LitStr, Token, Visibility, parenthesized,
    parse::{Parse, ParseStream, Parser as _},
    spanned::Spanned,
    token::Paren,
};

#[derive(Copy, Clone)]
enum Hub {
    ControlHub(Span),
    ExpansionHub(Span),
}

impl Parse for Hub {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;

        match ident.to_string().as_str() {
            "CTRL_HUB" => Ok(Self::ControlHub(ident.span())),
            "EXP_HUB" => Ok(Self::ExpansionHub(ident.span())),
            _ => Err(Error::new_spanned(
                ident,
                "invalid hub; expected one of CTRL_HUB, EXP_HUB",
            )),
        }
    }
}

#[derive(Clone)]
enum MotorKind {
    Generic(Token![.], Span),

    NeveRest37v1Gear(Token![.], Span),
    NeveRest20Gear(Token![.], Span),
    NeveRest40Gear(Token![.], Span),
    NeveRest60Gear(Token![.], Span),

    RevRobotics20HDHex(Token![.], Span),
    RevRobotics40HDHex(Token![.], Span),
    RevRoboticsCoreHex(Token![.], Span),

    GoBilda5201(Token![.], Span),
    /// Includes 5202/5203/5204 series motors.
    GoBilda5202(Token![.], Ident),

    Tetrix(Token![.], Span),
}

impl Parse for MotorKind {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let dot = input.parse()?;
        let name: Ident = input.parse()?;

        match name.to_string().as_str() {
            "Generic" => Ok(Self::Generic(dot, name.span())),
            "NeveRest37v1Gear" => Ok(Self::NeveRest37v1Gear(dot, name.span())),
            "NeveRest20Gear" => Ok(Self::NeveRest20Gear(dot, name.span())),
            "NeveRest40Gear" => Ok(Self::NeveRest40Gear(dot, name.span())),
            "NeveRest60Gear" => Ok(Self::NeveRest60Gear(dot, name.span())),

            "RevRobotics20HDHex" => Ok(Self::RevRobotics20HDHex(dot, name.span())),
            "RevRobotics40HDHex" => Ok(Self::RevRobotics40HDHex(dot, name.span())),
            "RevRoboticsCoreHex" => Ok(Self::RevRoboticsCoreHex(dot, name.span())),

            "GoBilda5201" => Ok(Self::GoBilda5201(dot, name.span())),
            "GoBilda5202" | "GoBilda5203" | "GoBilda5204" => {
                Ok(Self::GoBilda5202(dot, name))
            }

            "Tetrix" => Ok(Self::Tetrix(dot, name.span())),

            _ => Err(Error::new_spanned(
                name,
                "expected a valid motor kind (one of NeveRest37v1Gear, NeveRest20Gear, \
                 NeveRest40Gear, NeveRest60Gear, RevRobotics20HDHex, RevRobotics40HDHex, \
                 RevRoboticsCoreHex, GoBilda5201, GoBilda5202, GoBilda5203, GoBilda5204, Tetrix)",
            )),
        }
    }
}

#[derive(Copy, Clone)]
enum ServoKind {
    Servo(Span),
    CRServo(Span),
    RevSPARKMini(Span),
}

impl Parse for ServoKind {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;

        match name.to_string().as_str() {
            "Servo" => Ok(Self::Servo(name.span())),
            "CRServo" => Ok(Self::CRServo(name.span())),
            "RevSPARKMini" => Ok(Self::RevSPARKMini(name.span())),

            _ => Err(Error::new_spanned(
                name,
                "expected a valid servo kind (one of Servo, CRServo, RevSPARKMini)",
            )),
        }
    }
}

#[derive(Copy, Clone)]
enum I2CKind {
    RevVL53L0XRangeSensor(Token![.], Span),
    LynxColorSensor(Token![.], Span),
    RevColorSensorV3(Token![.], Span),
    RevBlinkinLedDriver(Token![.], Span),
}

impl Parse for I2CKind {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let dot = input.parse()?;
        let name: Ident = input.parse()?;

        match name.to_string().as_str() {
            "RevVL53L0XRangeSensor" => Ok(Self::RevVL53L0XRangeSensor(dot, name.span())),
            "LynxColorSensor" => Ok(Self::LynxColorSensor(dot, name.span())),
            "RevColorSensorV3" => Ok(Self::RevColorSensorV3(dot, name.span())),
            "RevBlinkinLedDriver" => Ok(Self::RevBlinkinLedDriver(dot, name.span())),

            _ => Err(Error::new_spanned(
                name,
                "expected a valid i2c device kind (one of RevVL53L0XRangeSensor, LynxColorSensor, \
                 RevColorSensorV3, RevBlinkinLedDriver)",
            )),
        }
    }
}

#[derive(Clone)]
enum DeviceKind {
    Motor {
        motor: Span,
        kind: MotorKind,
        parens: Paren,
        /// range 0..=3
        port: (u8, Span),
    },
    Servo {
        kind: ServoKind,
        parens: Paren,
        /// range 0..=5
        port: (u8, Span),
    },
    DigitalDevice {
        device: Span,
        parens: Paren,
        /// range 0..=5
        port: (u8, Span),
    },
    RevTouchSensor {
        device: Span,
        parens: Paren,
        /// range 0..=5
        port: (u8, Span),
    },
    I2C {
        i2c: Span,
        parens: Paren,
        /// range 0..=3
        bus: (u8, Span),
        kind: I2CKind,
    },
    EmbeddedIMU(Span),
}

impl DeviceKind {
    fn type_name(&self) -> TokenStream {
        match self {
            DeviceKind::Motor { .. } => quote! { ::ftc::hardware::DcMotor },
            DeviceKind::Servo { kind, .. } => match kind {
                ServoKind::Servo(_) => quote! { ::ftc::hardware::Servo },
                ServoKind::CRServo(_) => quote! { ::ftc::hardware::CRServo },
                ServoKind::RevSPARKMini(_) => quote! { ::ftc::hardware::DcMotorSimple },
            },
            DeviceKind::DigitalDevice { .. } => {
                todo!("digital devices are not currently implemented")
            }
            DeviceKind::RevTouchSensor { .. } => {
                todo!("rev touch sensors are not currently implemented")
            }
            DeviceKind::I2C { .. } => todo!("i2c devices are not currently implemented"),
            DeviceKind::EmbeddedIMU(_) => quote! { ::ftc::hardware::IMU },
        }
    }
}

impl Parse for DeviceKind {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let forked = input.fork();
        if let Ok(kind) = forked.parse() {
            let _ = input.parse::<ServoKind>();
            let port;
            let parens = parenthesized!(port in input);
            let port = port.parse::<LitInt>()?;

            Ok(DeviceKind::Servo {
                kind,
                parens,
                port: (port.base10_parse::<u8>()?, port.span()),
            })
        } else {
            let root: Ident = input.parse()?;

            match root.to_string().as_str() {
                "Motor" => {
                    let kind: MotorKind = input.parse()?;
                    let port;
                    let parens = parenthesized!(port in input);
                    let port = port.parse::<LitInt>()?;

                    Ok(DeviceKind::Motor {
                        motor: root.span(),
                        kind,
                        parens,
                        port: (port.base10_parse::<u8>()?, port.span()),
                    })
                }
                "DigitalDevice" => {
                    let port;
                    let parens = parenthesized!(port in input);
                    let port = port.parse::<LitInt>()?;

                    Ok(DeviceKind::DigitalDevice {
                        device: root.span(),
                        parens,
                        port: (port.base10_parse::<u8>()?, port.span()),
                    })
                }
                "RevTouchSensor" => {
                    let port;
                    let parens = parenthesized!(port in input);
                    let port = port.parse::<LitInt>()?;

                    Ok(DeviceKind::RevTouchSensor {
                        device: root.span(),
                        parens,
                        port: (port.base10_parse::<u8>()?, port.span()),
                    })
                }
                "I2C" => {
                    let kind: I2CKind = input.parse()?;
                    let bus;
                    let parens = parenthesized!(bus in input);
                    let bus = bus.parse::<LitInt>()?;

                    Ok(DeviceKind::I2C {
                        i2c: root.span(),
                        kind,
                        parens,
                        bus: (bus.base10_parse::<u8>()?, bus.span()),
                    })
                }
                "EmbeddedIMU" => Ok(DeviceKind::EmbeddedIMU(root.span())),

                _ => Err(Error::new_spanned(
                    root,
                    "expected a valid root device kind (one of Motor, Servo, CRServo, \
                     RevSPARKMini, DigitalDevice, RevTouchSensor, I2C, EmbeddedIMU)",
                )),
            }
        }
    }
}

#[derive(Clone)]
struct Device {
    attrs: Vec<Attribute>,
    vis: Visibility,
    name: Ident,
    hub: Hub,
    kind: DeviceKind,
}

#[derive(Clone)]
struct Config {
    config_name: LitStr,
    has_exp_hub: bool,
    devices: Vec<Device>,
}

impl Config {
    fn to_xml(&self) -> String {
        fn output_module(out: &mut String, name: impl AsRef<str>, port: u8) {
            let _ = writeln!(
                out,
                r#"        <LynxModule name="{}" port="{port}">"#,
                name.as_ref()
            );
        }
        fn end_module(out: &mut String) {
            out.push_str("        </LynxModule>\n");
        }

        fn output_devices(out: &mut String, devices: Vec<&Device>) {
            for device in devices {
                let (tag, attrs) = match &device.kind {
                    DeviceKind::Motor {
                        motor: _,
                        kind,
                        parens: _,
                        port: (port, _),
                    } => (
                        match kind {
                            MotorKind::Generic(_, _) => "Motor",
                            MotorKind::NeveRest37v1Gear(_, _) => "NeveRest3.7v1Gear",
                            MotorKind::NeveRest20Gear(_, _) => "NeveRest20Gear",
                            MotorKind::NeveRest40Gear(_, _) => "NeveRest40Gear",
                            MotorKind::NeveRest60Gear(_, _) => "NeveRest60Gear",
                            MotorKind::RevRobotics20HDHex(_, _) => "RevRobotics20HDHex",
                            MotorKind::RevRobotics40HDHex(_, _) => "RevRobotics40HDHex",
                            MotorKind::RevRoboticsCoreHex(_, _) => "RevRoboticsCoreHex",
                            MotorKind::GoBilda5201(_, _) => "goBILDA5201",
                            MotorKind::GoBilda5202(_, _) => "goBILDA5202",
                            MotorKind::Tetrix(_, _) => "Tetrix",
                        },
                        format!(r#" port="{port}""#),
                    ),
                    DeviceKind::Servo {
                        kind,
                        parens: _,
                        port: (port, _),
                    } => (
                        match kind {
                            ServoKind::Servo(_) => "Servo",
                            ServoKind::CRServo(_) => "ContinuousRotationServo",
                            ServoKind::RevSPARKMini(_) => "RevSPARKMini",
                        },
                        format!(r#" port="{port}""#),
                    ),
                    DeviceKind::DigitalDevice {
                        device: _,
                        parens: _,
                        port: (port, _),
                    } => ("DigitalDevice", format!(r#" port="{port}""#)),
                    DeviceKind::RevTouchSensor {
                        device: _,
                        parens: _,
                        port: (port, _),
                    } => ("RevTouchSensor", format!(r#" port="{port}""#)),
                    DeviceKind::I2C {
                        i2c: _,
                        parens: _,
                        bus: (bus, _),
                        kind,
                    } => (
                        match kind {
                            I2CKind::RevVL53L0XRangeSensor(_, _) => "REV_VL53L0X_RANGE_SENSOR",
                            I2CKind::LynxColorSensor(_, _) => "LynxColorSensor",
                            I2CKind::RevColorSensorV3(_, _) => "RevColorSensorV3",
                            I2CKind::RevBlinkinLedDriver(_, _) => "RevBlinkinLedDriver",
                        },
                        format!(r#" port="{}" bus="{bus}""#, u8::from(*bus == 0)),
                    ),
                    DeviceKind::EmbeddedIMU(_) => {
                        ("LynxEmbeddedIMU", r#" port="0" bus="0""#.to_string())
                    }
                };
                let _ = writeln!(
                    out,
                    r#"            <{} name="{}"{} />"#,
                    tag, device.name, attrs
                );
            }
        }

        let mut ctrl_hub_devices = Vec::with_capacity(self.devices.len());
        let mut exp_hub_devices = Vec::with_capacity(self.devices.len());

        for module in &self.devices {
            (match module.hub {
                Hub::ControlHub(_) => &mut ctrl_hub_devices,
                Hub::ExpansionHub(_) => &mut exp_hub_devices,
            })
            .push(module);
        }

        let mut out = format!(
            r#"<?xml version='1.0' encoding='UTF-8' standalone='yes' ?> <!-- DO NOT EDIT THIS FILE - it is machine generated by ftc-rust v{}. -->
<Robot type="FirstInspires-FTC">
    <LynxUsbDevice name="Control Hub Portal" serialNumber="(embedded)" parentModuleAddress="173">
"#,
            env!("CARGO_PKG_VERSION")
        );

        output_module(&mut out, "Control Hub", 173);
        output_devices(&mut out, ctrl_hub_devices);
        end_module(&mut out);

        if self.has_exp_hub {
            output_module(&mut out, "Expansion Hub", 3);
            output_devices(&mut out, exp_hub_devices);
            end_module(&mut out);
        }

        out.push_str(
            r"    </LynxUsbDevice>
</Robot>
",
        );
        out
    }
}

enum ConfigInputPropVal {
    String(LitStr),
    Bool(LitBool),
    Ident(Ident),
}

impl ToTokens for ConfigInputPropVal {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            ConfigInputPropVal::String(lit_str) => lit_str.to_tokens(tokens),
            ConfigInputPropVal::Bool(lit_bool) => lit_bool.to_tokens(tokens),
            ConfigInputPropVal::Ident(ident) => ident.to_tokens(tokens),
        }
    }
}

impl Parse for ConfigInputPropVal {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(LitStr) {
            input.parse().map(Self::String)
        } else if lookahead.peek(LitBool) {
            input.parse().map(Self::Bool)
        } else if lookahead.peek(Ident) {
            input.parse().map(Self::Ident)
        } else {
            Err(lookahead.error())
        }
    }
}

enum ConfigInput {
    Property {
        attrs: Vec<Attribute>,
        vis: Visibility,
        name: Ident,
        eq: Token![=],
        contents: ConfigInputPropVal,
        semi: Token![;],
    },
    Device {
        attrs: Vec<Attribute>,
        vis: Visibility,
        r#static: Token![static],
        name: Ident,
        eq: Token![=],
        hub: Hub,
        sep: Token![/],
        kind: DeviceKind,
        semi: Token![;],
    },
}

impl Parse for ConfigInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis: Visibility = input.parse()?;
        let lookahead = input.lookahead1();
        if lookahead.peek(Ident) {
            let name = Ident::parse(input)?;
            let eq = <Token![=]>::parse(input)?;
            let contents = ConfigInputPropVal::parse(input)?;
            let semi = <Token![;]>::parse(input)?;

            Ok(Self::Property {
                attrs,
                vis,
                name,
                eq,
                contents,
                semi,
            })
        } else if lookahead.peek(Token![static]) {
            let r#static = <Token![static]>::parse(input)?;
            let name = Ident::parse(input)?;
            let eq = <Token![=]>::parse(input)?;
            let hub = Hub::parse(input)?;
            let sep = <Token![/]>::parse(input)?;
            let kind = DeviceKind::parse(input)?;
            let semi = <Token![;]>::parse(input)?;

            Ok(Self::Device {
                attrs,
                vis,
                r#static,
                name,
                eq,
                hub,
                sep,
                kind,
                semi,
            })
        } else {
            Err(lookahead.error())
        }
    }
}

pub fn config(tokens: TokenStream) -> syn::Result<TokenStream> {
    let full_span = tokens.span();
    let parser = |input: ParseStream| -> syn::Result<Vec<ConfigInput>> {
        let mut items = Vec::new();

        while !input.is_empty() {
            items.push(input.parse()?);
        }

        Ok(items)
    };
    let items = parser.parse2(tokens)?;

    let mut config_name: Option<(Vec<Attribute>, Visibility, LitStr)> = None;
    let mut has_exp_hub: Option<LitBool> = None;
    let mut devices: Vec<Device> = Vec::with_capacity(items.len());
    let mut ftc: Option<Ident> = None;

    for item in items {
        match item {
            ConfigInput::Property {
                attrs,
                vis,
                name,
                contents,
                ..
            } => match name.to_string().as_str() {
                "CONFIG_NAME" => {
                    if let Some(first) = config_name {
                        let mut err = Error::new_spanned(name, "CONFIG_NAME redefined");
                        err.combine(Error::new_spanned(first.2, "original declaration"));
                        return Err(err);
                    }
                    config_name = Some(match contents {
                        ConfigInputPropVal::String(v) => (attrs, vis, v),
                        _ => return Err(Error::new_spanned(contents, "expected string")),
                    });
                }
                "HAS_EXP_HUB" => {
                    if let Some(first) = has_exp_hub {
                        let mut err = Error::new_spanned(name, "HAS_EXP_HUB redefined");
                        err.combine(Error::new_spanned(first, "original declaration"));
                        return Err(err);
                    }
                    has_exp_hub = Some(match contents {
                        ConfigInputPropVal::Bool(v) => v,
                        _ => return Err(Error::new_spanned(contents, "expected bool")),
                    });
                }
                "FTC_NAME" => {
                    if let Some(first) = ftc {
                        let mut err = Error::new_spanned(name, "FTC_NAME redefined");
                        err.combine(Error::new_spanned(first, "original declaration"));
                        return Err(err);
                    }
                    ftc = Some(match contents {
                        ConfigInputPropVal::Ident(v) => v,
                        _ => return Err(Error::new_spanned(contents, "expected ident")),
                    });
                }
                _ => {
                    return Err(Error::new_spanned(
                        name,
                        "expected a valid property (one of CONFIG_NAME, HAS_EXP_HUB, FTC_NAME)",
                    ));
                }
            },
            ConfigInput::Device {
                attrs,
                vis,
                name,
                hub,
                kind,
                ..
            } => {
                devices.push(Device {
                    attrs,
                    vis,
                    name,
                    hub,
                    kind,
                });
            }
        }
    }

    if config_name.is_none() {
        return Err(Error::new(full_span, "no configuration name specified"));
    }

    let ftc = ftc.unwrap_or_else(|| Ident::new("ftc", full_span));

    let config_name = config_name.unwrap();

    let cfg = Config {
        config_name: config_name.2,
        has_exp_hub: has_exp_hub.is_some_and(|v| v.value),
        devices,
    };

    let cfg_name = cfg.config_name.value();

    if cfg_name != cfg_name.trim()
        || cfg_name.is_empty()
        || cfg_name.contains(['/', '\\', '?', ':', '"', '*', '|', '<', '>'])
    {
        return Err(Error::new_spanned(
            cfg.config_name,
            "invalid config name specified",
        ));
    }

    let mut motors = HashMap::with_capacity(4);
    let mut servos = HashMap::with_capacity(6);
    let mut digitals = HashMap::with_capacity(6);
    let mut i2cs = HashMap::with_capacity(4);
    let mut has_imu = None;

    for device in &cfg.devices {
        let (hash_set, port) = match device.kind {
            DeviceKind::Motor { port, .. } => (&mut motors, port),
            DeviceKind::Servo { port, .. } => (&mut servos, port),
            DeviceKind::DigitalDevice { port, .. } | DeviceKind::RevTouchSensor { port, .. } => {
                (&mut digitals, port)
            }
            DeviceKind::I2C { bus, .. } => (&mut i2cs, bus),
            DeviceKind::EmbeddedIMU(span) => {
                if let Some(first) = has_imu {
                    let mut err = Error::new(span, "embedded IMU is already defined earlier");
                    err.combine(Error::new(first, "IMU is defined here"));
                    return Err(err);
                }
                has_imu = Some(span);
                if !matches!(device.hub, Hub::ControlHub(_)) {
                    return Err(Error::new(span, "embedded IMU can only be defined under control hub"));
                }
                continue;
            }
        };

        if let Some(first) = hash_set.get(&port.0) {
            let mut err = Error::new(port.1, "this port is already used earlier");
            err.combine(Error::new(*first, "port is used here"));
            return Err(err);
        }

        hash_set.insert(port.0, port.1);
    }

    let xml_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .join("src/main/res/xml");

    let xml = cfg.to_xml();

    let out_path = xml_dir.join(format!("{}.xml", cfg.config_name.value()));

    if out_path.exists() {
        let contents = std::fs::read_to_string(&out_path).unwrap();
        let contents = contents.trim();

        if !contents.starts_with(
            "<?xml version='1.0' encoding='UTF-8' standalone='yes' ?> <!-- DO NOT EDIT THIS FILE \
             - it is machine generated by ftc-rust v",
        ) && !contents.is_empty()
        {
            return Err(Error::new(
                cfg.config_name.span(),
                "configuration file already exists; remove file if you want to overwrite it or \
                 rename your config",
            ));
        }
    }

    std::fs::write(out_path, xml).unwrap();

    let shouty_config_name = cfg.config_name.value().TO_SHOUTY_SNEK_CASE();

    let mut is_valid_xid = unicode_ident::is_xid_start(shouty_config_name.chars().next().unwrap());

    if is_valid_xid {
        for ch in shouty_config_name.chars().skip(1) {
            is_valid_xid = unicode_ident::is_xid_continue(ch);
            if !is_valid_xid {
                break;
            }
        }
    }

    if !is_valid_xid {
        return Err(Error::new_spanned(
            cfg.config_name,
            "config name cannot be converted to valid rust identifier",
        ));
    }

    let vis = config_name.1;
    let attrs = config_name.0;
    let config_name = Ident::new(&shouty_config_name, cfg.config_name.span());
    let actual_config_name = cfg.config_name;

    let them = cfg.devices.iter().map(|v| {
        let vis = &v.vis;
        let name = &v.name;
        let ty = v.kind.type_name();
        let attrs = &v.attrs;
        quote_spanned! {name.span()=>
            #(#attrs)*
            #vis static #name: ::#ftc::hardware::config::HardwareItem<#ty> = ::#ftc::hardware::config::HardwareItem::new(stringify!(#name), &#config_name);
        }
    });

    let them2 = cfg.devices.iter().map(|v| {
        let name = &v.name;
        quote_spanned! {name.span()=>
            stringify!(#name)
        }
    });

    let them3 = cfg.devices.iter().map(|v| {
        let name = &v.name;
        let motor = match &v.kind {
            DeviceKind::Motor { motor, .. } => Some(quote_spanned! {*motor=> Motor:: }),
            _ => None,
        };
        let kind = match &v.kind {
            DeviceKind::Motor { kind, .. } => match kind {
                MotorKind::Generic(_, span) => quote_spanned!{*span=> Generic},
                MotorKind::NeveRest37v1Gear(_, span) => quote_spanned!{*span=> GeneNeveRest37v1Gearric},
                MotorKind::NeveRest20Gear(_, span) => quote_spanned!{*span=> NeveRest20Gear},
                MotorKind::NeveRest40Gear(_, span) => quote_spanned!{*span=> NeveRest40Gear},
                MotorKind::NeveRest60Gear(_, span) => quote_spanned!{*span=> NeveRest60Gear},
                MotorKind::RevRobotics20HDHex(_, span) => quote_spanned!{*span=> RevRobotics20HDHex},
                MotorKind::RevRobotics40HDHex(_, span) => quote_spanned!{*span=> RevRobotics40HDHex},
                MotorKind::RevRoboticsCoreHex(_, span) => quote_spanned!{*span=> RevRoboticsCoreHex},
                MotorKind::GoBilda5201(_, span) => quote_spanned!{*span=> GoBilda5201},
                MotorKind::GoBilda5202(_, name) => name.to_token_stream(),
                MotorKind::Tetrix(_, span) => quote_spanned!{*span=> Tetrix},
            },
            DeviceKind::Servo { kind, .. } => match kind {
                ServoKind::Servo(span) => quote_spanned!{*span=> Servo},
                ServoKind::CRServo(span) => quote_spanned!{*span=> CRServo},
                ServoKind::RevSPARKMini(span) => quote_spanned!{*span=> RevSPARKMini},
            },
            DeviceKind::DigitalDevice { .. } => todo!(),
            DeviceKind::RevTouchSensor { .. } => todo!(),
            DeviceKind::I2C { .. } => todo!(),
            DeviceKind::EmbeddedIMU(span) => quote_spanned! {*span=> EmbeddedIMU},
        };
        quote_spanned! {name.span()=>
            let _: ::#ftc::hardware::config::device_docs:: #motor #kind;
        }
    });

    Ok(quote_spanned! {full_span=>
        #(#attrs)*
        #vis static #config_name: ::#ftc::hardware::config::HardwareConfig =
            ::#ftc::hardware::config::HardwareConfig::new(#actual_config_name, &[#( #them2 ),*]);

        #(
            #them
        )*

        const _: () = {
            // documentation stuff
            #( #them3 )*
        };
    })
}
