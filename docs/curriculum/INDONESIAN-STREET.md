# Indonesian = street speech too (Mohamed, 2026-10-27)

Mohamed: "I want the language spoken in the street, not the textbook. I want to learn to talk with people." When asked whether this covers Indonesian too, he chose **both** (decision card, 15:18). The curriculum lead ("Write BIPA and Javanese curriculum") owns this file. Javanese has its own rules in JAVANESE-FOCUS.md §0.

## What changes

1. **Dialogues, Sinetron, Radio and chat sound like real casual Indonesian**, the kind spoken in Jogja and Madiun (so aku / kamu, not Jakarta gue / lo):
   - nggak / gak (tidak), udah (sudah), belum, aja (saja), gimana (bagaimana), kenapa / ngapain, gitu / gini (begitu / begini), banget, emang (memang), kayak (seperti), sama (dengan / kepada), bikin (membuat), pake (pakai), kasih (beri).
   - Particles: kok, sih, dong, deh, kan, ya, lho, tuh, nih.
   - Verbs: bare roots in speech (Aku beli, Mau makan?), and **-in** for -kan / -i (bikinin, beliin, tungguin).
   - Clipped words and greetings: makasih, permisi, misi, Mas / Mbak / Pak / Bu.
   - Chat: gk, udh, blm, aja, yg, dmn, otw, wkwk.
2. **Polite speech the way people use it.** Use Bapak / Ibu, saya and formal verbs with officials, at a clinic, in a bank, a job interview or a presentation. These scenes still teach the formal register, and the unit shows when to switch.
3. **The standard form stays visible.** Put the pair (nggak = tidak, udah = sudah, bikinin = membuatkan) in an existing slide's text or in the taught word's gloss (tidak: "on the street: nggak"). Don't add a new slide for it: in BIPA 1 that pushed lessons past the 420 s cap. The shared street→formal list is content/freq/id.street.tsv; add new pairs there, keeping it sorted, and reading coverage will count them. Reading texts, news, letters, forms and BIPA writing tasks stay in standard Indonesian.
4. **`new_words` keep the dictionary form**, with the street form in the gloss (e.g. `word: sudah`, `en: already (spoken: udah)`). A street word that has no formal twin (kok, dong, sih, nongkrong) is taught as itself.
5. **Closed exercises accept both.** Where an answer could be written either way, list the other form in `alternatives:` (the exercise schema supports it). Never mark a learner wrong for using the formal form.
6. Levels: BIPA 1–2 introduce nggak / udah / aja / gimana / kok / ya next to the formal forms from the first dialogues. BIPA 3–4 add -in, sih / dong / deh, and register switching. BIPA 5–7 keep both: casual conversation plus formal reading, writing and speeches.

## Owners

- BIPA 1–2 and the shared rules: curriculum lead.
- BIPA 3–7 dialogues: each level thread, in the next pass over its units.
- Sinetron, Radio and personas: "Link personas and Radio to every unit".
