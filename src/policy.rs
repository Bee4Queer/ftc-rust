//! JNI error policy that's actually reasonable.

use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};

use jni::{Env, errors::ErrorPolicy};
use log::info;

/// Version of the base JNI crate type that actually supports string types that aren't &'static str.
#[derive(Debug, Default)]
pub struct ThrowRuntimeExAndDefault;

/// Attempt to get a string for the provided panic payload
fn try_get_string_for_t<T: ToString + 'static>(
    payload: &(dyn Any + Send + 'static),
) -> Option<String> {
    payload.downcast_ref::<T>().map(ToString::to_string)
}

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
        // Note: `env.throw()` will return `Err(Error::JavaException)` after throwing but in this
        // case (where we are going to be letting the exception propagate to Java), we want
        // to ensure we don't return that as an error
        let _ = env.throw(err_string);
        Ok(T::default())
    }

    fn on_panic<'unowned_env_local: 'native_method, 'native_method>(
        env: &mut Env<'unowned_env_local>,
        _cap: &mut Self::Captures<'unowned_env_local, 'native_method>,
        payload: Box<dyn std::any::Any + Send + 'static>,
    ) -> jni::errors::Result<T> {
        info!("got here");
        // WHAT THE FUCK IS THE FUCKING TYPE OF THIS IT ISN'T A STRING LITERAL IT ISN'T AN ALLOC
        // STRING IT ISN'T FUCKING FORMATTING ARGUMENTS WHAT THE FUCK IS IT OMG I WANT TO STRANGLE
        // SOMEONE
        let panic_string = try_get_string_for_t::<&'static str>(&payload)
            .or_else(|| try_get_string_for_t::<String>(&payload))
            .or_else(|| try_get_string_for_t::<std::fmt::Arguments>(&payload)) // unlikely to actually show up but worth a shot
            .unwrap_or_else(|| {
                // Since it's possible that dropping a panic payload may itself panic,
                // we catch any panic and fallback to forgetting/leaking the payload.
                if let Err(drop_panic) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
                    log::error!("Panic while dropping panic payload: {drop_panic:?}");
                    std::mem::forget(drop_panic);
                }
                "non-string panic payload".to_string()
            });

        // Note: `env.throw()` will return `Err(Error::JavaException)` after throwing but in this
        // case (where we are going to be letting the exception propagate to Java), we want
        // to ensure we don't return that as an error
        let _ = env.throw(format!("Rust panic: {panic_string}"));
        Ok(T::default())
    }
}
