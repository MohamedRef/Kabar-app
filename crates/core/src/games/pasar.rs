//! Pasar! (spec §14): haggle with a market seller under a budget, with
//! numbers, prices, polite refusals and Javanese market words (piro, larang,
//! murah). Seller lines are build-time content in `content/games/pasar.toml`;
//! the seller's behaviour is deterministic for a seed.
//!
//! How the seller haggles:
//! - starts at the item's `ask`;
//! - "Boleh kurang?" lowers the price a bit, at most twice and less the second
//!   time; after that the price is "pas";
//! - an offer at or above the hidden `floor` is accepted (never above the
//!   current price);
//! - an offer below 70% of the floor gets a polite refusal and a small step down;
//! - any other offer gets a counter halfway between, trending toward the floor;
//! - walking away makes the seller call you back once near the floor (the
//!   classic market move), unless the price is already at the floor;
//! - after [`MAX_TURNS`] moves the seller names a final price, and the next
//!   move either takes it or walks away.

use std::fmt;
use std::sync::Arc;

use kabar_protocol::content::Trilingual;
use serde::Deserialize;

use super::Rng;
use super::rupiah::{format_rupiah, javanese_number_words, number_words};

/// Learner moves before the seller names a final price.
pub const MAX_TURNS: usize = 12;

/// Parsed `content/games/pasar.toml`.
#[derive(Clone, Debug, Default)]
pub struct PasarData {
    pub items: Vec<Item>,
    /// The shared seller line bank (item-specific lines live on the item).
    pub lines: Vec<SellerLine>,
    /// Whether the learner has approved the content. Until then it shows as unverified.
    pub reviewed: bool,
    pub version: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileToml {
    reviewed: bool,
    version: u32,
    items: Vec<ItemToml>,
    lines: Vec<LineToml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemToml {
    id: String,
    name: String,
    meaning: Trilingual,
    ask: u64,
    floor: u64,
    fair: u64,
    #[serde(default)]
    javanese: bool,
    #[serde(default)]
    bonus: bool,
    #[serde(default)]
    lines: Vec<LineToml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LineToml {
    situation: Situation,
    #[serde(default)]
    javanese: bool,
    text: String,
    arz: String,
    ar: String,
    en: String,
}

impl PasarData {
    pub fn parse(toml_text: &str) -> Result<Self, String> {
        let file: FileToml = toml::from_str(toml_text).map_err(|e| format!("pasar.toml: {e}"))?;
        let mut lines = Vec::with_capacity(file.lines.len());
        for l in file.lines {
            lines.push(SellerLine::from_toml(l)?);
        }
        for situation in Situation::ALL {
            for javanese in [false, true] {
                if !lines.iter().any(|l| l.situation == situation && l.javanese == javanese) {
                    let lang = if javanese { "Javanese" } else { "Indonesian" };
                    return Err(format!("pasar.toml: no {lang} line for {situation:?}"));
                }
            }
        }

        let mut items: Vec<Item> = Vec::with_capacity(file.items.len());
        for it in file.items {
            let id = it.id.clone();
            if id.is_empty() || it.name.trim().is_empty() {
                return Err("pasar.toml: an item has an empty id or name".into());
            }
            if items.iter().any(|i| i.id == id) {
                return Err(format!("pasar.toml: duplicate item {id}"));
            }
            if !(0 < it.floor && it.floor <= it.fair && it.fair <= it.ask) {
                return Err(format!("{id}: needs 0 < floor <= fair <= ask"));
            }
            for (name, v) in [("ask", it.ask), ("floor", it.floor), ("fair", it.fair)] {
                if v % step(v) != 0 {
                    return Err(format!("{id}: {name} {v} is not a round market price"));
                }
            }
            for (lang, s) in [("arz", &it.meaning.arz), ("ar", &it.meaning.ar), ("en", &it.meaning.en)] {
                if s.trim().is_empty() {
                    return Err(format!("{id}: empty {lang} meaning"));
                }
            }
            let mut own = Vec::with_capacity(it.lines.len());
            for l in it.lines {
                let l = SellerLine::from_toml(l)?;
                if l.javanese != it.javanese {
                    return Err(format!("{id}: item lines must match the item's language"));
                }
                own.push(l);
            }
            let mut bank: Vec<SellerLine> = Vec::new();
            for situation in Situation::ALL {
                let mine: Vec<&SellerLine> = own.iter().filter(|l| l.situation == situation).collect();
                if mine.is_empty() {
                    bank.extend(
                        lines
                            .iter()
                            .filter(|l| l.situation == situation && l.javanese == it.javanese)
                            .cloned(),
                    );
                } else {
                    bank.extend(mine.into_iter().cloned());
                }
            }
            items.push(Item {
                id,
                name: it.name,
                meaning: it.meaning,
                ask: it.ask,
                floor: it.floor,
                fair: it.fair,
                javanese: it.javanese,
                bonus: it.bonus,
                seller_lines: LineBank(Arc::new(bank)),
            });
        }
        if items.is_empty() {
            return Err("pasar.toml: no items".into());
        }
        Ok(Self { items, lines, reviewed: file.reviewed, version: file.version })
    }

    pub fn item(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    /// The bonus round's item, if the content has one.
    pub fn bonus(&self) -> Option<&Item> {
        self.items.iter().find(|i| i.bonus)
    }
}

/// When the seller says a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Situation {
    /// Opening words, before the price.
    Greeting,
    /// Naming the current price.
    Quote,
    /// Lowering the price after "Boleh kurang?".
    Discount,
    /// Answering an offer with a price in between.
    Counter,
    /// Politely refusing a much-too-low offer, with a counter.
    RefuseLow,
    /// Agreeing to a price.
    Accept,
    /// Calling the learner back after they walked away.
    CallBack,
    /// The last price: "harga pas", "wis pas".
    Final,
    /// Letting the learner go, kindly.
    LetGo,
}

impl Situation {
    pub const ALL: [Self; 9] = [
        Self::Greeting,
        Self::Quote,
        Self::Discount,
        Self::Counter,
        Self::RefuseLow,
        Self::Accept,
        Self::CallBack,
        Self::Final,
        Self::LetGo,
    ];

    /// Whether the line names a price (`{price}`).
    pub fn has_price(self) -> bool {
        !matches!(self, Self::Greeting | Self::LetGo)
    }
}

/// One seller line template. `{price}` in `text` is spoken as words
/// ("dua puluh lima ribu", or Javanese "selawe ewu"); in the meanings it is
/// written as shops write it ("Rp25.000").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SellerLine {
    pub situation: Situation,
    /// Javanese market speech (unverified until reviewed).
    pub javanese: bool,
    pub text: String,
    pub meaning: Trilingual,
}

impl SellerLine {
    fn from_toml(l: LineToml) -> Result<Self, String> {
        let line = Self {
            situation: l.situation,
            javanese: l.javanese,
            text: l.text,
            meaning: Trilingual { arz: l.arz, ar: l.ar, en: l.en },
        };
        for s in [&line.text, &line.meaning.arz, &line.meaning.ar, &line.meaning.en] {
            if s.trim().is_empty() {
                return Err(format!("{:?} line {:?}: empty text or meaning", line.situation, line.text));
            }
            let mut rest = s.as_str();
            let mut prices = 0;
            while let Some(open) = rest.find('{') {
                let close = rest[open..].find('}').ok_or_else(|| format!("unclosed {{ in {s:?}"))? + open;
                match &rest[open + 1..close] {
                    "price" => prices += 1,
                    other => return Err(format!("unknown placeholder {{{other}}} in {s:?}")),
                }
                rest = &rest[close + 1..];
            }
            if rest.contains('}') {
                return Err(format!("stray }} in {s:?}"));
            }
            if (prices > 0) != line.situation.has_price() {
                return Err(format!("{:?} line {s:?}: {{price}} used wrongly", line.situation));
            }
        }
        Ok(line)
    }

    /// The line with `{price}` filled in: (spoken text, meaning).
    pub fn render(&self, price: u64) -> (String, Trilingual) {
        let spoken = speak_price(price, self.javanese);
        let written = format_rupiah(price);
        let text = fill_spoken(&self.text, &spoken);
        let meaning = Trilingual {
            arz: self.meaning.arz.replace("{price}", &written),
            ar: self.meaning.ar.replace("{price}", &written),
            en: capitalize(&self.meaning.en.replace("{price}", &written)),
        };
        (text, meaning)
    }
}

/// A price as the seller says it: Indonesian words, or Javanese words for a
/// Javanese seller (falling back to Indonesian beyond the Javanese table,
/// as sellers do with big numbers).
pub fn speak_price(price: u64, javanese: bool) -> String {
    if javanese && let Some(w) = javanese_number_words(price) {
        return w;
    }
    number_words(price)
}

/// Fills `{price}` with spoken words, capitalized where it starts a sentence.
fn fill_spoken(template: &str, spoken: &str) -> String {
    let mut out = String::with_capacity(template.len() + spoken.len());
    let mut rest = template;
    while let Some(i) = rest.find("{price}") {
        out.push_str(&rest[..i]);
        let before = out.trim_end();
        if before.is_empty() || before.ends_with(['.', '?', '!']) {
            out.push_str(&capitalize(spoken));
        } else {
            out.push_str(spoken);
        }
        rest = &rest[i + "{price}".len()..];
    }
    out.push_str(rest);
    capitalize(&out)
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The seller lines for one item, shared cheaply between games.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct LineBank(Arc<Vec<SellerLine>>);

impl LineBank {
    pub fn new(lines: Vec<SellerLine>) -> Self {
        Self(Arc::new(lines))
    }

    pub fn lines(&self) -> &[SellerLine] {
        &self.0
    }

    fn for_situation(&self, s: Situation) -> Vec<&SellerLine> {
        self.0.iter().filter(|l| l.situation == s).collect()
    }
}

impl fmt::Debug for LineBank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LineBank({} lines)", self.0.len())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    /// As the seller says it, e.g. "kaos batik" or the bonus "iphone 17 pm 512gb resmi ibox".
    pub name: String,
    pub meaning: Trilingual,
    /// The seller's first price.
    pub ask: u64,
    /// The lowest price the seller accepts (hidden).
    pub floor: u64,
    /// A fair price a local would pay.
    pub fair: u64,
    /// Seller speaks Javanese market words (Javanese lines are unverified).
    pub javanese: bool,
    /// The bonus round from the spec's test texts.
    pub bonus: bool,
    /// What this item's seller says, filled by [`PasarData::parse`]. Empty for
    /// a hand-built item: the seller then only names prices.
    pub seller_lines: LineBank,
}

/// A budget for a round: a little above the fair price, so a good haggle
/// fits and paying the first ask does not. Deterministic for `seed`.
pub fn suggest_budget(item: &Item, seed: u64) -> u64 {
    let gap = item.ask.saturating_sub(item.fair);
    let pct = 15 + Rng::new(seed ^ 0x00B0_D6E7).below(21) as u64; // 15–35% of the way to the ask
    let raw = item.fair + gap * pct / 100;
    let s = step(raw);
    (raw / s * s).max(item.fair)
}

/// What the learner says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Move {
    /// "Berapa?" / "Piro?"
    AskPrice,
    /// "Boleh kurang?" / "Kurang ya?"
    AskLower,
    /// "Kalau Rp…?"
    Offer(u64),
    /// "Oke, deal."
    Accept,
    /// "Nggak, makasih" — a polite refusal; the seller may call you back.
    Refuse,
}

/// One seller turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SellerTurn {
    /// The seller's line in Indonesian (or Javanese market speech).
    pub text: String,
    pub meaning: Trilingual,
    /// The price on the table after this turn, if any.
    pub price: Option<u64>,
    pub outcome: Outcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Deal {
        price: u64,
    },
    /// The learner refused and the seller let them go.
    WalkedAway,
}

/// Price granularity a seller rounds to at this size.
fn step(n: u64) -> u64 {
    match n {
        0..20_000 => 500,
        20_000..200_000 => 1_000,
        200_000..1_000_000 => 5_000,
        1_000_000..10_000_000 => 50_000,
        _ => 100_000,
    }
}

fn round_up(n: u64) -> u64 {
    let s = step(n);
    n.div_ceil(s) * s
}

fn round_down(n: u64) -> u64 {
    let s = step(n);
    n / s * s
}

#[derive(Clone, Debug)]
pub struct Game {
    item: Item,
    budget: u64,
    seed: u64,
    turns: Vec<(Move, SellerTurn)>,
    /// The seller's current price.
    price: u64,
    discounts: u8,
    called_back: bool,
    /// The turn limit was reached and the final price named.
    last_call: bool,
    /// How often each situation has been said, to rotate wording.
    said: [u32; 9],
}

impl Game {
    pub fn new(item: &Item, budget: u64, seed: u64) -> Self {
        let mut said = [0; 9];
        // The opening uses the first greeting and quote.
        said[Situation::Greeting as usize] = 1;
        said[Situation::Quote as usize] = 1;
        Self {
            item: item.clone(),
            budget,
            seed,
            turns: Vec::new(),
            price: item.ask,
            discounts: 0,
            called_back: false,
            last_call: false,
            said,
        }
    }

    pub fn item(&self) -> &Item {
        &self.item
    }

    pub fn budget(&self) -> u64 {
        self.budget
    }

    /// The seller's current price.
    pub fn price(&self) -> u64 {
        self.price
    }

    pub fn is_over(&self) -> bool {
        self.turns.last().is_some_and(|(_, t)| t.outcome != Outcome::Continue)
    }

    /// The seller's greeting and opening line.
    pub fn opening(&self) -> SellerTurn {
        let ask = self.item.ask;
        let (g_text, g_meaning) = self.line(Situation::Greeting, 0, ask);
        let (q_text, q_meaning) = self.line(Situation::Quote, 0, ask);
        let join = |a: &str, b: &str| if a.is_empty() { b.to_string() } else { format!("{a} {b}") };
        SellerTurn {
            text: join(&g_text, &q_text),
            meaning: Trilingual {
                arz: join(&g_meaning.arz, &q_meaning.arz),
                ar: join(&g_meaning.ar, &q_meaning.ar),
                en: join(&g_meaning.en, &q_meaning.en),
            },
            price: Some(ask),
            outcome: Outcome::Continue,
        }
    }

    /// The seller's answer to a move. Once the game is over, returns the
    /// last turn again without recording anything.
    pub fn play(&mut self, mv: Move) -> SellerTurn {
        if let Some((_, last)) = self.turns.last()
            && last.outcome != Outcome::Continue
        {
            return last.clone();
        }
        let (situation, outcome) = self.decide(&mv);
        let mut situation = situation;
        if outcome == Outcome::Continue && self.turns.len() + 1 >= MAX_TURNS {
            self.last_call = true;
            situation = Situation::Final;
        }
        let n = self.said[situation as usize];
        self.said[situation as usize] += 1;
        let shown_price = match outcome {
            Outcome::Deal { price } => price,
            _ => self.price,
        };
        let (text, meaning) = self.line(situation, n, shown_price);
        let turn = SellerTurn {
            text,
            meaning,
            price: (outcome != Outcome::WalkedAway).then_some(shown_price),
            outcome,
        };
        self.turns.push((mv, turn.clone()));
        turn
    }

    /// Updates the price and picks what the seller says.
    fn decide(&mut self, mv: &Move) -> (Situation, Outcome) {
        let floor = self.item.floor;
        if self.last_call {
            return match *mv {
                Move::Accept => (Situation::Accept, Outcome::Deal { price: self.price }),
                Move::Offer(x) if x >= floor => {
                    (Situation::Accept, Outcome::Deal { price: x.min(self.price) })
                }
                _ => (Situation::LetGo, Outcome::WalkedAway),
            };
        }
        match *mv {
            Move::AskPrice => (Situation::Quote, Outcome::Continue),
            Move::AskLower => {
                if self.discounts >= 2 || self.price <= floor {
                    return (Situation::Final, Outcome::Continue);
                }
                let mut rng = Rng::new(self.seed ^ 0xD15C_0047 ^ u64::from(self.discounts));
                let pct = if self.discounts == 0 { 20 + rng.below(11) } else { 8 + rng.below(5) };
                let pct = pct as u64;
                let drop = (self.price - floor) * pct / 100;
                self.lower_to(round_down(self.price - drop));
                self.discounts += 1;
                (Situation::Discount, Outcome::Continue)
            }
            Move::Offer(x) => {
                if x >= self.price {
                    (Situation::Accept, Outcome::Deal { price: self.price })
                } else if x >= floor {
                    self.price = x;
                    (Situation::Accept, Outcome::Deal { price: x })
                } else if x.saturating_mul(10) < floor.saturating_mul(7) {
                    if self.price > floor {
                        self.lower_to(round_up(self.price - (self.price - floor) / 4));
                    }
                    (Situation::RefuseLow, Outcome::Continue)
                } else if self.price <= floor {
                    (Situation::Final, Outcome::Continue)
                } else {
                    self.lower_to(round_up(self.price / 2 + x / 2));
                    (Situation::Counter, Outcome::Continue)
                }
            }
            Move::Accept => (Situation::Accept, Outcome::Deal { price: self.price }),
            Move::Refuse => {
                if !self.called_back && self.price > floor {
                    self.called_back = true;
                    self.lower_to(round_up(floor + (self.price - floor) / 10));
                    (Situation::CallBack, Outcome::Continue)
                } else {
                    (Situation::LetGo, Outcome::WalkedAway)
                }
            }
        }
    }

    /// Moves the price down to `target`, at least one step and never below the floor.
    fn lower_to(&mut self, target: u64) {
        let floor = self.item.floor;
        let target = if target >= self.price { self.price.saturating_sub(step(self.price)) } else { target };
        self.price = target.max(floor).min(self.price);
    }

    /// Variant `n` (rotating from a seed-chosen start) of the seller's line.
    fn line(&self, s: Situation, n: u32, price: u64) -> (String, Trilingual) {
        let options = self.item.seller_lines.for_situation(s);
        if options.is_empty() {
            if !s.has_price() {
                let empty = Trilingual { arz: String::new(), ar: String::new(), en: String::new() };
                return (String::new(), empty);
            }
            let written = format_rupiah(price);
            let meaning = Trilingual {
                arz: format!("\u{2068}{written}\u{2069}"),
                ar: format!("\u{2068}{written}\u{2069}"),
                en: written,
            };
            return (capitalize(&speak_price(price, self.item.javanese)), meaning);
        }
        let start = Rng::new(self.seed ^ (s as u64 + 1).wrapping_mul(0x9E37_79B9)).below(options.len());
        options[(start + n as usize) % options.len()].render(price)
    }

    pub fn turns(&self) -> &[(Move, SellerTurn)] {
        &self.turns
    }

    /// Final score once the game is over.
    pub fn result(&self) -> Option<PasarResult> {
        let outcome = self.turns.last()?.1.outcome;
        let polite = self
            .turns
            .iter()
            .all(|(mv, _)| !matches!(mv, Move::Offer(x) if x.saturating_mul(10) < self.item.fair * 4));
        match outcome {
            Outcome::Continue => None,
            Outcome::Deal { price } => {
                let within_budget = price <= self.budget;
                let stars = u8::from(within_budget) + u8::from(price <= self.item.fair) + u8::from(polite);
                Some(PasarResult {
                    outcome,
                    saved: self.item.ask.saturating_sub(price),
                    within_budget,
                    stars,
                })
            }
            // Nothing spent; only politeness earns a star.
            Outcome::WalkedAway => {
                Some(PasarResult { outcome, saved: 0, within_budget: true, stars: u8::from(polite) })
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasarResult {
    pub outcome: Outcome,
    /// Rupiah saved against the first ask.
    pub saved: u64,
    pub within_budget: bool,
    /// 0–3 stars: within budget, near the fair price, polite throughout.
    pub stars: u8,
}

/// The learner's possible lines for a move, in Indonesian (or Javanese for a
/// Javanese seller), e.g. AskPrice → "Berapa, Bu?" / "Piro, Bu?".
///
/// Numbers are spoken as words. The Javanese refusal uses krama ("Mboten,
/// matur nuwun"): buyers are often polite to a market seller who is older.
/// Javanese lines are unverified until the learner approves them.
pub fn phrase(mv: &Move, javanese: bool) -> String {
    match (mv, javanese) {
        (Move::AskPrice, false) => "Berapa, Bu?".into(),
        (Move::AskPrice, true) => "Piro, Bu?".into(),
        (Move::AskLower, false) => "Boleh kurang, Bu?".into(),
        (Move::AskLower, true) => "Kurang ya, Bu?".into(),
        (Move::Offer(p), false) => format!("Kalau {}, boleh?", number_words(*p)),
        (Move::Offer(p), true) => format!("Nek {}, oleh?", speak_price(*p, true)),
        (Move::Accept, false) => "Oke, deal.".into(),
        (Move::Accept, true) => "Ya wis, tak jupuk.".into(),
        (Move::Refuse, false) => "Nggak, makasih, Bu.".into(),
        (Move::Refuse, true) => "Mboten, matur nuwun, Bu.".into(),
    }
}

/// What the learner's line means, for the hint under the buttons. Prices are
/// written as shops write them, isolated inside the Arabic text.
pub fn phrase_meaning(mv: &Move) -> Trilingual {
    let t = |arz: &str, ar: &str, en: &str| Trilingual { arz: arz.into(), ar: ar.into(), en: en.into() };
    match mv {
        Move::AskPrice => t("بكام يا حاجّة؟", "بكم يا سيدتي؟", "How much, ma'am?"),
        Move::AskLower => t("ممكن تنزّلي شوية؟", "هل يمكن أن تخفّضي السعر قليلًا؟", "Can you go lower?"),
        Move::Offer(p) => {
            let w = format_rupiah(*p);
            Trilingual {
                arz: format!("طب لو \u{2068}{w}\u{2069}، ينفع؟"),
                ar: format!("ماذا لو \u{2068}{w}\u{2069}، هل يمكن؟"),
                en: format!("What about {w}?"),
            }
        }
        Move::Accept => t("ماشي، اتفقنا.", "حسنًا، اتفقنا.", "Okay, deal."),
        Move::Refuse => t("لأ، شكرًا يا حاجّة.", "لا، شكرًا يا سيدتي.", "No, thank you, ma'am."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = include_str!("../../../../content/games/pasar.toml");

    fn data() -> PasarData {
        PasarData::parse(SOURCE).expect("content/games/pasar.toml parses")
    }

    fn is_arabic_letter(c: char) -> bool {
        ('\u{0600}'..='\u{06FF}').contains(&c)
    }

    /// Latin letters and `{price}` in Arabic text sit inside FSI … PDI.
    fn isolation_errors(v: &str) -> Vec<String> {
        let mut errors = Vec::new();
        let mut depth = 0i32;
        for (i, c) in v.char_indices() {
            match c {
                '\u{2068}' => depth += 1,
                '\u{2069}' => {
                    depth -= 1;
                    if depth < 0 {
                        errors.push("PDI without FSI".into());
                    }
                }
                '\u{202A}'..='\u{202E}' | '\u{2066}' | '\u{2067}' => {
                    errors.push(format!("use FSI/PDI, not U+{:04X}", c as u32))
                }
                '{' if depth == 0 => errors.push(format!("placeholder outside FSI…PDI at {i}")),
                c if c.is_ascii_alphabetic() && depth == 0 => {
                    errors.push(format!("Latin {c:?} outside FSI…PDI"))
                }
                _ => {}
            }
        }
        if depth != 0 {
            errors.push("unbalanced FSI/PDI".into());
        }
        errors
    }

    fn check_meaning(what: &str, m: &Trilingual) {
        for (lang, s) in [("arz", &m.arz), ("ar", &m.ar)] {
            assert!(!s.trim().is_empty(), "{what} {lang}: empty");
            assert!(s.chars().any(is_arabic_letter), "{what} {lang}: no Arabic in {s:?}");
            let errors = isolation_errors(s);
            assert!(errors.is_empty(), "{what} {lang}: {errors:?} in {s:?}");
        }
        assert!(!m.en.trim().is_empty(), "{what} en: empty");
        assert!(!m.en.chars().any(is_arabic_letter), "{what} en: Arabic in {:?}", m.en);
        assert!(!m.en.contains(['\u{2068}', '\u{2069}']), "{what} en: isolates belong to Arabic");
    }

    fn all_lines(d: &PasarData) -> Vec<&SellerLine> {
        d.lines.iter().chain(d.items.iter().flat_map(|i| i.seller_lines.lines())).collect()
    }

    #[test]
    fn content_parses_and_awaits_review() {
        let d = data();
        assert!(!d.reviewed, "set reviewed = true only after the learner approves every line");
        assert_eq!(d.version, 1);
        assert!(d.items.len() >= 8, "about eight items");
        assert!(d.items.iter().filter(|i| i.javanese).count() >= 2, "a Javanese-speaking seller");
        for item in &d.items {
            assert!(item.floor <= item.fair && item.fair <= item.ask, "{}", item.id);
            assert!(item.floor * 10 >= item.ask * 5, "{}: floor is a realistic share of the ask", item.id);
            check_meaning(&item.id, &item.meaning);
            for s in Situation::ALL {
                let lines = item.seller_lines.for_situation(s);
                assert!(!lines.is_empty(), "{}: no {s:?} line", item.id);
                assert!(
                    lines.iter().all(|l| l.javanese == item.javanese),
                    "{}: {s:?} in the wrong language",
                    item.id
                );
            }
            assert_eq!(d.item(&item.id), Some(item));
        }
        assert_eq!(d.item("nope"), None);
        // Several variants per situation in each language.
        for s in Situation::ALL {
            for jv in [false, true] {
                let n = d.lines.iter().filter(|l| l.situation == s && l.javanese == jv).count();
                assert!(n >= 2, "{s:?} javanese={jv}: only {n} variants");
            }
        }
    }

    #[test]
    fn bonus_round_is_the_spec_listing() {
        let d = data();
        let bonus = d.bonus().expect("a bonus item");
        assert_eq!(d.items.iter().filter(|i| i.bonus).count(), 1);
        // §24 test text: "iphone 17 pm 512gb resmi ibox mulus nomin garansi jalan minat wa".
        assert!(SOURCE.contains("iphone 17 pm 512gb resmi ibox mulus nomin garansi jalan"));
        assert_eq!(bonus.name, "iphone 17 pm 512gb resmi ibox mulus nomin garansi jalan");
        assert!(
            bonus.ask >= 10_000_000 && bonus.ask < 100_000_000,
            "a used-phone price in the tens of millions"
        );
        assert!(!bonus.javanese);
        let greeting = bonus.seller_lines.for_situation(Situation::Greeting);
        assert!(greeting.iter().all(|l| l.text.contains("nomin")), "the bonus seller uses the listing words");
        let g = Game::new(bonus, suggest_budget(bonus, 1), 1);
        assert!(g.opening().text.to_lowercase().contains("dua puluh empat juta lima ratus ribu"));
    }

    #[test]
    fn every_line_is_complete_and_isolated() {
        let d = data();
        for l in all_lines(&d) {
            let what = format!("{:?} {:?}", l.situation, l.text);
            assert!(!l.text.trim().is_empty());
            assert!(!l.text.chars().any(is_arabic_letter), "{what}: Arabic in the seller's line");
            check_meaning(&what, &l.meaning);
            for s in [&l.text, &l.meaning.arz, &l.meaning.ar, &l.meaning.en] {
                assert_eq!(s.contains("{price}"), l.situation.has_price(), "{what}: {s:?}");
                assert_eq!(s.matches('{').count(), s.matches("{price}").count(), "{what}: {s:?}");
            }
            let (text, meaning) = l.render(25_000);
            assert!(!text.contains('{') && !meaning.arz.contains('{') && !meaning.en.contains('{'));
            check_meaning(&what, &meaning);
            if l.situation.has_price() {
                let spoken = if l.javanese { "selawe ewu" } else { "dua puluh lima ribu" };
                assert!(text.to_lowercase().contains(spoken), "{what}: {text}");
                assert!(meaning.ar.contains("\u{2068}Rp25.000\u{2069}"), "{what}: {}", meaning.ar);
                assert!(meaning.en.contains("Rp25.000"));
            }
            assert!(text.starts_with(|c: char| c.is_uppercase()), "{text}");
            for sentence in text.split(". ").chain(text.split("? ")).skip(1) {
                assert!(!sentence.starts_with(|c: char| c.is_lowercase()), "sentence case in {text:?}");
            }
        }
    }

    #[test]
    fn friendly_banter_no_insults() {
        let id =
            ["bodoh", "goblok", "pelit", "gila", "miskin", "kere", "tolol", "norak", "kampungan", "jelek"];
        let jv = ["pekok", "goblog", "ndeso", "medhit", "kere", "edan", "elek"];
        let en = ["stupid", "idiot", "stingy", "cheapskate", "crazy", "poor", "ugly", "shame"];
        let ar = ["غبي", "بخيل", "مجنون", "فقير", "حرام عليك", "مش فاهم"];
        let d = data();
        for l in all_lines(&d) {
            let text = l.text.to_lowercase();
            let words: Vec<&str> = text.split(|c: char| !c.is_alphanumeric()).collect();
            for w in id.iter().chain(&jv) {
                assert!(!words.contains(w), "{w:?} in {:?}", l.text);
            }
            for w in en {
                assert!(!l.meaning.en.to_lowercase().contains(w), "{w:?} in {:?}", l.meaning.en);
            }
            for w in ar {
                assert!(!l.meaning.arz.contains(w) && !l.meaning.ar.contains(w), "{w:?} in {:?}", l.meaning);
            }
            for s in [&l.text, &l.meaning.arz, &l.meaning.ar, &l.meaning.en] {
                assert!(s.matches('!').count() <= 1, "{s:?}: too many exclamation marks");
                assert!(!s.contains("?!") && !s.contains("؟!"));
            }
        }
    }

    #[test]
    fn javanese_sellers_use_market_words() {
        let d = data();
        let jv: String = d
            .lines
            .iter()
            .filter(|l| l.javanese)
            .map(|l| l.text.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        for w in ["larang", "murah", "wis pas", "ora isa kurang", "monggo", "matur nuwun"] {
            assert!(jv.contains(w), "no {w:?} in the Javanese lines");
        }
        assert_eq!(phrase(&Move::AskPrice, true), "Piro, Bu?");
    }

    #[test]
    fn parse_rejects_bad_content() {
        assert!(PasarData::parse("nope = 1").is_err());
        let swap = |from: &str, to: &str| SOURCE.replacen(from, to, 1);
        assert!(
            PasarData::parse(&swap("text = \"Ini {price}, Kak.\"", "text = \"Ini {harga}, Kak.\"")).is_err()
        );
        assert!(PasarData::parse(&swap("text = \"Ini {price}, Kak.\"", "text = \"Ini, Kak.\"")).is_err());
        assert!(PasarData::parse(&swap("floor = 55000", "floor = 95000")).is_err());
        assert!(PasarData::parse(&swap("floor = 55000", "floor = 55123")).is_err());
        assert!(PasarData::parse(&swap("id = \"mangga\"", "id = \"kaos_batik\"")).is_err());
        assert!(PasarData::parse(&swap("situation = \"let_go\"", "situation = \"shout\"")).is_err());
        assert!(PasarData::parse(&swap("en = \"This one's {price}.\"", "en = \"\"")).is_err());
    }

    /// Plays `moves` from a fresh game and returns it.
    fn play(item: &Item, budget: u64, seed: u64, moves: &[Move]) -> Game {
        let mut g = Game::new(item, budget, seed);
        for m in moves {
            g.play(m.clone());
        }
        g
    }

    #[test]
    fn opening_greets_and_quotes_the_ask() {
        let d = data();
        for item in &d.items {
            let g = Game::new(item, item.fair, 3);
            let o = g.opening();
            assert_eq!(o.price, Some(item.ask));
            assert_eq!(o.outcome, Outcome::Continue);
            assert!(o.text.to_lowercase().contains(&speak_price(item.ask, item.javanese)), "{}", o.text);
            assert!(o.meaning.en.contains(&format_rupiah(item.ask)));
            check_meaning(&item.id, &o.meaning);
            assert_eq!(o, Game::new(item, item.fair, 3).opening(), "deterministic");
            assert!(g.result().is_none());
        }
        let kaos = d.item("kaos_batik").unwrap();
        let openings: std::collections::BTreeSet<String> =
            (0..20).map(|s| Game::new(kaos, 60_000, s).opening().text).collect();
        assert!(openings.len() > 1, "seeds vary the wording");
        let tahu = d.item("tahu_tempe").unwrap();
        assert!(
            Game::new(tahu, 10_000, 0).opening().text.to_lowercase().contains("rolas ewu"),
            "Javanese numbers"
        );
    }

    #[test]
    fn ask_lower_twice_then_pas() {
        let d = data();
        for item in &d.items {
            for seed in 0..10 {
                let mut g = Game::new(item, item.fair, seed);
                let a = g.play(Move::AskLower).price.unwrap();
                let b = g.play(Move::AskLower).price.unwrap();
                let c = g.play(Move::AskLower);
                assert!(a < item.ask && b < a && b >= item.floor, "{}: {a} {b}", item.id);
                assert!(item.ask - a > a - b, "{}: diminishing ({} then {})", item.id, item.ask - a, a - b);
                assert_eq!(c.price, Some(b), "no third discount");
                assert_eq!(g.turns()[2].1.text, c.text);
                let finals: Vec<String> =
                    item.seller_lines.for_situation(Situation::Final).iter().map(|l| l.render(b).0).collect();
                assert!(finals.contains(&c.text), "{}: {:?}", item.id, c.text);
                assert_eq!(c.outcome, Outcome::Continue);
            }
        }
    }

    #[test]
    fn offers_get_refused_countered_or_accepted() {
        let d = data();
        let kaos = d.item("kaos_batik").unwrap(); // ask 85.000, floor 55.000, fair 60.000

        // Way too low (< 70% of the floor): polite refusal with a small step down.
        let mut g = Game::new(kaos, 70_000, 1);
        let t = g.play(Move::Offer(20_000));
        assert_eq!(t.outcome, Outcome::Continue);
        let p = t.price.unwrap();
        assert!(p < 85_000 && p > 70_000, "{p}");
        let refusals: Vec<String> =
            kaos.seller_lines.for_situation(Situation::RefuseLow).iter().map(|l| l.render(p).0).collect();
        assert!(refusals.contains(&t.text));

        // In between: counter halfway, trending toward the floor.
        let mut g = Game::new(kaos, 70_000, 1);
        let c1 = g.play(Move::Offer(45_000)).price.unwrap();
        assert_eq!(c1, 65_000, "halfway between 85.000 and 45.000");
        let c2 = g.play(Move::Offer(45_000)).price.unwrap();
        assert_eq!(c2, 55_000, "halfway again, but never below the floor");
        let c3 = g.play(Move::Offer(45_000));
        assert_eq!(c3.price, Some(55_000));
        assert_eq!(c3.outcome, Outcome::Continue, "at the floor: final price, still talking");

        // At or above the floor: deal at the offer.
        let mut g = Game::new(kaos, 70_000, 1);
        let t = g.play(Move::Offer(58_000));
        assert_eq!(t.outcome, Outcome::Deal { price: 58_000 });
        assert_eq!(t.price, Some(58_000));
        let r = g.result().unwrap();
        assert_eq!(r, PasarResult { outcome: t.outcome, saved: 27_000, within_budget: true, stars: 3 });

        // Above the current price: the seller takes the current price, not more.
        let mut g = Game::new(kaos, 100_000, 1);
        assert_eq!(g.play(Move::Offer(90_000)).outcome, Outcome::Deal { price: 85_000 });
    }

    #[test]
    fn refuse_brings_one_call_back_then_let_go() {
        let d = data();
        let kaos = d.item("kaos_batik").unwrap();
        let mut g = Game::new(kaos, 60_000, 9);
        let back = g.play(Move::Refuse);
        assert_eq!(back.outcome, Outcome::Continue);
        let p = back.price.unwrap();
        assert!((55_000..=58_000).contains(&p), "called back near the floor: {p}");
        let bye = g.play(Move::Refuse);
        assert_eq!(bye.outcome, Outcome::WalkedAway);
        assert_eq!(bye.price, None);
        assert!(g.is_over());
        let r = g.result().unwrap();
        assert_eq!(r, PasarResult { outcome: Outcome::WalkedAway, saved: 0, within_budget: true, stars: 1 });
        // Moves after the end change nothing.
        assert_eq!(g.play(Move::Accept), bye);
        assert_eq!(g.turns().len(), 2);

        // Taking the call-back price is a deal.
        let mut g = play(kaos, 60_000, 9, &[Move::Refuse]);
        assert_eq!(g.play(Move::Accept).outcome, Outcome::Deal { price: p });

        // Already at the floor: no call back.
        let mut g = play(kaos, 60_000, 9, &[Move::Offer(45_000), Move::Offer(45_000)]);
        assert_eq!(g.price(), kaos.floor);
        assert_eq!(g.play(Move::Refuse).outcome, Outcome::WalkedAway);
    }

    #[test]
    fn accept_takes_the_current_price() {
        let d = data();
        let mangga = d.item("mangga").unwrap();
        let g = play(mangga, 20_000, 4, &[Move::AskPrice, Move::Accept]);
        let r = g.result().unwrap();
        assert_eq!(r.outcome, Outcome::Deal { price: mangga.ask });
        assert_eq!(r.saved, 0);
        assert!(!r.within_budget);
        assert_eq!(r.stars, 1, "only the politeness star");
    }

    #[test]
    fn budget_and_politeness_shape_the_stars() {
        let d = data();
        let kaos = d.item("kaos_batik").unwrap();
        // An offer over budget is allowed, but the result says so.
        let g = play(kaos, 50_000, 2, &[Move::Offer(56_000)]);
        let r = g.result().unwrap();
        assert!(!r.within_budget);
        assert_eq!(r.stars, 2);
        // A lowball under 40% of the fair price costs the politeness star.
        let g = play(kaos, 70_000, 2, &[Move::Offer(20_000), Move::Offer(56_000)]);
        let r = g.result().unwrap();
        assert_eq!(r.outcome, Outcome::Deal { price: 56_000 });
        assert_eq!(r.stars, 2);
        // Paying above the fair price within budget.
        let g = play(kaos, 70_000, 2, &[Move::Offer(65_000)]);
        assert_eq!(g.result().unwrap().stars, 2);
    }

    #[test]
    fn turn_limit_ends_with_a_final_price() {
        let d = data();
        let kaos = d.item("kaos_batik").unwrap();
        let mut g = Game::new(kaos, 60_000, 5);
        for i in 0..MAX_TURNS {
            let t = g.play(Move::AskPrice);
            assert_eq!(t.outcome, Outcome::Continue, "turn {i}");
        }
        let last = &g.turns()[MAX_TURNS - 1].1;
        let finals: Vec<String> =
            kaos.seller_lines.for_situation(Situation::Final).iter().map(|l| l.render(kaos.ask).0).collect();
        assert!(finals.contains(&last.text), "{}", last.text);
        assert_eq!(g.play(Move::AskPrice).outcome, Outcome::WalkedAway);

        let mut g = Game::new(kaos, 60_000, 5);
        for _ in 0..MAX_TURNS {
            g.play(Move::AskPrice);
        }
        assert_eq!(g.play(Move::Offer(56_000)).outcome, Outcome::Deal { price: 56_000 });
    }

    #[test]
    fn every_game_terminates_for_every_item() {
        let d = data();
        for item in &d.items {
            let budget = suggest_budget(item, 7);
            assert!(item.fair <= budget && budget < item.ask, "{}: budget {budget}", item.id);
            for seed in 0..40u64 {
                let mut g = Game::new(item, budget, seed);
                let mut rng = Rng::new(seed);
                let mut n = 0;
                while !g.is_over() {
                    let p = g.price();
                    let mv = match rng.below(6) {
                        0 => Move::AskPrice,
                        1 => Move::AskLower,
                        2 => Move::Refuse,
                        3 => Move::Accept,
                        _ => Move::Offer(p * (30 + rng.below(70) as u64) / 100),
                    };
                    let before = g.price();
                    let t = g.play(mv);
                    assert!(g.price() <= before, "{}: the price never goes up", item.id);
                    assert!(g.price() >= item.floor, "{}: never below the floor", item.id);
                    assert!(!t.text.is_empty() && !t.text.contains('{'));
                    if let (Some(p), Outcome::Continue) = (t.price, t.outcome) {
                        assert_eq!(p % step(p), 0, "{}: the seller names round prices ({p})", item.id);
                    }
                    n += 1;
                    assert!(n <= MAX_TURNS + 1, "{}: game did not end", item.id);
                }
                let r = g.result().unwrap();
                assert!(r.stars <= 3);
                if let Outcome::Deal { price } = r.outcome {
                    assert!(price >= item.floor && price <= item.ask);
                    assert_eq!(r.saved, item.ask - price);
                }
                // Same seed, same moves: same game.
                let replay =
                    play(item, budget, seed, &g.turns().iter().map(|(m, _)| m.clone()).collect::<Vec<_>>());
                assert_eq!(replay.turns(), g.turns());
            }
        }
    }

    #[test]
    fn hand_built_items_still_play() {
        let item = Item {
            id: "x".into(),
            name: "x".into(),
            meaning: Trilingual { arz: "س".into(), ar: "س".into(), en: "x".into() },
            ask: 10_000,
            floor: 6_000,
            fair: 7_000,
            javanese: false,
            bonus: false,
            seller_lines: LineBank::default(),
        };
        let mut g = Game::new(&item, 8_000, 1);
        assert_eq!(g.opening().text, "Sepuluh ribu");
        let t = g.play(Move::Offer(7_000));
        assert_eq!(t.outcome, Outcome::Deal { price: 7_000 });
        assert_eq!(t.meaning.en, "Rp7.000");
    }

    #[test]
    fn learner_phrases() {
        assert_eq!(phrase(&Move::AskPrice, false), "Berapa, Bu?");
        assert_eq!(phrase(&Move::AskLower, false), "Boleh kurang, Bu?");
        assert_eq!(phrase(&Move::Offer(25_000), false), "Kalau dua puluh lima ribu, boleh?");
        assert_eq!(phrase(&Move::Accept, false), "Oke, deal.");
        assert_eq!(phrase(&Move::Refuse, false), "Nggak, makasih, Bu.");
        assert_eq!(phrase(&Move::AskLower, true), "Kurang ya, Bu?");
        assert_eq!(phrase(&Move::Offer(25_000), true), "Nek selawe ewu, oleh?");
        assert_eq!(phrase(&Move::Offer(7_500), true), "Nek pitung ewu limang atus, oleh?");
        assert_eq!(phrase(&Move::Accept, true), "Ya wis, tak jupuk.");
        assert_eq!(phrase(&Move::Refuse, true), "Mboten, matur nuwun, Bu.");
        for mv in [Move::AskPrice, Move::AskLower, Move::Offer(25_000), Move::Accept, Move::Refuse] {
            check_meaning(&format!("{mv:?}"), &phrase_meaning(&mv));
        }
        assert!(phrase_meaning(&Move::Offer(25_000)).arz.contains("\u{2068}Rp25.000\u{2069}"));
    }
}
