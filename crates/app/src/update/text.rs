//! Updates' own strings (Settings › About › Updates, the banner and the
//! "new version" notification).

use kabar_core::locale::UiLanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum U {
    Title,
    ThisBuild,
    Build,
    // why updates are off here
    OffDev,
    OffKey,
    OffPlatform,
    OffBundle,
    // where things stand
    UpToDate,
    Checking,
    CheckNow,
    NotForDevice,
    Available,
    Download,
    Downloading,
    Ready,
    InstallNow,
    InstallRestart,
    Installing,
    InstallingAndroid,
    NeedsPermission,
    OpenPermission,
    TryAgain,
    Installed,
    RestartNow,
    NextLaunch,
    FailedNetwork,
    FailedDownload,
    FailedVerify,
    FailedInstall,
    TooNew,
    KeyChanged,
    WaitingWifi,
    Later,
    // mode and channel
    Mode,
    ModeAuto,
    ModeAutoHint,
    ModeNotify,
    ModeNotifyHint,
    ModeOff,
    ModeOffHint,
    Channel,
    ChannelHint,
    Alpha,
    Beta,
    Stable,
    Preview,
    PreviewHint,
    Safety,
    Packs,
    WhatsNew,
    // the notification Android shows once the new Kabar is in
    DoneTitle,
    DoneBody,
    ReadyTitle,
    // the "new version" notification (Android, Mac) and its switch
    NoticeAvailable,
    NoticeReady,
    NoticeInstalled,
    NoticeTapAvailable,
    NoticeTapReady,
    NoticeTapInstalled,
    NoticeChannel,
    NotifyToggle,
    NotifyToggleHint,
    // the "what's new" sheet (sheet.rs)
    WhatsNewAsk,
    History,
    HistoryHint,
    HistoryEmpty,
    UpdatedTitle,
    UpdatedSub,
    KindNew,
    KindImproved,
    KindFixed,
    BadgeLatest,
    BadgeInstalled,
    BadgeNew,
    Today,
    Yesterday,
    TwoDaysAgo,
    DaysAgo,
    ShowAll,
    ShowLess,
    Earlier,
    ShowOlder,
    Close,
    GotIt,
    NoDetails,
}

pub fn ut(lang: UiLanguage, u: U) -> &'static str {
    use U::*;
    use UiLanguage::*;
    match (u, lang) {
        (Title, ArEg | Ar) => "التحديثات",
        (Title, En) => "Updates",
        (ThisBuild, ArEg) => "النسخة اللي معاك",
        (ThisBuild, Ar) => "الإصدار المثبّت",
        (ThisBuild, En) => "This build",
        (Build, ArEg | Ar) => "بناء",
        (Build, En) => "build",

        (OffDev, ArEg) => {
            "دي نسخة تطوير، فمش بتدوّر على تحديثات. النسخ اللي على الموبايل والتابلت والماك هي اللي بتتحدّث."
        }
        (OffDev, Ar) => {
            "هذا إصدار تطوير، فلا يبحث عن تحديثات. الإصدارات المثبّتة على الهاتف واللوحي والماك هي التي تتحدّث."
        }
        (OffDev, En) => {
            "This is a development build, so it doesn't look for updates. The builds on your phone, tablet and Mac do."
        }
        (OffKey, ArEg) => {
            "التحديث التلقائي هيشتغل أول ما مفتاح التحديثات يتعمل على الماك (خطوة واحدة يوم النسخة الأولى)."
        }
        (OffKey, Ar) => {
            "سيعمل التحديث التلقائي بمجرد إنشاء مفتاح التحديثات على الماك (خطوة واحدة يوم الإصدار الأول)."
        }
        (OffKey, En) => "Updates switch on once the update key is made on the Mac (one step on release day).",
        (OffPlatform, ArEg) => "مفيش نسخ منشورة للجهاز ده.",
        (OffPlatform, Ar) => "لا توجد إصدارات منشورة لهذا الجهاز.",
        (OffPlatform, En) => "No builds are published for this device.",
        (OffBundle, ArEg) => "Kabar شغال من فولدر البناء مش من Kabar.app، فمش هيقدر يبدّل نفسه.",
        (OffBundle, Ar) => "يعمل Kabar من مجلد البناء لا من Kabar.app، فلا يمكنه استبدال نفسه.",
        (OffBundle, En) => {
            "Kabar is running from a build folder, not from Kabar.app, so it can't replace itself."
        }

        (UpToDate, ArEg) => "معاك أحدث نسخة",
        (UpToDate, Ar) => "لديك أحدث إصدار",
        (UpToDate, En) => "You have the newest Kabar",
        (Checking, ArEg) => "بندوّر على تحديث…",
        (Checking, Ar) => "جارٍ البحث عن تحديث…",
        (Checking, En) => "Looking for an update…",
        (CheckNow, ArEg) => "دوّر على تحديث",
        (CheckNow, Ar) => "ابحث عن تحديث",
        (CheckNow, En) => "Check for updates",
        (NotForDevice, ArEg) => "فيه نسخة جديدة، بس لسه مش جاهزة للجهاز ده.",
        (NotForDevice, Ar) => "يوجد إصدار جديد، لكنه غير جاهز لهذا الجهاز بعد.",
        (NotForDevice, En) => "A new version is out, but not for this device yet.",
        (Available, ArEg) => "نزلت نسخة جديدة",
        (Available, Ar) => "صدر إصدار جديد",
        (Available, En) => "A new Kabar is out",
        (Download, ArEg) => "نزّلها",
        (Download, Ar) => "نزّله",
        (Download, En) => "Download",
        (Downloading, ArEg) => "بتنزل…",
        (Downloading, Ar) => "جارٍ التنزيل…",
        (Downloading, En) => "Downloading…",
        (Ready, ArEg) => "النسخة الجديدة جاهزة",
        (Ready, Ar) => "الإصدار الجديد جاهز",
        (Ready, En) => "The new Kabar is ready",
        (InstallNow, ArEg) => "ثبّتها دلوقتي",
        (InstallNow, Ar) => "ثبّته الآن",
        (InstallNow, En) => "Install now",
        (InstallRestart, ArEg) => "حدّث وافتح من جديد",
        (InstallRestart, Ar) => "حدّث وأعد التشغيل",
        (InstallRestart, En) => "Update and restart",
        (Installing, ArEg) => "بنثبّت…",
        (Installing, Ar) => "جارٍ التثبيت…",
        (Installing, En) => "Installing…",
        (InstallingAndroid, ArEg) => {
            "لو أندرويد سألك، دوس «تثبيت». Kabar هيقفل ثانية ويرجع بالنسخة الجديدة، وكل حاجة محفوظة."
        }
        (InstallingAndroid, Ar) => {
            "إن سألك أندرويد فاضغط «تثبيت». سيُغلق Kabar لحظة ويعود بالإصدار الجديد، وكل شيء محفوظ."
        }
        (InstallingAndroid, En) => {
            "If Android asks, tap Install. Kabar closes for a moment and comes back new; everything is saved."
        }
        (NeedsPermission, ArEg) => {
            "عشان Kabar يقدر يحدّث نفسه، شغّل «السماح من هذا المصدر» في الصفحة اللي فتحت، وارجع هنا."
        }
        (NeedsPermission, Ar) => {
            "ليتمكن Kabar من تحديث نفسه، فعّل «السماح من هذا المصدر» في الصفحة التي فُتحت ثم عُد إلى هنا."
        }
        (NeedsPermission, En) => {
            "To let Kabar update itself, turn on “Allow from this source” on the page that opened, then come back."
        }
        (OpenPermission, ArEg) => "افتح الإعداد",
        (OpenPermission, Ar) => "افتح الإعداد",
        (OpenPermission, En) => "Open the setting",
        (TryAgain, ArEg) => "جرّب تاني",
        (TryAgain, Ar) => "حاول مجددًا",
        (TryAgain, En) => "Try again",
        (Installed, ArEg) => "النسخة الجديدة اتحطّت مكان القديمة",
        (Installed, Ar) => "حلّ الإصدار الجديد محل القديم",
        (Installed, En) => "The new Kabar is in place",
        (RestartNow, ArEg) => "افتح من جديد دلوقتي",
        (RestartNow, Ar) => "أعد التشغيل الآن",
        (RestartNow, En) => "Restart now",
        (NextLaunch, ArEg) => "أو هتشتغل لوحدها المرة الجاية اللي تفتح فيها Kabar.",
        (NextLaunch, Ar) => "أو سيعمل تلقائيًا في المرة القادمة التي تفتح فيها Kabar.",
        (NextLaunch, En) => "Or it starts by itself the next time you open Kabar.",
        (FailedNetwork, ArEg) => "ماعرفناش نوصل لمكان التحديثات. Kabar هيجرّب تاني بعدين لوحده.",
        (FailedNetwork, Ar) => "تعذّر الوصول إلى مصدر التحديثات. سيحاول Kabar مجددًا لاحقًا.",
        (FailedNetwork, En) => "Couldn't reach the update server. Kabar will try again later by itself.",
        (FailedDownload, ArEg) => "التنزيل ماكملش. النسخة اللي معاك سليمة، جرّب تاني بعد شوية.",
        (FailedDownload, Ar) => "لم يكتمل التنزيل. نسختك الحالية سليمة، حاول مجددًا بعد قليل.",
        (FailedDownload, En) => "The download didn't finish. Your current Kabar is fine; try again in a bit.",
        (FailedVerify, ArEg) => "التحديث ده ماعدّاش فحص الأمان (التوقيع أو البصمة)، فـKabar ماثبّتوش.",
        (FailedVerify, Ar) => "لم يجتز هذا التحديث فحص الأمان (التوقيع أو البصمة)، فلم يثبّته Kabar.",
        (FailedVerify, En) => {
            "This update didn't pass Kabar's safety check (signature or checksum), so it wasn't installed."
        }
        (FailedInstall, ArEg) => "التثبيت ماكملش. نسختك الحالية سليمة زي ما هي.",
        (FailedInstall, Ar) => "لم يكتمل التثبيت. إصدارك الحالي سليم كما هو.",
        (FailedInstall, En) => "Installing didn't finish. Your current Kabar is untouched.",
        (TooNew, ArEg) => "التحديث ده محتاج تثبيت بإيدك مرة واحدة (من الماك)، وبعدها يرجع تلقائي.",
        (TooNew, Ar) => "يحتاج هذا التحديث إلى تثبيت يدوي مرة واحدة (من الماك)، ثم يعود تلقائيًا.",
        (TooNew, En) => {
            "This update needs one install by hand (from the Mac); after that it's automatic again."
        }
        (KeyChanged, ArEg) => {
            "مفتاح توقيع نسخ المعاينة اتغيّر، فالنسخة دي مش هتقدر تتأكد من الجديد. نزّل kabar-preview.apk بإيدك مرة واحدة، وبعدها يرجع تلقائي."
        }
        (KeyChanged, Ar) => {
            "تغيّر مفتاح توقيع إصدارات المعاينة، فلا يستطيع هذا الإصدار التحقق من الجديد. نزّل kabar-preview.apk يدويًا مرة واحدة، ثم يعود تلقائيًا."
        }
        (KeyChanged, En) => {
            "The preview signing key changed, so this build can't verify new ones. Install the newest kabar-preview.apk by hand once; after that it's automatic again."
        }
        (WaitingWifi, ArEg) => "هتنزل لوحدها أول ما تتوصل بواي فاي، أو نزّلها دلوقتي.",
        (WaitingWifi, Ar) => "سينزل وحده عند الاتصال بشبكة Wi-Fi، أو نزّله الآن.",
        (WaitingWifi, En) => "It downloads by itself on Wi-Fi, or download it now.",
        (Later, ArEg) => "بعدين",
        (Later, Ar) => "لاحقًا",
        (Later, En) => "Later",

        (Mode, ArEg) => "إزاي يحصل التحديث",
        (Mode, Ar) => "طريقة التحديث",
        (Mode, En) => "How updates happen",
        (ModeAuto, ArEg) => "تلقائي",
        (ModeAuto, Ar) => "تلقائي",
        (ModeAuto, En) => "Automatic",
        (ModeAutoHint, ArEg) => {
            "بينزل لوحده في الخلفية، ويتثبّت في وقت هادي: على الماك المرة الجاية اللي تفتحه، وعلى أندرويد وKabar في الخلفية ومش في نص درس."
        }
        (ModeAutoHint, Ar) => {
            "ينزل وحده في الخلفية ويُثبَّت في وقت هادئ: على الماك عند الفتح التالي، وعلى أندرويد حين يكون Kabar في الخلفية وليس في منتصف درس."
        }
        (ModeAutoHint, En) => {
            "Downloads in the background and installs at a calm moment: on the Mac at the next launch, on Android while Kabar is in the background and not mid-lesson."
        }
        (ModeNotify, ArEg) => "قولّي الأول",
        (ModeNotify, Ar) => "أخبرني أولًا",
        (ModeNotify, En) => "Tell me first",
        (ModeNotifyHint, ArEg) => "بيدوّر لوحده ويقولّك، وانت تختار إمتى تنزّل وتثبّت.",
        (ModeNotifyHint, Ar) => "يبحث وحده ويخبرك، وأنت تختار متى تنزّل وتثبّت.",
        (ModeNotifyHint, En) => "Looks by itself and tells you; you choose when to download and install.",
        (ModeOff, ArEg) => "بس لما أدوس",
        (ModeOff, Ar) => "فقط عندما أطلب",
        (ModeOff, En) => "Only when I check",
        (ModeOffHint, ArEg) => "مش هيدوّر لوحده خالص.",
        (ModeOffHint, Ar) => "لن يبحث وحده أبدًا.",
        (ModeOffHint, En) => "Never looks by itself.",
        (Channel, ArEg) => "القناة",
        (Channel, Ar) => "القناة",
        (Channel, En) => "Channel",
        (ChannelHint, ArEg) => {
            "ألفا بتاخد كل جديد أول بأول (وبيتا والنسخ الثابتة كمان). بيتا أهدى، والثابتة أهدى وأهدى."
        }
        (ChannelHint, Ar) => {
            "ألفا تأخذ كل جديد أولًا بأول (ومعها بيتا والإصدارات الثابتة). بيتا أهدأ، والثابتة أهدأ منهما."
        }
        (ChannelHint, En) => {
            "Alpha gets everything first (betas and stable releases too). Beta is calmer, Stable calmest."
        }
        (Alpha, ArEg | Ar) => "ألفا",
        (Alpha, En) => "Alpha",
        (Beta, ArEg | Ar) => "بيتا",
        (Beta, En) => "Beta",
        (Stable, ArEg) => "ثابتة",
        (Stable, Ar) => "مستقرة",
        (Stable, En) => "Stable",
        (Preview, ArEg | Ar) => "معاينة",
        (Preview, En) => "Preview",
        (PreviewHint, ArEg) => "دي نسخة معاينة من GitHub: بتتحدّث مع كل تعديل يدخل main.",
        (PreviewHint, Ar) => "هذا إصدار معاينة من GitHub: يتحدّث مع كل تعديل يدخل main.",
        (PreviewHint, En) => "A preview build from GitHub: it updates with every change merged into main.",
        (Safety, ArEg) => {
            "كل تحديث بيتأكد إنه متوقّع بمفتاحك وإن بصمته (SHA-256) مظبوطة قبل ما يتثبّت، وأندرويد كمان بيتأكد إنه بنفس مفتاح التوقيع."
        }
        (Safety, Ar) => {
            "يتحقق كل تحديث من توقيعه بمفتاحك ومن بصمته (SHA-256) قبل التثبيت، ويتحقق أندرويد أيضًا من مفتاح التوقيع نفسه."
        }
        (Safety, En) => {
            "Every update is checked against your signing key and its SHA-256 before it installs; Android also checks the APK's signing key."
        }
        (Packs, ArEg) => "باكات الصوت اللي نزّلتها بتتحدّث مع نفس الفحص.",
        (Packs, Ar) => "تتحدّث حزم الصوت المنزّلة مع الفحص نفسه.",
        (Packs, En) => "Audio packs you downloaded update with the same check.",
        (WhatsNew, ArEg) => "الجديد",
        (WhatsNew, Ar) => "الجديد",
        (WhatsNew, En) => "What's new",

        (DoneTitle, ArEg) => "Kabar اتحدّث",
        (DoneTitle, Ar) => "تحدّث Kabar",
        (DoneTitle, En) => "Kabar was updated",
        (DoneBody, ArEg) => "افتحه وكمّل من مكانك.",
        (DoneBody, Ar) => "افتحه وتابع من حيث توقفت.",
        (DoneBody, En) => "Open it and carry on where you left off.",
        (ReadyTitle, ArEg) => "تحديث Kabar جاهز: دوس عشان تثبّته",
        (ReadyTitle, Ar) => "تحديث Kabar جاهز: المس للتثبيت",
        (ReadyTitle, En) => "Kabar's update is ready: tap to install",

        (NoticeAvailable, ArEg) => "نزلت نسخة جديدة من Kabar",
        (NoticeAvailable, Ar) => "صدر إصدار جديد من Kabar",
        (NoticeAvailable, En) => "A new Kabar is out",
        (NoticeReady, ArEg) => "نسخة Kabar الجديدة جاهزة تتثبّت",
        (NoticeReady, Ar) => "إصدار Kabar الجديد جاهز للتثبيت",
        (NoticeReady, En) => "The new Kabar is ready to install",
        (NoticeInstalled, ArEg) => "نسخة Kabar الجديدة اتحطّت",
        (NoticeInstalled, Ar) => "ثُبّت إصدار Kabar الجديد",
        (NoticeInstalled, En) => "The new Kabar is in place",
        (NoticeTapAvailable, ArEg) => "دوس تشوف الجديد وتنزّلها.",
        (NoticeTapAvailable, Ar) => "المس لترى الجديد وتنزّله.",
        (NoticeTapAvailable, En) => "Tap to see what's new and download it.",
        (NoticeTapReady, ArEg) => "اتنزّلت واتأكدنا منها. دوس عشان تثبّتها.",
        (NoticeTapReady, Ar) => "نُزّل وتُحقّق منه. المس للتثبيت.",
        (NoticeTapReady, En) => "Downloaded and checked. Tap to install it.",
        (NoticeTapInstalled, ArEg) => "هتشتغل أول ما تفتح Kabar تاني.",
        (NoticeTapInstalled, Ar) => "سيعمل عند فتح Kabar مرة أخرى.",
        (NoticeTapInstalled, En) => "It starts the next time you open Kabar.",
        (NoticeChannel, ArEg) => "نسخ Kabar الجديدة",
        (NoticeChannel, Ar) => "إصدارات Kabar الجديدة",
        (NoticeChannel, En) => "New Kabar versions",
        (NotifyToggle, ArEg) => "ابعتلي إشعار أول ما تنزل نسخة جديدة",
        (NotifyToggle, Ar) => "أرسل إشعارًا عند صدور إصدار جديد",
        (NotifyToggle, En) => "Send a notification when a new version is out",
        (NotifyToggleHint, ArEg) => {
            "على أندرويد حتى لو Kabar مقفول (بيبص كل كام ساعة)، وعلى الماك وهو مفتوح، ومعاه الجديد في سطرين. مش بيرنّ في ساعات الهدوء، ومفتاح الإشعارات في الجهاز له الكلمة الأخيرة."
        }
        (NotifyToggleHint, Ar) => {
            "على أندرويد حتى لو كان Kabar مغلقًا (يتحقق كل بضع ساعات)، وعلى الماك وهو مفتوح، ومعه الجديد في سطرين. لا يرنّ في ساعات الهدوء، ومفتاح الإشعارات في الجهاز له الكلمة الأخيرة."
        }
        (NotifyToggleHint, En) => {
            "On Android even while Kabar is closed (it looks every few hours), on the Mac while it's open, with what's new in a line or two. Never during your quiet hours, and the device's own notification switch has the last word."
        }

        (WhatsNewAsk, ArEg) => "إيه الجديد؟",
        (WhatsNewAsk, Ar) => "ما الجديد؟",
        (WhatsNewAsk, En) => "What's new?",
        (History, ArEg | Ar) => "سجل التغييرات",
        (History, En) => "Changelog",
        (HistoryHint, ArEg) => "كل نسخة واللي اتغيّر فيها، من الأحدث للأقدم.",
        (HistoryHint, Ar) => "كل إصدار وما تغيّر فيه، من الأحدث إلى الأقدم.",
        (HistoryHint, En) => "Every build and what it changed, newest first.",
        (HistoryEmpty, ArEg) => "لسه مفيش تغييرات متسجّلة. هتظهر هنا مع أول تحديث.",
        (HistoryEmpty, Ar) => "لا توجد تغييرات مسجّلة بعد. ستظهر هنا مع أول تحديث.",
        (HistoryEmpty, En) => "Nothing recorded yet. Changes show up here with the next update.",
        (UpdatedTitle, ArEg) => "Kabar اتحدّث",
        (UpdatedTitle, Ar) => "تم تحديث Kabar",
        (UpdatedTitle, En) => "Kabar was updated",
        (UpdatedSub, ArEg) => "ده اللي اتغيّر من آخر مرة فتحته.",
        (UpdatedSub, Ar) => "هذا ما تغيّر منذ آخر مرة فتحته.",
        (UpdatedSub, En) => "Here's what changed since you last opened it.",
        (KindNew, ArEg | Ar) => "جديد",
        (KindNew, En) => "New",
        (KindImproved, ArEg) => "اتحسّن",
        (KindImproved, Ar) => "تحسينات",
        (KindImproved, En) => "Improved",
        (KindFixed, ArEg) => "اتصلّح",
        (KindFixed, Ar) => "إصلاحات",
        (KindFixed, En) => "Fixed",
        (BadgeLatest, ArEg | Ar) => "الأحدث",
        (BadgeLatest, En) => "Latest",
        (BadgeInstalled, ArEg) => "اللي معاك",
        (BadgeInstalled, Ar) => "المثبّت",
        (BadgeInstalled, En) => "Installed",
        (BadgeNew, ArEg | Ar) => "جديد عليك",
        (BadgeNew, En) => "Not installed yet",
        (Today, ArEg) => "النهارده",
        (Today, Ar) => "اليوم",
        (Today, En) => "Today",
        (Yesterday, ArEg) => "امبارح",
        (Yesterday, Ar) => "أمس",
        (Yesterday, En) => "Yesterday",
        (TwoDaysAgo, ArEg) => "من يومين",
        (TwoDaysAgo, Ar) => "منذ يومين",
        (TwoDaysAgo, En) => "2 days ago",
        (DaysAgo, ArEg) => "من {n} أيام",
        (DaysAgo, Ar) => "منذ {n} أيام",
        (DaysAgo, En) => "{n} days ago",
        (ShowAll, ArEg) => "اعرض الكل ({n})",
        (ShowAll, Ar) => "عرض الكل ({n})",
        (ShowAll, En) => "Show all ({n})",
        (ShowLess, ArEg) => "اعرض أقل",
        (ShowLess, Ar) => "عرض أقل",
        (ShowLess, En) => "Show less",
        (Earlier, ArEg) => "نسخ قبل كده",
        (Earlier, Ar) => "إصدارات سابقة",
        (Earlier, En) => "Earlier builds",
        (ShowOlder, ArEg) => "اعرض الأقدم",
        (ShowOlder, Ar) => "عرض الأقدم",
        (ShowOlder, En) => "Show older",
        (Close, ArEg) => "اقفل",
        (Close, Ar) => "إغلاق",
        (Close, En) => "Close",
        (GotIt, ArEg) => "تمام",
        (GotIt, Ar) => "حسنًا",
        (GotIt, En) => "Got it",
        (NoDetails, ArEg) => "النسخة دي مفيهاش تفاصيل مكتوبة.",
        (NoDetails, Ar) => "لا توجد تفاصيل مكتوبة لهذا الإصدار.",
        (NoDetails, En) => "This build came without details.",
    }
}

/// "and 3 more" after a short list of changes, with Arabic's dual and plural.
pub fn more_changes(lang: UiLanguage, n: usize) -> String {
    match lang {
        UiLanguage::ArEg => match n {
            1 => "وحاجة كمان".to_owned(),
            2 => "وحاجتين كمان".to_owned(),
            3..=10 => format!("و{n} حاجات كمان"),
            _ => format!("و{n} حاجة كمان"),
        },
        UiLanguage::Ar => match n {
            1 => "وتغيير آخر".to_owned(),
            2 => "وتغييران آخران".to_owned(),
            3..=10 => format!("و{n} تغييرات أخرى"),
            _ => format!("و{n} تغييرًا آخر"),
        },
        UiLanguage::En => format!("and {n} more"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn more_changes_counts_like_arabic_does() {
        let eg: Vec<String> = [1, 2, 3, 11].map(|n| more_changes(UiLanguage::ArEg, n)).into();
        assert_eq!(eg, ["وحاجة كمان", "وحاجتين كمان", "و3 حاجات كمان", "و11 حاجة كمان"]);
        assert_eq!(more_changes(UiLanguage::Ar, 2), "وتغييران آخران");
        assert_eq!(more_changes(UiLanguage::En, 1), "and 1 more");
    }
}
