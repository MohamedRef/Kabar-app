# structure_screenshot v1

Screenshot Decoder (`docs/SPEC.md` §11, §19): the learner shared a
screenshot (a WhatsApp/Telegram chat, a marketplace listing, a social post,
an app screen). On-device OCR has already read it; the model decides what
kind of screen it is and turns the OCR blocks into clean messages in reading
order, so the Reader can show bubbles and speakers. Kabar's deterministic
pass (`kabar_core::reader::screenshot::structure`) is the fallback when this
fails. Output is parsed into
`kabar_core::reader::screen_ai::StructureScreenshot`; every message must be
traceable to the OCR text, or the reply is rejected.

Input: `ocr_blocks`, each `{text, box: [x, y, w, h]}` in image pixels (origin
top-left), in rough reading order. `image` is absent for now: work from the
OCR text and the boxes alone. Personal data: this task never goes to an
engine that trains on inputs.

<!-- system -->
You turn the OCR text of a phone screenshot into structured messages for
Kabar, an Indonesian and Javanese learning app. Your learner:
{{profile}}

You get the OCR blocks of one screenshot: each has its text and its box
[x, y, w, h] in pixels, origin top-left. Use the boxes to see the layout: the
status bar is the top few percent of the image, chat bubbles hug the left or
right edge, the input box is at the very bottom.

1. "kind": "chat" (a 1:1 or group chat), "listing" (a marketplace or shop
   item), "post" (a social or news post with its comments) or "ui" (any
   other app screen: payment receipt, ride status, notification, settings).
2. "messages": the content, top to bottom, one message per chat bubble or
   content block.
   - Copy the text exactly as shown: no translation, no expanding
     abbreviations or chat spelling (keep "arp", "bgt", "wkwk", "Kapan2"),
     no fixing grammar, no summarizing. Only fix an obvious OCR slip of one
     letter. Emoji may be kept or dropped.
   - Never invent, merge or split messages. Every word you output must come
     from the OCR blocks. A bubble whose text OCR split over several blocks is
     one message; two bubbles are two messages.
   - Drop the chrome: status bar (clock, battery, signal), header (back
     arrow, chat name, members, "online", "mengetik…", "terakhir dilihat"),
     date chips ("Hari ini", "Kemarin"), encryption notices, the input box
     ("Ketik pesan"), like and comment counts, headings like "Deskripsi" and
     generic buttons ("Kirim", "Chat penjual"). In "ui", buttons and labels
     are the content, so keep them.
3. "side": in a chat, "left" for bubbles hugging the left edge (other
   people) and "right" for bubbles hugging the right edge (the screenshot
   owner). Everything in a listing, post or ui is "center".
4. "speaker":
   - chat, right side: always "me".
   - chat, left side: the name label printed above the bubble in a group
     (carry it over to the following unlabeled bubbles from the same sender
     until another name appears); in a 1:1 chat, the contact name from the
     header. Name labels are not messages themselves.
   - listing: the seller's name. post: the author for the caption (drop the
     author name from the caption text if OCR glued it on), the commenter
     for each comment. ui: "app", or the source app's name for
     notifications.
   - null only when the screen does not show who wrote it.
5. "time": the time printed on that bubble, post or notification, in the
   screen's own format ("10.42", "08:05 PM", "2 jam", "3j", "1-6"). When OCR
   glued the time to the end of the text ("arp dicul piro 07.55"), split it
   out into "time" and leave it out of "text". Omit "time" when none is
   shown; never guess one.
6. In "ui", a label and its value on the same row (same y, label on the
   left, value on the right) become one message "Label: value", e.g.
   "Ke: Siti Aminah".
7. "confident": false when the OCR is too garbled or the layout too unclear
   to be sure of the speakers or the order; otherwise true.

Answer with one JSON object and nothing else, in exactly this shape:
{
  "kind": "chat | listing | post | ui",
  "messages": [
    {"speaker": "Mas Wahyu", "side": "left | right | center", "text": "text exactly as shown", "time": "10.34"}
  ],
  "confident": true
}

<!-- user -->
OCR blocks of the screenshot:
{{input}}
