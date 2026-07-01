//! Convinence macros.

/// Call a java method on a Device.
#[macro_export]
macro_rules! call_method_device {
    ($ty:ident $self:expr, $($tt:tt)+) => {
        $crate::call_method!($ty $self.inner.as_ref().unwrap(), $($tt)+)
    };
}

/// Call a java method.
#[macro_export]
macro_rules! call_method {
    (void $self:expr, $obj:expr, $name:expr, $sig:expr, $args:tt $(,)?) => {{
        $self.vm.attach_current_thread(|env| {$crate::call_method!(env env, $obj, $name, $sig, $args)?; Ok::<(), jni::errors::Error>(())}).unwrap();
    }};
    (obj $self:expr, $obj:expr, $name:expr, $sig:expr, $args:tt $(,)?) => {{
        $self.vm.attach_current_thread(|env| {
            let object = $crate::call_method!(env env, $obj, $name, $sig, $args)?.l()?;
            $crate::new_global!(env, object)
        }).unwrap()
    }};
    (double $self:expr, $obj:expr, $name:expr, $sig:expr, $args:tt $(,)?) => {{
        $self.vm.attach_current_thread(|env| {
            $crate::call_method!(env env, $obj, $name, $sig, $args)?.d()
        }).unwrap()
    }};
    (float $self:expr, $obj:expr, $name:expr, $sig:expr, $args:tt $(,)?) => {{
        $self.vm.attach_current_thread(|env| {
            $crate::call_method!(env env, $obj, $name, $sig, $args)?.f()
        }).unwrap()
    }};
    (int $self:expr, $obj:expr, $name:expr, $sig:expr, $args:tt $(,)?) => {{
        $self.vm.attach_current_thread(|env| {
            $crate::call_method!(env env, $obj, $name, $sig, $args)?.i()
        }).unwrap()
    }};
    (bool $self:expr, $obj:expr, $name:expr, $sig:expr, $args:tt $(,)?) => {{
        $self.vm.attach_current_thread(|env| {
            $crate::call_method!(env env, $obj, $name, $sig, $args)?.z()
        }).unwrap()
    }};
    (env $env:expr, $obj:expr, $name:expr, $sig:expr, [] $(,)?) => {{
        let env: &mut $crate::jni::Env = $env;
        let obj = env.new_local_ref(&$obj).unwrap();
        env
            .call_method(
                &obj,
                $crate::jni::strings::JNIString::new($name),
                $crate::jni::signature::RuntimeMethodSignature::from_str($sig).unwrap().method_signature(),
                &[],
            )
    }};
    (env $env:expr, $obj:expr, $name:expr, $sig:expr, $args:expr $(,)?) => {{
        let env: &mut $crate::jni::Env = $env;
        let obj = env.new_local_ref(&$obj).unwrap();
        env
            .call_method(
                &obj,
                $crate::jni::strings::JNIString::new($name),
                $crate::jni::signature::RuntimeMethodSignature::from_str($sig).unwrap().method_signature(),
                &$args.into_iter().map(|v| v.into()).collect::<Vec<$crate::jni::JValue>>(),
            )
    }};
}

/// Get a field of an object.
#[macro_export]
macro_rules! get_field {
    (local_obj $env:expr, $obj:expr, $name:expr, $sig:expr $(,)?) => {{
        let env: &mut $crate::jni::Env = $env;
        $crate::get_field!(env env, $obj, $name, $sig).unwrap().l().unwrap()
    }};
    (obj $env:expr, $obj:expr, $name:expr, $sig:expr $(,)?) => {{
        let env: &mut $crate::jni::Env = $env;
        $crate::new_global!(env, $crate::get_field!(local_obj env, $obj, $name, $sig)).unwrap()
    }};
    (double $env:expr, $obj:expr, $name:expr $(,)?) => {{
        let env: &mut $crate::jni::Env = $env;
        $crate::get_field!(env env, $obj, $name, "D").unwrap().d().unwrap()
    }};
    (float $env:expr, $obj:expr, $name:expr $(,)?) => {{
        let env: &mut $crate::jni::Env = $env;
        $crate::get_field!(env env, $obj, $name, "F").unwrap().f().unwrap()
    }};
    (int $env:expr, $obj:expr, $name:expr $(,)?) => {{
        let env: &mut $crate::jni::Env = $env;
        $crate::get_field!(env env, $obj, $name, "I").unwrap().i().unwrap()
    }};
    (bool $env:expr, $obj:expr, $name:expr $(,)?) => {{
        let env: &mut $crate::jni::Env = $env;
        $crate::get_field!(env env, $obj, $name, "Z").unwrap().z().unwrap()
    }};
    (str $env:expr, $obj:expr, $name:expr $(,)?) => {{
        let env: &mut $crate::jni::Env = $env;
        let obj = $crate::get_field!(local_obj env, $obj, $name, "Ljava/lang/String;");
        $crate::jni::objects::JString::cast_local(env, obj).unwrap().try_to_string(env).unwrap()
    }};
    (env $env:expr, $obj:expr, $name:expr, $sig:expr $(,)?) => {{
        let env: &mut $crate::jni::Env = $env;
        let obj = env.new_local_ref(&$obj).unwrap();
        env
            .get_field(
                &obj,
                $crate::jni::strings::JNIString::new($name),
                $crate::jni::signature::RuntimeFieldSignature::from_str($sig).unwrap().field_signature(),
            )
    }};
}

/// Create a new string.
#[macro_export]
macro_rules! new_string {
    (env $env:expr, $val:expr) => {
        $env.new_string($val)
    };
    (vm $vm:expr, $val:expr) => {
        $vm.attach_current_thread(|env| $crate::new_string!(env env, $val))
            .unwrap()
    };
    ($self:expr, $val:expr) => {{
        let this = $self;

        $crate::new_string!(vm this.vm, $val)
    }};
}

/// Create a new global around an object.
#[macro_export]
macro_rules! new_global {
    ($env:expr, $obj:expr) => {
        {
            let obj = $obj;
            $env.new_global_ref(obj)
        }
    };
    (vm $vm:expr, $obj:expr) => {
        $vm.attach_current_thread(|env| $crate::new_global!(env, $obj)).unwrap()
    };
    (obj $self:expr, $obj:expr) => {
        {
            let this = $self;

            $crate::new_global!(vm this.vm, $obj)
        }
    };
}

/// Index into a `JList`.
#[macro_export]
macro_rules! index_jlist {
    (float $env:expr, $obj:expr; [$i:expr]) => {
        {
            let env: &mut $crate::jni::Env = $env;
            let jlist = $crate::index_jlist!(env env, $obj; [$i]);
            call_method!(
                env env,
                jlist,
                "floatValue",
                "()F",
                []
            ).unwrap().f().unwrap()
        }
    };
    (env $env:expr, $obj:expr; [$i:expr]) => {
        {
            $obj.get($env, $i).unwrap()
        }
    };
}

/// Define a new `JList`.
#[macro_export]
macro_rules! jlist {
    [float $env:expr; $($args:expr),* $(,)?] => {
        {
            let env: &mut $crate::jni::Env = $env;
            let class = env.find_class(JList::class_name()).unwrap();
            let obj = env.new_object(class, $crate::jni::jni_sig!("()Ljava/util/List;"), &[]).unwrap();
            let out = $crate::jni::objects::JList::cast_local(env, obj).unwrap();
            let class = env.find_class($crate::jni::jni_str!("Ljava/lang/Float;")).unwrap();
            $(
                let arg = env.new_object(&class, $crate::jni::jni_sig!("(F)Ljava/lang/Float;"), &[$args.into()]).unwrap();
                out.add(env, &arg).unwrap();
            )*
            out
        }
    };
    [env $env:expr; $($args:expr),* $(,)?] => {
        {
            let env: &mut $crate::jni::Env = $env;
            let class = env.find_class(JList::class_name()).unwrap();
            let obj = env.new_object(class, jni_sig!("()Ljava/util/List;"), &[]).unwrap();
            let out = $crate::jni::objects::JList::cast_local(env, obj).unwrap();
            $(
                out.add(env, $args.into()).unwrap();
            )*
            out
        }
    };
    [env $env:expr; from $val:expr] => {
        {
            let env: &mut $crate::jni::Env = $env;
            let class = env.find_class(JList::class_name()).unwrap();
            let obj = env.new_object(class, jni_sig!("()Ljava/util/List;"), &[]).unwrap();
            let out = $crate::jni::objects::JList::cast_local(env, obj).unwrap();
            for val in $val {
                let val = val.into_jni_object(env);
                out.add(env, &val).unwrap();
            }
            out
        }
    };
}
