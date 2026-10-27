# Kabar — Codex prompts for visual assets

Every prompt below is complete on its own: the style, the rules, the sizes and where to save the files are inside it. Copy one whole block into Codex, run it, review the result, then move to the next.

Order: 1 → 2 (after you pick an icon concept) → 3 → 4 → 5 → 6 → 7 → 8. Prompt 8 sets up the free assets from the internet.

The last section is a reference list of the free assets and their licenses (checked 26 Oct 2026). It's for you, not for Codex.

---

## Prompt 1 — App icon: three concepts

```text
You are creating the app icon for "Kabar", a private Indonesian + Javanese learning app
for one learner (an Egyptian Arabic speaker). Create 3 icon concepts as images, then
stop so I can choose one.

STYLE (apply to every image):
- Calm, warm, grown-up travel-journal illustration. Flat vector-like shapes, soft
  layered depth, gentle paper grain, minimal gradients, clean edges.
- Never childish, never noisy, no glossy 3D, no photorealism.
- Use ONLY this palette: Merapi charcoal #22201E, sawah green #4F8A3C, Bromo dawn
  orange #EE8A5A, Bromo dawn pink #E9A3A0, batik indigo #2B3A70, warm paper #F5EFE3.
  Bridge gold #C9A23A only where I say so.
- Ornament, if any: simplified ORIGINAL motifs inspired by Javanese batik (four-petal
  kawung circles, diagonal parang bands). Never copy a real batik.

HARD RULES:
- No text, letters, numbers or script of any kind anywhere in the image (image models
  garble letters; the app renders all words itself).
- No logos, watermarks, signatures, real brands or existing characters. Do not imitate
  any named artist or studio.
- Square 1024x1024 PNG, full-bleed background (no rounded corners, no drop shadow; the
  operating system applies the mask).
- The emblem must fit inside the central 60% of the canvas and stay readable at
  16x16 px: bold, few shapes, strong contrast.

CONCEPTS:
A) "Kawung bubble": full-bleed batik indigo #2B3A70 background. A rounded speech
   bubble formed from one kawung motif (four soft oval petals around a small circle),
   petals in warm paper #F5EFE3, the small centre circle in Bridge gold #C9A23A, the
   bubble's tail pointing down-left.
B) "Dawn over sawah": full-bleed Merapi charcoal #22201E background. A rounded speech
   bubble; inside it three stepped rice-terrace bands in sawah green below a dawn sky
   fading from Bromo pink to Bromo orange, with a small rising sun.
C) "The bridge": full-bleed warm paper #F5EFE3 background. Two overlapping rounded
   speech bubbles, the left in batik indigo and the right in sawah green, linked by a
   thin elegant arch in Bridge gold like a small bridge.

SAVE AS:
assets/src/icon/concept-a.png, concept-b.png, concept-c.png (create folders if
missing). Also save a preview sheet assets/src/icon/concepts-preview.png showing each
concept at 1024, 180, 64, 32 and 16 px side by side, so I can judge small sizes.
Do not commit; tell me the paths when done.
```

---

## Prompt 2 — App icon: layers and every platform size

Replace `{X}` with the concept you picked (A, B or C).

```text
I picked concept {X} from assets/src/icon/concept-{x}.png for the Kabar app icon.
Recreate the SAME design (same shapes, same colours, same proportions) as separate
layers, then export every size Android and macOS need.

KEEP THE STYLE: flat vector-like shapes, gentle paper grain, palette only Merapi
charcoal #22201E, sawah green #4F8A3C, Bromo orange #EE8A5A, Bromo pink #E9A3A0, batik
indigo #2B3A70, warm paper #F5EFE3, Bridge gold #C9A23A. No text, letters or numbers.
No logos or watermarks.

1) LAYERS (1024x1024 PNG each), save in assets/src/icon/:
   - background.png: only the full-bleed background, no emblem.
   - foreground.png: only the emblem on a fully transparent background. Keep the
     emblem inside the central circle of diameter 61% of the canvas (Android adaptive
     icon safe zone: 66dp of 108dp).
   - monochrome.png: the emblem as one flat solid white (#FFFFFF) silhouette on
     transparent background, same size and position (Android 13+ themed icons,
     notification icon, macOS menu-bar glyph).
   - full.png: background + foreground combined.

2) EXPORTS: write a small, reproducible script (Python with Pillow, or ImageMagick;
   free tools only, no online services) at scripts/assets/export_icons.py (or .sh) that
   generates everything below from the four layers. High-quality downscaling (Lanczos).
   Put outputs under assets/export/.

   Android -> assets/export/android/res/
   - Adaptive icon layers, 108dp: mipmap-mdpi 108px, -hdpi 162, -xhdpi 216,
     -xxhdpi 324, -xxxhdpi 432: ic_launcher_foreground.png, ic_launcher_background.png,
     ic_launcher_monochrome.png
   - mipmap-anydpi-v26/ic_launcher.xml and ic_launcher_round.xml (adaptive-icon XML
     with background, foreground and monochrome).
   - Legacy launcher 48dp: mipmap-mdpi 48px, -hdpi 72, -xhdpi 96, -xxhdpi 144,
     -xxxhdpi 192: ic_launcher.png (square) and ic_launcher_round.png (circle crop).
   - Notification small icon 24dp, white silhouette from monochrome.png: drawable-mdpi
     24px, -hdpi 36, -xhdpi 48, -xxhdpi 72, -xxxhdpi 96: ic_stat_kabar.png.
   - Splash screen (Android 12+): drawable/splash_icon.png at 960x960 (240dp at
     xxxhdpi) with the emblem kept inside the central 2/3 circle.

   macOS -> assets/export/macos/
   - Kabar.iconset/ with icon_16x16.png, icon_16x16@2x.png (32), icon_32x32.png,
     icon_32x32@2x.png (64), icon_128x128.png, icon_128x128@2x.png (256),
     icon_256x256.png, icon_256x256@2x.png (512), icon_512x512.png,
     icon_512x512@2x.png (1024).
   - For these, place the artwork as a macOS-style rounded square: an 824x824 body
     centered on a transparent 1024 canvas, continuous-corner radius about 185px, so
     macOS 26 does not put it on a grey plate.
   - If running on macOS, also build Kabar.icns with `iconutil -c icns`; otherwise
     leave a README line saying to run that command on the Mac.
   - menubar-template.png 18x18 and menubar-template@2x.png 36x36: black silhouette
     with alpha (template image), from monochrome.png.

   Web -> assets/export/web/
   - icon-192.png, icon-512.png, maskable-512.png (emblem inside the central 80%),
     favicon-32.png.

3) Save a preview sheet assets/export/preview.png showing the Android icon in circle,
   squircle and square masks, the macOS icon at 1024/128/32/16, and the notification
   icon on a dark background.

Do not commit. List every file you created.
```

---

## Prompt 3 — In-app icon set (SVG code, not images)

```text
Write custom SVG icons for "Kabar", an Indonesian + Javanese learning app. The app
already uses the Lucide icon set for generic icons (home, settings, search, play, mic,
pen, etc.), so these custom icons MUST look like they belong to Lucide.

WRITE CODE, NOT IMAGES: hand-write each icon as clean SVG markup. Do not generate
raster images and do not trace any existing icon.

GRID AND STYLE (identical to Lucide):
- <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"
  fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"
  stroke-linejoin="round">
- 2px padding inside the 24px grid; coordinates on whole or half pixels.
- Outline only, no fills (except tiny solid dots drawn as small circles where noted),
  no text, no letters, no numbers.
- Few simple paths per icon; readable at 16px.
- Colour comes only from currentColor, so CSS can recolour them (word statuses,
  Bridge gold #C9A23A, etc.). No hard-coded colours.
- Must look right mirrored horizontally (the Arabic UI is right-to-left), except
  icons where direction carries meaning; list those in the README.

ICONS (filename = kebab-case name + .svg):
- bridge-word: a small arch bridge with a dot above it (Arabic-origin word badge)
- false-friend: two overlapping speech bubbles with an exclamation mark between them
- souvenir: a luggage tag with a four-petal flower punched in it
- suitcase: a vintage suitcase with one four-petal flower sticker
- take-a-flight: a small plane rising over a dashed arc (placement test)
- journey-island: a simple island with one palm tree and a wave line
- train-stop: a front-view train with a map pin above it
- respect-meter: a half-circle gauge with three ticks and a needle
- ngoko, krama, krama-inggil: three variants of one hand gesture (open palm; palm with
  one small bow line; palm with two bow lines) for the Javanese respect levels
- pasar: a market stall awning with a scalloped edge
- rupiah: a banknote outline with a single circle in the middle (no letters)
- kabar-pagi: a sun half-risen behind an open book
- kabar-radio: a vintage radio with two small sound waves
- sinetron: a TV screen with a small house roof inside
- coach: a speech bubble with a small heart inside
- lasso: a dashed closed loop with a pen tip at one end
- card-wall: three overlapping cards in a fan
- gong: a hanging gong on a simple frame
- kendang: a side-view barrel drum
- streak-ember: a small calm flame merged with a leaf shape (encouraging, not
  alarming)

SAVE TO: assets/icons/custom/<name>.svg (create folders if missing).
Also create assets/icons/custom/preview.html that shows every custom icon next to a
few Lucide icons (load lucide from a pinned CDN version, e.g. unpkg) at 16, 20, 24 and
48px, in black on white and white on #22201E, and once mirrored, so I can review
consistency. Add assets/icons/custom/README.md listing each icon, its meaning, and
"Original, created for Kabar; same license as the project".

Do not commit. Tell me when preview.html is ready.
```

---

## Prompt 4 — Souvenir cards

```text
Create souvenir card illustrations for "Kabar", an Indonesian + Javanese learning
app. Each finished lesson stop gives the learner a collectible card with a short
culture note. You make ONLY the artwork; the app overlays the title and the note.

STYLE (identical for every card, so they look like one collection):
- Calm, warm, grown-up travel-journal illustration. Flat vector-like shapes, soft
  layered depth, gentle paper grain, minimal gradients, clean edges. Feels like a
  collectible travel stamp crossed with a postcard.
- Never childish, never noisy, no glossy 3D, no photorealism.
- Palette only: Merapi charcoal #22201E, sawah green #4F8A3C, Bromo orange #EE8A5A,
  Bromo pink #E9A3A0, batik indigo #2B3A70, warm paper #F5EFE3. No gold.
- Respectful, accurate depiction of Indonesian culture; no stereotypes or caricature.

COMPOSITION (every card):
- Portrait 1024x1536 PNG.
- Subject centered in the upper two thirds.
- Thin rounded border with a subtle ORIGINAL kawung (four-petal circles) or parang
  (diagonal bands) pattern in batik indigo at low contrast. Never copy a real batik.
- Bottom third: calm plain warm paper #F5EFE3 with no detail (text goes there).

HARD RULES: no text, letters, numbers or script anywhere (not on signs, packaging or
cloth). No logos, watermarks, signatures, brands, or existing characters. Do not
imitate any named artist.

CARDS (filename: subject):
- batik.png: hands drawing wax lines with a canting tool on white cloth, a small pot
  of wax beside it
- wayang.png: a shadow-puppet silhouette cast on a glowing screen by an oil lamp; an
  original puppet design, not a known character
- gamelan.png: a row of bronze bonang kettle gongs on a carved wooden frame
- rendang.png: a clay bowl of dark rendang with a banana leaf and red chillies
- angklung.png: three bamboo angklung instruments of different heights
- kawung.png: an elegant original kawung motif tile, like a ceramic tile
- becak.png: a cycle rickshaw parked under a tree in soft morning light
- kopi-tubruk.png: a glass of thick Javanese coffee with grounds settling, on a saucer
- sate.png: skewers of satay on a small charcoal grill with soft smoke
- tongkonan.png: a Toraja tongkonan house with its sweeping boat-shaped roof
- rumah-gadang.png: a Minangkabau house with horn-shaped roof peaks
- raja-ampat.png: small green karst islets in turquoise water seen from above
- kotagede-silver.png: a small silver filigree brooch on a cloth

SAVE: masters to assets/src/souvenirs/<name>.png. Then, with a small reproducible script in
scripts/assets/ (Pillow or ImageMagick, free tools only), export to assets/export/souvenirs/:
<name>.webp at 768x1152 (quality 85) and <name>-thumb.webp at 256x384.
Make assets/src/souvenirs/contact-sheet.png with all cards in a grid so I can check
they look like one consistent set; regenerate any card that doesn't match the others.

Do not commit. List the files when done.
```

---

## Prompt 4b — Souvenir cards, redo (one card per image, native resolution)

Use this instead of prompt 4. The first try split one small sheet into ten cards and blew each up about 3x, and a colour filter turned the green leaves orange.

```text
Redo the souvenir card illustrations for "Kabar", an Indonesian + Javanese learning
app. Each finished lesson stop gives the learner a collectible card; you make ONLY the
artwork, the app overlays the title and the culture note.

HOW TO GENERATE (strict):
- Generate EACH card as its own separate image request, directly at 1024x1536
  (portrait). Thirteen cards = thirteen separate generations.
- Never generate a sheet, grid or collage and split it. Never upscale a smaller image.
  If the tool can't output 1024x1536, generate the nearest native portrait size
  (e.g. 1024x1536 from 2:3) and only crop, never enlarge.
- No post-processing recolour, hue shift or filter on the artwork. Get the colours
  right in the generation itself.
- Delete the previous batch first: assets/src/souvenirs/*, assets/export/souvenirs/*,
  scripts/assets/prepare_souvenirs.py.

STYLE (identical for every card, so they look like one collection; generate them in
one session and reuse the same wording):
- Calm, warm, grown-up travel-journal illustration. Flat vector-like shapes, soft
  layered depth, gentle paper grain, minimal gradients, clean edges. Feels like a
  collectible travel stamp crossed with a postcard.
- Never childish, never noisy, no glossy 3D, no photorealism.
- Palette led by: Merapi charcoal #22201E, sawah green #4F8A3C, Bromo orange #EE8A5A,
  Bromo pink #E9A3A0, batik indigo #2B3A70, warm paper #F5EFE3. Plants and leaves stay
  green. No gold or yellow-metallic highlights (gold is reserved in the app).
- Respectful, accurate depiction of Indonesian culture; no stereotypes or caricature.

COMPOSITION (every card):
- The illustration fills the upper two thirds of the card.
- A thin rounded border with a subtle ORIGINAL kawung (four-petal circles) or parang
  (diagonal bands) pattern in batik indigo at low contrast. Never copy a real batik.
- Bottom third: calm, plain warm paper #F5EFE3 with no detail (text goes there).

HARD RULES: no text, letters, numbers or script anywhere (not on signs, packaging or
cloth). No logos, watermarks, signatures, brands, or existing characters. Do not
imitate any named artist.

CARDS (filename: subject):
- batik.png: hands drawing wax lines with a canting (a tiny copper cup with a thin
  spout on a bamboo handle) on white cloth, a small pot of warm wax beside it
- wayang.png: a shadow-puppet silhouette cast on a glowing screen by an oil lamp; an
  original puppet design, not a known character
- gamelan.png: a row of bronze bonang kettle gongs on a carved wooden frame
- rendang.png: a clay bowl of dark rendang with a banana leaf and red chillies
- angklung.png: three bamboo angklung instruments of different heights
- kawung.png: an elegant original kawung motif tile, like a ceramic tile
- becak.png: a cycle rickshaw parked under a green tree in soft morning light
- kopi-tubruk.png: a glass of thick Javanese coffee with grounds settling, on a saucer
- sate.png: skewers of satay on a small charcoal grill with soft smoke
- tongkonan.png: a Toraja tongkonan house with its sweeping boat-shaped roof
- rumah-gadang.png: a Minangkabau house with horn-shaped roof peaks
- raja-ampat.png: small green karst islets in turquoise water seen from above
- kotagede-silver.png: a small silver filigree brooch on a cloth

SAVE: masters to assets/src/souvenirs/<name>.png (1024x1536). Then, with the existing
scripts/assets/export_souvenirs.py (update its NAMES list to all 13), export to
assets/export/souvenirs/: <name>.webp at 768x1152 (quality 85) and
<name>-thumb.webp at 256x384. Make assets/src/souvenirs/contact-sheet.png with all
13 cards in a grid; regenerate any card that doesn't match the others.

Do not commit. List the files when done, and confirm each master was generated
natively at its final size.
```

---

## Prompt 4c — Souvenir cards, six to redo

Review of the 4b batch: batik, rendang, angklung, kawung, kopi-tubruk, sate and tongkonan are approved and already in the repo. Three cards drifted into a detailed, painterly style (rumah-gadang, raja-ampat, kotagede-silver) and three have accuracy problems (becak, gamelan, wayang). This prompt redoes only those six.

```text
Redo SIX souvenir card illustrations for "Kabar", an Indonesian + Javanese learning
app. Seven cards of this set are already approved; the six new ones must look like
they belong to the same collection.

BEFORE YOU START:
- Pull the latest main. Open the approved masters in assets/src/souvenirs/ (batik.png,
  rendang.png, angklung.png, kawung.png, kopi-tubruk.png, sate.png, tongkonan.png) and
  assets/src/souvenirs/contact-sheet.png. Match them exactly: the same flat shapes,
  the same thick dark outlines, the same pink sky with a pale sun disc, the same
  rolling hills, the same card frame with a thin rounded border and small indigo
  marks, the same plain warm paper bottom area, and the same picture size and
  position on the card.

HOW TO GENERATE (strict):
- Generate EACH card as its own separate image request, directly at 1024x1536
  (portrait). Six cards = six separate generations.
- Never generate a sheet, grid or collage and split it. Never upscale a smaller image.
  Only crop, never enlarge.
- No post-processing recolour, hue shift or filter. Get the colours right in the
  generation itself.

STYLE (identical to the approved cards):
- Calm, warm, grown-up travel-journal illustration. FLAT vector-like shapes with
  bold dark outlines, a few large colour areas, very little texture, minimal
  gradients. NOT painterly, NOT detailed rendering, NOT realistic lighting, no fine
  filigree shading, no photographic water. If a card looks more detailed than
  tongkonan.png or sate.png, simplify it.
- Palette led by: Merapi charcoal #22201E, sawah green #4F8A3C, Bromo orange #EE8A5A,
  Bromo pink #E9A3A0, batik indigo #2B3A70, warm paper #F5EFE3. Plants and leaves stay
  green. No gold or yellow-metallic highlights (gold is reserved in the app).
- Respectful, accurate depiction of Indonesian culture; no stereotypes or caricature.

COMPOSITION (as on the approved cards):
- Picture in a rounded panel in the upper part of the card, same size and position
  as the approved cards.
- Thin rounded border with the same small indigo marks as the approved cards.
- Bottom area: calm, plain warm paper #F5EFE3 with no detail (text goes there).

HARD RULES: no text, letters, numbers or script anywhere (not on signs, packaging or
cloth). No logos, watermarks, signatures, brands, or existing characters. Do not
imitate any named artist.

CARDS (filename: subject, with what must be right):
- becak.png: an Indonesian becak parked under a green tree in soft morning light.
  It must be the Indonesian kind: the hooded PASSENGER SEAT IS AT THE FRONT, between
  two front wheels, and the rider's bicycle seat and pedals are BEHIND it over a
  single rear wheel. Three wheels in total. Not an Indian/Bangladeshi rickshaw (cab
  behind the bike), not a motorbike.
- gamelan.png: a bonang, a row of bronze kettle gongs resting on crossed cords in a
  long, low carved wooden frame. Each kettle is a rounded bronze pot with a raised
  knob (pencu) ON TOP, seen from slightly above and to the side, so the knobs read
  as round bosses on the top of each pot. They must NOT look like eyes or faces.
  Two padded beaters lie beside the frame.
- wayang.png: a wayang kulit shadow on a glowing white screen (kelir), lit by an oil
  lamp (blencong) hanging in front. The puppet is an ORIGINAL design in the wayang
  kulit manner: a flat, finely pierced figure seen in strict profile, long slender
  arms on thin control rods, a stylised head with a long nose and a headdress, and
  a central handle rod going down into a banana-trunk base. A pure dark shadow with
  no face drawn on it. Not a stick figure or robot, not a known character.
- rumah-gadang.png: a Minangkabau rumah gadang with several horn-shaped (gonjong)
  roof peaks sweeping up, a long wooden house on stilts, green rice fields in
  front. Flat style like tongkonan.png, not detailed.
- raja-ampat.png: small green karst islets in turquoise water seen from above.
  Flat style: islets as simple rounded green shapes with a darker outline and a
  pale ring of shallow water, water as two or three flat turquoise tones. Not
  painterly, not photographic.
- kotagede-silver.png: a small silver filigree brooch (a flower) lying on an indigo
  cloth. Draw the silver as a light grey shape with simple dark outlines and a few
  curled line details, like the other flat cards. Not a detailed 3D render.

SAVE: masters to assets/src/souvenirs/<name>.png (1024x1536), replacing nothing else.
Then run: python3 scripts/assets/export_souvenirs.py
It exports all 13 cards to assets/export/souvenirs/ and rebuilds
assets/src/souvenirs/contact-sheet.png. Look at the contact sheet: if any of the six
new cards stands out from the seven approved ones, regenerate it.

Do not commit. List the files when done, and confirm each master was generated
natively at its final size.
```

---

## Prompt 4d — Souvenir cards, last four

Review of the 4c batch (2026-10-27): rumah-gadang and kotagede-silver are approved and in the repo. Four still miss: becak (drawn as a full two-wheeled bicycle towing a cart, three wheels in a row), gamelan (kettles look like bells, a stray see-through frame over the sky, random cords), wayang (a walking cartoon figure with legs and dots on the head, not a wayang kulit profile, no banana-trunk base) and raja-ampat (islets read as lily pads with leaf veins, seen from the side, one cut by the frame). 4c was drawn with code (SVG), which matches the set's flat look; that is fine, but the shapes must be right.

```text
Redo FOUR souvenir card illustrations for "Kabar", an Indonesian + Javanese learning
app. Nine cards of this set are approved; the four new ones must look like they
belong to the same collection.

BEFORE YOU START:
- Pull the latest main. Open the approved masters in assets/src/souvenirs/ (batik,
  rendang, angklung, kawung, kopi-tubruk, sate, tongkonan, rumah-gadang,
  kotagede-silver .png) and assets/src/souvenirs/contact-sheet.png. Match them
  exactly: the same flat shapes, the same thick dark outlines, the same pink sky with
  a pale sun disc, the same rolling hills, the same card frame with a thin rounded
  border and small indigo marks, the same plain warm paper bottom area, and the same
  picture size and position on the card.

HOW TO MAKE THEM:
- Each card is its own image, made directly at 1024x1536 (portrait). Never make a
  sheet and split it, never upscale, no recolour filters afterwards.
- Drawing with code (SVG rendered at full size) is fine, since the approved cards
  look that way, but every subject below must be recognisable and correct. Before
  saving, look at each card at 256x384 and ask: would an Indonesian recognise this
  object at a glance? If not, redraw it.

STYLE (identical to the approved cards):
- FLAT vector-like shapes with bold dark outlines, a few large colour areas, very
  little texture. Not painterly, not realistic.
- Palette led by: Merapi charcoal #22201E, sawah green #4F8A3C, Bromo orange #EE8A5A,
  Bromo pink #E9A3A0, batik indigo #2B3A70, warm paper #F5EFE3. Plants stay green.
  No gold or yellow-metallic highlights.
- Respectful, accurate depiction of Indonesian culture; no stereotypes.

HARD RULES: no text, letters, numbers or script anywhere. No logos, watermarks,
signatures, brands or existing characters. Do not imitate any named artist. Nothing
may be see-through or overlap the frame; every object sits fully inside the picture
panel.

CARDS (filename: subject, with exactly what must be right):
- becak.png: an Indonesian becak, side view, parked under a green tree, facing right.
  Exactly THREE wheels, and in side view you see only TWO of them: one front wheel
  under the passenger seat (its twin is hidden behind it) and one rear wheel. At the
  FRONT (right): a wide passenger bench seat for two with a folding hood (a curved
  canopy) over it and a low footrest in front of it. BEHIND it (left): a single
  bicycle saddle raised above the rear wheel, pedals and chain below the saddle,
  joined to the seat's back by one sturdy frame. The rider's handlebar is a bar
  behind the hood, NOT a separate bicycle with its own front wheel. No rider.
- gamelan.png: a Javanese bonang seen from the front and slightly above. A long, low
  wooden frame (rancak) with carved orange ends, standing on short legs on the
  ground. On it, TWO straight rows of five bronze kettle gongs each, all the same
  size, resting on cords stretched along the frame. Each kettle: a wide, low, round
  pot (wider than tall, like an upturned bowl with a flat shoulder), in Bromo orange
  with a dark outline, with a small round raised knob (pencu) in the CENTRE OF ITS
  TOP, and the knob is a solid filled circle, never a ring or a dot pair. Two
  padded beaters (sticks with indigo cord-wrapped heads) lie on the frame. Nothing
  else above the frame: just the pink sky and the pale sun.
- wayang.png: a wayang kulit puppet seen as a shadow on a glowing white screen
  (kelir) with a dark wooden frame, lit by an oil lamp (blencong) hanging in front
  near the top. The shadow is ONE solid dark silhouette in STRICT SIDE PROFILE,
  facing right, standing still (no walking pose): a tall slender body with a long
  flowing cloth (wiron) falling to the feet, both feet side by side pointing right,
  a small head with a long pointed nose and a tall pointed headdress, and two very
  long thin arms hanging down in front, each held by a thin rod (cempurit). Inside
  the silhouette, a few small cut-out patterns (simple diamonds, circles and curls)
  so it looks pierced like real leather puppets. No eyes, no face, no dots on the
  head. One central rod (gapit) runs from the head down through the body into a
  horizontal banana-trunk (gedebog) at the bottom of the screen, drawn as a pale
  green log. Original design, not a known character.
- raja-ampat.png: small karst islets in a turquoise sea, seen from directly ABOVE
  (map view: no hills, no horizon, no sky, no sun; the whole picture panel is sea).
  Water in two or three flat turquoise tones, darker in the middle, lighter near the
  islets. Six or seven islets of different sizes, each a lumpy irregular rounded
  shape (like a mushroom cap seen from above), filled with two greens (darker rim,
  lighter top), a dark outline and a pale sand/shallow-water ring around it. No
  leaf veins, no midribs, no pointed tips: they must not look like leaves or lily
  pads. All islets fully inside the panel, one small boat (a white speck with a
  short wake) for scale.

SAVE: masters to assets/src/souvenirs/<name>.png (1024x1536), replacing nothing else.
Then run: python3 scripts/assets/export_souvenirs.py
It exports all 13 cards to assets/export/souvenirs/ and rebuilds
assets/src/souvenirs/contact-sheet.png. Look at the contact sheet: if any of the four
new cards stands out from the nine approved ones, or any subject is hard to
recognise at thumbnail size, redo it.

Do not commit. List the files when done.
```

---

## Prompt 5 — Journey Map decorations

```text
Create decorative artwork for the Journey Map in "Kabar", an Indonesian + Javanese
learning app. IMPORTANT: the map itself (coastlines, routes, stop markers, the plane
and train tokens) is drawn by the app in SVG code from Natural Earth public-domain
geodata. Do NOT draw a map. You only make (1) a paper backdrop and (2) small landmark
vignettes that sit on the map next to each stop.

STYLE (every image):
- Calm, warm, grown-up travel-journal illustration. Flat vector-like shapes, soft
  layered depth, gentle paper grain, minimal gradients. Never childish or noisy, no
  glossy 3D, no photorealism.
- Palette only: Merapi charcoal #22201E, sawah green #4F8A3C, Bromo orange #EE8A5A,
  Bromo pink #E9A3A0, batik indigo #2B3A70, warm paper #F5EFE3.
- Respectful, recognisable but simplified depictions of real places.

HARD RULES: no text, letters, numbers, labels or signage anywhere. No logos,
watermarks, signatures or brands. Do not imitate any named artist.

1) BACKDROPS (1024x1024 PNG, must tile seamlessly on all four edges; verify by tiling
   2x2 and fixing any seams):
   - paper-light.png: warm handmade paper #F5EFE3 with very faint fibres and an
     extremely subtle sea-wave line pattern in batik indigo at about 4% opacity.
   - paper-dark.png: the same idea on Merapi charcoal #22201E paper with the waves in
     warm paper at about 4% opacity.
   No objects on the backdrops.

2) VIGNETTES (1024x1024 PNG, transparent background): each a tiny self-contained scene
   inside a soft circle, like a sticker on a travel map; simple shapes, readable at
   128px, all in one consistent style.
   Indonesian route (islands):
   - jakarta.png: the National Monument tower with its flame top above city rooftops
   - bali.png: a split temple gate (candi bentar) with rice terraces behind it
   - lombok.png: Mount Rinjani with its crater lake, a small fishing boat in front
   - sulawesi.png: two tongkonan houses with boat-shaped roofs
   - kalimantan.png: a wooden longboat on a jungle river under tall trees
   - sumatra.png: a Minangkabau rumah gadang with horn roofs and a volcano behind
   - raja-ampat.png: green karst islets in turquoise water
   Javanese train line (cities):
   - surabaya.png: a long suspension bridge over a strait at dusk
   - madiun.png: an old train station platform with a locomotive and a small pecel
     food stall
   - solo.png: a royal palace pavilion (pendopo) with batik cloth hanging nearby
   - yogyakarta.png: Prambanan temple spires at dawn
   - semarang.png: a colonial building with many doors and arched windows
   - kotagede.png: an old Javanese mosque gate with a banyan tree

SAVE: masters to assets/src/map/. With a small reproducible script in scripts/assets/ (Pillow or
ImageMagick, free tools only), export to assets/export/map/: backdrops as WebP at
1024 and 512; each vignette as <name>.webp at 512x512 (tablet/Mac map) and
<name>-sm.webp at 256x256 (phone route), keeping transparency.
Make assets/src/map/contact-sheet.png with all vignettes on paper-light so I can check
consistency; regenerate any that don't match.

Do not commit. List the files when done.
```

---

## Prompt 6 — Personas and the Sinetron cast

```text
Create character portraits for "Kabar", an Indonesian + Javanese learning app. These
are ORIGINAL fictional adult characters: AI conversation partners and the cast of a
short serialized story. Draw all of them in this one session with the same hand so
they look like one cast.

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
  viewer. Face centered with room around it so a circle crop doesn't cut the head.
- Drawn with dignity and respect: realistic proportions, no caricature, no exaggerated
  features, no stereotypes. Must not resemble any real person, celebrity or existing
  character. Do not imitate any named artist.
- Background: a soft, blurred hint of the setting in the palette.

HARD RULES: no text, letters, numbers, logos, name tags with writing, watermarks or
signatures. Square 1024x1024 PNG.

CHARACTERS (filename: description):
- dimas.png (Mas Dimas): a Javanese university student in Yogyakarta, early 20s, short
  black hair, relaxed batik shirt, canvas backpack strap on one shoulder; background: a
  campus courtyard with a frangipani tree.
- siti.png (Bu Siti): a Javanese market seller in Solo, around 50, warm confident
  smile, neat headscarf, simple kebaya-style blouse; background: baskets of produce in
  a traditional market.
- gadis.png (Mbak Gadis): a young professional woman in Jakarta, late 20s, smart
  casual blazer, lanyard without text, coffee cup in hand; background: a modern office
  window with city towers.
- ardi.png (Mas Ardi): a hotel receptionist in Bali, around 30, neat uniform shirt with
  a small frangipani pin, calm welcoming smile; background: an open-air hotel lobby
  with carved wood.
- budi.png (Pak Budi): a Javanese warung (small food stall) owner in Madiun, around 55,
  kind face, moustache, black peci cap, apron over a short-sleeve shirt; background: a
  small food stall with a glass display case.
- sari.png (Sari): Pak Budi's daughter, about 22, a student living in Cairo, headscarf,
  tote bag of books; background: a Cairo street with mashrabiya windows in warm
  evening light.

THEN, for each character, 4 extra expressions with the SAME face, hair, clothes,
framing and background: <name>-listening.png, <name>-laughing.png,
<name>-thinking.png, <name>-explaining.png (a light hand gesture). Keep identity
consistent; regenerate any that drift.

SAVE: masters to assets/src/personas/. With a small reproducible script in scripts/assets/ (Pillow or
ImageMagick, free tools only), export to assets/export/personas/: <file>.webp at
1024 (intro screen), 256x256 and 96x96 (avatars; the app crops them to a circle in
CSS, so keep them square). Make assets/src/personas/cast-sheet.png showing all six
neutral portraits side by side plus each one's expressions, so I can check
consistency.

Do not commit. List the files when done, and confirm each image was generated
natively at 1024x1024 with the image tool.
```

---

## Prompt 7 — First-launch hero, empty states, celebration, notebook paper

```text
Create supporting illustrations for "Kabar", an Indonesian + Javanese learning app
whose tone is a calm, warm study companion: part travel journal, part market, part
tutor. Encouraging, never guilt, fear or sadness.

STYLE (every image):
- Calm, warm, grown-up travel-journal illustration. Flat vector-like shapes, soft
  layered depth, gentle paper grain, minimal gradients. Never childish or noisy, no
  glossy 3D, no photorealism.
- Palette only: Merapi charcoal #22201E, sawah green #4F8A3C, Bromo orange #EE8A5A,
  Bromo pink #E9A3A0, batik indigo #2B3A70, warm paper #F5EFE3, and Bridge gold
  #C9A23A only where noted.

HARD RULES: no text, letters, numbers or script anywhere (the app renders all words,
including the "Apa kabar?" greeting and the Arabic word on top of the hero). No
logos, watermarks, signatures, brands or existing characters. Do not imitate any named
artist.

IMAGES:
1) hero-landscape.png (1536x1024) and hero-portrait.png (1024x1536): a quiet dawn over
   Javanese rice terraces, a volcano softly in the distance, sky from Bromo pink to
   Bromo orange, one simple empty speech-bubble shape floating in the sky with a tiny
   Bridge-gold glint. Leave a calm empty area in the sky (top centre) for a big
   greeting the app will draw.
2) Empty states, 1024x1024 transparent background, simple centered scene with lots of
   empty space, gentle and encouraging, no sad faces:
   - empty-suitcase.png: an open vintage suitcase waiting to be filled, one flower
     sticker on it
   - all-reviewed.png: a cup of tea next to a closed notebook on a small table by a
     window at sunset (nothing due, rest)
   - offline.png: a small boat resting by a calm shore under a starry sky
3) celebrate-gong.png, 1024x1024 transparent: a hanging gamelan gong on a carved frame
   with soft concentric rings of sound. Static image; the app animates it in CSS.
4) notebook-paper.png, 1024x1024, must tile seamlessly on all edges (verify by tiling
   2x2): warm paper #F5EFE3 with faint fibres and very faint dot grid in batik indigo
   at about 5% opacity.

SAVE: masters to assets/src/misc/. With a small reproducible script in scripts/assets/ (Pillow or
ImageMagick, free tools only), export to assets/export/misc/: hero images as WebP at
full size and half size; empty states and the gong as WebP at 640 and 320 px with
transparency; notebook-paper as WebP at 1024 and 512.

Do not commit. List the files when done.
```

---

## Prompt 8 — Set up the free, openly licensed assets

```text
Set up the free third-party assets for "Kabar", a private, non-commercial Indonesian +
Javanese learning app (Rust + Dioxus, macOS + Android). Rules for this task:
- $0: only free, openly licensed assets. Nothing that needs an API key, account or
  payment. No online conversion services.
- For EACH asset, open its own LICENSE file from the official source and confirm the
  license before using it. If the license is not exactly what is listed below, stop
  and tell me instead of using it.
- Pin exact versions (release tag or commit hash) and record them.
- Copy each license text next to the asset.
- Do NOT use: Flaticon, Freepik, Vecteezy, unDraw, Storyset, GADM, Google Maps or
  Mapbox tiles, or any photos of batik or wayang from the web (including Wikimedia
  Commons). The app uses original art only.

1) ICONS: Lucide (ISC license; some icons derive from Feather, MIT; keep both
   notices). Copy only the SVGs we use into assets/icons/lucide/ with LICENSE. Start
   with: house, settings, search, play, pause, mic, pen-line, highlighter, eraser,
   book-open, notebook-pen, languages, volume-2, check, x, undo-2, chevron-left,
   chevron-right, map, plane, train-front, trophy, calendar-days, bell, cloud-off,
   refresh-cw, star, heart, message-circle, image, camera, share-2, download, info.
   Fallback only if a needed icon is missing: Tabler Icons (MIT).

2) FONTS (all must be SIL Open Font License 1.1). Put each in assets/fonts/<family>/
   with its OFL.txt. Prefer variable fonts and subset nothing yet.
   - Plus Jakarta Sans (github.com/tokotype/PlusJakartaSans): Latin UI, Indonesian and
     Javanese text.
   - IBM Plex Sans Arabic (github.com/IBM/plex): Arabic UI and explanations.
   - Noto Naskh Arabic (github.com/notofonts/arabic): Arabic words on Bridge cards.
   - Amiri (github.com/aliftype/amiri): the decorative "خبر → kabar" reveal.
   - Noto Sans Javanese (github.com/notofonts/javanese): Javanese script (Aksara Jawa).
   - Noto Emoji, monochrome (github.com/googlefonts/noto-emoji): consistent emoji.
   Make assets/fonts/preview.html showing "Apa kabar? Sugeng enjing. Selamat pagi."
   and "كلمة ممكن خبر فكر" in every face, so I can check Arabic shaping and RTL.

3) GEODATA for the Journey Map: Natural Earth 1:10m (public domain; the official
   license says no permission or credit needed, but credit it anyway). Download from
   naturalearthdata.com or github.com/nvkelso/natural-earth-vector:
   ne_10m_land, ne_10m_minor_islands, ne_10m_populated_places, ne_10m_railroads,
   ne_10m_admin_0_countries. Store raw files in data/natural-earth/ (git-ignore the
   raw shapefiles if over 5 MB and add a download script instead). Write a script in scripts/assets/ that
   crops to Indonesia (about 94°E–142°E, 11°S–6°N), simplifies the shapes, and writes
   assets/map/indonesia.svg with: island outlines, a separate group for Java, and
   points for Jakarta, Bali (Denpasar), Lombok (Mataram), Sulawesi (Makassar),
   Kalimantan (Balikpapan), Sumatra (Padang), Raja Ampat (Waisai), and on Java:
   Surabaya, Madiun, Solo (Surakarta), Yogyakarta, Semarang, Kotagede. Place the Java
   railway lines from ne_10m_railroads. Keep the SVG under 300 KB, no styling inside
   (the app styles it with CSS classes). If a town is missing from populated_places,
   add it by hand with its coordinates and note that in the README.

4) SOUNDS:
   - Kenney "Interface Sounds" (kenney.nl/assets/interface-sounds, CC0): download,
     keep only a few soft clicks/toggles for buttons and page turns, put them in
     assets/sounds/ui/ with the license file.
   - Gong (lesson complete) and kendang tap (correct answer): do NOT download; write a
     small script in scripts/assets/ that synthesizes them, so there is no license risk. Gong: a few
     inharmonic partials (e.g. base around 110 Hz with partials at roughly 1, 1.48,
     2.03, 2.71 ratios), slow exponential decay over about 2.5 s, a gentle attack and
     a slight pitch glide. Kendang tap: a short pitched membrane thump around
     180–220 Hz plus a little filtered noise, about 150 ms. Render 48 kHz mono, export
     OGG (Vorbis) and keep the source script. Make them soft and warm, never harsh;
     normalize to about -16 LUFS. Make 2–3 variations of each so I can choose.
   - Optional: real gamelan recordings from Freesound ONLY if the individual file's
     license is "Creative Commons 0"; check each file's own page; record the URL,
     author and license. Skip this if unsure.

5) ATTRIBUTION: create or update ATTRIBUTION.md at the repo root with one row per
   asset: name, source URL, version/commit, license, what we use it for, files. Include
   the Codex-generated art as "Original artwork created for Kabar with Codex".

Do not commit. Summarize what you added and any license that didn't match.
```

---

## Reference: free assets and licenses (for you, not for Codex)

I checked each project's own LICENSE file on 26 Oct 2026.

| Asset | License | Use |
|---|---|---|
| [Lucide](https://github.com/lucide-icons/lucide) | ISC | Main UI icons |
| [Tabler Icons](https://github.com/tabler/tabler-icons) | MIT | Fallback icons |
| [Phosphor Icons](https://github.com/phosphor-icons/core) | MIT | Alternative with softer duotone weights |
| [Material Symbols](https://github.com/google/material-design-icons) | Apache 2.0 | Android-native bits if needed |
| [Plus Jakarta Sans](https://github.com/tokotype/PlusJakartaSans) | OFL 1.1 | Latin UI (made by a Jakarta studio) |
| [Inter](https://github.com/rsms/inter) | OFL 1.1 | Alternative Latin face |
| [IBM Plex Sans Arabic](https://github.com/IBM/plex) | OFL 1.1 | Arabic UI |
| [Noto Naskh Arabic](https://github.com/notofonts/arabic) | OFL 1.1 | Arabic on Bridge cards |
| [Amiri](https://github.com/aliftype/amiri) | OFL 1.1 | Decorative Arabic |
| [Noto Sans Javanese](https://github.com/notofonts/javanese) | OFL 1.1 | Aksara Jawa |
| [Noto Emoji](https://github.com/googlefonts/noto-emoji) | OFL 1.1 (font) | Emoji |
| [Natural Earth](https://www.naturalearthdata.com/) 1:10m | Public domain | Map geodata, including the `railroads` layer for the Java train line |
| [Kenney Interface Sounds](https://kenney.nl/assets/interface-sounds) | CC0 | UI clicks |
| Freesound, "Creative Commons 0" filter | CC0 per file | Real gamelan sounds, only after checking each file by hand (the site blocks automated checks, so I couldn't pre-verify any) |
| [Fluent Emoji](https://github.com/microsoft/fluentui-emoji) | MIT | Optional pictures for vocabulary words |
| [OpenMoji](https://github.com/hfg-gmuend/openmoji) | CC BY-SA 4.0 | Optional; share-alike, so edited images must stay CC BY-SA |
| [geoBoundaries](https://www.geoboundaries.org/) | CC BY 4.0 | Only if we want province outlines |

Avoid: Flaticon, Freepik, Vecteezy (per-file restrictions), unDraw and Storyset (custom non-open licenses), GADM (redistribution restricted), Google Maps and Mapbox (cost or keys), and web photos of batik or wayang (mixed licenses, and the spec says original art only).
