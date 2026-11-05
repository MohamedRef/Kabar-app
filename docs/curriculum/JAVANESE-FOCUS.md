# Javanese = Yogyakarta + Madiun only (Mohamed, 2026-10-27)

Mohamed's standing rule: the Javanese track teaches only the Javanese spoken in **Yogyakarta and Madiun**. Both are Mataraman Javanese: the standard Solo–Jogja ngoko and krama, with Madiun's everyday ngoko.

Indonesian (BIPA) is unchanged.

This file is the plan every Javanese thread follows. The curriculum lead ("Write BIPA and Javanese curriculum") owns it.

## 0. Street speech, not textbook (Mohamed, 13:45)

Mohamed: "I want the language spoken in the street, not the textbook. I want to learn to talk with people."

For Javanese this means:
- **Dialogues sound like Jogja and Madiun people talking.**
  - Use the contracted ngoko people actually say: *ra* (ora), *wis*, *sik*, *ki* / *kuwi*, *neng*, *piye*.
  - Use the particles *lho*, *to*, *ta*, *ya*, *wae*.
  - Jogja's *je* and *og* ("Aku ra ngerti je", "Larang og") are welcome. Mark them **unverified** until checked against a source.
- **Krama as people use it**: shop and neighbour krama ("Pinten, Bu?", "Monggo", "Matur nuwun"), and ngoko alus with elders. Don't put full krama inggil in a market scene. J3 and J4 still teach the full system, but every unit shows when people really use each form.
- **Every lesson keeps the standard form visible.** The slide gives the full form next to the spoken one (ra = ora), so a learner can read and write it too.
- Each lesson's `new_words` lists the dictionary form. The spoken form goes in the slide and in the dialogue.

Indonesian gets the same treatment (Mohamed chose "both", 2026-10-27 15:18): see INDONESIAN-STREET.md.

## 1. Settings: new map

| Level | Was | Now | Notes |
|---|---|---|---|
| J1 | Surabaya (Suroboyo) | **Yogyakarta**, level title "Malioboro" | The biggest change. Landmarks: Malioboro, Tugu, angkringan, Prawirotaman, Kotabaru, Parangtritis (replaces Kenjeran), Trans Jogja (replaces Suroboyo Bus), Stasiun Tugu (replaces Gubeng). |
| J2 | Madiun | Madiun, no change | Already Mataraman. |
| J3 | Sala (Solo) | **Yogyakarta** | Pasar Klewer → Pasar Beringharjo. Laweyan batik → Giriloyo/Imogiri batik village. Oleh-oleh saking Jogja (bakpia is a Jogja souvenir anyway). The language doesn't change, because Solo krama is the same standard. |
| J4 | Ngayogyakarta | No change | |
| J5 | Semarang | **Madiun**, journey place "Kutha Pendekar (Madiun)" (Kota Madiun is officially "Kota Pendekar" since 2017: Kompas 2023, madiunkota.go.id) | Mbak Ayu moves to Madiun. Drop the Semarangan particles *ik*, *ndes* and *gak*. The Kudus and Demak episodes can stay as trips to Walisongo sites, but everyone speaks Mataraman. Campursari fits Madiun/Ngawi well. |
| J6 | Kutha Gedhe (Kotagede, Jogja) | No change | Sinetron e106 and e107 mention Surabaya and Semarang; reset them to Jogja/Madiun. |

## 2. Cast changes (owner: "Link personas and Radio to every unit")

| Who | Was | Now |
|---|---|---|
| Mbak Sekar (Radio host) | Solo | Moves to Jogja. Standard Mataraman with krama. |
| Mas Galih (Radio co-host) | Surabaya, Arek | Replaced by a Madiun host speaking Madiun ngoko. The name is the Radio thread's choice, but **not "Bayu"**: `bayu` is already a Madiun cast id. |
| Cak Heru | Surabaya, Arek | Becomes **Mas Heru**, an angkringan owner near Malioboro in Jogja. |
| Mas Yudha | Surabaya (Gubeng) | Moves to Jogja (Kotabaru). |
| Mbak Ayu | Semarang | Moves to Madiun. Same student role. |

Follow the cast registry rules: `content/cast.yaml` + `pending.yaml`, with renames logged.

## 3. Swap list (Arek, Semarangan, East-Java forms → Mataraman)

| Out | In |
|---|---|
| rek, cak (as "mate") | cah, mas, dhik, mbak |
| koen, awakmu | kowe (ngoko), sampeyan / panjenengan (krama) |
| gak | ora |
| onok | ana |
| kate | arep |
| yok opo | piye |
| lapo, opo'o | ngapa, kenapa |
| ae | wae |
| ambek | karo |
| sampek | nganti |
| mek | mung |
| ndek | neng, ing |
| engkok | mengko |
| maeng | mau |
| cangkruk | nongkrong |
| lek ("if") | yen |
| eroh | ngerti, weruh |
| nemen | banget, tenan |
| sepurane | ngapura, nyuwun sewu |
| garai | marakake, gawe |
| kakean | kakehan |
| arek ("kid") | bocah, cah |
| teko ("from") | saka |
| sek ("wait") | sik, dhisik |
| nang | neng (casual), menyang / ing (standard) |
| ik, ndes (Semarang) | drop them, or use *lho*, *to*, *ta* |

Chat spellings (piro, opo, ora → gk) are fine. They are how people type in Jogja too. Keep J1.u03 "Chat Jawa".

Madiun-specific everyday words can be added. Mark each one **unverified** until it has been checked against a source. Never use a Madiun word that contradicts the Jogja standard.

## 4. Level work (owners)

- **J1 (owner: J1–J2 thread):** relocate u01–u07 to Jogja. Rewrite **u05 "Arek Suroboyo"** as a casual Jogja ngoko unit, e.g. "Piye, cah?". It teaches karo, nganti, mung, neng, mengko, mau, nongkrong, yen, banget, kakehan, ngapura in place of the Arek set. Also j1.u02.l04 "arek" → bocah, and the Arek particles across u03–u07. Update the J1 titles in `curriculum/javanese.yaml`: level title, u01 "Piye kabare, Mas?", u04 "Sik, tak lapor", u05.
- **J3 (owner: J3 thread):** relocate from Solo to Jogja as in table 1, including the unit titles u05 and u06 in the curriculum file.
- **J5 (owner: J5–J6 thread):** relocate from Semarang to Madiun, Mbak Ayu included. Remove ik/ndes/gak.
- **J6 (owner: J5–J6 thread):** check the remaining Surabaya and Semarang mentions.
- **Sinetron + Radio (owner: personas/Radio thread):**
  - Sinetron e71–e77 (J1, Surabaya), e97, e99, e102, e106, e107.
  - Radio j1.u01–u07, plus any `nang`/`rek` in later units.
  - Apply the swap list, and use the new settings once each level's lessons land.
- **Glosses:** reference notes like "In Surabaya you'll hear …" may stay as notes. They must not be taught or tested.
- **Teach Kabar golden set:** it still parses pasted Surabaya text, because Teach handles whatever text the learner brings. No change.

## 5. Guard (owner: curriculum lead)

`content/javanese/dialect-guard.yaml` lists the forms from table 3. The content validator reports any of them as whole words in Javanese lessons, Sinetron and Radio.

It starts as an **advisory CI warning**. It becomes an **error** once the rewrites above have merged.

Exceptions go in its `allow` list, each with a reason (for example a gloss note or a quoted song title).

## 6. Journey Map (curriculum lead, geo.rs)

The route runs Jogja → Madiun → Jogja → Jogja → Madiun → Kotagede, along the Jogja–Madiun line and back. Each level gets its own pin, nudged apart where the real places sit within a few km of each other:

- J1 "Malioboro, Yogyakarta"
- J3 "Pakualaman, Yogyakarta"
- J4 "Yogyakarta" (the Kraton)
- J6 Kotagede
- J2 "Madiun"
- J5 "Kutha Pendekar (Madiun)"

The map label is the text before the comma or bracket.

Souvenirs and landmarks belong to one level each. J4 has Tugu, Malioboro street, Kraton, Taman Sari, Kotabaru and Prambanan. J3 has Lempuyangan, Kranggan, Kauman, Pakualaman, Beringharjo and Giriloyo. J1 has Alun-alun Kidul, Jalan Kaliurang, Parangtritis, Trans Jogja/Prawirotaman and Klithikan.
