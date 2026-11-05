//! UI strings for the cost screen, typed like `crate::i18n` (a missing
//! translation is a compile error). Moved out of the shared `Msg` catalog so
//! this feature's changes touch only its own file.

use kabar_core::locale::UiLanguage;

crate::i18n::catalog! {
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// Names kept from the shared catalog they moved out of.
#[allow(clippy::enum_variant_names)]
pub enum CostMsg {
    CostThisMonth,
    CostOffline,
    CostLoading,
    CostServicesTitle,
    CostAtTime,
    CostTomorrowAt,
    CostOnDate,
    CostResets,
    CostNearDay,
    CostNearMonth,
    CostPausedDay,
    CostPausedMonth,
    CostOff,
    CostNotSetUp,
    CostPlatformTitle,
    CostClaudeTitle,
    CostClaudeOffBody,
    CostClaudeOnBody,
    CostClaudeTurnOn,
    CostConfirmTitle,
    CostConfirmBody,
    CostConfirmNo,
    CostConfirmYes,
    CostClaudeHowTo,
}
}

impl crate::i18n::Text for CostMsg {
    fn text(self, lang: UiLanguage) -> &'static str {
        text(lang, self)
    }
}

fn text(lang: UiLanguage, msg: CostMsg) -> &'static str {
    use CostMsg::*;
    use UiLanguage::*;
    match (msg, lang) {
        (CostThisMonth, ArEg) => "الشهر ده",
        (CostThisMonth, Ar) => "هذا الشهر",
        (CostThisMonth, En) => "this month",
        (CostOffline, ArEg) => {
            "\u{2068}Kabar Cloud\u{2069} مش متوصل دلوقتي، فمفيش عدادات نعرضها. بس برضه صفر: كل خدمة بتقف عند حدها المجاني."
        }
        (CostOffline, Ar) => {
            "\u{2068}Kabar Cloud\u{2069} غير متصل الآن، فلا توجد عدادات لعرضها. ومع ذلك فالتكلفة صفر: كل خدمة تتوقف عند حدها المجاني."
        }
        (CostOffline, En) => {
            "Kabar Cloud isn't connected right now, so there are no live counters to show. It's still $0: every service stops at its free limit."
        }
        (CostLoading, ArEg) => "بنشوف العدادات…",
        (CostLoading, Ar) => "جارٍ قراءة العدادات…",
        (CostLoading, En) => "Checking the counters…",
        (CostServicesTitle, ArEg | Ar) => "الحصص المجانية",
        (CostServicesTitle, En) => "Free allowances",
        (CostAtTime, ArEg | Ar) => "الساعة {time}",
        (CostAtTime, En) => "at {time}",
        (CostTomorrowAt, ArEg) => "بكرة الساعة {time}",
        (CostTomorrowAt, Ar) => "غدًا الساعة {time}",
        (CostTomorrowAt, En) => "tomorrow at {time}",
        (CostOnDate, ArEg | Ar) => "يوم {date}",
        (CostOnDate, En) => "on {date}",
        (CostResets, ArEg) => "بتتجدد {when}",
        (CostResets, Ar) => "تتجدد {when}",
        (CostResets, En) => "Resets {when}",
        (CostNearDay, ArEg) => {
            "الحصة المجانية بتاعة {service} النهارده قربت تخلص. لما توصل للحد \u{2068}Kabar\u{2069} هيوقفها بدل ما يدفع، وترجع {when}."
        }
        (CostNearDay, Ar) => {
            "اقتربت حصة {service} المجانية لليوم من النفاد. عند الحد سيوقفها \u{2068}Kabar\u{2069} بدل أن يدفع، وتعود {when}."
        }
        (CostNearDay, En) => {
            "{service}'s free allowance for today is almost used. At the limit Kabar pauses it rather than pay; it's back {when}."
        }
        (CostNearMonth, ArEg) => {
            "الحصة المجانية بتاعة {service} الشهر ده قربت تخلص. لما توصل للحد \u{2068}Kabar\u{2069} هيوقفها بدل ما يدفع، وترجع {when}."
        }
        (CostNearMonth, Ar) => {
            "اقتربت حصة {service} المجانية لهذا الشهر من النفاد. عند الحد سيوقفها \u{2068}Kabar\u{2069} بدل أن يدفع، وتعود {when}."
        }
        (CostNearMonth, En) => {
            "{service}'s free allowance for this month is almost used. At the limit Kabar pauses it rather than pay; it's back {when}."
        }
        (CostPausedDay, ArEg) => {
            "{service} خلّص حصته المجانية النهارده، فـ\u{2068}Kabar\u{2069} وقّفه بدل ما يدفع. هيرجع {when}."
        }
        (CostPausedDay, Ar) => {
            "استنفد {service} حصته المجانية لليوم، فأوقفه \u{2068}Kabar\u{2069} بدل أن يدفع. سيعود {when}."
        }
        (CostPausedDay, En) => {
            "{service} has used today's free allowance, so Kabar paused it rather than pay. It's back {when}."
        }
        (CostPausedMonth, ArEg) => {
            "{service} خلّص حصته المجانية الشهر ده، فـ\u{2068}Kabar\u{2069} وقّفه بدل ما يدفع. هيرجع {when}."
        }
        (CostPausedMonth, Ar) => {
            "استنفد {service} حصته المجانية لهذا الشهر، فأوقفه \u{2068}Kabar\u{2069} بدل أن يدفع. سيعود {when}."
        }
        (CostPausedMonth, En) => {
            "{service} has used this month's free allowance, so Kabar paused it rather than pay. It's back {when}."
        }
        (CostOff, ArEg) => "مقفول",
        (CostOff, Ar) => "متوقف",
        (CostOff, En) => "Off",
        (CostNotSetUp, ArEg) => "مش متظبط",
        (CostNotSetUp, Ar) => "غير مُعدّ",
        (CostNotSetUp, En) => "Not set up",
        (CostPlatformTitle, ArEg | Ar) => "\u{2068}Cloudflare\u{2069} (الخطة المجانية)",
        (CostPlatformTitle, En) => "Cloudflare (Free plan)",
        (CostClaudeTitle, ArEg) => "\u{2068}Claude\u{2069} (محرّك بفلوس)",
        (CostClaudeTitle, Ar) => "\u{2068}Claude\u{2069} (محرّك مدفوع)",
        (CostClaudeTitle, En) => "Claude (paid engine)",
        (CostClaudeOffBody, ArEg) => {
            "\u{2068}Claude\u{2069} مقفول. \u{2068}Kabar\u{2069} بيستعمل المحركات المجانية بس، إلا لو إنت شغّلته."
        }
        (CostClaudeOffBody, Ar) => {
            "\u{2068}Claude\u{2069} متوقف. يستخدم \u{2068}Kabar\u{2069} المحركات المجانية فقط ما لم تشغّله."
        }
        (CostClaudeOffBody, En) => "Claude is off. Kabar only uses free engines unless you turn it on.",
        (CostClaudeOnBody, ArEg) => {
            "\u{2068}Claude\u{2069} شغّال. بيبقى ببلاش طول ما رصيد \u{2068}Claude Max\u{2069} مغطّيه، وبيقف عند الحد الشهري."
        }
        (CostClaudeOnBody, Ar) => {
            "\u{2068}Claude\u{2069} يعمل. يكون مجانيًا ما دام رصيد \u{2068}Claude Max\u{2069} يغطيه، ويتوقف عند الحد الشهري."
        }
        (CostClaudeOnBody, En) => {
            "Claude is on. It's free while Claude Max credits cover it, and it stops at the monthly limit."
        }
        (CostClaudeTurnOn, ArEg) => "شغّل \u{2068}Claude\u{2069}…",
        (CostClaudeTurnOn, Ar) => "تشغيل \u{2068}Claude\u{2069}…",
        (CostClaudeTurnOn, En) => "Turn on Claude…",
        (CostConfirmTitle, ArEg) => "ده ممكن يكلّف فلوس",
        (CostConfirmTitle, Ar) => "قد يكلّف هذا مالًا",
        (CostConfirmTitle, En) => "This could cost money",
        (CostConfirmBody, ArEg) => {
            "\u{2068}Claude\u{2069} خدمة بفلوس. بيبقى ببلاش بس لو رصيد اشتراك \u{2068}Claude Max\u{2069} بتاعك مغطّيه؛ غير كده \u{2068}Anthropic\u{2069} هتحاسب الحساب اللي عليه المفتاح. \u{2068}Kabar\u{2069} برضه هيقف عند الحد الشهري. تحب تشغّله؟"
        }
        (CostConfirmBody, Ar) => {
            "\u{2068}Claude\u{2069} خدمة مدفوعة. تكون مجانية فقط إذا غطّاها رصيد اشتراك \u{2068}Claude Max\u{2069}؛ وإلا فستحاسب \u{2068}Anthropic\u{2069} الحساب المرتبط بالمفتاح. ومع ذلك سيتوقف \u{2068}Kabar\u{2069} عند الحد الشهري. هل تريد تشغيله؟"
        }
        (CostConfirmBody, En) => {
            "Claude is a paid service. It's free only if your Claude Max plan's credits cover it; otherwise Anthropic bills the account behind the API key. Kabar still stops at the monthly limit. Turn it on?"
        }
        (CostConfirmNo, ArEg) => "لأ، خلّيه مقفول",
        (CostConfirmNo, Ar) => "لا، أبقِه متوقفًا",
        (CostConfirmNo, En) => "No, keep it off",
        (CostConfirmYes, ArEg) => "أيوه، شغّله",
        (CostConfirmYes, Ar) => "نعم، شغّله",
        (CostConfirmYes, En) => "Yes, turn it on",
        (CostClaudeHowTo, ArEg) => {
            "عشان تشغّله: حط \u{2068}CLAUDE_ENABLED = \"true\"\u{2069} في \u{2068}wrangler.toml\u{2069} بتاع \u{2068}Kabar Cloud\u{2069} وشغّل \u{2068}just deploy-cloud\u{2069}. زرار التشغيل جوه التطبيق جاي مع شاشة محركات الذكاء الاصطناعي."
        }
        (CostClaudeHowTo, Ar) => {
            "لتشغيله: اضبط \u{2068}CLAUDE_ENABLED = \"true\"\u{2069} في ملف \u{2068}wrangler.toml\u{2069} الخاص بـ\u{2068}Kabar Cloud\u{2069} ثم شغّل \u{2068}just deploy-cloud\u{2069}. سيأتي زر التشغيل داخل التطبيق مع شاشة محركات الذكاء الاصطناعي."
        }
        (CostClaudeHowTo, En) => {
            "To switch it on, set CLAUDE_ENABLED = \"true\" in Kabar Cloud's wrangler.toml and run just deploy-cloud. An in-app switch comes with the AI engines screen."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_reads_right_in_every_language() {
        crate::i18n::checks::catalog(CostMsg::ALL, text, |_| false);
    }
}
