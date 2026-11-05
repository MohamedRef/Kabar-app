LESSONS = []

LESSONS.append(("bipa2.u05", dict(
    title="Sup, keju, madu",
    can_do=("تطلب شوربة ونودلز وعصير وحلويات في الوارونج.",
            "أن تطلب الحساء والمعكرونة والعصير والحلويات في المطعم الشعبي.",
            "Order soup, noodles, juice and sweets at a warung."),
    slides=[dict(
        title="Di warung",
        body=("""sup = شوربة، mi = نودلز (mi ayam، mi goreng)، sayur = خضار (sup sayur).
keju = جبنة، madu = عسل (teh madu).
jus = عصير: jus jeruk. es krim = آيس كريم، permen = بونبوني.""",
              """sup = حساء، mi = معكرونة النودلز (mi ayam، mi goreng)، sayur = خضار (sup sayur).
keju = جبن، madu = عسل (teh madu).
jus = عصير: jus jeruk. es krim = بوظة (آيس كريم)، permen = حلوى (سكاكر).""",
              """sup = soup, mi = noodles (mi ayam, mi goreng), sayur = vegetables (sup sayur).
keju = cheese, madu = honey (teh madu).
jus = juice: jus jeruk. es krim = ice cream, permen = sweets, candy."""),
        examples=[
            ("Saya pesan mi ayam dan jus jeruk.", "أنا طالب نودلز بالفراخ وعصير برتقان.", "طلبت نودلز بالدجاج وعصير برتقال.", "I'll have chicken noodles and orange juice."),
            ("Anak-anak suka es krim dan permen.", "العيال بيحبوا الآيس كريم والبونبوني.", "يحبّ الأطفال البوظة والحلوى.", "Children love ice cream and sweets."),
            ("Teh dengan madu enak sekali.", "الشاي بالعسل حلو أوي.", "الشاي بالعسل لذيذ جدًا.", "Tea with honey is delicious."),
        ])],
    ex=[
        ("match", [("sup", "شوربة", "حساء", "soup"), ("mi", "نودلز", "نودلز", "noodles"),
                   ("keju", "جبنة", "جبن", "cheese"), ("madu", "عسل", "عسل", "honey")]),
        ("match", [("permen", "بونبوني", "حلوى", "sweets"), ("jus", "عصير", "عصير", "juice"),
                   ("es krim", "آيس كريم", "بوظة", "ice cream"), ("sayur", "خضار", "خضار", "vegetables")]),
        ("listen", "Saya mau sup sayur dan nasi, ya.", ["شوربة خضار", "آيس كريم", "عصير"], 0,
         ("عايز شوربة خضار ورز لو سمحت.", "أريد حساء خضار وأرزًا من فضلك.", "Vegetable soup and rice, please."), ["sup", "sayur"],
         ("طالب إيه؟", "ماذا يطلب؟", "What is he ordering?")),
        ("fill", "Roti ini pakai ___ dan tomat.", ("العيش ده بـ ___ وطماطم.", "هذا الخبز مع ___ وطماطم.", "This bread comes with ___ and tomato."),
         ["keju", "jus", "permen"], "keju", ["keju", "jus", "permen"]),
        ("mc", "", ("النحل بيعمل إيه؟", "ماذا يصنع النحل؟", "What do bees make?"),
         ["madu", "keju", "sup"], 0,
         ("madu = عسل.", "madu = عسل.", "madu = honey."), ["madu", "keju", "sup"]),
        ("fill", "Hari ini panas sekali. Ayo makan ___!", ("النهارده حر أوي. يلا ناكل ___!", "اليوم حارّ جدًا. هيّا نأكل ___!", "It's so hot today. Let's have ___!"),
         ["es krim", "sup", "mi"], "es krim", ["es krim", "sup", "mi"]),
        ("build", ("عايز عصير مانجة من غير سكر.", "أريد عصير مانجو بلا سكّر.", "I'd like mango juice without sugar."),
         "Saya mau jus mangga, tidak pakai gula.", ["madu", "permen"], ["jus"]),
        ("tr_from", "Anak-anak suka es krim dan permen.",
         {"ar_eg": ["العيال بيحبوا الآيس كريم والبونبوني", "الأطفال بيحبوا الآيس كريم والحلويات"],
          "ar": ["يحبّ الأطفال البوظة والحلوى", "الأطفال يحبون الآيس كريم والسكاكر"],
          "en": ["Children like ice cream and sweets", "Kids love ice cream and candy"]}, ["es krim", "permen"]),
        ("speak", "Mi ayam dan sup sayur, ya.", ("نودلز بالفراخ وشوربة خضار لو سمحت.", "نودلز بالدجاج وحساء خضار من فضلك.", "Chicken noodles and vegetable soup, please."), ["mi", "sup", "sayur"]),
        ("mc", "Teh ini manis karena ada ___.", None, ["madu", "keju", "sayur"], 0,
         ("العسل بيحلّي الشاي.", "العسل يحلّي الشاي.", "Honey makes tea sweet."), ["madu", "keju", "sayur"]),
    ],
    dialogue=[
        ("Ni Luh", "Mohamed, mau makan apa? Di warung ini ada mi dan sup.", "يا محمد، تاكل إيه؟ الوارونج ده فيه نودلز وشوربة.", "ماذا تريد أن تأكل يا محمد؟ في هذا المطعم نودلز وحساء.", "Mohamed, what do you want? This warung has noodles and soup."),
        ("Mohamed", "Saya mau mi ayam. Ada sayurnya?", "عايز نودلز بالفراخ. فيها خضار؟", "أريد نودلز بالدجاج. هل فيها خضار؟", "Chicken noodles for me. Do they come with vegetables?"),
        ("Ni Luh", "Ada. Minumnya apa? Jus atau teh?", "أيوه. تشرب إيه؟ عصير ولا شاي؟", "نعم. وماذا تشرب؟ عصيرًا أم شايًا؟", "Yes. What to drink? Juice or tea?"),
        ("Mohamed", "Jus jeruk, tidak pakai gula. Kamu?", "عصير برتقان من غير سكر. وإنتي؟", "عصير برتقال بلا سكّر. وأنتِ؟", "Orange juice, no sugar. You?"),
        ("Ni Luh", "Teh panas dengan madu. Nanti kita beli es krim, ya!", "شاي سخن بالعسل. وبعدين نشتري آيس كريم!", "شاي ساخن بالعسل. وبعدها نشتري البوظة!", "Hot tea with honey. Then we'll get ice cream!"),
        ("Mohamed", "Wah, ide bagus!", "الله، فكرة حلوة!", "رائع، فكرة جميلة!", "Ooh, good idea!"),
    ],
    words=[
        ("sup", "noun", "شوربة", "حساء", "soup"),
        ("mi", "noun", "نودلز", "معكرونة النودلز", "noodles"),
        ("keju", "noun", "جبنة", "جبن", "cheese"),
        ("madu", "noun", "عسل", "عسل", "honey"),
        ("permen", "noun", "بونبوني", "حلوى، سكاكر", "sweets, candy"),
        ("jus", "noun", "عصير", "عصير", "juice"),
        ("es krim", "noun", "آيس كريم", "بوظة، آيس كريم", "ice cream"),
        ("sayur", "noun", "خضار", "خضار", "vegetables"),
    ])))

LESSONS.append(("bipa2.u05", dict(
    title="Kompor, panci, kulkas",
    can_do=("تتكلم في المطبخ: البوتاجاز والحلة والتلاجة، والأكل ني ولا مستوي ولا بايت.",
            "أن تتحدث في المطبخ: الموقد والقدر والثلاجة، والطعام نيّء أو ناضج أو فاسد.",
            "Talk in the kitchen: stove, pot and fridge, and whether food is raw, cooked or off."),
    slides=[dict(
        title="Di dapur",
        body=("""kompor = بوتاجاز، panci = حلة، kulkas = تلاجة، mangkuk = سلطانية (semangkuk sup).
mentah = ني، matang = مستوي (والفاكهة matang = مستوية).
basi = بايت وريحته وحشة. rasa = طعم: Rasanya enak!""",
              """kompor = موقد، panci = قدر، kulkas = ثلاجة، mangkuk = وعاء عميق (semangkuk sup).
mentah = نيّء، matang = ناضج (مطهو، والفاكهة matang = ناضجة).
basi = فاسد (بائت). rasa = طعم: Rasanya enak!""",
              """kompor = stove, panci = pot, kulkas = fridge, mangkuk = bowl (semangkuk sup).
mentah = raw, matang = cooked, ripe.
basi = stale, gone off. rasa = taste: Rasanya enak!"""),
        examples=[
            ("Panci di atas kompor masih panas.", "الحلة اللي على البوتاجاز لسه سخنة.", "القدر فوق الموقد ما زال ساخنًا.", "The pot on the stove is still hot."),
            ("Dagingnya masih mentah, belum matang.", "اللحمة لسه نية، لسه ما استوتش.", "اللحم ما زال نيئًا، لم ينضج بعد.", "The meat is still raw, not cooked yet."),
            ("Rasanya enak sekali!", "طعمه حلو أوي!", "طعمه لذيذ جدًا!", "It tastes great!"),
        ])],
    ex=[
        ("match", [("kompor", "بوتاجاز", "موقد", "stove"), ("panci", "حلة", "قدر", "pot"),
                   ("kulkas", "تلاجة", "ثلاجة", "fridge"), ("mangkuk", "سلطانية", "وعاء", "bowl")]),
        ("match", [("mentah", "ني", "نيّء", "raw"), ("matang", "مستوي", "ناضج", "cooked, ripe"),
                   ("basi", "بايت", "فاسد", "gone off"), ("rasa", "طعم", "طعم", "taste")]),
        ("listen", "Nasinya sudah basi. Kenapa tidak di kulkas?", ["بايت", "ني", "مستوي"], 0,
         ("الرز باظ. ليه ما كانش في التلاجة؟", "فسد الأرز. لماذا لم يكن في الثلاجة؟", "The rice has gone off. Why wasn't it in the fridge?"), ["basi", "kulkas"],
         ("الرز ماله؟", "ما حال الأرز؟", "What's wrong with the rice?")),
        ("fill", "Ayamnya belum ___, masih merah di dalam.", ("الفرخة لسه ما ___، لسه حمرا من جوه.", "الدجاج لم ___ بعد، ما زال أحمر من الداخل.", "The chicken isn't ___ yet; it's still red inside."),
         ["matang", "basi", "kulkas"], "matang", ["matang", "basi", "kulkas"]),
        ("mc", "", ("فين بنحط اللبن عشان يفضل ساقع؟", "أين نضع الحليب ليبقى باردًا؟", "Where do we keep milk cold?"),
         ["kulkas", "kompor", "panci"], 0,
         ("kulkas = تلاجة.", "kulkas = ثلاجة.", "kulkas = fridge."), ["kulkas", "kompor", "panci"]),
        ("fill", "Saya suka ___ manis.", ("بحب الـ ___ المسكّر.", "أحبّ ___ الحلو.", "I like a sweet ___."),
         ["rasa", "mangkuk", "kompor"], "rasa", ["rasa", "mangkuk", "kompor"]),
        ("build", ("حط الشوربة في السلطانية.", "ضع الحساء في الوعاء.", "Put the soup in the bowl."),
         "Masukkan sup ke dalam mangkuk.", ["panci", "kompor"], ["mangkuk", "panci"]),
        ("tr_from", "Saya suka sayur mentah. Rasanya segar.",
         {"ar_eg": ["بحب الخضار ني. طعمه فريش", "بحب الخضار النية. طعمها منعش"],
          "ar": ["أحبّ الخضار نيئة. طعمها منعش"],
          "en": ["I like raw vegetables. They taste fresh", "I like vegetables raw. They taste fresh"]}, ["mentah", "rasa"]),
        ("speak", "Panci di atas kompor masih panas.", ("الحلة اللي على البوتاجاز لسه سخنة.", "القدر فوق الموقد ما زال ساخنًا.", "The pot on the stove is still hot."), ["panci", "kompor"]),
        ("mc", "Mangga ini kuning dan manis. Sudah ___.", None, ["matang", "mentah", "basi"], 0,
         ("المانجة الصفرا المسكّرة مستوية: matang.", "المانجو الصفراء الحلوة ناضجة: matang.", "A yellow, sweet mango is ripe: matang."), ["matang", "mentah", "basi"]),
    ],
    dialogue=[
        ("Bu Yanti", "Mohamed, tolong ambil telur di kulkas.", "يا محمد، هات البيض من التلاجة لو سمحت.", "يا محمد، أحضر البيض من الثلاجة من فضلك.", "Mohamed, could you get the eggs from the fridge?"),
        ("Mohamed", "Ini, Bu. Mau masak apa?", "اتفضلي. هتطبخي إيه؟", "تفضّلي. ماذا ستطبخين؟", "Here you go. What are you cooking?"),
        ("Bu Yanti", "Nasi goreng. Pancinya di atas kompor.", "رز مقلي. الحلة على البوتاجاز.", "أرز مقلي. القدر فوق الموقد.", "Fried rice. The pot's on the stove."),
        ("Mohamed", "Nasi kemarin tidak basi, Bu?", "رز امبارح مش بايت؟", "أليس أرز الأمس فاسدًا؟", "Hasn't yesterday's rice gone off?"),
        ("Bu Yanti", "Tidak. Coba rasanya... enak!", "لأ. دوق طعمه... حلو!", "لا. تذوّق طعمه... لذيذ!", "No. Taste it... delicious!"),
        ("Mohamed", "Saya ambil mangkuk, ya.", "أنا هجيب السلطانيات.", "سأحضر الأوعية.", "I'll get the bowls."),
    ],
    words=[
        ("kompor", "noun", "بوتاجاز", "موقد", "stove"),
        ("panci", "noun", "حلة", "قدر", "pot, pan"),
        ("kulkas", "noun", "تلاجة", "ثلاجة", "fridge"),
        ("mangkuk", "noun", "سلطانية", "وعاء (زبدية)", "bowl"),
        ("mentah", "adjective", "ني", "نيّء", "raw"),
        ("matang", "adjective", "مستوي", "ناضج، مطهو", "cooked, ripe"),
        ("basi", "adjective", "بايت (باظ)", "فاسد، بائت", "stale, gone off"),
        ("rasa", "noun", "طعم؛ إحساس", "طعم؛ شعور", "taste; feeling"),
    ])))

LESSONS.append(("bipa2.u06", dict(
    title="Sofa, karpet, bantal",
    can_do=("توصف شقتك وأوضها وعفشها: الكنبة والسجادة والمخدة والبطانية.",
            "أن تصف شقتك وغرفها وأثاثها: الأريكة والسجادة والوسادة والبطانية.",
            "Describe your flat, its rooms and furnishings: sofa, carpet, pillow and blanket."),
    slides=[dict(
        title="Di apartemen",
        body=("""apartemen = شقة في عمارة، garasi = جراج، ruangan = أوضة أو قاعة، tembok = حيطة.
sofa = كنبة، karpet = سجادة.
bantal = مخدة، selimut = بطانية (أو لحاف).""",
              """apartemen = شقة في بناية، garasi = مرآب، ruangan = غرفة أو قاعة، tembok = جدار.
sofa = أريكة، karpet = سجادة.
bantal = وسادة، selimut = بطانية (لحاف).""",
              """apartemen = flat, apartment, garasi = garage, ruangan = room, hall, tembok = wall.
sofa = sofa, karpet = carpet, rug.
bantal = pillow, selimut = blanket."""),
        examples=[
            ("Apartemen saya kecil, tapi ada garasi.", "شقتي صغيرة، بس فيها جراج.", "شقتي صغيرة لكن فيها مرآب.", "My flat is small, but it has a garage."),
            ("Ada foto keluarga di tembok.", "فيه صورة العيلة على الحيطة.", "على الجدار صورة العائلة.", "There's a family photo on the wall."),
            ("Malam ini dingin. Mana selimut saya?", "الليلة برد. فين البطانية بتاعتي؟", "الليلة باردة. أين بطانيتي؟", "It's cold tonight. Where's my blanket?"),
        ])],
    ex=[
        ("match", [("apartemen", "شقة", "شقة", "flat"), ("garasi", "جراج", "مرآب", "garage"),
                   ("tembok", "حيطة", "جدار", "wall"), ("ruangan", "أوضة، قاعة", "غرفة، قاعة", "room")]),
        ("match", [("sofa", "كنبة", "أريكة", "sofa"), ("karpet", "سجادة", "سجادة", "carpet"),
                   ("bantal", "مخدة", "وسادة", "pillow"), ("selimut", "بطانية", "بطانية", "blanket")]),
        ("listen", "Apartemen saya punya garasi.", ["جراج", "جنينة", "حمام سباحة"], 0,
         ("شقتي فيها جراج.", "في شقتي مرآب.", "My flat has a garage."), ["apartemen", "garasi"],
         ("الشقة فيها إيه؟", "ماذا في الشقة؟", "What does the flat have?")),
        ("fill", "Saya tidur pakai dua ___.", ("أنا بنام على مخدتين: ___.", "أنام على ___ اثنتين.", "I sleep with two ___."),
         ["bantal", "karpet", "garasi"], "bantal", ["bantal", "karpet", "garasi"]),
        ("mc", "", ("إيه اللي بنفرشه على الأرض؟", "ماذا نفرش على الأرض؟", "What do we lay on the floor?"),
         ["karpet", "selimut", "tembok"], 0,
         ("karpet = سجادة.", "karpet = سجادة.", "karpet = carpet."), ["karpet", "selimut", "tembok"]),
        ("fill", "Malam ini dingin. Pakai ___, ya.", ("الليلة برد. اتغطى بـ ___.", "الليلة باردة. استخدم ___.", "It's cold tonight. Use a ___."),
         ["selimut", "sofa", "ruangan"], "selimut", ["selimut", "sofa", "ruangan"]),
        ("build", ("الشقة دي فيها أوضتين وجراج.", "في هذه الشقة غرفتان ومرآب.", "This flat has two rooms and a garage."),
         "Apartemen ini punya dua ruangan dan garasi.", ["sofa", "tembok"], ["apartemen", "ruangan", "garasi"]),
        ("tr_from", "Ada foto keluarga di tembok.",
         {"ar_eg": ["فيه صورة العيلة على الحيطة", "على الحيطة فيه صورة العيلة"],
          "ar": ["على الجدار صورة العائلة", "هناك صورة عائلية على الجدار"],
          "en": ["There's a family photo on the wall", "There is a family picture on the wall"]}, ["tembok"]),
        ("speak", "Kucing Bu Yanti tidur di sofa, di atas bantal.", ("قطة طنط يانتي نايمة على الكنبة فوق المخدة.", "قطة السيدة يانتي نائمة على الأريكة فوق الوسادة.", "Bu Yanti's cat sleeps on the sofa, on a cushion."), ["sofa", "bantal"]),
        ("mc", "Tamu duduk di ___.", None, ["sofa", "selimut", "tembok"], 0,
         ("الضيوف بيقعدوا على الكنبة.", "يجلس الضيوف على الأريكة.", "Guests sit on the sofa."), ["sofa", "selimut", "tembok"]),
    ],
    dialogue=[
        ("Raka", "Wah, apartemenmu bagus! Ruangannya besar.", "الله، شقتك حلوة! الأوضة واسعة.", "شقتك جميلة! الغرفة كبيرة.", "Wow, your flat is nice! The room's big."),
        ("Mohamed", "Terima kasih. Duduk di sofa, Raka.", "شكرًا. اقعد على الكنبة يا راكا.", "شكرًا. اجلس على الأريكة يا راكا.", "Thanks. Have a seat on the sofa, Raka."),
        ("Raka", "Karpetnya dari mana? Bagus sekali.", "السجادة دي منين؟ حلوة أوي.", "من أين السجادة؟ جميلة جدًا.", "Where's the carpet from? It's lovely."),
        ("Mohamed", "Dari pasar di Ubud. Murah!", "من سوق في أوبود. رخيصة!", "من سوق في أوبود. رخيصة!", "From a market in Ubud. Cheap!"),
        ("Raka", "Di tembok itu ada foto siapa?", "مين اللي في الصورة اللي على الحيطة؟", "صورة من على ذلك الجدار؟", "Whose photo is on that wall?"),
        ("Mohamed", "Keluarga saya di Kairo.", "عيلتي في القاهرة.", "عائلتي في القاهرة.", "My family in Cairo."),
    ],
    words=[
        ("apartemen", "noun", "شقة", "شقة", "flat, apartment"),
        ("garasi", "noun", "جراج", "مرآب", "garage"),
        ("tembok", "noun", "حيطة", "جدار", "wall"),
        ("ruangan", "noun", "أوضة، قاعة", "غرفة، قاعة", "room, hall"),
        ("sofa", "noun", "كنبة", "أريكة", "sofa"),
        ("karpet", "noun", "سجادة", "سجادة", "carpet, rug"),
        ("bantal", "noun", "مخدة", "وسادة", "pillow, cushion"),
        ("selimut", "noun", "بطانية", "بطانية", "blanket"),
    ])))

LESSONS.append(("bipa2.u06", dict(
    title="Kampung yang damai",
    can_do=("توصف قريتك أو حتتك: هادية ولا زحمة، والبوسطة والطريق الرئيسي، والمطافي والحراسة بالليل.",
            "أن تصف قريتك أو حيّك: هادئ أم مزدحم، والبريد والطريق الرئيسي، والإطفاء والحراسة الليلية.",
            "Describe your village or neighbourhood: quiet or busy, the post office and main road, the fire brigade and the night watch."),
    slides=[dict(
        title="Kampung saya",
        body=("""kampung = قرية أو حتة شعبية. jalan raya = الطريق الرئيسي. kantor pos = البوسطة.
sepi = هادي ومفيهوش ناس (عكس ramai)، damai = في سلام وأمان.
kebakaran = حريقة (bakar = يحرق)، وpemadam kebakaran = المطافي.
ronda = الحراسة بالليل: رجالة الحتة بيلفوا على البيوت.""",
              """kampung = قرية أو حيّ شعبي. jalan raya = الطريق الرئيسي. kantor pos = مكتب البريد.
sepi = هادئ خالٍ من الناس (عكس ramai)، damai = في سلام.
kebakaran = حريق (bakar = يحرق)، وpemadam kebakaran = الإطفاء.
ronda = الحراسة الليلية: يطوف رجال الحيّ على البيوت ليلًا.""",
              """kampung = village, neighbourhood. jalan raya = main road. kantor pos = post office.
sepi = quiet, empty (opposite of ramai), damai = peaceful.
kebakaran = a fire (bakar = burn); pemadam kebakaran = fire brigade.
ronda = night watch: the men of the neighbourhood walk round the houses at night."""),
        examples=[
            ("Kampung saya sepi dan damai.", "قريتي هادية ومسالمة.", "قريتي هادئة وآمنة.", "My village is quiet and peaceful."),
            ("Kantor pos ada di jalan raya.", "البوسطة على الطريق الرئيسي.", "مكتب البريد على الطريق الرئيسي.", "The post office is on the main road."),
            ("Ada kebakaran! Telepon pemadam kebakaran!", "فيه حريقة! كلّم المطافي!", "هناك حريق! اتصل بالإطفاء!", "Fire! Call the fire brigade!"),
        ])],
    ex=[
        ("match", [("kampung", "قرية، حتة", "قرية، حيّ", "village"), ("jalan raya", "طريق رئيسي", "طريق رئيسي", "main road"),
                   ("kantor pos", "بوسطة", "مكتب البريد", "post office"), ("sepi", "هادي، فاضي", "هادئ، خالٍ", "quiet")]),
        ("match", [("damai", "في سلام", "مسالم", "peaceful"), ("kebakaran", "حريقة", "حريق", "fire"),
                   ("pemadam kebakaran", "المطافي", "الإطفاء", "fire brigade"), ("ronda", "حراسة بالليل", "حراسة ليلية", "night watch")]),
        ("listen", "Ada kebakaran di pasar! Panggil pemadam kebakaran!", ["حريقة", "حادثة", "مطر"], 0,
         ("فيه حريقة في السوق! نادوا المطافي!", "حريق في السوق! استدعوا الإطفاء!", "There's a fire at the market! Call the fire brigade!"), ["kebakaran", "pemadam kebakaran"],
         ("إيه اللي حصل؟", "ماذا حدث؟", "What happened?")),
        ("fill", "Saya kirim surat dari ___.", ("ببعت جواب من ___.", "أرسل رسالة من ___.", "I send a letter from the ___."),
         ["kantor pos", "jalan raya", "kampung"], "kantor pos", ["kantor pos", "jalan raya", "kampung"]),
        ("mc", "", ("عكس ramai؟", "ما عكس ramai؟", "The opposite of ramai?"),
         ["sepi", "damai", "ronda"], 0,
         ("ramai = زحمة، sepi = هادي.", "ramai = مزدحم، sepi = هادئ.", "ramai = busy, sepi = quiet."), ["sepi", "damai", "ronda"]),
        ("fill", "Setiap malam ada ___ di kampung kami.", ("كل ليلة فيه ___ في حتتنا.", "كل ليلة هناك ___ في حيّنا.", "Every night there's a ___ in our village."),
         ["ronda", "kebakaran", "kantor pos"], "ronda", ["ronda", "kebakaran", "kampung"]),
        ("build", ("قريتي هادية ومسالمة.", "قريتي هادئة ومسالمة.", "My village is quiet and peaceful."),
         "Kampung saya sepi dan damai.", ["ramai", "besar"], ["kampung", "sepi", "damai"]),
        ("tr_from", "Kantor pos ada di jalan raya.",
         {"ar_eg": ["البوسطة على الطريق الرئيسي", "مكتب البريد في الشارع الرئيسي"],
          "ar": ["مكتب البريد على الطريق الرئيسي"],
          "en": ["The post office is on the main road", "The post office is on the high street"]}, ["kantor pos", "jalan raya"]),
        ("speak", "Pemadam kebakaran datang cepat sekali.", ("المطافي جت بسرعة أوي.", "جاء الإطفاء بسرعة كبيرة.", "The fire brigade came really fast."), ["pemadam kebakaran"]),
        ("mc", "Ada api besar di rumah itu. Ini ___.", None, ["kebakaran", "ronda", "kampung"], 0,
         ("نار كبيرة في بيت = حريقة.", "نار كبيرة في بيت = حريق.", "A big fire in a house = kebakaran."), ["kebakaran", "ronda", "kampung"]),
    ],
    dialogue=[
        ("Ni Luh", "Ini kampung saya. Sepi, ya?", "دي قريتي. هادية، صح؟", "هذه قريتي. هادئة، أليس كذلك؟", "This is my village. Quiet, isn't it?"),
        ("Mohamed", "Iya, damai sekali. Beda dengan Jakarta!", "أيوه، هادية خالص. غير جاكرتا خالص!", "نعم، هادئة جدًا. تختلف عن جاكرتا!", "Yes, so peaceful. Not like Jakarta!"),
        ("Ni Luh", "Kantor pos dan pasar ada di jalan raya.", "البوسطة والسوق على الطريق الرئيسي.", "مكتب البريد والسوق على الطريق الرئيسي.", "The post office and market are on the main road."),
        ("Mohamed", "Kalau malam, aman?", "وبالليل، أمان؟", "وفي الليل، هل هي آمنة؟", "Is it safe at night?"),
        ("Ni Luh", "Aman. Ada ronda setiap malam.", "أمان. الرجالة بيعملوا حراسة كل ليلة.", "آمنة. يحرسها الرجال كل ليلة.", "Safe. The men do the night watch every night."),
        ("Mohamed", "Wah, bagus sekali!", "الله، حلو أوي!", "رائع جدًا!", "Oh, that's great!"),
    ],
    words=[
        ("kampung", "noun", "قرية، حتة شعبية", "قرية، حيّ شعبي", "village, neighbourhood"),
        ("jalan raya", "noun", "الطريق الرئيسي", "الطريق الرئيسي", "main road"),
        ("kantor pos", "noun", "البوسطة", "مكتب البريد", "post office"),
        ("sepi", "adjective", "هادي، فاضي", "هادئ، خالٍ", "quiet, deserted"),
        ("damai", "adjective", "في سلام، هادي", "مسالم، في سلام", "peaceful"),
        ("kebakaran", "noun", "حريقة", "حريق", "fire (a blaze)"),
        ("pemadam kebakaran", "noun", "المطافي", "الإطفاء", "fire brigade"),
        ("ronda", "noun", "حراسة الحتة بالليل", "حراسة الحيّ الليلية", "night watch"),
    ])))

LESSONS.append(("bipa2.u07", dict(
    title="Gol! Skornya berapa?",
    can_do=("تتفرج على ماتش وتتكلم عن الجول والحكم والنتيجة والكاس، وعن الباسكت والفولي.",
            "أن تشاهد مباراة وتتحدث عن الهدف والحكم والنتيجة والكأس، وعن السلة والطائرة.",
            "Watch a match and talk about goals, the referee, the score and the cup, plus basketball and volleyball."),
    slides=[dict(
        title="Nonton bola",
        body=("""gol = جول، wasit = حكم، skor = النتيجة: Skornya dua-satu. piala = كاس (البطولة).
basket = باسكت، voli = فولي (voli pantai = فولي شاطئ).
kaos kaki = شراب (حرفيًا «تيشيرت رجل»!)، celana pendek = شورت.""",
              """gol = هدف، wasit = حكم، skor = النتيجة: Skornya dua-satu. piala = كأس البطولة.
basket = كرة السلة، voli = الكرة الطائرة (voli pantai = الطائرة الشاطئية).
kaos kaki = جوارب (حرفيًا «قميص القدم»!)، celana pendek = سروال قصير.""",
              """gol = goal, wasit = referee, skor = score: Skornya dua-satu. piala = cup, trophy.
basket = basketball, voli = volleyball (voli pantai = beach volleyball).
kaos kaki = socks (literally "foot T-shirt"!), celana pendek = shorts."""),
        examples=[
            ("Gol! Skornya dua-satu!", "جوووول! النتيجة ٢–١!", "هدف! النتيجة ٢–١!", "Goal! It's two-one!"),
            ("Wasit itu tidak adil!", "الحكم ده ظالم!", "ذلك الحكم غير عادل!", "That referee isn't fair!"),
            ("Kami main voli pantai di Kuta.", "بنلعب فولي شاطئ في كوتا.", "نلعب الكرة الطائرة الشاطئية في كوتا.", "We play beach volleyball in Kuta."),
        ])],
    ex=[
        ("match", [("gol", "جول", "هدف", "goal"), ("wasit", "حكم", "حكم", "referee"),
                   ("piala", "كاس", "كأس", "cup, trophy"), ("skor", "النتيجة", "النتيجة", "score")]),
        ("match", [("basket", "باسكت", "كرة السلة", "basketball"), ("voli", "فولي", "الكرة الطائرة", "volleyball"),
                   ("kaos kaki", "شراب", "جوارب", "socks"), ("celana pendek", "شورت", "سروال قصير", "shorts")]),
        ("listen", "Gol! Skornya sekarang dua-dua!", ["٢–٢", "٢–١", "١–١"], 0,
         ("جول! النتيجة دلوقتي ٢–٢!", "هدف! النتيجة الآن ٢–٢!", "Goal! It's two-two now!"), ["gol", "skor"],
         ("النتيجة كام؟", "كم النتيجة؟", "What's the score?")),
        ("fill", "Indonesia menang! Mereka angkat ___.", ("إندونيسيا كسبت! رفعوا الـ ___.", "فازت إندونيسيا! رفعوا ___.", "Indonesia won! They lift the ___."),
         ["piala", "wasit", "kaos kaki"], "piala", ["piala", "wasit", "kaos kaki"]),
        ("mc", "", ("مين اللي بيصفّر في الماتش؟", "من يصفّر في المباراة؟", "Who blows the whistle in a match?"),
         ["wasit", "piala", "skor"], 0,
         ("wasit = حكم.", "wasit = حكم.", "wasit = referee."), ["wasit", "piala", "skor"]),
        ("fill", "Untuk main bola, pakai ___ dan sepatu.", ("عشان تلعب كورة، البس ___ وجزمة.", "لتلعب الكرة، البس ___ وحذاءً.", "To play football, wear ___ and shoes."),
         ["kaos kaki", "piala", "gol"], "kaos kaki", ["kaos kaki", "piala", "gol"]),
        ("build", ("بنلعب فولي على الشاطئ.", "نلعب الكرة الطائرة على الشاطئ.", "We play volleyball on the beach."),
         "Kami main voli di pantai.", ["basket", "skor"], ["voli", "basket"]),
        ("tr_from", "Panas sekali. Saya pakai celana pendek.",
         {"ar_eg": ["الجو حر أوي. أنا لابس شورت", "حر جدًا. هلبس شورت"],
          "ar": ["الجو حارّ جدًا. ألبس سروالًا قصيرًا"],
          "en": ["It's very hot. I'm wearing shorts", "It's so hot. I'll wear shorts"]}, ["celana pendek"]),
        ("speak", "Saya suka main basket pakai celana pendek.", ("بحب ألعب باسكت بالشورت.", "أحبّ لعب السلة بسروال قصير.", "I like playing basketball in shorts."), ["basket", "celana pendek"]),
        ("mc", "Di pantai Kuta, orang main ___ pantai.", None, ["voli", "basket", "piala"], 0,
         ("voli pantai = فولي شاطئ.", "voli pantai = الطائرة الشاطئية.", "voli pantai = beach volleyball."), ["voli", "basket", "piala"]),
    ],
    dialogue=[
        ("Raka", "Ayo nonton bola! Indonesia lawan Vietnam.", "يلا نتفرج على الكورة! إندونيسيا ضد فيتنام.", "هيّا نشاهد الكرة! إندونيسيا ضد فيتنام.", "Let's watch the football! Indonesia against Vietnam."),
        ("Mohamed", "Skornya berapa?", "النتيجة كام؟", "كم النتيجة؟", "What's the score?"),
        ("Raka", "Masih nol-nol... Gol! Satu-nol!", "لسه صفر صفر... جول! واحد صفر!", "ما زالت صفر–صفر... هدف! واحد–صفر!", "Still nil-nil... Goal! One-nil!"),
        ("Mohamed", "Wah! Tapi wasit bilang apa?", "الله! بس الحكم قال إيه؟", "رائع! لكن ماذا قال الحكم؟", "Wow! But what did the referee say?"),
        ("Raka", "Golnya bagus! Indonesia menang!", "الجول صح! إندونيسيا كسبت!", "الهدف صحيح! إندونيسيا فازت!", "It's a good goal! Indonesia wins!"),
        ("Mohamed", "Ayo, Indonesia! Ambil pialanya!", "يلا يا إندونيسيا! هاتوا الكاس!", "هيّا إندونيسيا! خذوا الكأس!", "Come on, Indonesia! Win the cup!"),
    ],
    words=[
        ("gol", "noun", "جول", "هدف", "goal"),
        ("wasit", "noun", "حكم", "حكم", "referee"),
        ("piala", "noun", "كاس (بطولة)", "كأس", "cup, trophy"),
        ("skor", "noun", "النتيجة", "النتيجة", "score"),
        ("basket", "noun", "باسكت", "كرة السلة", "basketball"),
        ("voli", "noun", "فولي", "الكرة الطائرة", "volleyball"),
        ("kaos kaki", "noun", "شراب", "جوارب", "socks"),
        ("celana pendek", "noun", "شورت", "سروال قصير", "shorts"),
    ])))

LESSONS.append(("bipa2.u07", dict(
    title="Dada, lengan, jari",
    can_do=("تقول أجزاء جسم أكتر في التمرين: الصدر والدراع والصوابع والوسط، والنفَس والعرق.",
            "أن تسمّي أجزاء جسم أكثر في التمرين: الصدر والذراع والأصابع والخصر، والنفَس والعرق.",
            "Name more body parts during exercise: chest, arm, fingers, waist, plus breath and sweat."),
    slides=[dict(
        title="Badan kita",
        body=("""dada = صدر، lengan = دراع، jari = صباع (jari kaki = صباع رجل)، pinggang = وسط.
kulit = جلد، darah = دم.
napas = نفَس: Tarik napas! = خد نفس! وkeringat = عرق.""",
              """dada = صدر، lengan = ذراع، jari = إصبع (jari kaki = إصبع القدم)، pinggang = خصر.
kulit = جلد، darah = دم.
napas = نفَس: Tarik napas! = خذ نفَسًا! وkeringat = عرق.""",
              """dada = chest, lengan = arm, jari = finger (jari kaki = toe), pinggang = waist.
kulit = skin, darah = blood.
napas = breath: Tarik napas! = breathe in! keringat = sweat."""),
        examples=[
            ("Tarik napas, angkat lengan!", "خد نفس، وارفع دراعاتك!", "خذ نفَسًا وارفع ذراعيك!", "Breathe in, raise your arms!"),
            ("Sesudah lari, badan saya penuh keringat.", "بعد الجري، جسمي كله عرق.", "بعد الجري امتلأ جسمي بالعرق.", "After running I'm covered in sweat."),
            ("Jari saya kecil, lengan saya panjang.", "صوابعي صغيرة ودراعاتي طويلة.", "أصابعي صغيرة وذراعاي طويلتان.", "My fingers are small, my arms are long."),
        ])],
    ex=[
        ("match", [("dada", "صدر", "صدر", "chest"), ("lengan", "دراع", "ذراع", "arm"),
                   ("jari", "صباع", "إصبع", "finger"), ("pinggang", "وسط", "خصر", "waist")]),
        ("match", [("kulit", "جلد", "جلد", "skin"), ("darah", "دم", "دم", "blood"),
                   ("napas", "نفَس", "نفَس", "breath"), ("keringat", "عرق", "عرق", "sweat")]),
        ("listen", "Tarik napas, lalu angkat kedua lengan.", ["خد نفس وارفع دراعاتك", "اقعد على الأرض", "اجري بسرعة"], 0,
         ("خد نفس وبعدين ارفع دراعاتك الاتنين.", "خذ نفَسًا ثم ارفع ذراعيك.", "Breathe in, then raise both arms."), ["napas", "lengan"],
         ("المدرب بيقول إيه؟", "ماذا يقول المدرّب؟", "What does the coach say?")),
        ("fill", "Tangan saya punya lima ___.", ("إيدي فيها خمس ___.", "في يدي خمسة ___.", "My hand has five ___."),
         ["jari", "dada", "darah"], "jari", ["jari", "dada", "darah"]),
        ("mc", "Warna ___ merah.", None, ["darah", "kulit", "keringat"], 0,
         ("الدم لونه أحمر: darah.", "الدم أحمر: darah.", "Blood is red: darah."), ["darah", "kulit", "keringat"]),
        ("fill", "Jangan lama di bawah matahari, nanti ___ kamu merah.", ("ما تقعدش كتير في الشمس، جلدك هيحمر: ___.", "لا تبقَ طويلًا تحت الشمس وإلا احمرّ ___ك.", "Don't stay long in the sun, or your ___ will go red."),
         ["kulit", "napas", "pinggang"], "kulit", ["kulit", "napas", "pinggang"]),
        ("build", ("إيدك على وسطك، وبعدين خد نفس.", "يداك على خصرك، ثم خذ نفَسًا.", "Hands on your waist, then breathe in."),
         "Tangan di pinggang, lalu tarik napas.", ["dada", "jari"], ["pinggang", "napas"]),
        ("tr_from", "Sesudah lari, badan saya penuh keringat.",
         {"ar_eg": ["بعد الجري جسمي كله عرق", "بعد ما جريت جسمي غرقان عرق"],
          "ar": ["بعد الجري امتلأ جسمي بالعرق"],
          "en": ["After running, my body is covered in sweat", "After running I'm all sweaty"]}, ["keringat"]),
        ("speak", "Tarik napas, lalu buang pelan-pelan.", ("خد نفس، وبعدين طلّعه بالراحة.", "خذ نفَسًا ثم أخرجه ببطء.", "Breathe in, then breathe out slowly."), ["napas"]),
        ("mc", "", ("إيه اللي بين الكتف والإيد؟", "ما الذي بين الكتف واليد؟", "What's between the shoulder and the hand?"),
         ["lengan", "jari", "dada"], 0,
         ("lengan = دراع.", "lengan = ذراع.", "lengan = arm."), ["lengan", "jari", "dada"]),
    ],
    dialogue=[
        ("Pelatih", "Angkat kedua lengan. Tarik napas!", "ارفع دراعاتك الاتنين. خد نفس!", "ارفع ذراعيك. خذ نفَسًا!", "Raise both arms. Breathe in!"),
        ("Mohamed", "Berat sekali untuk pinggang saya!", "ده تقيل أوي على وسطي!", "هذا ثقيل جدًا على خصري!", "That's hard on my waist!"),
        ("Pelatih", "Pelan-pelan saja. Tangan di pinggang.", "بالراحة. حط إيدك على وسطك.", "على مهلك. ضع يديك على خصرك.", "Take it slowly. Hands on your waist."),
        ("Mohamed", "Badan saya penuh keringat!", "جسمي كله عرق!", "جسمي مليء بالعرق!", "I'm dripping with sweat!"),
        ("Pelatih", "Bagus! Minum air, ya.", "برافو! اشرب مية.", "أحسنت! اشرب ماءً.", "Good! Drink some water."),
        ("Mohamed", "Terima kasih, Pak.", "شكرًا يا كابتن.", "شكرًا يا سيدي.", "Thanks, coach."),
    ],
    words=[
        ("dada", "noun", "صدر", "صدر", "chest"),
        ("lengan", "noun", "دراع", "ذراع", "arm"),
        ("jari", "noun", "صباع", "إصبع", "finger"),
        ("pinggang", "noun", "وسط", "خصر", "waist"),
        ("kulit", "noun", "جلد", "جلد", "skin"),
        ("darah", "noun", "دم", "دم", "blood"),
        ("napas", "noun", "نفَس", "نفَس", "breath"),
        ("keringat", "noun", "عرق", "عرق", "sweat"),
    ])))
