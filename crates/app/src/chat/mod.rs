//! Persona chats (SPEC §15, PLAN §16 C): pick Mas Dimas, Bu Siti, Mbak Gadis
//! or Mas Ardi and a scene, then talk. Each turn is a `roleplay_turn` through
//! the AI router; corrections show under the learner's message, every word
//! of a reply opens the Reader's word card, every reply plays in the
//! persona's Azure voice, and a whole chat replays as a dialogue. Chats sync
//! (`conversations`, one row per conversation and per message).

mod text;

use std::sync::LazyLock;

/// Handwritten replies (the ✍ toggle). Paused with the other writing features.
const HANDWRITING: bool = false;

use dioxus::prelude::*;
use kabar_core::ai::FriendlyReason;
use kabar_core::ai::tasks::{RoleplayTurn, TranslateText};
use kabar_core::chat::{
    Scenario, chat_level, is_javanese, persona_voice, registers, replay, reply_idea, request, scenarios,
};
use kabar_core::lang::Lang as CoreLang;
use kabar_core::lesson::pick;
use kabar_core::locale::UiLanguage;
use kabar_core::reader::translate;
use kabar_core::settings::TranslationMode;
use kabar_core::story::parse_persona;
use kabar_protocol::ai::Task;
use kabar_protocol::chat::{ChatCorrection, ChatMessage, ChatRole, ChatRow, Conversation, MAX_MESSAGE_CHARS};
use kabar_protocol::coach::{ActivityKind, LanguageFocus, Skill};
use kabar_protocol::content::Lang;
use kabar_protocol::story::Persona;

use self::text::{T, tt};
use crate::annotated::Annotated;
use crate::coach::Coach;
use crate::coach::labels::level_name;
use crate::embedded::{self, PERSONAS};
use crate::embedded::{Css, css};
use crate::lesson::{Rich, target_tag};
use crate::listen::player::ProgramPlayer;
use crate::portraits::{self, Face, Mood};
use crate::reader::translate::in_voice;
use crate::store::now_ms;
use crate::studio::pad::{InkAnswer, InkPad};
use crate::teach::Ai;
use crate::{Db, Screen, Ui, icons};

const CHAT_CSS: Css = css!("assets/chat.css");

/// Every persona file, parsed once, Indonesian partners first and each
/// language by level; a broken file stays listed with its error.
static PERSONA_LIST: LazyLock<Vec<Result<Persona, (String, String)>>> = LazyLock::new(|| {
    const ORDER: [&str; 10] = [
        "mbak-gadis",
        "mas-ardi",
        "bu-lestari",
        "bang-togar",
        "kaka-yohana",
        "mas-dimas",
        "bu-siti",
        "mbah-putri",
        "mbak-ayu",
        "pak-slamet",
    ];
    let mut all: Vec<_> = embedded::all(PERSONAS)
        .map(|(id, yaml)| parse_persona(&yaml).map_err(|e| (id.to_string(), e.to_string())))
        .collect();
    all.sort_by_key(|p| {
        let id = match p {
            Ok(p) => p.id.as_str(),
            Err((id, _)) => id.as_str(),
        };
        ORDER.iter().position(|o| id.contains(o)).unwrap_or(ORDER.len())
    });
    all
});

/// The partners in the learner's focus language first (the list order
/// when the focus is both), so a learner of one language isn't offered the
/// other one first.
fn personas_for(focus: LanguageFocus) -> Vec<&'static Result<Persona, (String, String)>> {
    let javanese_first = focus == LanguageFocus::Javanese;
    let mut all: Vec<_> = PERSONA_LIST.iter().collect();
    all.sort_by_key(|p| p.as_ref().map_or(true, |p| is_javanese(p) != javanese_first));
    all
}

fn persona(id: &str) -> Option<&'static Persona> {
    PERSONA_LIST.iter().flatten().find(|p| p.id == id)
}

/// The scene a conversation was started in (a free chat when its example
/// dialogue is gone).
fn scene_of(p: &Persona, c: &Conversation) -> Scenario {
    let all = scenarios(p);
    all.iter()
        .find(|s| s.id.is_some() && s.id == c.scenario)
        .or_else(|| all.iter().find(|s| s.id.is_none() && s.level == c.level))
        .or_else(|| all.first())
        .cloned()
        .unwrap_or_else(|| Scenario {
            id: None,
            level: c.level.clone(),
            situation: p.role.clone(),
            opener: String::new(),
            opener_meaning: Default::default(),
        })
}

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

fn save(db: &Db, row: ChatRow) {
    if let Err(e) = db.0.borrow_mut().save_chat_row(&row) {
        crate::diag::report!("saving the chat failed: {e}");
    }
}

fn register_label(r: &str) -> T {
    match r {
        "ngoko" => T::Ngoko,
        "krama" => T::Krama,
        "casual" => T::Casual,
        _ => T::Standard,
    }
}

/// The correction's reason in the UI's language.
fn why(c: &ChatCorrection, lang: UiLanguage) -> &str {
    if lang == UiLanguage::En { &c.why_en } else { &c.why_ar }
}

fn friendly(r: FriendlyReason) -> T {
    match r {
        FriendlyReason::ResetsLater => T::ResetsLater,
        FriendlyReason::Unreachable => T::Unreachable,
        FriendlyReason::NotAvailable => T::NotConnected,
        FriendlyReason::TryAgain => T::TryAgain,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum View {
    Home,
    Scenes(String),
    Chat(String),
    Replay(String),
}

#[component]
pub fn ChatScreen() -> Element {
    use_hook(crate::reader::warm_up);
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let mut view = use_signal(|| View::Home);

    let start_db = db.clone();
    let mut start = move |(p, s): (&'static Persona, Scenario)| {
        let db = &start_db;
        let (id_level, jv_level) = crate::journey::learner_levels(db, coach.profile.peek().as_ref());
        let level = chat_level(p, &id_level, jv_level.as_deref());
        let now = now_ms();
        let id = new_id();
        let conversation = Conversation {
            id: id.clone(),
            doc: id.clone(),
            persona: p.id.clone(),
            scenario: s.id.clone(),
            level,
            register: registers(p)[0].to_string(),
            created_ms: now,
            updated_ms: now,
        };
        save(db, ChatRow::Conversation(conversation));
        let opener = ChatMessage {
            id: new_id(),
            doc: id.clone(),
            role: ChatRole::Persona,
            text: s.opener.clone(),
            corrections: vec![],
            register: None,
            unverified: is_javanese(p) && !p.review.verified,
            created_ms: now,
        };
        save(db, ChatRow::Message(opener));
        view.set(View::Chat(id));
    };

    let body = match view() {
        View::Home => rsx! {
            Home { on_persona: move |id| view.set(View::Scenes(id)), on_chat: move |id| view.set(View::Chat(id)) }
        },
        View::Scenes(pid) => match persona(&pid) {
            Some(p) => rsx! {
                Scenes { persona: p.clone(), on_back: move |_| view.set(View::Home), on_pick: move |s| start((p, s)) }
            },
            None => rsx! {},
        },
        View::Chat(cid) => rsx! {
            Chat {
                key: "{cid}",
                id: cid.clone(),
                on_back: move |_| view.set(View::Home),
                on_replay: move |id| view.set(View::Replay(id)),
                on_persona: move |id| view.set(View::Scenes(id)),
            }
        },
        View::Replay(cid) => {
            let conv = db.0.borrow().conversations().unwrap_or_default().into_iter().find(|c| c.id == cid);
            match conv.as_ref().and_then(|c| persona(&c.persona).map(|p| (c, p))) {
                Some((c, p)) => {
                    let messages = db.0.borrow().chat_messages(&cid).unwrap_or_default();
                    let you = tt(ui.lang(), T::You);
                    let program = replay(p, &cid, &scene_of(p, c).situation, &messages, you);
                    let back = cid.clone();
                    rsx! {
                        ProgramPlayer {
                            key: "replay-{cid}",
                            program,
                            title: p.name.clone(),
                            downloadable: false,
                            on_back: move |_| view.set(View::Chat(back.clone())),
                            on_end: move |_| {},
                        }
                    }
                }
                None => rsx! {},
            }
        }
    };

    rsx! {
        style { {CHAT_CSS.text()} }
        style { {crate::listen::LISTEN_CSS.text()} }
        div { class: "chat", {body} }
    }
}

/// The partner's portrait (`assets/portraits.yaml`), or their initial until
/// Codex has drawn them.
#[component]
fn Avatar(name: String, javanese: bool, #[props(default)] mood: Mood) -> Element {
    let class = if javanese { "ch-avatar jv" } else { "ch-avatar" };
    if let Some(src) = portraits::of(&name, mood) {
        return rsx! { Face { src, class } };
    }
    let initial: String = name.split_whitespace().last().and_then(|w| w.chars().next()).into_iter().collect();
    rsx! {
        span { class, aria_hidden: "true", "{initial}" }
    }
}

/// How the partner looks at you: thinking while they write, explaining when
/// your last message got a fix, pleased when it didn't, listening otherwise.
fn mood_of(busy: bool, all: &[ChatMessage]) -> Mood {
    if busy {
        return Mood::Thinking;
    }
    let mine = all.iter().rev().find(|m| m.role == ChatRole::Learner);
    match (all.last().map(|m| m.role), mine) {
        (Some(ChatRole::Persona), Some(m)) if !m.corrections.is_empty() => Mood::Explaining,
        (Some(ChatRole::Persona), Some(_)) => Mood::Laughing,
        _ => Mood::Listening,
    }
}

/// Past chats: rows by default, newest first or by partner.
const HISTORY_VIEW: crate::views::Collection = crate::views::Collection {
    key: "chat.history",
    layout: crate::views::Layout::List,
    sorts: &[crate::views::Sort::Recent, crate::views::Sort::Name],
};
/// Partners: rows by default, in the course's order or by name.
const PARTNERS_VIEW: crate::views::Collection = crate::views::Collection {
    key: "chat.partners",
    layout: crate::views::Layout::List,
    sorts: &[crate::views::Sort::Course, crate::views::Sort::Name],
};

#[component]
fn Home(on_persona: EventHandler<String>, on_chat: EventHandler<String>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let db = use_context::<Db>();
    let mut revision = use_signal(|| 0u32);
    let _ = revision();
    let chats = db.0.borrow().conversations().unwrap_or_else(|e| {
        crate::diag::report!("reading chats failed: {e}");
        Vec::new()
    });

    let focus = try_use_context::<Coach>()
        .and_then(|c| c.profile.read().as_ref().map(|p| p.focus))
        .unwrap_or(LanguageFocus::Both);
    // Past chats arrive newest first; partners in the course's order.
    let history_pick = HISTORY_VIEW.pick(&db);
    let mut chats: Vec<_> = chats.into_iter().filter_map(|c| persona(&c.persona).map(|p| (c, p))).collect();
    crate::views::sort_by(&mut chats, history_pick.sort, |_| false, |(_, p)| p.name.clone());
    let partners_pick = PARTNERS_VIEW.pick(&db);
    let mut partners = personas_for(focus);
    crate::views::sort_by(
        &mut partners,
        partners_pick.sort,
        |_| false,
        |e| match e {
            Ok(p) => p.name.clone(),
            Err((id, _)) => id.to_string(),
        },
    );

    rsx! {
        header { class: "ch-page-head",
            h2 { "{ui.t(crate::i18n::Msg::NavChat)}" }
            p { class: "muted", "{tt(lang, T::Intro)}" }
        }
        // Where the learner left off comes first: picking up a chat is the
        // usual next step once there is one.
        if !chats.is_empty() {
            h3 { class: "ch-section", "{tt(lang, T::YourChats)}" }
            crate::views::ViewBar { of: HISTORY_VIEW }
            ul { class: "coll ch-history", "data-layout": history_pick.layout.as_str(),
                for (c , p) in chats {
                    {
                        let last = db.0.borrow().chat_messages(&c.id).unwrap_or_default();
                        let count = last.len();
                        let preview = last.last().map(|m| m.text.clone()).unwrap_or_default();
                        let (open_id, delete_id) = (c.id.clone(), c.id.clone());
                        let db = db.clone();
                        let target = target_tag(&p.language);
                        rsx! {
                            li { key: "{c.id}",
                                button { class: "coll-btn ch-open", onclick: move |_| on_chat.call(open_id.clone()),
                                    span { class: "coll-lead", Avatar { name: p.name.clone(), javanese: is_javanese(p) } }
                                    span { class: "coll-main ch-who",
                                        span { class: "coll-title ch-name",
                                            strong { bdi { class: "latin", "{p.name}" } }
                                            span { class: "pill neutral small coll-when-roomy", "{level_name(&c.level)}" }
                                        }
                                        bdi { class: "latin muted ch-preview coll-sub", lang: target, "{preview}" }
                                        span { class: "muted small coll-extra", "{ui.num(count as u64)} {tt(lang, T::Messages)}" }
                                    }
                                    span { class: "chev", aria_hidden: "true" }
                                }
                                button {
                                    class: "ch-icon quiet coll-aside",
                                    aria_label: "{tt(lang, T::Delete)}",
                                    title: "{tt(lang, T::Delete)}",
                                    onclick: move |_| {
                                        if let Err(e) = db.0.borrow_mut().delete_conversation(&delete_id) {
                                            crate::diag::report!("deleting the chat failed: {e}");
                                        }
                                        revision += 1;
                                    },
                                    {icons::trash()}
                                }
                            }
                        }
                    }
                }
            }
        }
        h3 { class: "ch-section", "{tt(lang, T::Partners)}" }
        crate::views::ViewBar { of: PARTNERS_VIEW }
        ul { class: "coll ch-personas", "data-layout": partners_pick.layout.as_str(),
            for entry in partners {
                match entry {
                    Ok(p) => {
                        let id = p.id.clone();
                        let jv = is_javanese(p);
                        // The levels the persona is written for, so the
                        // learner can find the partner for their unit.
                        let bands = p.style.levels.iter().map(|b| level_name(&b.level)).collect::<Vec<_>>().join(" · ");
                        rsx! {
                            li { key: "{p.id}",
                                button { class: "coll-btn ch-partner", onclick: move |_| on_persona.call(id.clone()),
                                    span { class: "coll-lead", Avatar { name: p.name.clone(), javanese: jv } }
                                    span { class: "coll-main ch-who",
                                        span { class: "coll-title ch-name",
                                            strong { bdi { class: "latin", "{p.name}" } }
                                            span { class: if jv { "pill small ch-lang jv" } else { "pill small ch-lang" },
                                                if jv { "{tt(lang, T::Javanese)}" } else { "{tt(lang, T::Indonesian)}" }
                                            }
                                        }
                                        span { class: "coll-sub", "{pick(&p.role, lang)}" }
                                        span { class: "coll-extra ch-levels small latin", dir: "ltr", "{bands}" }
                                    }
                                    span { class: "chev", aria_hidden: "true" }
                                }
                            }
                        }
                    }
                    Err((id, e)) => rsx! {
                        crate::ui::Card { element: crate::ui::CardElement::ListItem, key: "{id}", class: "ch-broken",
                            p { "{tt(lang, T::Broken)}" }
                            p { class: "muted small latin", dir: "ltr", "{id}: {e}" }
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn Scenes(persona: Persona, on_back: EventHandler, on_pick: EventHandler<Scenario>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let target = target_tag(&persona.language);
    rsx! {
        header { class: "ch-bar",
            button { class: "quiet ch-back", "data-back": "true", onclick: move |_| on_back.call(()), "{tt(lang, T::Back)}" }
        }
        crate::ui::Card { class: "ch-profile",
            div { class: "ch-name",
                Avatar { name: persona.name.clone(), javanese: is_javanese(&persona) }
                h2 { bdi { class: "latin", "{persona.name}" } }
            }
            p { Rich { text: pick(&persona.personality, lang).to_string(), target } }
            p { class: "ch-register-line muted", Rich { text: pick(&persona.style.register, lang).to_string(), target } }
        }
        h3 { class: "ch-section", "{tt(lang, T::StartWith)}" }
        p { class: "ch-scene-hint muted small", "{tt(lang, T::SceneHint)}" }
        ul { class: "ch-scenes",
            for s in scenarios(&persona) {
                {
                    let key = format!("{}-{}", s.id.clone().unwrap_or_default(), s.level);
                    let pick_it = s.clone();
                    rsx! {
                        li { key: "{key}",
                            crate::ui::CardButton { class: "tap ch-scene", onclick: move |_| on_pick.call(pick_it.clone()),
                                span { class: "ch-who",
                                    span { class: "ch-name",
                                        if s.id.is_none() {
                                            strong { "{tt(lang, T::FreeChat)}" }
                                        } else {
                                            strong { Rich { text: pick(&s.situation, lang).to_string(), target } }
                                        }
                                        span { class: "pill neutral small latin", "{level_name(&s.level)}" }
                                    }
                                    bdi { class: "latin muted ch-preview", lang: target, "“{s.opener}”" }
                                    if s.id.is_some() && !pick(&s.opener_meaning, lang).trim().is_empty() {
                                        span { class: "ch-scene-meaning muted small", "{pick(&s.opener_meaning, lang)}" }
                                    }
                                }
                                span { class: "chev", aria_hidden: "true" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Status {
    Idle,
    Thinking,
    Problem(T),
}

#[component]
fn Chat(
    id: String,
    on_back: EventHandler,
    on_replay: EventHandler<String>,
    on_persona: EventHandler<String>,
) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let db = use_context::<Db>();
    let ai = use_context::<Ai>();
    let coach = use_context::<Coach>();
    let conv =
        use_signal(|| db.0.borrow().conversations().unwrap_or_default().into_iter().find(|c| c.id == id));
    let mut messages = use_signal(|| db.0.borrow().chat_messages(&id).unwrap_or_default());
    let mut draft = use_signal(String::new);
    let mut status = use_signal(|| Status::Idle);
    let mut ink = use_signal(|| false);
    let mut idea_open = use_signal(|| false);
    // This visit's turns, for Kak Nur's activity log: (sent, without a fix).
    let mut tally = use_signal(|| (0u32, 0u32));
    let opened = use_hook(now_ms);

    {
        let db = db.clone();
        let conv = conv.peek().clone();
        use_drop(move || {
            let (sent, clean) = *tally.peek();
            if sent == 0 {
                return;
            }
            let topic =
                conv.as_ref().and_then(|c| persona(&c.persona)).map(|p| p.name.clone()).unwrap_or_default();
            let practice =
                ActivityKind::Practice { skill: Skill::Writing, topic, correct: clean, total: sent };
            coach.record(&db, practice, ((now_ms() - opened) / 1000) as u32);
        });
    }

    let Some(c) = conv() else { return rsx! {} };
    let Some(p) = persona(&c.persona) else { return rsx! {} };
    let target = target_tag(&p.language);
    let scene = scene_of(p, &c);
    let ink_lang = if is_javanese(p) { Lang::Jv } else { Lang::Id };

    // Asks the persona to answer the newest learner message.
    let ask = {
        let db = db.clone();
        let c = c.clone();
        let scene = scene.clone();
        move || {
            let all = messages.peek().clone();
            let Some(last) = all.last().filter(|m| m.role == ChatRole::Learner).cloned() else { return };
            let history = &all[..all.len() - 1];
            let register = conv.peek().as_ref().map_or(c.register.clone(), |c| c.register.clone());
            let req = request(p, &c.level, &scene, &register, history, &last.text);
            let (ai, db, mut c) = (ai.clone(), db.clone(), c.clone());
            status.set(Status::Thinking);
            scroll_to_end();
            spawn(async move {
                match ai.0.run::<RoleplayTurn>(&req).await {
                    Ok(answer) => {
                        let turn = answer.value;
                        let now = now_ms();
                        let mut learner = last.clone();
                        learner.corrections = turn
                            .corrections
                            .iter()
                            .map(|k| ChatCorrection {
                                text: k.text.clone(),
                                fix: k.fix.clone(),
                                why_en: k.why_en.clone(),
                                why_ar: k.why_ar.clone(),
                            })
                            .collect();
                        learner.unverified = answer.needs_review && !learner.corrections.is_empty();
                        let register = serde_json::to_value(turn.register)
                            .ok()
                            .and_then(|v| v.as_str().map(str::to_string));
                        let reply = ChatMessage {
                            id: new_id(),
                            doc: c.id.clone(),
                            role: ChatRole::Persona,
                            text: turn.reply.clone(),
                            corrections: vec![],
                            register,
                            unverified: answer.needs_review,
                            created_ms: now.max(learner.created_ms + 1),
                        };
                        save(&db, ChatRow::Message(learner.clone()));
                        save(&db, ChatRow::Message(reply.clone()));
                        c.updated_ms = reply.created_ms;
                        save(&db, ChatRow::Conversation(c));
                        {
                            let mut m = messages.write();
                            if let Some(slot) = m.iter_mut().find(|m| m.id == learner.id) {
                                *slot = learner.clone();
                            }
                            m.push(reply);
                        }
                        let (sent, clean) = *tally.peek();
                        tally.set((sent + 1, clean + u32::from(learner.corrections.is_empty())));
                        status.set(Status::Idle);
                        scroll_to_end();
                    }
                    Err(e) => {
                        status.set(Status::Problem(friendly(e.friendly())));
                        scroll_to(".ch-problem");
                    }
                }
            });
        }
    };

    let send = {
        let db = db.clone();
        let mut ask = ask.clone();
        let doc = c.id.clone();
        move || {
            let text: String = draft.peek().trim().chars().take(MAX_MESSAGE_CHARS).collect();
            if text.is_empty() || *status.peek() == Status::Thinking {
                return;
            }
            let after = messages.peek().last().map_or(0, |m| m.created_ms + 1);
            let m = ChatMessage {
                id: new_id(),
                doc: doc.clone(),
                role: ChatRole::Learner,
                text,
                corrections: vec![],
                register: None,
                unverified: false,
                created_ms: now_ms().max(after),
            };
            save(&db, ChatRow::Message(m.clone()));
            messages.write().push(m);
            draft.set(String::new());
            ask();
        }
    };

    let set_register = {
        let db = db.clone();
        move |r: &'static str| {
            let mut conv = conv;
            let mut next = conv.peek().clone();
            if let Some(c) = next.as_mut() {
                c.register = r.to_string();
                save(&db, ChatRow::Conversation(c.clone()));
            }
            conv.set(next);
        }
    };

    let busy = status() == Status::Thinking;
    let all = messages();
    let answered = |i: usize| all.get(i + 1).is_some_and(|n| n.role == ChatRole::Persona);
    let mut send_click = send.clone();
    let mut send_key = send.clone();
    let mut retry = ask.clone();

    // The scene's next learner line, as an idea the learner can open and
    // copy into the box (never sent for them).
    let turn = all.iter().filter(|m| m.role == ChatRole::Learner).count();
    let their_turn_done = all.last().is_some_and(|m| m.role == ChatRole::Persona);
    let idea = reply_idea(p, c.scenario.as_deref(), turn).filter(|_| !busy && their_turn_done);
    let idea_flag = is_javanese(p) && !p.review.verified;

    rsx! {
        div { class: "ch-chat",
            header { class: "ch-bar",
                button { class: "quiet ch-back", "data-back": "true", onclick: move |_| on_back.call(()), "{tt(lang, T::Back)}" }
                {
                    let id = c.id.clone();
                    rsx! {
                        button {
                            class: "ch-replay",
                            disabled: all.len() < 2,
                            onclick: move |_| on_replay.call(id.clone()),
                            {icons::play()}
                            span { "{tt(lang, T::Replay)}" }
                        }
                    }
                }
            }
            {
                let pid = p.id.clone();
                rsx! {
                    crate::ui::CardButton {
                        class: "tap ch-context",
                        aria_label: "{p.name}: {tt(lang, T::AboutPartner)}",
                        onclick: move |_| on_persona.call(pid.clone()),
                        Avatar { name: p.name.clone(), javanese: is_javanese(p), mood: mood_of(busy, &all) }
                        span { class: "ch-who",
                            span { class: "ch-name",
                                strong { bdi { class: "latin", "{p.name}" } }
                                span { class: "pill small latin", "{level_name(&c.level)}" }
                            }
                            span { class: "ch-scene-line muted small", Rich { text: pick(&scene.situation, lang).to_string(), target } }
                        }
                        span { class: "chev", aria_hidden: "true" }
                    }
                }
            }
            div { class: "ch-register", role: "group", aria_label: "{tt(lang, T::TalkToMe)}",
                span { class: "muted small", "{tt(lang, T::TalkToMe)}" }
                for r in registers(p) {
                    {
                        let set = set_register.clone();
                        rsx! {
                            button {
                                key: "{r}",
                                class: "ch-chip",
                                aria_pressed: if c.register == r { "true" } else { "false" },
                                onclick: move |_| set(r),
                                "{tt(lang, register_label(r))}"
                            }
                        }
                    }
                }
            }
            p { class: "ch-note small",
                {icons::lock()}
                span { "{tt(lang, T::TapWord)} {tt(lang, T::Privacy)}" }
            }
            ol { class: "ch-messages",
                for (i , m) in all.iter().enumerate() {
                    match m.role {
                        ChatRole::Persona => rsx! {
                            li { key: "{m.id}", class: "ch-msg them",
                                div { class: "ch-bubble",
                                    PersonaText {
                                        text: m.text.clone(),
                                        tag: target,
                                        javanese: is_javanese(p),
                                        unverified: m.unverified,
                                        children: rsx! {
                                            ListenButton { text: m.text.clone(), language: p.language.clone(), voice: persona_voice(p) }
                                        },
                                    }
                                }
                            }
                        },
                        ChatRole::Learner => rsx! {
                            li { key: "{m.id}", class: "ch-msg me",
                                div { class: "ch-bubble",
                                    bdi { class: "latin", lang: target, "{m.text}" }
                                }
                                if !m.corrections.is_empty() {
                                    ul { class: "ch-fixes",
                                        for (k , fix) in m.corrections.iter().enumerate() {
                                            li { key: "{k}",
                                                span { class: "ch-was latin", lang: target, s { "{fix.text}" } }
                                                " "
                                                span { class: "muted small", "{tt(lang, T::MoreNatural)}" }
                                                " "
                                                bdi { class: "latin ch-fix", lang: target, "{fix.fix}" }
                                                p { class: "small", "{why(fix, lang)}" }
                                            }
                                        }
                                        if m.unverified {
                                            li { span { class: "pill small ch-unverified", "{tt(lang, T::Unverified)}" } }
                                        }
                                    }
                                } else if answered(i) {
                                    span { class: "pill good small ch-ok", "{tt(lang, T::AllGood)}" }
                                }
                            }
                        },
                    }
                }
                if busy {
                    li { class: "ch-msg them", role: "status",
                        div { class: "ch-bubble ch-typing", bdi { class: "latin", "{p.name}" } " {tt(lang, T::Typing)}" }
                    }
                }
            }
            if let Status::Problem(why) = status() {
                div { class: "ch-problem", role: "alert",
                    p { "{tt(lang, why)}" }
                    if why == T::NotConnected {
                        button {
                            onclick: move |_| {
                                let mut s = ui.screen;
                                s.set(Screen::Devices);
                            },
                            "{tt(lang, T::OpenDevices)}"
                        }
                    } else {
                        button { onclick: move |_| retry(), "{tt(lang, T::Retry)}" }
                    }
                }
            }
            div { class: if ink() { "ch-compose ink" } else { "ch-compose" },
                if let Some(line) = idea {
                    if idea_open() {
                        div { class: "ch-idea", role: "note",
                            span { class: "ch-who",
                                bdi { class: "latin", lang: target, "{line.text}" }
                                span { class: "muted small", "{pick(&line.meaning, lang)}" }
                                if idea_flag {
                                    span { class: "pill small ch-unverified", "{tt(lang, T::Unverified)}" }
                                }
                            }
                            {
                                let text = line.text.clone();
                                rsx! {
                                    button {
                                        class: "ch-chip",
                                        onclick: move |_| {
                                            draft.set(text.clone());
                                            idea_open.set(false);
                                        },
                                        "{tt(lang, T::UseIdea)}"
                                    }
                                }
                            }
                        }
                    }
                    div { class: "ch-chips",
                        button {
                            class: "ch-chip",
                            aria_pressed: if idea_open() { "true" } else { "false" },
                            onclick: move |_| idea_open.toggle(),
                            {icons::bulb()}
                            span { "{tt(lang, T::Idea)}" }
                        }
                    }
                }
                div { class: "ch-field",
                    if ink() {
                        InkPad {
                            lang: ink_lang,
                            on_submit: move |a: InkAnswer| {
                                if let Some(t) = a.text {
                                    draft.set(t);
                                }
                                ink.set(false);
                            },
                        }
                    } else {
                        textarea {
                            class: "latin",
                            dir: "ltr",
                            lang: target,
                            rows: "1",
                            maxlength: "{MAX_MESSAGE_CHARS}",
                            placeholder: "{tt(lang, T::Placeholder)}",
                            aria_label: "{tt(lang, T::Placeholder)}",
                            value: "{draft}",
                            oninput: move |e| draft.set(e.value()),
                            onkeydown: move |e| {
                                if e.key() == Key::Enter && !e.modifiers().shift() {
                                    e.prevent_default();
                                    send_key();
                                }
                            },
                        }
                    }
                    // Handwriting is paused (Mohamed, 2026-10-26): the toggle
                    // comes back with the tablet pen work.
                    if HANDWRITING {
                        button { class: "quiet small", onclick: move |_| ink.toggle(),
                            if ink() { "{tt(lang, T::Type)}" } else { "{tt(lang, T::Write)}" }
                        }
                    }
                    button {
                        class: "primary ch-send",
                        disabled: busy || draft().trim().is_empty(),
                        aria_label: "{tt(lang, T::Send)}",
                        title: "{tt(lang, T::Send)}",
                        onclick: move |_| send_click(),
                        {icons::send()}
                    }
                }
            }
        }
    }
}

fn scroll_to_end() {
    scroll_to(".ch-messages > li:last-child");
}

/// Brings `selector` (the newest bubble, or a problem) above the composer.
fn scroll_to(selector: &str) {
    document::eval(&format!(
        r#"setTimeout(() => document.querySelector('{selector}')?.scrollIntoView({{
            block: "end",
            behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
        }}), 30);"#
    ));
}

#[component]
fn ListenButton(text: String, language: String, voice: kabar_protocol::cloud::Voice) -> Element {
    let ui = use_context::<Ui>();
    let missing = use_signal(|| false);
    rsx! {
        button {
            class: "ch-icon quiet ch-listen",
            aria_label: "{tt(ui.lang(), T::Listen)}",
            title: "{tt(ui.lang(), T::Listen)}",
            onclick: move |_| crate::lesson::speak_in(&text, &language, voice, missing),
            {icons::speaker()}
        }
    }
}

/// A persona's reply with its translation under it, in the learner's voice.
/// The `translate_text` task runs once per reply (the router caches it, so
/// reopening a chat costs nothing); with no engine the reply still shows
/// word colours and meanings from the local dictionary.
#[component]
fn PersonaText(
    text: String,
    tag: &'static str,
    javanese: bool,
    unverified: bool,
    children: Element,
) -> Element {
    let ui = use_context::<Ui>();
    let ai = try_use_context::<Ai>();
    let lang = ui.lang();
    let source = text.clone();
    let translated = use_resource(move || {
        let (ai, text) = (ai.clone(), source.clone());
        // Read here so turning translation on later fetches it.
        let off = ui.settings.read().translation == TranslationMode::Off;
        async move {
            let ai = ai.filter(|_| !off)?;
            ai.0.plan(Task::TranslateText, true).ok()?;
            let code = if javanese { CoreLang::Javanese } else { CoreLang::Indonesian };
            match ai.0.run::<TranslateText>(&translate::request(code, &[&text])).await {
                Ok(a) => {
                    let line = a.value.lines.into_iter().find(|l| l.i == 0)?;
                    Some((line, a.needs_review || a.provisional))
                }
                Err(e) => {
                    crate::diag::report!("chat translate_text failed: {e:?}");
                    None
                }
            }
        }
    });
    let (meaning, guessed) = match &*translated.read() {
        Some(Some((line, guessed))) => (Some(in_voice(line, lang).to_string()), *guessed),
        _ => (None, false),
    };
    // Javanese and engine answers stay flagged until the learner approves them.
    let flag = unverified || (meaning.is_some() && (javanese || guessed));
    rsx! {
        Annotated { text, tag, class: "ch-text", meaning, say: false }
        div { class: "ch-tools",
            {children}
            if flag {
                span { class: "pill small ch-unverified", "{tt(lang, T::Unverified)}" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn languages(focus: LanguageFocus) -> Vec<bool> {
        personas_for(focus).into_iter().flatten().map(is_javanese).collect()
    }

    #[test]
    fn the_focus_language_leads_the_partner_list() {
        let id = languages(LanguageFocus::Indonesian);
        assert!(id.contains(&true) && !id[0], "Indonesian partners first");
        assert!(id.windows(2).all(|w| w[0] <= w[1]), "then every Javanese one");
        let jv = languages(LanguageFocus::Javanese);
        assert!(jv[0] && jv.windows(2).all(|w| w[0] >= w[1]));
        assert_eq!(languages(LanguageFocus::Both), id);
    }
}
