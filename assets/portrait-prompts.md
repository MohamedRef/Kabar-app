# Portrait prompts (Codex, prompt 6a-6e)
Generated from `content/cast.yaml` and `assets/portraits.yaml` by `scripts/assets/portrait_prompts.py`; do not edit by hand. Send one batch at a time; each is self-contained.

## Prompt 6a (7 characters)

```text
Create character portraits for "Kabar", an Indonesian + Javanese learning app.
This batch is the people the learner talks to: the coach, the chat partners and the two Radio hosts.
These are ORIGINAL fictional characters. Draw all of them in this one session
with the same hand so they look like one cast.

HOW TO GENERATE (strict):
- Use your image generation tool. Do NOT draw the portraits with code (no SVG,
  Pillow, Cairo or canvas drawing).
- Generate EACH image as its own separate request, directly at 1024x1024. Never
  generate a sheet, grid or collage and split it. Never upscale a smaller image; only
  crop, never enlarge. No post-processing recolour, hue shift or filter.

STYLE (every portrait):
- Calm, warm, grown-up illustration. Flat vector-like shapes, soft layered depth,
  gentle paper grain, warm soft light. Not photorealistic, not anime, not cartoonish,
  no glossy 3D.
- Palette led by: Merapi charcoal #22201E, sawah green #4F8A3C, Bromo orange #EE8A5A,
  Bromo pink #E9A3A0, batik indigo #2B3A70, warm paper #F5EFE3, plus natural skin and
  hair tones.
- Head and shoulders, three-quarter view, friendly natural expression, eyes toward the
  viewer. Face centred with room around it so a circle crop never cuts the head.
- Drawn with dignity and respect: realistic proportions, no caricature, no exaggerated
  features, no stereotypes. Must not resemble any real person, celebrity or existing
  character. Do not imitate any named artist.
- Background: a soft, blurred hint of the setting, in the palette.
- If assets/src/personas/ already holds approved portraits, match their hand, light
  and framing exactly, so every batch looks like one cast.

HARD RULES: no text, letters, numbers, logos, name tags or cards with writing,
watermarks or signatures. Square 1024x1024 PNG.

CHARACTERS. Each file is named by the character's id; never rename a file.
- kak-nur.png (Kak Nur, Jakarta): a warm, encouraging Indonesian woman in her early 30s, soft pastel headscarf, simple indigo blouse, a small notebook held to her chest. Background: a bright Jakarta study corner with plants and a window.
  Who they are: The coach. Greets you in onboarding, plans the day and cheers you on.
- mbak-gadis.png (Mbak Gadis, Jakarta): a young professional Jakarta woman, late 20s, shoulder-length black hair, smart casual blazer, a lanyard with a blank card, a coffee cup in hand. Background: a modern office window with city towers.
  Who they are: Your office colleague in Jakarta; teaches the Jakarta slang people really speak.
- mas-ardi.png (Mas Ardi, Bali): a Balinese hotel receptionist around 30, short neat hair, a crisp uniform shirt with a small frangipani pin, a calm welcoming smile. Background: an open-air hotel lobby with carved wood and frangipani.
  Who they are: A hotel receptionist in Bali; also runs the two-minute placement chat when you first open Kabar.
- mas-dimas.png (Mas Dimas, Yogyakarta): a Javanese university student in Yogyakarta, early 20s, short black hair, a relaxed batik shirt, a canvas backpack strap on one shoulder. Background: a campus courtyard with a frangipani tree.
  Who they are: A university student in Yogyakarta, the friend who teaches you Javanese; Mbah Putri is his grandmother.
- bu-siti.png (Bu Siti, Solo (Pasar Gede)): a Javanese fruit-and-tempe seller in Solo, around 50, warm confident smile, a neat headscarf, a simple kebaya-style blouse. Background: baskets of fruit and tempe in the Pasar Gede market hall.
  Who they are: A fruit-and-tempe seller at Pasar Gede in Solo who haggles with you and models polite krama.
- kak-tiara.png (Kak Tiara, Jakarta): a cheerful Jakarta schoolteacher in her early 30s, wavy hair tied back, a light cardigan, studio headphones around her neck. Background: a cosy radio booth with a microphone on an arm.
  Who they are: Kabar Radio host, a Jakarta teacher who travels the journey with you. Shares a voice with Mbak Gadis.
- kak-reza.png (Kak Reza, Bandung): a friendly Bandung office worker in his mid 30s, short hair, rimless glasses, a plain shirt with rolled sleeves, studio headphones around his neck. Background: a cosy radio booth with a microphone on an arm.
  Who they are: Kabar Radio co-host, a Bandung office worker married to Laras. Shares a voice with Mas Ardi.

THEN, for kak-nur, mbak-gadis, mas-ardi, mas-dimas, bu-siti: 4 extra expressions each, with the SAME face, hair,
clothes, framing and background: <id>-listening.png, <id>-laughing.png,
<id>-thinking.png, <id>-explaining.png (a light hand gesture). Keep the identity
consistent; regenerate any that drift.

SAVE: all 27 masters to assets/src/personas/ with exactly the file names above.
Nothing else: Kabar's own script (scripts/assets/export_personas.py) makes the
web sizes and the cast sheet.

Do not commit. List the files when done, and confirm each image was generated
natively at 1024x1024 with the image tool.
```

## Prompt 6b (11 characters)

```text
Create character portraits for "Kabar", an Indonesian + Javanese learning app.
This batch is the cast of the Sinetron, a serialized story about a warung family in Madiun and their friends in Cairo.
These are ORIGINAL fictional characters. Draw all of them in this one session
with the same hand so they look like one cast.

HOW TO GENERATE (strict):
- Use your image generation tool. Do NOT draw the portraits with code (no SVG,
  Pillow, Cairo or canvas drawing).
- Generate EACH image as its own separate request, directly at 1024x1024. Never
  generate a sheet, grid or collage and split it. Never upscale a smaller image; only
  crop, never enlarge. No post-processing recolour, hue shift or filter.

STYLE (every portrait):
- Calm, warm, grown-up illustration. Flat vector-like shapes, soft layered depth,
  gentle paper grain, warm soft light. Not photorealistic, not anime, not cartoonish,
  no glossy 3D.
- Palette led by: Merapi charcoal #22201E, sawah green #4F8A3C, Bromo orange #EE8A5A,
  Bromo pink #E9A3A0, batik indigo #2B3A70, warm paper #F5EFE3, plus natural skin and
  hair tones.
- Head and shoulders, three-quarter view, friendly natural expression, eyes toward the
  viewer. Face centred with room around it so a circle crop never cuts the head.
- Drawn with dignity and respect: realistic proportions, no caricature, no exaggerated
  features, no stereotypes. Must not resemble any real person, celebrity or existing
  character. Do not imitate any named artist.
- Background: a soft, blurred hint of the setting, in the palette.
- If assets/src/personas/ already holds approved portraits, match their hand, light
  and framing exactly, so every batch looks like one cast.

HARD RULES: no text, letters, numbers, logos, name tags or cards with writing,
watermarks or signatures. Square 1024x1024 PNG.

CHARACTERS. Each file is named by the character's id; never rename a file.
- pak-budi.png (Pak Budi, Madiun): a Javanese warung owner in Madiun, around 55, kind face, a moustache, a black peci cap, an apron over a short-sleeved shirt. Background: a small food stall with a glass display case.
  Who they are: Runs the family warung in Madiun; Sari and Bayu's father.
- bu-rahayu.png (Bu Rahayu, Madiun): Pak Budi's wife and the warung's cook, early 50s, a practical headscarf, a batik apron, a wooden spoon in hand, a motherly smile. Background: a warm warung kitchen with a wok and steam.
  Who they are: Pak Budi's wife and the warung's cook; speaks Javanese at home.
- sari.png (Sari, Madiun (studies in Cairo)): Pak Budi's daughter, about 22, a student at Al-Azhar in Cairo, a headscarf, a tote bag of books on her shoulder. Background: a Cairo street with mashrabiya windows in warm evening light.
  Who they are: Pak Budi's daughter, a student at Al-Azhar in Cairo; later works in Balikpapan and marries Karim.
- bayu.png (Bayu, Madiun): Sari's younger brother, about 18, messy black hair, a T-shirt under an open flannel shirt, a phone in hand, an eager grin. Background: the front of the family warung in Madiun.
  Who they are: Sari's little brother at the warung; later a journalist with a fellowship in Papua.
- nadia.png (Nadia, Cairo): an Egyptian Al-Azhar student, about 22, a patterned headscarf, a long cardigan, bright lively eyes. Background: a Cairo university courtyard with arches.
  Who they are: Sari's Egyptian classmate and best friend in Cairo; Karim's sister.
- karim.png (Karim, Cairo (comes to Madiun)): Nadia's older brother, an Egyptian man in his late 20s, short dark curly hair, a trimmed beard, a simple shirt, a cooking towel over one shoulder. Background: the Madiun warung's kitchen, a pot of koshari on the stove.
  Who they are: Nadia's brother, who travels to Madiun, cooks koshari at the warung and marries Sari.
- mas-joko.png (Mas Joko, Madiun): the warung helper, mid 20s, a round cheerful face, a towel around his neck, a striped T-shirt, a puzzled but happy smile. Background: the warung's serving counter.
  Who they are: Works at Pak Budi's warung; always guessing wrong, always cheerful.
- hagga-fatma.png (Hagga Fatma, Cairo): an Egyptian landlady in her 60s, a dark headscarf and a long galabiya, a glass of tea in hand, a kind motherly face. Background: a Cairo apartment balcony with potted plants and old buildings behind.
  Who they are: Sari's landlady in Cairo, who offers everyone tea and comforts her when things go wrong.
- bu-tini.png (Bu Tini, Madiun): a talkative warung regular in her late 40s, a bright headscarf, a shopping basket on her arm, a knowing smile. Background: a Madiun street outside the warung.
  Who they are: A warung regular who knows everyone, and later adds three hundred guests to the wedding.
- kak-nella.png (Kak Nella, Sorong): a Papuan journalist in her late 20s from Sorong, natural curly hair, a colourful noken bag over her shoulder, a press vest without any writing. Background: Sorong harbour with boats.
  Who they are: Bayu's colleague in Sorong who teaches him Papuan Malay.
- pak-hendra.png (Pak Hendra, Balikpapan): a company manager in Balikpapan, late 40s, neat side-parted hair, a batik long-sleeved shirt, a composed, fair expression. Background: a meeting room with a view of Balikpapan bay.
  Who they are: The manager who interviews you in Balikpapan; in the Sinetron he is Sari's boss.

SAVE: all 11 masters to assets/src/personas/ with exactly the file names above.
Nothing else: Kabar's own script (scripts/assets/export_personas.py) makes the
web sizes and the cast sheet.

Do not commit. List the files when done, and confirm each image was generated
natively at 1024x1024 with the image tool.
```

## Prompt 6c (11 characters)

```text
Create character portraits for "Kabar", an Indonesian + Javanese learning app.
This batch is the people in the Indonesian lessons, from Jakarta to Papua.
These are ORIGINAL fictional characters. Draw all of them in this one session
with the same hand so they look like one cast.

HOW TO GENERATE (strict):
- Use your image generation tool. Do NOT draw the portraits with code (no SVG,
  Pillow, Cairo or canvas drawing).
- Generate EACH image as its own separate request, directly at 1024x1024. Never
  generate a sheet, grid or collage and split it. Never upscale a smaller image; only
  crop, never enlarge. No post-processing recolour, hue shift or filter.

STYLE (every portrait):
- Calm, warm, grown-up illustration. Flat vector-like shapes, soft layered depth,
  gentle paper grain, warm soft light. Not photorealistic, not anime, not cartoonish,
  no glossy 3D.
- Palette led by: Merapi charcoal #22201E, sawah green #4F8A3C, Bromo orange #EE8A5A,
  Bromo pink #E9A3A0, batik indigo #2B3A70, warm paper #F5EFE3, plus natural skin and
  hair tones.
- Head and shoulders, three-quarter view, friendly natural expression, eyes toward the
  viewer. Face centred with room around it so a circle crop never cuts the head.
- Drawn with dignity and respect: realistic proportions, no caricature, no exaggerated
  features, no stereotypes. Must not resemble any real person, celebrity or existing
  character. Do not imitate any named artist.
- Background: a soft, blurred hint of the setting, in the palette.
- If assets/src/personas/ already holds approved portraits, match their hand, light
  and framing exactly, so every batch looks like one cast.

HARD RULES: no text, letters, numbers, logos, name tags or cards with writing,
watermarks or signatures. Square 1024x1024 PNG.

CHARACTERS. Each file is named by the character's id; never rename a file.
- mas-rizal.png (Mas Rizal, Jakarta): the lesson guide, a friendly Jakarta man around 30, short tidy hair, a plain polo shirt, an open welcoming gesture. Background: a sunny Jakarta street with a bus stop.
  Who they are: The lesson guide. Friendly standard Indonesian; walks you through scenes from BIPA 1 to BIPA 5.
- rina.png (Rina, Jakarta): a Jakarta woman in her mid 20s, long black hair in a low ponytail, a denim jacket, a lively friendly smile. Background: Taman Fatahillah in the old town.
  Who they are: Your Jakarta friend from the first lesson; later a colleague and debate partner. Also in Kabar Pagi.
- ibu-rina.png (Ibu Rina, Jakarta): Rina's mother, late 50s, short greying hair, a long batik dress, welcoming open arms. Background: the front porch of a Jakarta family home with potted plants.
  Who they are: Rina's mother, who welcomes you to the family home.
- raka.png (Raka, Jakarta): a Jakarta man in his mid 20s who becomes a tour guide in Bali, a sun hat pushed back, a short-sleeved shirt, a lanyard with a blank card. Background: a Bali beach path with palm trees.
  Who they are: A Jakarta friend of Rina's who becomes a tour guide in Bali.
- ni-luh.png (Ni Luh, Bali (lives in Jakarta)): a Balinese woman in her mid 20s, long hair with a frangipani tucked behind one ear, a lace kebaya with a sash. Background: a Balinese temple gate with offerings.
  Who they are: Rina's Balinese friend; your guide to Bali in BIPA 2.
- pak-irwan.png (Pak Irwan, Jakarta): the Indonesian teacher, early 50s, glasses, greying temples, a patient smile, a batik shirt, a book under his arm. Background: a classroom with a whiteboard (blank) and a map of the islands without labels.
  Who they are: Your Indonesian teacher, patient and wears glasses.
- bu-yanti.png (Bu Yanti, Jakarta): the landlady (ibu kos), around 60, a headscarf, a house dress, a ginger cat in her arms. Background: a boarding-house courtyard with laundry lines.
  Who they are: Your landlady, who has a cat and cooks fried rice.
- andi.png (Andi, Makassar): a Makassar man in his late 20s, short hair, a smart shirt, a confident debating expression, one hand mid-gesture. Background: Losari beach in Makassar at sunset.
  Who they are: A Makassar friend who argues about careers, tourism and the news with you in BIPA 4.
- bu-lestari.png (Bu Lestari, Balikpapan): an HR head in Balikpapan, mid 40s, an elegant headscarf, a Sasirangan-patterned blouse, a warm professional smile. Background: a modern office lobby with a Dayak carving on the wall.
  Who they are: The Balikpapan company's HR head who hires you as a translator; she knows Kalimantan's arts and food. BIPA 5 chat partner.
- bang-togar.png (Bang Togar, Medan (family in Samosir)): a Batak man in Medan, late 30s, a broad laugh, a short beard, an ulos cloth over one shoulder of a plain shirt. Background: a Medan coffee shop.
  Who they are: A Batak perantau in Medan who loves short stories, satire and a good joke. BIPA 6 chat partner.
- kaka-yohana.png (Kaka Yohana, Sorong): a Papuan radio reporter in her 30s from Sorong, natural curly hair, a bright floral blouse, a small microphone in hand. Background: a small radio studio with a window on the sea.
  Who they are: A reporter at the local radio in Sorong who has you help write stories and explains Papuan Malay. BIPA 7 chat partner.

SAVE: all 11 masters to assets/src/personas/ with exactly the file names above.
Nothing else: Kabar's own script (scripts/assets/export_personas.py) makes the
web sizes and the cast sheet.

Do not commit. List the files when done, and confirm each image was generated
natively at 1024x1024 with the image tool.
```

## Prompt 6d (11 characters)

```text
Create character portraits for "Kabar", an Indonesian + Javanese learning app.
This batch is the people in the Javanese lessons and the politeness game, from Surabaya to Kotagede.
These are ORIGINAL fictional characters. Draw all of them in this one session
with the same hand so they look like one cast.

HOW TO GENERATE (strict):
- Use your image generation tool. Do NOT draw the portraits with code (no SVG,
  Pillow, Cairo or canvas drawing).
- Generate EACH image as its own separate request, directly at 1024x1024. Never
  generate a sheet, grid or collage and split it. Never upscale a smaller image; only
  crop, never enlarge. No post-processing recolour, hue shift or filter.

STYLE (every portrait):
- Calm, warm, grown-up illustration. Flat vector-like shapes, soft layered depth,
  gentle paper grain, warm soft light. Not photorealistic, not anime, not cartoonish,
  no glossy 3D.
- Palette led by: Merapi charcoal #22201E, sawah green #4F8A3C, Bromo orange #EE8A5A,
  Bromo pink #E9A3A0, batik indigo #2B3A70, warm paper #F5EFE3, plus natural skin and
  hair tones.
- Head and shoulders, three-quarter view, friendly natural expression, eyes toward the
  viewer. Face centred with room around it so a circle crop never cuts the head.
- Drawn with dignity and respect: realistic proportions, no caricature, no exaggerated
  features, no stereotypes. Must not resemble any real person, celebrity or existing
  character. Do not imitate any named artist.
- Background: a soft, blurred hint of the setting, in the palette.
- If assets/src/personas/ already holds approved portraits, match their hand, light
  and framing exactly, so every batch looks like one cast.

HARD RULES: no text, letters, numbers, logos, name tags or cards with writing,
watermarks or signatures. Square 1024x1024 PNG.

CHARACTERS. Each file is named by the character's id; never rename a file.
- cak-heru.png (Cak Heru, Surabaya): a Surabaya man in his late 20s, a big easygoing grin, short hair, a T-shirt and an open shirt, a motorbike helmet in hand. Background: a Surabaya street with old buildings.
  Who they are: Your Surabaya mate in J1; speaks Arek Javanese.
- mas-yudha.png (Mas Yudha, Surabaya (Gubeng)): a Surabaya university student, early 20s, glasses, a hoodie, earbuds around his neck. Background: Gubeng station hall.
  Who they are: A student in Surabaya you meet in J1; later sends you code-mixed voice notes.
- bu-darmi.png (Bu Darmi, Caruban): the pecel warung owner in Caruban, late 50s, a headscarf, a batik apron, a plate of pecel on a banana leaf in hand. Background: a roadside warung with a bench.
  Who they are: Runs the pecel warung in Caruban where you order food in J2.
- pak-harjo.png (Pak Harjo, Solo): an older Javanese gentleman from Solo, late 60s, a blangkon headcloth, a lurik jacket, a dignified, patient face. Background: a Solo pendopo with carved wooden pillars.
  Who they are: An older gentleman who corrects your krama. Your Javanese teacher in Solo, later the batik maker and tomb keeper you meet on the way to Kotagede.
- bu-harjo.png (Bu Harjo, Solo): Pak Harjo's wife, mid 60s, hair in a neat sanggul bun, a kebaya with a batik kain, a gentle smile. Background: a Javanese house interior with a batik cloth on a stand.
  Who they are: Pak Harjo's wife.
- mbah-putri.png (Mbah Putri, a village near Yogyakarta): Mas Dimas's grandmother, late 70s, silver hair in a bun, a kebaya and a selendang over her shoulder, deep laugh lines. Background: a village house porch near Yogyakarta with rice fields beyond.
  Who they are: Mas Dimas's grandmother. You use krama inggil for her; she answers a guest in simple polite krama. J4 chat partner.
- pak-karto.png (Pak Karto, Solo): an elderly neighbour from Solo, about 75, a black peci, a white beard, a sarong and a plain shirt. Background: a quiet Solo lane with a low fence.
  Who they are: Your elderly neighbour in the Unggah-Ungguh game; krama with a little krama inggil.
- pak-harsono.png (Pak Harsono, Yogyakarta): the Yogyakarta host father, early 60s, a blangkon, a surjan striped jacket, a calm, formal kindness. Background: a Yogyakarta joglo house with a garden.
  Who they are: Your host father in Yogyakarta; the krama inggil lessons use him and his wife.
- bu-harsono.png (Bu Harsono, Yogyakarta): the Yogyakarta host mother, late 50s, a headscarf, a kebaya, a tray of tea glasses in her hands. Background: a Yogyakarta joglo house with a garden.
  Who they are: Your host mother in Yogyakarta.
- mbak-ayu.png (Mbak Ayu, Semarang): a Semarang woman in her late 20s, a headscarf, glasses, a laptop held under her arm, an attentive look. Background: a library reading room in Semarang.
  Who they are: A Javanese literature student in Semarang who checks your writing and reads news, posts, stories and songs with you. J5 chat partner.
- pak-slamet.png (Pak Slamet, Kotagede): an aksara Jawa teacher in Kotagede, mid 50s, a blangkon, a batik shirt, a brush pen in hand. Background: a Kotagede lane with old walls.
  Who they are: A Kotagede silversmith who engraves names in aksara Jawa and teaches you the script and Pegon. J6 chat partner.

SAVE: all 11 masters to assets/src/personas/ with exactly the file names above.
Nothing else: Kabar's own script (scripts/assets/export_personas.py) makes the
web sizes and the cast sheet.

Do not commit. List the files when done, and confirm each image was generated
natively at 1024x1024 with the image tool.
```

## Prompt 6e (6 characters)

```text
Create character portraits for "Kabar", an Indonesian + Javanese learning app.
This batch is the people in Kabar Pagi, short morning stories set in Yogyakarta.
These are ORIGINAL fictional characters. Draw all of them in this one session
with the same hand so they look like one cast.

HOW TO GENERATE (strict):
- Use your image generation tool. Do NOT draw the portraits with code (no SVG,
  Pillow, Cairo or canvas drawing).
- Generate EACH image as its own separate request, directly at 1024x1024. Never
  generate a sheet, grid or collage and split it. Never upscale a smaller image; only
  crop, never enlarge. No post-processing recolour, hue shift or filter.

STYLE (every portrait):
- Calm, warm, grown-up illustration. Flat vector-like shapes, soft layered depth,
  gentle paper grain, warm soft light. Not photorealistic, not anime, not cartoonish,
  no glossy 3D.
- Palette led by: Merapi charcoal #22201E, sawah green #4F8A3C, Bromo orange #EE8A5A,
  Bromo pink #E9A3A0, batik indigo #2B3A70, warm paper #F5EFE3, plus natural skin and
  hair tones.
- Head and shoulders, three-quarter view, friendly natural expression, eyes toward the
  viewer. Face centred with room around it so a circle crop never cuts the head.
- Drawn with dignity and respect: realistic proportions, no caricature, no exaggerated
  features, no stereotypes. Must not resemble any real person, celebrity or existing
  character. Do not imitate any named artist.
- Background: a soft, blurred hint of the setting, in the palette.
- If assets/src/personas/ already holds approved portraits, match their hand, light
  and framing exactly, so every batch looks like one cast.

HARD RULES: no text, letters, numbers, logos, name tags or cards with writing,
watermarks or signatures. Square 1024x1024 PNG.

CHARACTERS. Each file is named by the character's id; never rename a file.
- omar.png (Omar, Yogyakarta (from Egypt)): an Egyptian student in Yogyakarta, early 20s, short dark hair, a light beard, a backpack, a friendly homesick smile. Background: a Yogyakarta campus path with a mosque dome behind.
  Who they are: An Egyptian student in Yogyakarta. Class, the mosque, Ramadan and calls home to Cairo.
- adit.png (Adit, Yogyakarta): Omar's Indonesian friend, early 20s, a round face, a baseball cap, a bowl of noodles in his hands. Background: Bu Murni's warung.
  Who they are: Omar's friend, always hungry, a regular at Bu Murni's warung.
- bu-murni.png (Bu Murni, Yogyakarta): the warung owner in Yogyakarta, early 50s, a headscarf, a batik apron, a ladle in hand, a busy happy face. Background: a small warung with pots on the stove.
  Who they are: Runs the little warung Adit loves.
- tasya.png (Tasya, Yogyakarta): a schoolgirl of about 12 in a white-and-red primary school uniform, hair in two braids, a backpack, a bright smile. Background: a family home's front yard in Yogyakarta.
  Who they are: A schoolgirl whose family's days at home and at the beach fill several Pagi stories.
- nisa.png (Nisa, Yogyakarta): a young woman of about 20, a pastel headscarf, a sling bag, a gentle smile. Background: a Yogyakarta street with trees.
  Who they are: Appears in a couple of Pagi stories.
- pak-udin.png (Pak Udin, Yogyakarta): an office worker in his mid 40s, a short-sleeved office shirt, an ojek helmet in hand, a market bag on his arm. Background: a Sunday market in Yogyakarta.
  Who they are: An office worker who shops at the market on Sundays and rides an ojek to work.

SAVE: all 6 masters to assets/src/personas/ with exactly the file names above.
Nothing else: Kabar's own script (scripts/assets/export_personas.py) makes the
web sizes and the cast sheet.

Do not commit. List the files when done, and confirm each image was generated
natively at 1024x1024 with the image tool.
```
