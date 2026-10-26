//! Pasar! (spec §14): haggle with a market seller under a budget, with
//! numbers, prices, polite refusals and Javanese market words (piro, larang,
//! murah). Seller lines are build-time content in `content/games/pasar.toml`;
//! the seller's behaviour is deterministic for a seed.

use kabar_protocol::content::Trilingual;

/// Parsed `content/games/pasar.toml`.
#[derive(Clone, Debug, Default)]
pub struct PasarData {
    pub items: Vec<Item>,
}

impl PasarData {
    pub fn parse(toml_text: &str) -> Result<Self, String> {
        let _ = toml_text;
        todo!("m10 agent A")
    }

    pub fn item(&self, id: &str) -> Option<&Item> {
        let _ = id;
        todo!("m10 agent A")
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

#[derive(Clone, Debug)]
pub struct Game {
    item: Item,
    budget: u64,
    seed: u64,
    turns: Vec<(Move, SellerTurn)>,
}

impl Game {
    pub fn new(item: &Item, budget: u64, seed: u64) -> Self {
        Self { item: item.clone(), budget, seed, turns: Vec::new() }
    }

    pub fn item(&self) -> &Item {
        &self.item
    }

    pub fn budget(&self) -> u64 {
        self.budget
    }

    /// The seller's greeting and opening line.
    pub fn opening(&self) -> SellerTurn {
        let _ = self.seed;
        todo!("m10 agent A")
    }

    pub fn play(&mut self, mv: Move) -> SellerTurn {
        let _ = (mv, &self.turns);
        todo!("m10 agent A")
    }

    pub fn turns(&self) -> &[(Move, SellerTurn)] {
        &self.turns
    }

    /// Final score once the game is over.
    pub fn result(&self) -> Option<PasarResult> {
        todo!("m10 agent A")
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
pub fn phrase(mv: &Move, javanese: bool) -> String {
    let _ = (mv, javanese);
    todo!("m10 agent A")
}
