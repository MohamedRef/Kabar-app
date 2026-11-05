//! UI strings for the account and sign-in screens, typed like `crate::i18n` (a missing
//! translation is a compile error). Moved out of the shared `Msg` catalog so
//! this feature's changes touch only its own file.

use kabar_core::locale::UiLanguage;

crate::i18n::catalog! {
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountMsg {
    WelcomeHaveAccount,
    CloudIntro,
    CloudIntroWeb,
    CloudSigningIn,
    CloudSignOut,
    CloudSignedInAs,
    SyncPending,
    SyncLast,
    ProblemSignInServer,
    DevicesTitle,
    DeviceThis,
    DeviceNeverSynced,
    DeviceSignOut,
    DeviceSignedOut,
    TrySyncTitle,
    TrySyncSave,
    TrySyncSaved,
    JustNow,
    // Username + password sign-in (docs/CLOUD.md § Username and password).
    PwTitle,
    PwIntro,
    PwUsername,
    PwPassword,
    PwNotMine,
    PwNotMineHint,
    PwSignIn,
    PwChecking,
    PwYours,
    PwHow,
    PwNone,
    PwUsernameHint,
    PwNew,
    PwPasswordHint,
    PwAgain,
    PwSave,
    PwSaving,
    PwSaved,
    PwChange,
    PwRemove,
    PwCancel,
    PwTemporary,
}
}

impl crate::i18n::Text for AccountMsg {
    fn text(self, lang: UiLanguage) -> &'static str {
        text(lang, self)
    }
}

fn text(lang: UiLanguage, msg: AccountMsg) -> &'static str {
    use AccountMsg::*;
    use UiLanguage::*;
    match (msg, lang) {
        (WelcomeHaveAccount, ArEg) => "بتتعلّم مع Kabar قبل كده؟ ادخل على حسابك",
        (WelcomeHaveAccount, Ar) => "تتعلّم مع Kabar من قبل؟ سجّل الدخول إلى حسابك",
        (WelcomeHaveAccount, En) => "Already learning with Kabar? Sign in",
        (CloudIntro, ArEg) => "سجّل دخول بجوجل عشان تقدّمك يتزامن بين الموبايل والتابلت والماك.",
        (CloudIntro, Ar) => "سجّل الدخول بحساب Google لمزامنة تقدّمك بين الهاتف واللوحي والماك.",
        (CloudIntro, En) => "Sign in with Google so your progress syncs between your phone, tablet and Mac.",
        (CloudIntroWeb, ArEg) => {
            "ادخل باسم المستخدم والباسورد عشان تقدّمك يتزامن مع الموبايل والتابلت والماك."
        }
        (CloudIntroWeb, Ar) => "ادخل باسم المستخدم وكلمة المرور لمزامنة تقدّمك مع الهاتف واللوحي والماك.",
        (CloudIntroWeb, En) => {
            "Sign in with your username and password to sync with your phone, tablet and Mac."
        }
        (CloudSigningIn, ArEg) => "كمّل تسجيل الدخول في المتصفح، وبعدين ارجع هنا.",
        (CloudSigningIn, Ar) => "أكمل تسجيل الدخول في المتصفح، ثم عد إلى هنا.",
        (CloudSigningIn, En) => "Finish signing in in your browser, then come back here.",
        (CloudSignOut, ArEg) => "سجّل خروج من الجهاز ده",
        (CloudSignOut, Ar) => "تسجيل الخروج من هذا الجهاز",
        (CloudSignOut, En) => "Sign out on this device",
        (CloudSignedInAs, ArEg) => "متسجّل باسم",
        (CloudSignedInAs, Ar) => "مسجّل الدخول باسم",
        (CloudSignedInAs, En) => "Signed in as",
        (SyncPending, ArEg) => "تغييرات مستنية تتبعت",
        (SyncPending, Ar) => "تغييرات بانتظار الإرسال",
        (SyncPending, En) => "changes waiting to sync",
        (SyncLast, ArEg | Ar) => "آخر مزامنة",
        (SyncLast, En) => "Last sync",
        (ProblemSignInServer, ArEg) => "\u{2068}Kabar Cloud\u{2069} فيه مشكلة دلوقتي. جرّب تاني بعد شوية.",
        (ProblemSignInServer, Ar) => "يواجه \u{2068}Kabar Cloud\u{2069} مشكلة الآن. حاول مجددًا بعد قليل.",
        (ProblemSignInServer, En) => "Kabar Cloud is having trouble right now. Try again in a bit.",
        (DevicesTitle, ArEg | Ar) => "الأجهزة",
        (DevicesTitle, En) => "Devices",
        (DeviceThis, ArEg) => "الجهاز ده",
        (DeviceThis, Ar) => "هذا الجهاز",
        (DeviceThis, En) => "This device",
        (DeviceNeverSynced, ArEg) => "لسه ما اتزامنش",
        (DeviceNeverSynced, Ar) => "لم يُزامَن بعد",
        (DeviceNeverSynced, En) => "Not synced yet",
        (DeviceSignOut, ArEg) => "سجّل خروجه",
        (DeviceSignOut, Ar) => "تسجيل خروجه",
        (DeviceSignOut, En) => "Sign it out",
        (DeviceSignedOut, ArEg) => "متسجّل خروجه",
        (DeviceSignedOut, Ar) => "مسجَّل خروجه",
        (DeviceSignedOut, En) => "Signed out",
        (TrySyncTitle, ArEg | Ar) => "جرّب المزامنة",
        (TrySyncTitle, En) => "Try sync",
        (TrySyncSave, ArEg | Ar) => "احفظ",
        (TrySyncSave, En) => "Save",
        (TrySyncSaved, ArEg) => "الكلمات المحفوظة",
        (TrySyncSaved, Ar) => "الكلمات المحفوظة",
        (TrySyncSaved, En) => "Saved words",
        (JustNow, ArEg) => "دلوقتي",
        (JustNow, Ar) => "الآن",
        (JustNow, En) => "just now",
        (PwTitle, ArEg) => "الدخول باسم مستخدم وباسورد",
        (PwTitle, Ar) => "الدخول باسم مستخدم وكلمة مرور",
        (PwTitle, En) => "Username and password",
        (PwIntro, ArEg) => {
            "على جهاز مش بتاعك أو على الويب: ادخل باسم المستخدم والباسورد اللي ظبطتهم من جهازك، وهتلاقي تقدّمك كله وآخر حتة وقفت عندها."
        }
        (PwIntro, Ar) => {
            "على جهاز ليس لك أو على الويب: ادخل باسم المستخدم وكلمة المرور اللذين ضبطتهما من جهازك، وستجد تقدّمك كله وآخر موضع توقفت عنده."
        }
        (PwIntro, En) => {
            "On a device that isn't yours, or on the web: sign in with the username and password you set on your own device, and everything comes back, right where you left off."
        }
        (PwUsername, ArEg) => "اسم المستخدم",
        (PwUsername, Ar) => "اسم المستخدم",
        (PwUsername, En) => "Username",
        (PwPassword, ArEg) => "الباسورد",
        (PwPassword, Ar) => "كلمة المرور",
        (PwPassword, En) => "Password",
        (PwNotMine, ArEg) => "الجهاز ده مش بتاعي",
        (PwNotMine, Ar) => "هذا الجهاز ليس لي",
        (PwNotMine, En) => "This isn't my device",
        (PwNotMineHint, ArEg) => "هتخرج لوحدك أول ما تقفل التاب أو التطبيق.",
        (PwNotMineHint, Ar) => "ستخرج تلقائيًا عند إغلاق التبويب أو التطبيق.",
        (PwNotMineHint, En) => "You're signed out when the tab or app closes.",
        (PwSignIn, ArEg) => "ادخل",
        (PwSignIn, Ar) => "ادخل",
        (PwSignIn, En) => "Sign in",
        (PwChecking, ArEg) => "بيدخل…",
        (PwChecking, Ar) => "جارٍ الدخول…",
        (PwChecking, En) => "Signing in…",
        (PwYours, ArEg) => "بتدخل من أي جهاز بـ",
        (PwYours, Ar) => "تدخل من أي جهاز بـ",
        (PwYours, En) => "On any device, sign in as",
        (PwHow, ArEg) => {
            "على أي جهاز: افتح Kabar، ومن «الإعدادات › الحساب والمزامنة» اكتب اسم المستخدم والباسورد."
        }
        (PwHow, Ar) => {
            "على أي جهاز: افتح Kabar، ومن «الإعدادات › الحساب والمزامنة» اكتب اسم المستخدم وكلمة المرور."
        }
        (PwHow, En) => "On any device: open Kabar and type them under Settings › Account & sync.",
        (PwNone, ArEg) => {
            "اختار اسم مستخدم وباسورد عشان تدخل من أي جهاز أو من الويب من غير جيميل. حسابك هو هو، فكل حاجة بتتزامن."
        }
        (PwNone, Ar) => {
            "اختر اسم مستخدم وكلمة مرور لتدخل من أي جهاز أو من الويب دون Gmail. الحساب نفسه، فكل شيء يتزامن."
        }
        (PwNone, En) => {
            "Pick a username and password to sign in on any device or on the web without Gmail. It's the same account, so everything syncs."
        }
        (PwUsernameHint, ArEg) => "من ٣ لـ ٣٢: حروف إنجليزي صغيرة وأرقام و . _ -",
        (PwUsernameHint, Ar) => "من ٣ إلى ٣٢: حروف إنجليزية صغيرة وأرقام و . _ -",
        (PwUsernameHint, En) => "3 to 32 of a–z, 0–9 and . _ -",
        (PwNew, ArEg) => "الباسورد الجديد",
        (PwNew, Ar) => "كلمة المرور الجديدة",
        (PwNew, En) => "New password",
        (PwPasswordHint, ArEg) => "١٠ حروف على الأقل. جملة قصيرة تفتكرها أحسن من كلمة صعبة.",
        (PwPasswordHint, Ar) => "١٠ أحرف على الأقل. جملة قصيرة تتذكرها أفضل من كلمة صعبة.",
        (PwPasswordHint, En) => "At least 10 characters. A short phrase you'll remember beats a hard word.",
        (PwAgain, ArEg) => "اكتبه تاني",
        (PwAgain, Ar) => "أعد كتابتها",
        (PwAgain, En) => "Type it again",
        (PwSave, ArEg) => "احفظ",
        (PwSave, Ar) => "احفظ",
        (PwSave, En) => "Save",
        (PwSaving, ArEg) => "بيحفظ…",
        (PwSaving, Ar) => "جارٍ الحفظ…",
        (PwSaving, En) => "Saving…",
        (PwSaved, ArEg) => "اتحفظ. تقدر تدخل بيه من أي جهاز دلوقتي.",
        (PwSaved, Ar) => "تم الحفظ. يمكنك الدخول به من أي جهاز الآن.",
        (PwSaved, En) => "Saved. You can sign in with it on any device now.",
        (PwChange, ArEg) => "غيّر",
        (PwChange, Ar) => "غيّر",
        (PwChange, En) => "Change",
        (PwRemove, ArEg) => "اقفل الدخول بالباسورد",
        (PwRemove, Ar) => "أوقف الدخول بكلمة المرور",
        (PwRemove, En) => "Turn off password sign-in",
        (PwCancel, ArEg) => "إلغاء",
        (PwCancel, Ar) => "إلغاء",
        (PwCancel, En) => "Cancel",
        (PwTemporary, ArEg) => {
            "دخول مؤقت: هتخرج لوحدك لما تقفل التاب أو التطبيق. لو خلصت قبلها، دوس «سجّل خروج من الجهاز ده»."
        }
        (PwTemporary, Ar) => {
            "دخول مؤقت: ستخرج تلقائيًا عند إغلاق التبويب أو التطبيق. إن انتهيت قبل ذلك فاضغط «تسجيل الخروج من هذا الجهاز»."
        }
        (PwTemporary, En) => {
            "Temporary sign-in: you're signed out when the tab or app closes. Done sooner? Tap “Sign out on this device”."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_reads_right_in_every_language() {
        crate::i18n::checks::catalog(AccountMsg::ALL, text, |_| false);
    }
}
