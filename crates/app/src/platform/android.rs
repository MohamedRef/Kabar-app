//! Android bridge spike: reaches Java through JNI from Rust, using the
//! JavaVM and Activity that tao/wry register in `ndk_context`.
//!
//! M1 proves the path with framework classes only. Our own Kotlin classes
//! (sign-in, FCM, ML Kit) need the Activity's class loader; that lands in M2.

#![allow(unsafe_code)]

use jni::objects::{JObject, JString};
use jni::{Env, JavaVM, jni_sig, jni_str};

fn with_activity<T>(f: impl FnOnce(&mut Env, &JObject) -> jni::errors::Result<T>) -> Result<T, String> {
    let ctx = ndk_context::android_context();
    // SAFETY: tao initializes ndk_context with the process JavaVM and a global
    // reference to the Activity before any app code runs; both live for the
    // whole process.
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) };
    vm.attach_current_thread(|env| {
        // SAFETY: see above; the Activity is a valid global reference.
        let activity = unsafe { JObject::from_raw(env, ctx.context().cast()) };
        f(env, &activity)
    })
    .map_err(|e: jni::errors::Error| e.to_string())
}

/// `Build.MANUFACTURER + " " + Build.MODEL`, e.g. "samsung SM-S928B".
pub fn device_model() -> Result<String, String> {
    with_activity(|env, _| {
        let mut field = |name| -> jni::errors::Result<String> {
            let v =
                env.get_static_field(jni_str!("android/os/Build"), name, jni_sig!("Ljava/lang/String;"))?;
            let s = env.cast_local::<JString>(v.l()?)?;
            Ok(s.mutf8_chars(env)?.to_string())
        };
        let maker = field(jni_str!("MANUFACTURER"))?;
        let model = field(jni_str!("MODEL"))?;
        Ok(format!("{maker} {model}"))
    })
}

/// `Context.getFilesDir().getAbsolutePath()`: private storage for the database.
pub fn files_dir() -> Result<String, String> {
    with_activity(|env, activity| {
        let dir =
            env.call_method(activity, jni_str!("getFilesDir"), jni_sig!("()Ljava/io/File;"), &[])?.l()?;
        let path =
            env.call_method(&dir, jni_str!("getAbsolutePath"), jni_sig!("()Ljava/lang/String;"), &[])?.l()?;
        let path = env.cast_local::<JString>(path)?;
        Ok(path.mutf8_chars(env)?.to_string())
    })
}
