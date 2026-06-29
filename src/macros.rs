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
        $self.vm.attach_current_thread(|env| {$crate::call_method!(env env, $obj, $name, $sig, $args).unwrap(); Ok::<(), jni::errors::Error>(())}).unwrap();
    }};
    (obj $self:expr, $obj:expr, $name:expr, $sig:expr, $args:tt $(,)?) => {{
        $self.vm.attach_current_thread(|env| {
            let object = $crate::call_method!(env env, $obj, $name, $sig, $args).unwrap().l().unwrap();
            $crate::new_global!(env, object)
        }).unwrap()
    }};
    (double $self:expr, $obj:expr, $name:expr, $sig:expr, $args:tt $(,)?) => {{
        $self.vm.attach_current_thread(|env| {
            $crate::call_method!(env env, $obj, $name, $sig, $args).unwrap().d()
        }).unwrap()
    }};
    (float $self:expr, $obj:expr, $name:expr, $sig:expr, $args:tt $(,)?) => {{
        $self.vm.attach_current_thread(|env| {
            $crate::call_method!(env env, $obj, $name, $sig, $args).unwrap().f()
        }).unwrap()
    }};
    (int $self:expr, $obj:expr, $name:expr, $sig:expr, $args:tt $(,)?) => {{
        $self.vm.attach_current_thread(|env| {
            $crate::call_method!(env env, $obj, $name, $sig, $args).unwrap().i()
        }).unwrap()
    }};
    (bool $self:expr, $obj:expr, $name:expr, $sig:expr, $args:tt $(,)?) => {{
        $self.vm.attach_current_thread(|env| {
            $crate::call_method!(env env, $obj, $name, $sig, $args).unwrap().z()
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
