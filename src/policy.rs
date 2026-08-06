//! JNI error policy that includes `PanicInfo` as a type here as well as `String`.

use jni::{Env, errors::ErrorPolicy};

use crate::{CURRENT_OPMODE_ID, CURRENT_PANIC_TEXT};

/// Version of the base JNI crate type that actually supports string types that
/// aren't &'static str.
#[derive(Debug, Default)]
pub struct ThrowRuntimeExAndDefault;

impl<T: Default, E: std::error::Error> ErrorPolicy<T, E> for ThrowRuntimeExAndDefault {
    type Captures<'unowned_env_local: 'native_method, 'native_method> = (); // no captures

    fn on_error<'unowned_env_local: 'native_method, 'native_method>(
        env: &mut Env<'unowned_env_local>,
        _cap: &mut Self::Captures<'unowned_env_local, 'native_method>,
        err: E,
    ) -> jni::errors::Result<T> {
        if env.exception_check() {
            return Ok(T::default()); // already thrown
        }
        let err_string = format!("Rust error: {err}");
        // Note: `env.throw()` will return `Err(Error::JavaException)` after throwing
        // but in this case (where we are going to be letting the exception
        // propagate to Java), we want to ensure we don't return that as an
        // error
        let _ = env.throw(err_string);
        Ok(T::default())
    }

    fn on_panic<'unowned_env_local: 'native_method, 'native_method>(
        env: &mut Env<'unowned_env_local>,
        _cap: &mut Self::Captures<'unowned_env_local, 'native_method>,
        _payload: Box<dyn std::any::Any + Send + 'static>,
    ) -> jni::errors::Result<T> {
        // Note: `env.throw()` will return `Err(Error::JavaException)` after throwing
        // but in this case (where we are going to be letting the exception
        // propagate to Java), we want to ensure we don't return that as an
        // error
        let _ = env.throw(format!("panic @ {:?}: {}", *CURRENT_OPMODE_ID.lock(), CURRENT_PANIC_TEXT.lock().take().unwrap()));
        Ok(T::default())
    }
}
