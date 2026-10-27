# Kabar cast

Who is who in Kabar, which units each character belongs to, and the rules
that keep every person, story and episode tied to the curriculum.

The source of truth is [`content/cast.yaml`](../content/cast.yaml)
(`kabar.cast/v1`); units still waiting for content are in
[`content/pending.yaml`](../content/pending.yaml) (`kabar.pending/v1`).
`cargo test -p kabar-content` checks both (`crates/content/src/cast.rs`).

## Rules (enforced in CI)

1. **One name, one person.** Two characters never share a name once
   honorifics are dropped (Pak, Bu, Mas, Mbak, Kak, Mbah, Hagga …), unless
   they are listed as `family` (Pak and Bu Harjo). A speaker anywhere in the
   content whose name matches a listed character must *be* that character
   (by `name` or `aliases`).
2. **Everyone who matters is listed.** Personas, Radio hosts and game
   partners must be in the registry, and so must anyone who speaks in more
   than 2 units' lessons or more than 4 Sinetron episodes. One-off extras
   (a fishmonger, a ticket officer) don't need an entry; role labels
   (`Petugas`, `Bakul`, `Narrator` …) are listed under `roles`.
3. **Every character belongs to the curriculum.** Each entry lists the
   `units` (or whole `levels`) it belongs to, and `in` names every program
   it appears in (lessons, tests, sinetron, radio, chat, placement, games,
   pagi, coach). A persona's own `units:` must be a subset of its entry's.
4. **No orphan programs.** Every Sinetron episode has `unlock_after`, every
   Radio script a `unit`, every persona `units`, and every Unggah-Ungguh
   scene is opened by some lesson (`game: uu.<scene>`).
5. **Every unit gets everything.** Each curriculum unit has a Radio episode
   and a Sinetron episode, and each level has a chat partner, or the unit
   (level) is listed in `pending.yaml`. Listing a unit that already has its
   episode fails too, so the pending list only ever shrinks.

Portraits (`assets/src/personas/<id>.png`) use the registry `id`. Never
rename an `id`: persona ids are stored in synced chats. Rename `name`
instead and keep the old name as an alias only if it is the same person.

## Name decisions (2026-10-27)

The integration audit (`critique/2026-10-27-integration-map.md`) found one
name used for several people. The main character kept the name; the others
were renamed everywhere (Latin, Arabic lines, aksara spellings, curriculum
titles).

| Name | Kept for | Renamed |
|---|---|---|
| Bu Siti | the Solo market persona (Pasar Gede), also the J3 lessons and the `uu.market` scene | Kabar Pagi warung → Bu Murni; Gianyar warung (bipa2.u05, Radio) → Bu Aminah; Banjarmasin host (bipa5.u07) → Bu Hamidah; Nganjuk seller (j2.u03) → Bu Karti; aksara girl (j6.u02) → Wati |
| Mas Ardi | the Bali receptionist persona, who also runs the placement chat | Radio co-host → Kak Reza; Palangka Raya MC (bipa5.u06) → Mas Iwan |
| Mbak Gadis | the Jakarta office persona | Radio host → Kak Tiara |
| Mas Dimas | the Yogyakarta persona, the J lessons and the `uu.friend` scene (was Bayu) | BIPA lessons' Jakarta friend → Raka |
| Pak Budi | the Sinetron warung father | BIPA lessons' teacher → Pak Irwan |
| Sari | the Sinetron daughter (also J2 lessons) | BIPA lessons' landlady Bu Sari → Bu Yanti |
| Bayu | the Sinetron son (also J2 lessons) | Rina's brother (bipa1.u03) → Fadil; Surabaya student (J1, J5) → Mas Yudha; Kotagede silversmith (j6.u04) → Mas Bimo |
| Mas Joko | the Sinetron warung helper | bipa1.u04 → Mas Eko; Papua (bipa7.u04) → Mas Tarno; Yogya (J4) Pak Joko → Pak Marto; Kabar Pagi Pak Joko → Pak Udin |
| Nadia | the Sinetron friend in Cairo | bipa6.u07 Kak Nadia → Kak Intan |
| Bu Tini | the Sinetron warung regular | j4.u04 → Bu Warsih |
| Pak Hendra | the Balikpapan manager (BIPA 5 lessons and Sinetron) | bipa1.u03 → Pak Haris; j4.u05 Mas Hendra → Mas Rudi |
| Bu Lestari | the Balikpapan HR head (BIPA 5) | Sinetron e60 → Bu Laksmi (the same academic as e70) |
| Kaka Yohana | the Sorong radio reporter (BIPA 7) | Sinetron e67 Bu Yohana → Bu Theresia |
| Mbah Putri | Dimas's grandmother, also the `uu.grandmother` scene (was Mbah Sri) | j3.u04 Mbah Darmi → Mbah Karsih |
| Mbak Ayu | the Semarang reader (J5) | bipa4.u04 Kak Ayu → Kak Nirmala |
| Kak Nur | the coach | bipa3.u08 Bu Nur → Bu Endah; j5.u05 Bu Nur → Bu Retno |
| Andi | the Makassar friend (BIPA 4) | bipa1.u02 Mas Andi → Mas Taufik; Kabar Pagi Andi → Adit |
| Rina | the Jakarta friend (lessons and Kabar Pagi are the same Rina) | j1.u06 Mbak Wulan → Mbak Indri (so Wulan is no longer a recurring name) |
| (Kabar Pagi) | | Dewi → Tasya, Lina → Nisa |

## Voices

Azure's free tier has two neural voices per language, so characters share
them: `id-ID-GadisNeural` speaks for Mbak Gadis, Kak Tiara and every
Indonesian woman; `id-ID-ArdiNeural` for Mas Ardi, Kak Reza, Pak Budi and
every Indonesian man; `jv-ID-SitiNeural` and `jv-ID-DimasNeural` do the same
in Javanese. The Sinetron tells its cast apart by pitch
(`SINETRON_CAST` in `crates/core/src/story.rs`); lessons pick the voice from
the honorific or the name (`crates/core/src/packs.rs`).

## The cast

| id | Name | Home | In | Belongs to |
|---|---|---|---|---|
| `mohamed` | Mohamed | Cairo | lessons, tests, chat | levels bipa1–j6 |
| `kak-nur` | Kak Nur | Jakarta | coach | levels bipa1–j6 |
| `mas-rizal` | Mas Rizal | Jakarta | lessons | levels bipa1, bipa2, bipa3, bipa4, bipa5, j5 |
| `rina` | Rina | Jakarta | lessons, pagi | levels bipa1, bipa2, bipa3, bipa4, bipa5 |
| `ibu-rina` | Ibu Rina | Jakarta | lessons | bipa1.u03 |
| `raka` | Raka | Jakarta | lessons, tests | levels bipa1, bipa2, bipa3, bipa6 |
| `ni-luh` | Ni Luh | Bali (lives in Jakarta) | lessons | levels bipa1, bipa2, bipa3 |
| `pak-irwan` | Pak Irwan | Jakarta | lessons, tests, radio | levels bipa1, bipa2, bipa3, bipa4 |
| `bu-yanti` | Bu Yanti | Jakarta | lessons, tests, radio | levels bipa1, bipa2, bipa3 |
| `andi` | Andi | Makassar | lessons | bipa4.u01, bipa4.u04, bipa4.u06 |
| `pak-hendra` | Pak Hendra | Balikpapan | lessons, sinetron | 7 units, bipa5.u01 … bipa5.u10 |
| `bu-lestari` | Bu Lestari | Balikpapan | lessons | 6 units, bipa5.u01 … bipa5.u06 |
| `bang-togar` | Bang Togar | Medan (family in Samosir) | lessons | bipa6.u01, bipa6.u06 |
| `kaka-yohana` | Kaka Yohana | Sorong | lessons | bipa7.u01, bipa7.u04 |
| `mbak-gadis` | Mbak Gadis | Jakarta | chat | 6 units, bipa1.u02 … bipa4.u01 |
| `mas-ardi` | Mas Ardi | Bali | chat, placement | 6 units, bipa1.u02 … bipa4.u05 |
| `mas-dimas` | Mas Dimas | Yogyakarta | chat, lessons, games | 16 units, j1.u01 … j6.u06 |
| `bu-siti` | Bu Siti | Solo (Pasar Gede) | chat, lessons, games | 8 units, bipa2.u04 … j3.u06 |
| `kak-tiara` | Kak Tiara | Jakarta | radio | levels bipa1, bipa2, bipa3, bipa4, bipa5, bipa6, bipa7 |
| `kak-reza` | Kak Reza | Bandung | radio | levels bipa1, bipa2, bipa3, bipa4, bipa5, bipa6, bipa7 |
| `cak-heru` | Cak Heru | Surabaya | lessons | j1.u01, j1.u02, j1.u03, j1.u04, j1.u05, j1.u06, j1.u07 |
| `mas-yudha` | Mas Yudha | Surabaya (Gubeng) | lessons | j1.u02, j1.u07, j5.u04 |
| `bu-darmi` | Bu Darmi | Caruban | lessons | j2.u02 |
| `pak-harjo` | Pak Harjo | Solo | lessons, games | j3.u03, j3.u06, j5.u02, j6.u05 |
| `bu-harjo` | Bu Harjo | Solo | lessons | j3.u03 |
| `mbah-putri` | Mbah Putri | a village near Yogyakarta | lessons, games | j3.u01, j3.u02, j4.u01, j4.u02, j4.u06 |
| `pak-karto` | Pak Karto | Solo | games | j3.u04, j4.u04 |
| `pak-harsono` | Pak Harsono | Yogyakarta | lessons | j4.u01, j4.u03, j4.u06 |
| `bu-harsono` | Bu Harsono | Yogyakarta | lessons | j4.u01, j4.u02, j4.u03 |
| `mbak-ayu` | Mbak Ayu | Semarang | lessons | j5.u01, j5.u02, j5.u03 |
| `pak-slamet` | Pak Slamet | Kotagede | lessons | j4.u02, j6.u01, j6.u04 |
| `pak-budi` | Pak Budi | Madiun | sinetron, lessons | levels bipa1, bipa2, bipa3, bipa4, bipa5, bipa6, bipa7, j2 |
| `bu-rahayu` | Bu Rahayu | Madiun | sinetron, lessons | levels bipa1, bipa2, bipa3, bipa4, bipa5, bipa6, bipa7, j2 |
| `sari` | Sari | Madiun (studies in Cairo) | sinetron, lessons | levels bipa1, bipa2, bipa3, bipa4, bipa5, bipa6, bipa7, j2 |
| `bayu` | Bayu | Madiun | sinetron, lessons | levels bipa1, bipa2, bipa3, bipa4, bipa5, bipa6, bipa7, j2 |
| `nadia` | Nadia | Cairo | sinetron | levels bipa1, bipa2, bipa3, bipa4, bipa5, bipa6, bipa7 |
| `karim` | Karim | Cairo (comes to Madiun) | sinetron | levels bipa1, bipa2, bipa3, bipa4, bipa5, bipa6, bipa7 |
| `mas-joko` | Mas Joko | Madiun | sinetron | levels bipa1, bipa2, bipa3, bipa4, bipa5, bipa6, bipa7 |
| `hagga-fatma` | Hagga Fatma | Cairo | sinetron | levels bipa1, bipa2, bipa3, bipa4, bipa5, bipa7 |
| `bu-tini` | Bu Tini | Madiun | sinetron | levels bipa2, bipa3, bipa4, bipa5, bipa6 |
| `kak-nella` | Kak Nella | Sorong | sinetron | bipa7.u02, bipa7.u03, bipa7.u04, bipa7.u06, bipa7.u07 |
| `omar` | Omar | Yogyakarta (from Egypt) | pagi | levels bipa1, bipa2 |
| `adit` | Adit | Yogyakarta | pagi | levels bipa1 |
| `bu-murni` | Bu Murni | Yogyakarta | pagi | levels bipa1 |
| `tasya` | Tasya | Yogyakarta | pagi | levels bipa1, bipa2 |
| `nisa` | Nisa | Yogyakarta | pagi | levels bipa1, bipa2 |
| `pak-udin` | Pak Udin | Yogyakarta | pagi | levels bipa1, bipa2 |
