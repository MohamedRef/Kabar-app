//! Android's `android.speech.tts.TextToSpeech`, reached through JNI with the
//! JavaVM and Activity that tao/wry register in `ndk_context`.
//!
//! Adapted from kabar-app main: crates/app/src/platform/android.rs (the
//! text-to-speech section and `with_activity`). Changes for Batna: an English
//! voice (en-US, then en-GB, then any English), and `isSpeaking()` / `stop()`
//! so oral mode knows when a question has been read.
//!
//! One engine per process, created lazily with a null OnInitListener. It
//! binds to the TTS service asynchronously, so readiness is polled
//! (`getVoices()` is null until the service is connected) from a short-lived
//! background thread and the UI thread never waits on it.

#![allow(unsafe_code)]

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use jni::objects::JObject;
use jni::refs::Global;
use jni::{Env, JValue, JavaVM, jni_sig, jni_str};

use super::tts::SpeakError;

/// How long a fresh engine gets to connect before we call it unavailable.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
/// After a failed start, wait this long before trying a new engine.
const RETRY_AFTER: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(100);

/// `TextToSpeech.QUEUE_FLUSH`, `ERROR`, `LANG_MISSING_DATA`.
const QUEUE_FLUSH: i32 = 0;
const ERROR: i32 = -1;
const LANG_MISSING_DATA: i32 = -1;

pub fn with_activity<T>(f: impl FnOnce(&mut Env, &JObject) -> jni::errors::Result<T>) -> Result<T, String> {
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
    .map_err(java_error)
}

/// A readable message; Java exceptions arrive already caught and cleared.
fn java_error(e: jni::errors::Error) -> String {
    match e {
        jni::errors::Error::CaughtJavaException { name, msg, .. } => format!("{name}: {msg}"),
        other => other.to_string(),
    }
}

struct Utterance {
    text: String,
    rate: f32,
}

struct Tts {
    engine: Option<Global<JObject<'static>>>,
    created: Option<Instant>,
    failed: Option<Instant>,
    pending: Option<Utterance>,
    waiting: bool,
}

static TTS: Mutex<Tts> =
    Mutex::new(Tts { engine: None, created: None, failed: None, pending: None, waiting: false });

fn tts() -> MutexGuard<'static, Tts> {
    TTS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Speaks `text` with an English system voice. `Ok` means it was spoken, or
/// queued while the engine is still starting.
pub fn speak(text: &str, rate: f32) -> Result<(), SpeakError> {
    let mut guard = tts();
    let t = &mut *guard;
    if t.engine.is_none() {
        if t.failed.is_some_and(|at| at.elapsed() < RETRY_AFTER) {
            return Err(SpeakError::NoEngine);
        }
        match create_engine() {
            Ok(engine) => {
                t.engine = Some(engine);
                t.created = Some(Instant::now());
                t.failed = None;
            }
            Err(e) => {
                eprintln!("TextToSpeech could not start: {e}");
                t.failed = Some(Instant::now());
                return Err(SpeakError::NoEngine);
            }
        }
    }
    let utterance = Utterance { text: text.to_owned(), rate };
    let Some(engine) = t.engine.as_ref().map(|g| g.as_obj()) else {
        return Err(SpeakError::NoEngine);
    };
    match is_ready(engine) {
        Ok(true) => say(engine, &utterance),
        Ok(false) => {
            // Still binding to the service: keep only the latest request and
            // let a background thread speak it once the engine is up.
            t.pending = Some(utterance);
            if !t.waiting {
                t.waiting = true;
                std::thread::spawn(wait_until_ready);
            }
            Ok(())
        }
        Err(e) => Err(SpeakError::Failed(e)),
    }
}

/// Whether the engine is reading something now.
pub fn is_speaking() -> Result<bool, String> {
    let guard = tts();
    let Some(engine) = guard.engine.as_ref().map(|g| g.as_obj()) else {
        return Ok(false);
    };
    with_activity(|env, _| env.call_method(engine, jni_str!("isSpeaking"), jni_sig!("()Z"), &[])?.z())
}

/// Nothing queued: the engine is connected (or gave up) and has no
/// utterance waiting for it.
pub fn idle() -> bool {
    let t = tts();
    !t.waiting && t.pending.is_none()
}

/// Stops speaking and drops anything still queued.
pub fn stop() -> Result<(), String> {
    let mut guard = tts();
    guard.pending = None;
    let Some(engine) = guard.engine.as_ref().map(|g| g.as_obj()) else {
        return Ok(());
    };
    with_activity(|env, _| env.call_method(engine, jni_str!("stop"), jni_sig!("()I"), &[]).map(drop))
}

fn wait_until_ready() {
    loop {
        std::thread::sleep(POLL);
        let mut guard = tts();
        let t = &mut *guard;
        let Some(engine) = t.engine.as_ref().map(|g| g.as_obj()) else {
            t.waiting = false;
            return;
        };
        if is_ready(engine).unwrap_or(false) {
            if let Some(u) = t.pending.take()
                && let Err(e) = say(engine, &u)
            {
                eprintln!("TextToSpeech: {e:?}");
            }
            t.waiting = false;
            return;
        }
        if t.created.is_none_or(|at| at.elapsed() > CONNECT_TIMEOUT) {
            // No engine ever answered (none installed or all disabled).
            // Release this one and allow a fresh try later.
            eprintln!("TextToSpeech did not connect within {CONNECT_TIMEOUT:?}");
            if let Some(engine) = t.engine.take() {
                let _ = with_activity(|env, _| {
                    env.call_method(engine.as_obj(), jni_str!("shutdown"), jni_sig!("()V"), &[])?;
                    Ok(())
                });
            }
            t.pending = None;
            t.failed = Some(Instant::now());
            t.waiting = false;
            return;
        }
    }
}

/// `new TextToSpeech(activity.getApplicationContext(), null)`, held globally.
fn create_engine() -> Result<Global<JObject<'static>>, String> {
    with_activity(|env, activity| {
        let app = env
            .call_method(
                activity,
                jni_str!("getApplicationContext"),
                jni_sig!("()Landroid/content/Context;"),
                &[],
            )?
            .l()?;
        let listener = JObject::null();
        let engine = env.new_object(
            jni_str!("android/speech/tts/TextToSpeech"),
            jni_sig!("(Landroid/content/Context;Landroid/speech/tts/TextToSpeech$OnInitListener;)V"),
            &[JValue::Object(&app), JValue::Object(&listener)],
        )?;
        env.new_global_ref(engine)
    })
}

/// The service is connected once `getVoices()` stops returning null.
fn is_ready(engine: &JObject<'static>) -> Result<bool, String> {
    with_activity(|env, _| {
        let voices =
            env.call_method(engine, jni_str!("getVoices"), jni_sig!("()Ljava/util/Set;"), &[])?.l()?;
        Ok(!voices.is_null())
    })
}

/// `setLanguage(new Locale(language, country))`, returning a `LANG_*` code.
fn set_language(
    env: &mut Env,
    engine: &JObject<'static>,
    language: &str,
    country: &str,
) -> jni::errors::Result<i32> {
    let language = env.new_string(language)?;
    let country = env.new_string(country)?;
    let locale = env.new_object(
        jni_str!("java/util/Locale"),
        jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
        &[JValue::Object(&language), JValue::Object(&country)],
    )?;
    env.call_method(
        engine,
        jni_str!("setLanguage"),
        jni_sig!("(Ljava/util/Locale;)I"),
        &[JValue::Object(&locale)],
    )?
    .i()
}

/// Picks an English voice and speaks.
fn say(engine: &JObject<'static>, u: &Utterance) -> Result<(), SpeakError> {
    let outcome = with_activity(|env, _| {
        let mut code = -2;
        for (language, country) in [("en", "US"), ("en", "GB"), ("en", "")] {
            code = set_language(env, engine, language, country)?;
            if code >= 0 {
                break;
            }
        }
        if code < 0 {
            return Ok(Err(code));
        }
        env.call_method(engine, jni_str!("setSpeechRate"), jni_sig!("(F)I"), &[JValue::Float(u.rate)])?;
        let text = env.new_string(&u.text)?;
        let id = env.new_string("batna")?;
        let bundle = JObject::null();
        let spoken = env
            .call_method(
                engine,
                jni_str!("speak"),
                jni_sig!("(Ljava/lang/CharSequence;ILandroid/os/Bundle;Ljava/lang/String;)I"),
                &[
                    JValue::Object(&text),
                    JValue::Int(QUEUE_FLUSH),
                    JValue::Object(&bundle),
                    JValue::Object(&id),
                ],
            )?
            .i()?;
        Ok(Ok(spoken))
    })
    .map_err(SpeakError::Failed)?;
    match outcome {
        Ok(ERROR) => Err(SpeakError::Failed("TextToSpeech.speak returned ERROR".into())),
        Ok(_) => Ok(()),
        Err(LANG_MISSING_DATA) => Err(SpeakError::MissingVoice),
        Err(_) => Err(SpeakError::Unsupported),
    }
}
