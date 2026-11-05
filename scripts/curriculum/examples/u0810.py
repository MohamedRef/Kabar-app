LESSONS = []

LESSONS.append(("bipa2.u08", dict(
    title="Bensin, ban, bengkel",
    can_do=("تتصرف لو العربية أو الموتوسيكل عطل: البنزين والكاوتش والورشة والرخصة والمرور.",
            "أن تتصرّف إذا تعطّلت السيارة أو الدراجة النارية: الوقود والإطار والورشة والرخصة والمرور.",
            "Cope when a car or scooter breaks down: fuel, tyres, the garage, your licence and traffic."),
    slides=[dict(
        title="Di jalan",
        body=("""kendaraan = عربية أو أي حاجة بتمشي على الطريق. bensin = بنزين، ban = كاوتش (ban bocor = الكاوتش مخرّم).
bengkel = ورشة ميكانيكي. mogok = العربية عطلت ووقفت (ومعناها كمان إضراب).
jalan tol = طريق سريع بفلوس. lalu lintas = المرور. SIM = رخصة السواقة (Surat Izin Mengemudi).""",
              """kendaraan = مركبة (كل ما يسير على الطريق). bensin = وقود، ban = إطار (ban bocor = إطار مثقوب).
bengkel = ورشة إصلاح. mogok = تعطّلت وتوقّفت (وتعني أيضًا إضرابًا).
jalan tol = طريق سريع برسوم. lalu lintas = حركة المرور. SIM = رخصة القيادة (Surat Izin Mengemudi).""",
              """kendaraan = vehicle. bensin = petrol, ban = tyre (ban bocor = a flat tyre).
bengkel = repair garage. mogok = break down, stall (also: go on strike).
jalan tol = toll road. lalu lintas = traffic. SIM = driving licence (Surat Izin Mengemudi)."""),
        examples=[
            ("Motor saya mogok di jalan.", "الموتوسيكل بتاعي عطل في الطريق.", "تعطّلت دراجتي النارية في الطريق.", "My scooter broke down on the road."),
            ("Ban mobil ini bocor. Ayo ke bengkel.", "كاوتش العربية دي مخرّم. يلا على الورشة.", "إطار هذه السيارة مثقوب. هيّا إلى الورشة.", "This car has a flat tyre. Let's go to the garage."),
            ("Jalan tol ini bayar, tapi cepat.", "الطريق السريع ده بفلوس، بس سريع.", "هذا الطريق برسوم، لكنه سريع.", "This toll road costs money, but it's fast."),
        ])],
    ex=[
        ("match", [("kendaraan", "عربية، مركبة", "مركبة", "vehicle"), ("bensin", "بنزين", "وقود", "petrol"),
                   ("ban", "كاوتش", "إطار", "tyre"), ("jalan tol", "طريق سريع بفلوس", "طريق برسوم", "toll road")]),
        ("match", [("bengkel", "ورشة", "ورشة", "garage"), ("mogok", "عطل", "تعطّل", "break down"),
                   ("SIM", "رخصة سواقة", "رخصة قيادة", "driving licence"), ("lalu lintas", "المرور", "المرور", "traffic")]),
        ("listen", "Motornya mogok. Bensinnya kosong!", ["البنزين خلص", "الكاوتش مخرّم", "الرخصة ضاعت"], 0,
         ("الموتوسيكل عطل. مفيش بنزين!", "تعطّلت الدراجة. لا وقود فيها!", "The scooter's broken down. It's out of petrol!"), ["mogok", "bensin"],
         ("الموتوسيكل ماله؟", "ما مشكلة الدراجة؟", "What's wrong with the scooter?")),
        ("fill", "Ban motor saya bocor. Saya bawa ke ___.", ("كاوتش الموتوسيكل مخرّم. هوديه الـ ___.", "إطار دراجتي مثقوب. سآخذها إلى ___.", "My scooter has a flat. I'll take it to the ___."),
         ["bengkel", "jalan tol", "SIM"], "bengkel", ["bengkel", "ban", "jalan tol", "SIM"]),
        ("mc", "", ("لازم يكون معاك إيه عشان تسوق؟", "ماذا يجب أن يكون معك لتقود؟", "What must you carry to drive?"),
         ["SIM", "ban", "bensin"], 0,
         ("SIM = رخصة السواقة.", "SIM = رخصة القيادة.", "SIM = driving licence."), ["SIM", "ban", "bensin"]),
        ("fill", "Jam lima sore, ___ di Jakarta macet sekali.", ("الساعة خمسة العصر، ___ في جاكرتا واقف خالص.", "في الخامسة مساءً تكون ___ في جاكرتا مزدحمة جدًا.", "At five p.m. the ___ in Jakarta is jammed."),
         ["lalu lintas", "bengkel", "kendaraan"], "lalu lintas", ["lalu lintas", "bengkel", "kendaraan"]),
        ("build", ("الطريق السريع ده بفلوس، بس سريع.", "هذا الطريق برسوم لكنه سريع.", "This toll road costs money, but it's fast."),
         "Jalan tol ini bayar, tapi cepat.", ["ban", "lambat"], ["jalan tol"]),
        ("tr_from", "Banyak kendaraan di jalan raya pagi ini.",
         {"ar_eg": ["فيه عربيات كتير في الطريق الرئيسي النهارده الصبح", "الطريق الرئيسي مليان عربيات الصبح ده"],
          "ar": ["هناك مركبات كثيرة على الطريق الرئيسي هذا الصباح"],
          "en": ["There are lots of vehicles on the main road this morning", "Many vehicles on the main road this morning"]}, ["kendaraan"]),
        ("speak", "Mobil saya mogok di jalan tol.", ("عربيتي عطلت على الطريق السريع.", "تعطّلت سيارتي على الطريق السريع.", "My car broke down on the toll road."), ["mogok", "jalan tol"]),
        ("mc", "Mobil minum ___, bukan air!", None, ["bensin", "ban", "lalu lintas"], 0,
         ("العربية بتشرب بنزين!", "السيارة تشرب الوقود!", "Cars drink petrol!"), ["bensin", "ban", "lalu lintas"]),
    ],
    dialogue=[
        ("Mohamed", "Ni Luh, motor saya mogok di jalan!", "يا ني لوه، الموتوسيكل عطل مني في الطريق!", "يا ني لوه، تعطّلت دراجتي في الطريق!", "Ni Luh, my scooter's broken down on the road!"),
        ("Ni Luh", "Bensinnya kosong?", "البنزين خلص؟", "هل نفد الوقود؟", "Out of petrol?"),
        ("Mohamed", "Tidak. Bannya bocor.", "لأ. الكاوتش مخرّم.", "لا. الإطار مثقوب.", "No. It's got a flat."),
        ("Ni Luh", "Ada bengkel dekat kantor pos. Bawa ke sana.", "فيه ورشة جنب البوسطة. وديه هناك.", "هناك ورشة قرب مكتب البريد. خذها إليها.", "There's a garage near the post office. Take it there."),
        ("Mohamed", "Oke. Lalu lintasnya ramai sekali hari ini.", "ماشي. المرور زحمة أوي النهارده.", "حسنًا. المرور مزدحم جدًا اليوم.", "OK. The traffic's really heavy today."),
        ("Ni Luh", "Hati-hati, ya! Jangan lupa SIM-mu.", "خلي بالك! ما تنساش رخصتك.", "انتبه! ولا تنسَ رخصتك.", "Be careful! Don't forget your licence."),
    ],
    words=[
        ("kendaraan", "noun", "عربية، مركبة", "مركبة", "vehicle"),
        ("bensin", "noun", "بنزين", "وقود (بنزين)", "petrol"),
        ("ban", "noun", "كاوتش", "إطار", "tyre"),
        ("jalan tol", "noun", "طريق سريع بفلوس", "طريق برسوم", "toll road"),
        ("bengkel", "noun", "ورشة ميكانيكي", "ورشة إصلاح", "repair garage, workshop"),
        ("mogok", "verb", "يعطل؛ يضرب عن الشغل", "يتعطّل؛ يُضرب", "break down; go on strike"),
        ("SIM", "noun", "رخصة سواقة", "رخصة قيادة", "driving licence"),
        ("lalu lintas", "noun", "المرور", "حركة المرور", "traffic"),
    ])))

LESSONS.append(("bipa2.u09", dict(
    title="Hutan dan air terjun",
    can_do=("تخطط رحلة طبيعة: غابة وشلال وشعاب مرجانية، وشنطة ضهر وبوصلة، وتختار مكان تبات فيه.",
            "أن تخطط لرحلة في الطبيعة: غابة وشلال وشعاب مرجانية، وحقيبة ظهر وبوصلة، وتختار مكان المبيت.",
            "Plan a nature trip: forest, waterfall and coral reef, a backpack and compass, and somewhere to stay."),
    slides=[dict(
        title="Ke alam",
        body=("""hutan = غابة، air terjun = شلال (حرفيًا «مية واقعة»)، batu = حجر أو صخرة، karang = شعاب مرجانية.
kompas = بوصلة، ransel = شنطة ضهر.
penginapan = مكان تبات فيه (من inap = يبات)، vila = فيلا.""",
              """hutan = غابة، air terjun = شلال (حرفيًا «ماء ساقط»)، batu = حجر أو صخرة، karang = شعاب مرجانية.
kompas = بوصلة، ransel = حقيبة ظهر.
penginapan = مكان المبيت (من inap = يبيت)، vila = فيلا.""",
              """hutan = forest, air terjun = waterfall (literally "falling water"), batu = stone, rock, karang = coral reef.
kompas = compass, ransel = backpack.
penginapan = lodging, guesthouse (inap = stay overnight), vila = villa."""),
        examples=[
            ("Di Bali ada air terjun yang indah.", "في بالي فيه شلال جميل.", "في بالي شلال جميل.", "Bali has a beautiful waterfall."),
            ("Bawa ransel dan kompas ke hutan.", "خد شنطة الضهر والبوصلة للغابة.", "خذ حقيبة الظهر والبوصلة إلى الغابة.", "Take a backpack and compass to the forest."),
            ("Kami tinggal di vila dekat pantai.", "إحنا قاعدين في فيلا جنب البحر.", "نقيم في فيلا قرب الشاطئ.", "We're staying in a villa near the beach."),
        ])],
    ex=[
        ("match", [("hutan", "غابة", "غابة", "forest"), ("air terjun", "شلال", "شلال", "waterfall"),
                   ("batu", "حجر، صخرة", "حجر، صخرة", "stone"), ("karang", "شعاب مرجانية", "شعاب مرجانية", "coral")]),
        ("match", [("kompas", "بوصلة", "بوصلة", "compass"), ("ransel", "شنطة ضهر", "حقيبة ظهر", "backpack"),
                   ("penginapan", "مكان تبات فيه", "مكان المبيت", "lodging"), ("vila", "فيلا", "فيلا", "villa")]),
        ("listen", "Air terjunnya tinggi dan airnya dingin.", ["شلال", "غابة", "شاطئ"], 0,
         ("الشلال عالي ومَيّته ساقعة.", "الشلال عالٍ وماؤه بارد.", "The waterfall is tall and the water is cold."), ["air terjun"],
         ("بيتكلم عن إيه؟", "عمّ يتحدث؟", "What is he talking about?")),
        ("fill", "Di bawah laut ada ___ yang indah dan ikan warna-warni.", ("تحت البحر فيه ___ جميلة وسمك ملوّن.", "تحت البحر ___ جميلة وأسماك ملوّنة.", "Under the sea there's beautiful ___ and colourful fish."),
         ["karang", "kompas", "ransel"], "karang", ["karang", "kompas", "ransel"]),
        ("mc", "", ("بتعرف الشمال فين بإيه؟", "بمَ تعرف اتجاه الشمال؟", "What shows you which way is north?"),
         ["kompas", "batu", "vila"], 0,
         ("kompas = بوصلة.", "kompas = بوصلة.", "kompas = compass."), ["kompas", "batu", "vila"]),
        ("fill", "Di ___ ada banyak pohon dan monyet.", ("في الـ ___ فيه شجر وقرود كتير.", "في ___ أشجار وقرود كثيرة.", "There are lots of trees and monkeys in the ___."),
         ["hutan", "penginapan", "ransel"], "hutan", ["hutan", "penginapan", "ransel"]),
        ("build", ("إحنا قاعدين في فيلا جنب البحر.", "نقيم في فيلا قرب الشاطئ.", "We're staying in a villa near the beach."),
         "Kami tinggal di vila dekat pantai.", ["hutan", "batu"], ["vila"]),
        ("tr_from", "Penginapan ini murah dan bersih.",
         {"ar_eg": ["المكان ده اللي هنبات فيه رخيص ونضيف", "اللوكاندة دي رخيصة ونضيفة"],
          "ar": ["مكان المبيت هذا رخيص ونظيف", "هذا النُّزُل رخيص ونظيف"],
          "en": ["This guesthouse is cheap and clean", "This lodging is cheap and clean"]}, ["penginapan"]),
        ("speak", "Saya bawa ransel dan kompas ke hutan.", ("واخد شنطة الضهر والبوصلة للغابة.", "آخذ حقيبة الظهر والبوصلة إلى الغابة.", "I'm taking a backpack and compass to the forest."), ["ransel", "kompas", "hutan"]),
        ("mc", "Rumah itu dari ___, bukan kayu.", None, ["batu", "karang", "air terjun"], 0,
         ("البيت من حجر: batu.", "البيت من حجر: batu.", "The house is stone: batu."), ["batu", "karang", "air terjun"]),
    ],
    dialogue=[
        ("Ni Luh", "Besok kita ke air terjun Sekumpul, ya.", "بكرة نروح شلال سِكومبول.", "غدًا نذهب إلى شلال سكومبول.", "Tomorrow we're going to Sekumpul waterfall."),
        ("Mohamed", "Jauh dari sini?", "بعيد من هنا؟", "هل هو بعيد من هنا؟", "Is it far from here?"),
        ("Ni Luh", "Lumayan. Kita jalan lewat hutan.", "شوية. هنمشي في الغابة.", "قليلًا. سنمشي عبر الغابة.", "Fairly. We walk through the forest."),
        ("Mohamed", "Saya bawa ransel dan air.", "هاخد شنطة الضهر ومية.", "سآخذ حقيبة الظهر والماء.", "I'll bring my backpack and water."),
        ("Ni Luh", "Bagus. Malamnya kita tidur di penginapan dekat sana.", "حلو. بالليل ننام في مكان قريب من هناك.", "جيّد. وفي الليل ننام في نُزُل قريب.", "Good. At night we'll sleep in a guesthouse nearby."),
        ("Mohamed", "Vila mahal, ya? Penginapan saja!", "الفيلا غالية، صح؟ خلّينا في اللوكاندة!", "الفيلا غالية، أليس كذلك؟ النُّزُل يكفي!", "Villas are pricey, right? A guesthouse is fine!"),
    ],
    words=[
        ("hutan", "noun", "غابة", "غابة", "forest"),
        ("air terjun", "noun", "شلال", "شلال", "waterfall"),
        ("batu", "noun", "حجر، صخرة", "حجر، صخرة", "stone, rock"),
        ("karang", "noun", "شعاب مرجانية", "شعاب مرجانية", "coral, reef"),
        ("kompas", "noun", "بوصلة", "بوصلة", "compass"),
        ("ransel", "noun", "شنطة ضهر", "حقيبة ظهر", "backpack"),
        ("penginapan", "noun", "مكان تبات فيه، لوكاندة", "مكان المبيت، نُزُل", "lodging, guesthouse"),
        ("vila", "noun", "فيلا", "فيلا", "villa"),
    ])))

LESSONS.append(("bipa2.u09", dict(
    title="Kemeja, dasi, perak",
    can_do=("تختار لبس لفرح أو حفلة، وتتكلم عن الحرير والفضة والإزاز.",
            "أن تختار ملابس لحفل أو عرس، وتتحدث عن الحرير والفضة والزجاج.",
            "Choose clothes for a party or wedding and talk about silk, silver and glass."),
    slides=[dict(
        title="Ke pesta",
        body=("""gaun = فستان سهرة، jas = جاكت بدلة، kemeja = قميص بزراير، dasi = كرافتة، sandal = صندل.
sutra = حرير، perak = فضة (Celuk في بالي = قرية الفضة).
kaca = إزاز، وفاكر kacamata؟ = نضارة («إزاز العين»).""",
              """gaun = فستان سهرة، jas = سترة البدلة، kemeja = قميص بأزرار، dasi = ربطة عنق، sandal = صندل.
sutra = حرير، perak = فضّة (قرية Celuk في بالي مشهورة بالفضّة).
kaca = زجاج، وتذكّر kacamata = نظارة («زجاج العين»).""",
              """gaun = gown, dress, jas = suit jacket, kemeja = shirt (with buttons), dasi = tie, sandal = sandals.
sutra = silk, perak = silver (Celuk in Bali is the silver village).
kaca = glass; remember kacamata = glasses ("eye glass")."""),
        examples=[
            ("Ke pesta, saya pakai kemeja putih dan dasi.", "في الحفلة هلبس قميص أبيض وكرافتة.", "ألبس في الحفل قميصًا أبيض وربطة عنق.", "For the party I'll wear a white shirt and tie."),
            ("Gaun Rina dari sutra.", "فستان رينا حرير.", "فستان رينا من الحرير.", "Rina's dress is silk."),
            ("Di Celuk ada banyak toko perak.", "في تشيلوك فيه محلات فضة كتير.", "في تشيلوك متاجر فضة كثيرة.", "Celuk has lots of silver shops."),
        ])],
    ex=[
        ("match", [("gaun", "فستان سهرة", "فستان سهرة", "gown"), ("jas", "جاكت بدلة", "سترة بدلة", "suit jacket"),
                   ("kemeja", "قميص", "قميص", "shirt"), ("dasi", "كرافتة", "ربطة عنق", "tie")]),
        ("match", [("sandal", "صندل", "صندل", "sandals"), ("sutra", "حرير", "حرير", "silk"),
                   ("kaca", "إزاز", "زجاج", "glass"), ("perak", "فضة", "فضّة", "silver")]),
        ("listen", "Di Celuk ada banyak toko perak.", ["فضة", "دهب", "حرير"], 0,
         ("في تشيلوك فيه محلات فضة كتير.", "في تشيلوك متاجر فضة كثيرة.", "Celuk has lots of silver shops."), ["perak"],
         ("المحلات بتبيع إيه؟", "ماذا تبيع المتاجر؟", "What do the shops sell?")),
        ("fill", "Ke pantai, pakai ___ saja.", ("على البحر، البس ___ وخلاص.", "إلى الشاطئ، البس ___ فقط.", "For the beach, just wear ___."),
         ["sandal", "jas", "dasi"], "sandal", ["sandal", "jas", "dasi"]),
        ("mc", "", ("الراجل بيلبس إيه حوالين رقبته مع القميص؟", "ماذا يلبس الرجل حول عنقه مع القميص؟", "What does a man wear round his neck with a shirt?"),
         ["dasi", "gaun", "sutra"], 0,
         ("dasi = كرافتة.", "dasi = ربطة عنق.", "dasi = tie."), ["dasi", "gaun", "sutra"]),
        ("fill", "Jendela ini dari ___.", ("الشباك ده من ___.", "هذه النافذة من ___.", "This window is made of ___."),
         ["kaca", "sutra", "perak"], "kaca", ["kaca", "sutra", "perak"]),
        ("build", ("هلبس قميص أبيض وجاكت أسود.", "سألبس قميصًا أبيض وسترة سوداء.", "I'll wear a white shirt and a black jacket."),
         "Saya pakai kemeja putih dan jas hitam.", ["gaun", "sandal"], ["kemeja", "jas"]),
        ("tr_from", "Gaun Rina dari sutra.",
         {"ar_eg": ["فستان رينا حرير", "فستان رينا معمول من الحرير"],
          "ar": ["فستان رينا من الحرير"],
          "en": ["Rina's dress is silk", "Rina's gown is made of silk"]}, ["gaun", "sutra"]),
        ("speak", "Ke pesta, saya pakai kemeja dan dasi.", ("في الحفلة هلبس قميص وكرافتة.", "ألبس في الحفل قميصًا وربطة عنق.", "For the party I'll wear a shirt and tie."), ["kemeja", "dasi"]),
        ("mc", "Kain ini dari ___, jadi mahal.", None, ["sutra", "kaca", "sandal"], 0,
         ("القماش الحرير غالي.", "قماش الحرير غالٍ.", "Silk cloth is expensive."), ["sutra", "kaca", "sandal"]),
    ],
    dialogue=[
        ("Rina", "Mohamed, Sabtu ini ada pesta teman saya di Ubud.", "يا محمد، السبت ده فيه حفلة لصاحبتي في أوبود.", "يا محمد، يوم السبت حفل لصديقتي في أوبود.", "Mohamed, my friend's having a party in Ubud this Saturday."),
        ("Mohamed", "Saya pakai apa? Jas dan dasi?", "ألبس إيه؟ بدلة وكرافتة؟", "ماذا ألبس؟ سترة وربطة عنق؟", "What should I wear? Jacket and tie?"),
        ("Rina", "Kemeja batik saja. Di Bali tidak perlu jas.", "قميص باتيك وخلاص. في بالي مش محتاج جاكت.", "قميص باتيك يكفي. لا حاجة للسترة في بالي.", "Just a batik shirt. You don't need a jacket in Bali."),
        ("Mohamed", "Kamu pakai gaun?", "وإنتي هتلبسي فستان؟", "وأنتِ، ستلبسين فستانًا؟", "Are you wearing a dress?"),
        ("Rina", "Ya, gaun sutra dari ibu saya.", "أيوه، فستان حرير من ماما.", "نعم، فستان حرير من أمي.", "Yes, a silk dress from my mum."),
        ("Mohamed", "Bagus! Saya mau beli perak di Celuk untuk mereka.", "حلو! أنا عايز أشتري لهم فضة من تشيلوك.", "جميل! أريد شراء فضّة لهم من تشيلوك.", "Lovely! I want to buy them some silver in Celuk."),
    ],
    words=[
        ("gaun", "noun", "فستان سهرة", "فستان سهرة", "gown, dress"),
        ("jas", "noun", "جاكت بدلة", "سترة البدلة", "suit jacket"),
        ("kemeja", "noun", "قميص بزراير", "قميص بأزرار", "shirt"),
        ("dasi", "noun", "كرافتة", "ربطة عنق", "tie"),
        ("sandal", "noun", "صندل", "صندل", "sandals"),
        ("sutra", "noun", "حرير", "حرير", "silk"),
        ("kaca", "noun", "إزاز", "زجاج", "glass"),
        ("perak", "noun", "فضة", "فضّة", "silver"),
    ])))

LESSONS.append(("bipa2.u10", dict(
    title="Alergi dan gatal",
    can_do=("تقول للدكتور عندك إيه: جرح، كسر، حساسية، هرش أو برد، وتفهم الحقنة والفيتامين.",
            "أن تخبر الطبيب بما لديك: جرح أو كسر أو حساسية أو حكّة أو زكام، وتفهم الحقنة والفيتامين.",
            "Tell the doctor what's wrong: a cut, a break, an allergy, an itch or flu, and follow talk of injections and vitamins."),
    slides=[dict(
        title="Di klinik",
        body=("""pasien = المريض اللي عند الدكتور. luka = جرح، patah = مكسور: Kaki saya patah.
suntik = حقنة. vitamin = فيتامين.
alergi = حساسية: Saya alergi kacang. gatal = بيهرش (بياكلني). flu = برد، إنفلونزا.""",
              """pasien = المريض عند الطبيب. luka = جرح، patah = مكسور: Kaki saya patah.
suntik = حقنة. vitamin = فيتامين.
alergi = حساسية: Saya alergi kacang. gatal = يسبّب حكّة. flu = زكام، إنفلونزا.""",
              """pasien = patient. luka = wound, cut, patah = broken: Kaki saya patah.
suntik = injection. vitamin = vitamin.
alergi = allergy: Saya alergi kacang. gatal = itchy. flu = flu, a cold."""),
        examples=[
            ("Kaki saya patah waktu main bola.", "رجلي اتكسرت وأنا بلعب كورة.", "انكسرت ساقي وأنا ألعب الكرة.", "I broke my leg playing football."),
            ("Saya alergi kacang. Kulit saya gatal.", "عندي حساسية من الفول السوداني. جلدي بيهرشني.", "لديّ حساسية من الفول السوداني. جلدي يحكّني.", "I'm allergic to peanuts. My skin itches."),
            ("Pasien itu perlu suntik.", "المريض ده محتاج حقنة.", "يحتاج ذلك المريض إلى حقنة.", "That patient needs an injection."),
        ])],
    ex=[
        ("match", [("pasien", "مريض", "مريض", "patient"), ("luka", "جرح", "جرح", "wound"),
                   ("patah", "مكسور", "مكسور", "broken"), ("suntik", "حقنة", "حقنة", "injection")]),
        ("match", [("vitamin", "فيتامين", "فيتامين", "vitamin"), ("alergi", "حساسية", "حساسية", "allergy"),
                   ("gatal", "بيهرش", "يحكّ", "itchy"), ("flu", "برد", "زكام", "flu")]),
        ("listen", "Saya alergi kacang. Kulit saya gatal sekali.", ["حساسية من الفول السوداني", "برد", "رجل مكسورة"], 0,
         ("عندي حساسية من الفول السوداني. جلدي بيهرشني أوي.", "لديّ حساسية من الفول السوداني، وجلدي يحكّني كثيرًا.", "I'm allergic to peanuts. My skin is really itchy."), ["alergi", "gatal"],
         ("عنده إيه؟", "ماذا لديه؟", "What's his problem?")),
        ("fill", "Tangan saya ___ karena jatuh dari sepeda.", ("إيدي ___ عشان وقعت من العجلة.", "يدي ___ لأنني سقطت عن الدراجة.", "My arm is ___ because I fell off my bike."),
         ["patah", "gatal", "flu"], "patah", ["patah", "gatal", "flu"]),
        ("mc", "", ("اللي بيتعالج عند الدكتور اسمه إيه؟", "ماذا نسمّي من يعالجه الطبيب؟", "What do you call someone a doctor treats?"),
         ["pasien", "vitamin", "luka"], 0,
         ("pasien = مريض.", "pasien = مريض.", "pasien = patient."), ["pasien", "vitamin", "luka"]),
        ("fill", "Saya batuk, pilek, dan pusing. Saya ___.", ("بكح ومزكّم ودايخ. عندي ___.", "أسعل وأنا مزكوم وأشعر بالدوار. لديّ ___.", "I'm coughing, sniffly and dizzy. I've got ___."),
         ["flu", "luka", "suntik"], "flu", ["flu", "luka", "suntik"]),
        ("build", ("كل يوم الصبح باخد فيتامين.", "آخذ فيتامينًا كل صباح.", "I take a vitamin every morning."),
         "Setiap pagi saya minum vitamin.", ["suntik", "luka"], ["vitamin"]),
        ("tr_from", "Ada luka kecil di jari saya.",
         {"ar_eg": ["فيه جرح صغير في صباعي", "صباعي فيه جرح صغير"],
          "ar": ["في إصبعي جرح صغير"],
          "en": ["There's a small cut on my finger", "I have a little cut on my finger"]}, ["luka"]),
        ("speak", "Dokter, saya takut suntik!", ("يا دكتور، أنا بخاف من الحقن!", "يا دكتور، أخاف من الحقن!", "Doctor, I'm scared of injections!"), ["suntik"]),
        ("mc", "Dokter punya banyak ___ hari ini.", None, ["pasien", "alergi", "patah"], 0,
         ("الدكتور عنده عيانين كتير: pasien.", "لدى الطبيب مرضى كثيرون: pasien.", "The doctor has many patients: pasien."), ["pasien", "alergi", "patah"]),
    ],
    dialogue=[
        ("Dokter", "Selamat pagi. Sakit apa?", "صباح الخير. عندك إيه؟", "صباح الخير. ممّ تشكو؟", "Good morning. What's wrong?"),
        ("Mohamed", "Kulit saya gatal, Dok. Merah semua.", "جلدي بيهرشني يا دكتور. كله أحمر.", "جلدي يحكّني يا دكتور، وكله أحمر.", "My skin itches, doctor. It's all red."),
        ("Dokter", "Kemarin makan apa? Ada alergi?", "امبارح كلت إيه؟ عندك حساسية؟", "ماذا أكلت أمس؟ هل لديك حساسية؟", "What did you eat yesterday? Any allergies?"),
        ("Mohamed", "Saya makan sate kacang. Apa saya alergi kacang?", "كلت ساتيه بالفول السوداني. هو أنا عندي حساسية منه؟", "أكلت ساتيه بالفول السوداني. هل لديّ حساسية منه؟", "I had satay with peanut sauce. Am I allergic to peanuts?"),
        ("Dokter", "Ya, ini alergi. Minum obat ini, tidak perlu suntik.", "أيوه، دي حساسية. خد الدوا ده، مش محتاج حقنة.", "نعم، هذه حساسية. تناول هذا الدواء، لا حاجة إلى حقنة.", "Yes, it's an allergy. Take this medicine; no injection needed."),
        ("Mohamed", "Bagus! Saya takut suntik.", "الحمد لله! أنا بخاف من الحقن.", "الحمد لله! أخاف من الحقن.", "Phew! I'm scared of injections."),
    ],
    words=[
        ("pasien", "noun", "مريض (عند الدكتور)", "مريض", "patient"),
        ("luka", "noun", "جرح", "جرح", "wound, cut"),
        ("patah", "adjective", "مكسور", "مكسور", "broken (bone, stick)"),
        ("suntik", "noun", "حقنة", "حقنة", "injection"),
        ("vitamin", "noun", "فيتامين", "فيتامين", "vitamin"),
        ("alergi", "noun", "حساسية", "حساسية", "allergy"),
        ("gatal", "adjective", "بيهرش", "مسبّب للحكّة", "itchy"),
        ("flu", "noun", "برد، إنفلونزا", "زكام، إنفلونزا", "flu, cold"),
    ])))

LESSONS.append(("bipa2.u10", dict(
    title="Ambulans dan operasi",
    can_do=("تتصرف في طوارئ: حد أغمى عليه أو بيرجّع، تطلب الإسعاف، وتسأل عن حالة مريض بعد عملية.",
            "أن تتصرّف في الطوارئ: شخص أُغمي عليه أو يتقيّأ، تطلب الإسعاف، وتسأل عن حال مريض بعد عملية.",
            "Handle an emergency: someone faints or is sick, call an ambulance, and ask how a patient is after an operation."),
    slides=[dict(
        title="Di rumah sakit",
        body=("""muntah = يرجّع (يتقيأ)، pingsan = يغمى عليه. ambulans = عربية إسعاف.
operasi = عملية جراحية. infus = محلول في الوريد، perban = شاش ورباط. termometer = ترمومتر.
kondisi = الحالة: Bagaimana kondisinya?""",
              """muntah = يتقيّأ، pingsan = يُغمى عليه. ambulans = سيارة إسعاف.
operasi = عملية جراحية. infus = محلول وريدي، perban = ضمادة. termometer = ميزان حرارة.
kondisi = الحالة: Bagaimana kondisinya?""",
              """muntah = vomit, pingsan = faint. ambulans = ambulance.
operasi = operation, surgery. infus = drip, perban = bandage. termometer = thermometer.
kondisi = condition: Bagaimana kondisinya?"""),
        examples=[
            ("Ada orang pingsan! Telepon ambulans!", "فيه حد أغمى عليه! اتصل بالإسعاف!", "هناك شخص مغمى عليه! اتصل بالإسعاف!", "Someone's fainted! Call an ambulance!"),
            ("Sesudah operasi, kondisinya baik.", "بعد العملية، حالته كويسة.", "بعد العملية حاله جيّدة.", "After the operation, his condition is good."),
            ("Pasien itu pakai infus.", "المريض ده متعلق له محلول.", "ذلك المريض على محلول وريدي.", "That patient is on a drip."),
        ])],
    ex=[
        ("match", [("muntah", "يرجّع", "يتقيّأ", "vomit"), ("operasi", "عملية", "عملية جراحية", "operation"),
                   ("ambulans", "إسعاف", "إسعاف", "ambulance"), ("kondisi", "حالة", "حالة", "condition")]),
        ("match", [("pingsan", "يغمى عليه", "يُغمى عليه", "faint"), ("infus", "محلول", "محلول وريدي", "drip"),
                   ("perban", "شاش", "ضمادة", "bandage"), ("termometer", "ترمومتر", "ميزان حرارة", "thermometer")]),
        ("listen", "Tolong! Ada orang pingsan! Telepon ambulans!", ["حد أغمى عليه", "حد اتخانق", "حريقة"], 0,
         ("الحقوني! فيه حد أغمى عليه! اتصلوا بالإسعاف!", "النجدة! شخص مغمى عليه! اتصلوا بالإسعاف!", "Help! Someone's fainted! Call an ambulance!"), ["pingsan", "ambulans"],
         ("إيه اللي حصل؟", "ماذا حدث؟", "What happened?")),
        ("fill", "Badannya panas? Coba pakai ___.", ("جسمه سخن؟ جرّب الـ ___.", "هل حرارته مرتفعة؟ جرّب ___.", "Is he hot? Try the ___."),
         ["termometer", "perban", "infus"], "termometer", ["termometer", "perban", "infus"]),
        ("mc", "", ("عربية المستشفى اللي ليها سرينة اسمها إيه؟", "ما اسم سيارة المستشفى ذات الصفّارة؟", "What's the hospital vehicle with a siren?"),
         ["ambulans", "operasi", "kondisi"], 0,
         ("ambulans = إسعاف.", "ambulans = إسعاف.", "ambulans = ambulance."), ["ambulans", "operasi", "kondisi"]),
        ("fill", "Sesudah ___, pasien harus istirahat seminggu.", ("بعد الـ ___، المريض لازم يرتاح أسبوع.", "بعد ___ يجب أن يرتاح المريض أسبوعًا.", "After the ___, the patient must rest for a week."),
         ["operasi", "muntah", "perban"], "operasi", ["operasi", "muntah", "perban"]),
        ("build", ("حالته كويسة النهارده.", "حاله جيّدة اليوم.", "His condition is good today."),
         "Kondisinya baik hari ini.", ["pingsan", "sakit"], ["kondisi"]),
        ("tr_from", "Anak itu muntah lalu pingsan.",
         {"ar_eg": ["الولد ده رجّع وبعدين أغمى عليه", "العيل رجّع وبعدها أغمى عليه"],
          "ar": ["تقيّأ ذلك الطفل ثم أُغمي عليه"],
          "en": ["That child was sick and then fainted", "The child vomited, then fainted"]}, ["muntah", "pingsan"]),
        ("speak", "Pasien itu pakai infus dan perban.", ("المريض ده متعلق له محلول وعليه شاش.", "ذلك المريض على محلول وعليه ضمادة.", "That patient has a drip and a bandage."), ["infus", "perban"]),
        ("mc", "Ada luka di tangan. Pakai ___.", None, ["perban", "termometer", "ambulans"], 0,
         ("الجرح محتاج شاش: perban.", "الجرح يحتاج ضمادة: perban.", "A cut needs a bandage: perban."), ["perban", "termometer", "ambulans"]),
    ],
    dialogue=[
        ("Mohamed", "Bu, bagaimana kondisi teman saya?", "يا مدام، حالة صاحبي عاملة إيه؟", "يا سيدتي، كيف حال صديقي؟", "Excuse me, how is my friend doing?"),
        ("Perawat", "Sudah lebih baik. Kemarin dia muntah dan pingsan.", "أحسن دلوقتي. امبارح كان بيرجّع وأغمى عليه.", "صار أفضل. أمس تقيّأ وأُغمي عليه.", "Better now. Yesterday he vomited and fainted."),
        ("Mohamed", "Perlu operasi?", "محتاج عملية؟", "هل يحتاج إلى عملية؟", "Does he need an operation?"),
        ("Perawat", "Tidak perlu. Sekarang dia pakai infus saja.", "لأ. دلوقتي عليه محلول بس.", "لا. الآن عليه محلول وريدي فقط.", "No. He's just on a drip now."),
        ("Mohamed", "Baik. Boleh saya masuk?", "تمام. ممكن أدخل؟", "حسنًا. هل يمكنني الدخول؟", "OK. May I go in?"),
        ("Perawat", "Boleh, tapi jangan lama, ya.", "ممكن، بس ما تطوّلش.", "نعم، لكن لا تُطل.", "Yes, but don't stay long."),
    ],
    words=[
        ("muntah", "verb", "يرجّع", "يتقيّأ", "vomit, be sick"),
        ("operasi", "noun", "عملية جراحية", "عملية جراحية", "operation, surgery"),
        ("ambulans", "noun", "عربية إسعاف", "سيارة إسعاف", "ambulance"),
        ("kondisi", "noun", "حالة", "حالة", "condition"),
        ("pingsan", "verb", "يغمى عليه", "يُغمى عليه", "faint"),
        ("infus", "noun", "محلول في الوريد", "محلول وريدي", "drip, IV"),
        ("perban", "noun", "شاش، رباط", "ضمادة", "bandage"),
        ("termometer", "noun", "ترمومتر", "ميزان حرارة", "thermometer"),
    ])))
