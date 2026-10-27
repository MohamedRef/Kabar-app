//! Arabic-script pronunciation (تعريب النطق) under Indonesian and Javanese
//! text, and the guide that explains its letters. The transliteration itself
//! is `kabar_core::pron`.
//!
//! Shown where the learner meets a word (slides, dialogue, new words, answer
//! feedback, the back of review cards, the Reader), never on a question that
//! the Latin text alone should answer.

use dioxus::prelude::*;
use kabar_core::lang::Lang;
use kabar_core::locale::UiLanguage;
use kabar_core::pron::PronDict;
use kabar_core::settings::PronMode;

use crate::Ui;
use crate::i18n::Msg;

/// Noto Naskh Arabic, cut to the Arabic block: its marks sit well on
/// fully vowelled text and it has every extra letter (ڠ ێ ۆ پ چ), which
/// not every system Arabic font does.
const FONT: &[u8] = include_bytes!("../../../content/fonts/NotoNaskhArabic-Kabar.ttf");

/// The `@font-face` for `.pron`, once per app.
pub fn font_css() -> String {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(FONT);
    format!(
        "@font-face {{ font-family: \"Kabar Naskh\"; src: url(data:font/ttf;base64,{b64}) format(\"truetype\"); }}"
    )
}

/// `lang` attribute of the target text ("id", "jv") → the rules to use.
pub fn lang_of(tag: &str) -> Lang {
    if tag == "jv" { Lang::Javanese } else { Lang::Indonesian }
}

/// The pronunciation of `text` (a word or a sentence in `tag`), following
/// the setting: always shown, behind a tap, or not at all.
#[component]
pub fn Pron(text: String, tag: String, #[props(default)] inline: bool) -> Element {
    let ui = use_context::<Ui>();
    let mode = ui.settings.read().pronunciation;
    let mut revealed = use_signal(|| false);
    let arabic = use_memo(use_reactive!(|(text, tag)| PronDict::builtin().text(text.trim(), lang_of(&tag))));
    if mode == PronMode::Off || arabic.read().is_empty() {
        return rsx! {};
    }
    let class = if inline { "pron inline" } else { "pron" };
    if mode == PronMode::Tap && !revealed() {
        // A span, not a button: it often sits inside a button (examples, dialogue).
        return rsx! {
            span {
                class: "{class} pron-reveal",
                role: "button",
                tabindex: "0",
                onclick: move |e| {
                    e.stop_propagation();
                    revealed.set(true);
                },
                onkeydown: move |e: KeyboardEvent| {
                    if e.key() == Key::Enter {
                        e.stop_propagation();
                        revealed.set(true);
                    }
                },
                "{ui.t(Msg::PronShow)}"
            }
        };
    }
    rsx! {
        span { class: "{class}", lang: "ar", dir: "rtl", title: ui.t(Msg::PronTitle), "{arabic}" }
    }
}

/// One row of the guide: the Arabic sign, the Latin spelling it stands for,
/// an example word, and how to say it.
struct Row {
    sign: &'static str,
    latin: &'static str,
    example: &'static str,
    lang: Lang,
    ar_eg: &'static str,
    ar: &'static str,
    en: &'static str,
}

const ROWS: &[Row] = &[
    Row {
        sign: "بَا بِي بُو",
        latin: "a i u",
        example: "kita",
        lang: Lang::Indonesian,
        ar_eg: "الحركة والحرف مع بعض بيقولولك أنهي صوت، مش إنه ممدود: الحروف المتحركة في الإندونيسي كلها قصيرة.",
        ar: "الحركة مع الحرف تدل على نوع الصوت لا على المدّ، فحروف العلة الإندونيسية كلها قصيرة.",
        en: "Mark plus letter names the vowel, not a long one: Indonesian vowels are all short.",
    },
    Row {
        sign: "بِ",
        latin: "e",
        example: "besar",
        lang: Lang::Indonesian,
        ar_eg: "كسرة لوحدها من غير ي: دي الـ e الخفيفة المكتومة، زي الـ e في the الإنجليزي. بتتقال بسرعة.",
        ar: "كسرة وحدها بلا ياء: هي e الخفيفة المهموسة، كصوت e في the الإنجليزية.",
        en: "A kasra with no letter after it is the weak e (schwa), like the e in English \"the\".",
    },
    Row {
        sign: "بێ",
        latin: "e",
        example: "sore",
        lang: Lang::Indonesian,
        ar_eg: "e مفتوحة، زي «بيت» و«ليه» بالمصري.",
        ar: "e مفتوحة، كالياء الممالة في نطق «بيت» بالعامية المصرية.",
        en: "The open e, like the vowel in Egyptian «بيت» (bēt).",
    },
    Row {
        sign: "بۆ",
        latin: "o",
        example: "toko",
        lang: Lang::Indonesian,
        ar_eg: "o، زي «يوم» و«لون» بالمصري.",
        ar: "o، كالواو الممالة في نطق «يوم» بالعامية المصرية.",
        en: "o, like the vowel in Egyptian «يوم» (yōm).",
    },
    Row {
        sign: "پ",
        latin: "p",
        example: "pagi",
        lang: Lang::Indonesian,
        ar_eg: "p زي الإنجليزي: باء من غير ما تهزّ صوتك.",
        ar: "p كما في الإنجليزية: باء مهموسة.",
        en: "p as in English.",
    },
    Row {
        sign: "ج",
        latin: "g",
        example: "gula",
        lang: Lang::Indonesian,
        ar_eg: "الجيم هنا مصرية بتاعتنا (g)، زي «جمل» بالمصري.",
        ar: "الجيم هنا قاهرية (g) كما في «جمل» بالعامية المصرية.",
        en: "ج is read the Egyptian way: a hard g.",
    },
    Row {
        sign: "چ",
        latin: "j",
        example: "jalan",
        lang: Lang::Indonesian,
        ar_eg: "جيم معطّشة زي الفصحى (dj)، زي «چاكيت». الـ j في الإندونيسي عمرها ما بتبقى g.",
        ar: "الجيم المعطّشة الفصيحة (dj)، وتكتب چ لتمييزها عن g.",
        en: "j as in \"jam\" (the Classical ج), written چ to keep it apart from g.",
    },
    Row {
        sign: "تْش",
        latin: "c",
        example: "cari",
        lang: Lang::Indonesian,
        ar_eg: "زي «تشاي». حرف c في الإندونيسي دايمًا كده.",
        ar: "كصوت «تش»؛ وحرف c في الإندونيسية ينطق هكذا دائمًا.",
        en: "ch as in \"chai\". Indonesian c always sounds like this.",
    },
    Row {
        sign: "ڠ",
        latin: "ng",
        example: "orang",
        lang: Lang::Indonesian,
        ar_eg: "النون اللي في «بنك» قبل الكاف بالظبط، بس من غير الكاف. ng مش n وبعدها g.",
        ar: "النون الأنفية التي تسبق الكاف في «بنك»، بلا كاف بعدها.",
        en: "ng as in \"sing\", with no g after it.",
    },
    Row {
        sign: "نْي",
        latin: "ny",
        example: "nyanyi",
        lang: Lang::Indonesian,
        ar_eg: "نون ساكنة وبعدها ياء في نَفَس واحد، زي ñ في الإسباني. ny صوت واحد.",
        ar: "نون وياء في صوت واحد، كـ ñ الإسبانية.",
        en: "ny as one sound, like Spanish ñ.",
    },
    Row {
        sign: "ءْ",
        latin: "k",
        example: "tidak",
        lang: Lang::Indonesian,
        ar_eg: "الـ k في آخر الكلمة بتتقطع زي الهمزة في «لأ» بالمصري، من غير كاف.",
        ar: "تنطق k في آخر المقطع همزةً ساكنة، كما في «لأ».",
        en: "A final k is a catch in the throat, like the stop in \"uh-oh\", not a k.",
    },
    Row {
        sign: "ض ط",
        latin: "dh th",
        example: "dhahar",
        lang: Lang::Javanese,
        ar_eg: "في الجاوي بس: طرف لسانك يرجع لورا شوية لسقف بُقّك. صوت تقيل، بس من غير تفخيم العربي.",
        ar: "في الجاوية فقط: يرتد طرف اللسان إلى سقف الحلق، صوت ثقيل بلا تفخيم عربي كامل.",
        en: "Javanese only: d and t with the tongue tip curled back. Heavy, but not a full Arabic emphatic.",
    },
    Row {
        sign: "بۆ",
        latin: "a",
        example: "apa",
        lang: Lang::Javanese,
        ar_eg: "في الجاوي، الـ a في آخر الكلمة (واللي قبلها) بتتقال o مفتوحة.",
        ar: "في الجاوية تنطق a في آخر الكلمة (وما قبلها) o مفتوحة.",
        en: "In Javanese, a final a (and the a before it) sounds like an open o.",
    },
];

fn pick(lang: UiLanguage, r: &Row) -> &'static str {
    match lang {
        UiLanguage::ArEg => r.ar_eg,
        UiLanguage::Ar => r.ar,
        UiLanguage::En => r.en,
    }
}

/// How to read the pronunciation: one row per sign that isn't plain Arabic.
#[component]
pub fn PronGuide() -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let dict = PronDict::builtin();
    rsx! {
        div { class: "pron-guide",
            p { "{ui.t(Msg::PronGuideIntro)}" }
            table {
                tbody {
                    for r in ROWS {
                        tr { key: "{r.sign}{r.latin}",
                            td { class: "pron-sign", lang: "ar", "{r.sign}" }
                            td {
                                bdi { class: "latin", dir: "ltr", "{r.latin}" }
                            }
                            td { class: "pron-example",
                                bdi { class: "latin", dir: "ltr", lang: if r.lang == Lang::Javanese { "jv" } else { "id" }, "{r.example}" }
                                " "
                                span { class: "pron", lang: "ar", dir: "rtl", "{dict.word(r.example, r.lang)}" }
                            }
                            td { "{pick(lang, r)}" }
                        }
                    }
                }
            }
            p { class: "muted small", "{ui.t(Msg::PronGuideNote)}" }
        }
    }
}
