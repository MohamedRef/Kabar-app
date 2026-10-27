//! Oral mode with the screen off. On Android a foreground media service
//! (android/…/OralService.kt) keeps the app running, shows the question on
//! the lock screen with Again / Good, and takes headset buttons. Elsewhere
//! these calls do nothing: a desktop session simply keeps running.

/// A button pressed outside the app (notification, lock screen, headset).
#[allow(dead_code)] // built on Android only
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Remote {
    /// Play/pause: show the answer, or Good once it is shown.
    Main,
    Good,
    Again,
    End,
}

/// The labels the lock screen shows (UI text, from i18n).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub struct Labels {
    pub show: &'static str,
    pub again: &'static str,
    pub good: &'static str,
    pub end: &'static str,
}

/// Starts background mode when a session starts (call it while in the foreground).
pub fn start(labels: &Labels) {
    #[cfg(target_os = "android")]
    report(android::start(labels));
    #[cfg(not(target_os = "android"))]
    let _ = labels;
}

/// The current question, for the lock screen.
pub fn update(title: &str, text: &str, revealed: bool) {
    #[cfg(target_os = "android")]
    report(android::update(title, text, revealed));
    #[cfg(not(target_os = "android"))]
    let _ = (title, text, revealed);
}

pub fn stop() {
    #[cfg(target_os = "android")]
    report(android::stop());
}

/// The last button pressed since the previous call.
pub fn take() -> Option<Remote> {
    #[cfg(target_os = "android")]
    {
        match android::take() {
            Ok(code) => Some(match code {
                1 => Remote::Main,
                2 => Remote::Good,
                3 => Remote::Again,
                4 => Remote::End,
                _ => return None,
            }),
            Err(e) => {
                eprintln!("OralBridge.take: {e}");
                None
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    None
}

#[cfg(target_os = "android")]
fn report(result: Result<(), String>) {
    if let Err(e) = result {
        eprintln!("OralBridge: {e}");
    }
}

#[cfg(target_os = "android")]
mod android {
    use jni::objects::{JClass, JClassLoader, JObject};
    use jni::refs::LoaderContext;
    use jni::{Env, JValue, jni_sig, jni_str};

    use super::Labels;
    use crate::platform::tts_android::with_activity;

    /// Tells `dx` to build `android/` (repo root) into the APK: it copies the
    /// folder into its generated Gradle project as a library module and the
    /// manifest merger adds OralService. The macro only embeds that metadata
    /// in the binary; the JNI calls below are hand-written.
    /// (Adapted from kabar-app main: crates/app/src/platform/android.rs.)
    mod kotlin_module {
        #![allow(unused_imports)]

        #[manganis::ffi("../../android")]
        extern "Kotlin" {}
    }

    /// Runs `f` with `app.batna.OralBridge`, loaded through the Activity's
    /// class loader (`FindClass` from a native thread only sees system classes).
    fn bridge<T>(f: impl FnOnce(&mut Env, &JClass, &JObject) -> jni::errors::Result<T>) -> Result<T, String> {
        with_activity(|env, activity| {
            let loader = env
                .call_method(
                    activity,
                    jni_str!("getClassLoader"),
                    jni_sig!("()Ljava/lang/ClassLoader;"),
                    &[],
                )?
                .l()?;
            let loader = env.cast_local::<JClassLoader>(loader)?;
            let class =
                LoaderContext::Loader(&loader).load_class(env, jni_str!("app.batna.OralBridge"), true)?;
            f(env, &class, activity)
        })
    }

    pub fn start(labels: &Labels) -> Result<(), String> {
        bridge(|env, class, activity| {
            let show = env.new_string(labels.show)?;
            let again = env.new_string(labels.again)?;
            let good = env.new_string(labels.good)?;
            let end = env.new_string(labels.end)?;
            env.call_static_method(
                class,
                jni_str!("start"),
                jni_sig!(
                    "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V"
                ),
                &[
                    JValue::Object(activity),
                    JValue::Object(&show),
                    JValue::Object(&again),
                    JValue::Object(&good),
                    JValue::Object(&end),
                ],
            )
            .map(drop)
        })
    }

    pub fn update(title: &str, text: &str, revealed: bool) -> Result<(), String> {
        bridge(|env, class, _| {
            let title = env.new_string(title)?;
            let text = env.new_string(text)?;
            env.call_static_method(
                class,
                jni_str!("update"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;Z)V"),
                &[JValue::Object(&title), JValue::Object(&text), JValue::Bool(revealed)],
            )
            .map(drop)
        })
    }

    pub fn stop() -> Result<(), String> {
        bridge(|env, class, activity| {
            env.call_static_method(
                class,
                jni_str!("stop"),
                jni_sig!("(Landroid/content/Context;)V"),
                &[JValue::Object(activity)],
            )
            .map(drop)
        })
    }

    pub fn take() -> Result<i32, String> {
        bridge(|env, class, _| env.call_static_method(class, jni_str!("take"), jni_sig!("()I"), &[])?.i())
    }
}
