"""Who lives in Maple Cove and Pine Hollow, their things, and the animals.

Everyone's work leans on someone else's: wood and charcoal and honey come
down from the hamlet, fish and bread and news go up; the tavern needs the
fiddler, the baker needs the orchard's apples, the boatwright needs the
woodcutters' oak."""

BASE = {"skin": 0.2, "undertone": 0.6, "eyes": 0.5, "eye_shape": 0.4, "nose": 0.4, "hair_color": 0.4}


def person(name, age, home, appearance, look, personality, goals, voice, rel, traits, aims=None):
    lk = dict(BASE)
    lk.update(look)
    c = {"name": name, "age": age, "appearance": appearance, "look": lk, "personality": personality, "goals": goals, "voice": voice,
         "tone": {}, "traits": traits, "home": home, "home_x": 0.0, "home_z": 0.0, "relationships": rel, "species": "human", "variety": "", "layers": []}
    if aims:
        c["aims"] = aims
    return c


T = lambda s, p, c, b, g, cr: {"sociable": s, "playful": p, "curious": c, "brave": b, "generous": g, "crafty": cr}
M = lambda **k: dict({"shape": 0.0}, **k)
F = lambda **k: dict({"shape": 1.0}, **k)
HAS = lambda who, what: {"kind": "has", "who": who, "what": what}
HOLD = lambda what: {"kind": "hold", "what": what}


def villagers():
    return [
        # The mayor's house at the head of the green.
        person("Harriet Wells", 58, "the mayor's house on the green",
               "a tall, upright woman with silver hair in a neat bun, rimless spectacles, a long plum wool coat over a grey dress, a notebook always in her hand",
               F(height=1.74, hair=0.7, hair_style=0.8, hair_color=0.9, skin=0.12, top=0.86, top_shade=0.3, top_cover=1.0, sleeves=1.0, skirt=0.9, bottom=0.08, bottom_shade=0.4, shoes=0.45),
               "brisk, kind, organised, remembers everyone's birthday", "is mayor of Maple Cove: settles quarrels, keeps the village accounts and is planning the harvest supper on the green; has asked Rosie for apple pies and Finn to play",
               "clear and warm, a little formal", ["Walter Wells: husband", "Rosie Ames: baking the pies for her supper", "Gus Brennan: old friend"], T(0.8, 0.4, 0.6, 0.6, 0.8, 0.7),
               aims=[{"text": "hold the harvest supper on the green"}, {"text": "get apple pies from Rosie for the supper", "want": HAS("Harriet Wells", "an apple pie")}]),
        person("Walter Wells", 63, "the mayor's house on the green",
               "a stocky, white-bearded old sea captain in a navy peacoat and a knitted watch cap, a pipe he never lights",
               M(height=1.76, build=1.15, hair=0.2, hair_color=1.0, beard=0.8, skin=0.15, top=0.66, top_shade=0.15, top_cover=1.0, sleeves=1.0, bottom=0.66, bottom_shade=0.2, legs=1.0, shoes=0.8, hat=0.5),
               "slow, twinkly, full of sea stories, fond of children", "sailed the big schooners for forty years; now keeps the tide book, whittles on the porch and walks the pier every evening to see the sun go down",
               "deep and slow, salty sayings", ["Harriet Wells: wife", "Abe Haskell: old shipmate", "Sam Hartley: teaching him knots"], T(0.7, 0.6, 0.5, 0.8, 0.8, 0.6),
               aims=[{"text": "teach Sam to tie a bowline"}]),
        # The Anchor tavern's keepers, in a cottage on the bluff.
        person("Gus Brennan", 52, "a cottage on Bluff Lane",
               "a big, red-bearded, laughing man with his sleeves rolled up, a leather apron over a checked flannel shirt, a bar towel on his shoulder",
               M(height=1.86, build=1.35, hair=0.15, hair_color=0.75, beard=0.6, skin=0.08, top=0.0, top_shade=0.45, top_cover=0.8, sleeves=0.5, bottom=0.66, bottom_shade=0.3, legs=1.0, shoes=0.8),
               "jovial, generous, a good listener, never forgets a regular's drink", "keeps the Anchor tavern on the landing: pours the amber ale and the cider, keeps the fire going; wants honey from Martha for his autumn honey ale, and Finn playing on Friday nights",
               "booming and warm", ["Molly Brennan: wife", "Finn O'Leary: plays at the tavern", "Martha Larsen: buys her honey"], T(1.0, 0.7, 0.4, 0.6, 0.9, 0.4),
               aims=[{"text": "get a jar of honey from Martha for the honey ale", "want": HAS("Gus Brennan", "honey")}, {"text": "hear Finn play the fiddle in the tavern"}]),
        person("Molly Brennan", 49, "a cottage on Bluff Lane",
               "a small, round, quick woman with curly brown hair under a kerchief, a blue dress and a flour-dusted apron, a wooden spoon in her pocket",
               F(height=1.58, build=1.15, hair=0.6, hair_style=0.5, hair_color=0.35, skin=0.1, top=0.6, top_shade=0.6, top_cover=0.8, sleeves=0.6, skirt=0.8, bottom=0.6, bottom_shade=0.6, shoes=0.45, hat=0.2),
               "fierce in her kitchen, soft-hearted out of it", "cooks the Anchor's clam chowder and fish pie, buying fish from Ruth every morning; wants her chowder to win at the harvest supper",
               "quick and teasing", ["Gus Brennan: husband", "Ruth Haskell: buys her fish", "Nell Hartley: friend"], T(0.8, 0.5, 0.4, 0.6, 0.8, 0.6),
               aims=[{"text": "buy a basket of fish from Ruth for the chowder", "want": HAS("Molly Brennan", "a basket of fish")}]),
        # The inn on the green.
        person("Nell Hartley", 44, "the Harvest Moon Inn",
               "a slender, freckled woman with a long auburn braid, a green cardigan over a cream dress, keys jingling at her belt",
               F(height=1.68, hair=0.85, hair_style=0.8, hair_color=0.65, skin=0.05, top=0.33, top_shade=0.35, top_cover=0.9, sleeves=1.0, skirt=0.8, bottom=0.1, bottom_shade=0.85, shoes=0.45),
               "welcoming, calm, notices when someone is tired or sad", "keeps the Harvest Moon Inn on the green: airs the rooms, lights the common room fire, and makes every traveler feel at home; buys Rosie's bread each morning",
               "soft and friendly", ["Sam Hartley: son", "Molly Brennan: friend", "Rosie Ames: buys her bread"], T(0.8, 0.4, 0.5, 0.5, 0.9, 0.5),
               aims=[{"text": "get fresh bread from Rosie for the inn's breakfast", "want": HAS("Nell Hartley", "bread")}]),
        person("Sam Hartley", 12, "the Harvest Moon Inn",
               "a gangly, freckled boy with a shock of red hair, a striped jersey and patched trousers, a fishing rod over his shoulder",
               M(height=1.48, build=0.8, hair=0.35, hair_style=0.5, hair_color=0.7, skin=0.05, top=0.15, top_shade=0.6, top_cover=0.6, sleeves=0.8, bottom=0.66, bottom_shade=0.35, legs=0.8, shoes=0.45),
               "curious, chatty, brave, wants to know how everything works", "helps his mother at the inn and runs errands for anyone with a penny; wants to learn the light from Agnes and to sail with Walter's knots",
               "fast and full of questions", ["Nell Hartley: mother", "Ben Haskell: best friend", "Agnes Moore: hero", "Walter Wells: teaching him knots"], T(0.9, 0.9, 1.0, 0.8, 0.6, 0.6),
               aims=[{"text": "visit Agnes at the lighthouse"}]),
        # The general store.
        person("Dale Pruitt", 60, "the rooms behind the general store",
               "a lean, stooped man with a grey moustache, a cardigan with leather elbows, a pencil behind his ear",
               M(height=1.78, build=0.9, hair=0.25, hair_color=0.85, beard=0.2, skin=0.18, top=0.33, top_shade=0.3, top_cover=0.9, sleeves=1.0, bottom=0.08, bottom_shade=0.3, legs=1.0, shoes=0.45),
               "dry, honest, slow to smile, a soft touch for children", "keeps the general store: flour, syrup, rope, lamp oil and everything else; trades with the hamlet and wants Martha's honey and Owen's apples on his shelves",
               "dry and drawling", ["June Pruitt: wife", "Martha Larsen: sells him honey", "Owen Fairbanks: sells him apples"], T(0.6, 0.3, 0.4, 0.5, 0.7, 0.7),
               aims=[{"text": "get a jar of honey from Martha for the store", "want": HAS("Dale Pruitt", "honey")}]),
        person("June Pruitt", 57, "the rooms behind the general store",
               "a plump, rosy woman with short white curls, a lilac sweater and a string of beads, a satchel of letters",
               F(height=1.6, build=1.15, hair=0.3, hair_color=0.95, skin=0.15, top=0.82, top_shade=0.75, top_cover=0.9, sleeves=1.0, skirt=0.8, bottom=0.08, bottom_shade=0.4, shoes=0.45),
               "cheerful, chatty, knows everyone's news before they do", "runs the post from the store's counter and carries the letters round the village; wants to see Clara and Finn walking out together",
               "bright and gossipy", ["Dale Pruitt: husband", "Harriet Wells: friend", "Clara Ames: dotes on her"], T(1.0, 0.6, 0.8, 0.4, 0.8, 0.5),
               aims=[{"text": "carry the letters round the village"}]),
        # The bakery.
        person("Rosie Ames", 36, "the rooms behind the bakery",
               "a strong, cheerful woman with dark brown skin, her hair wrapped in a yellow scarf, a white apron over a rust-coloured dress, flour to her elbows",
               F(height=1.7, build=1.05, hair=0.5, hair_style=0.8, hair_color=0.05, skin=0.85, undertone=0.7, top=0.05, top_shade=0.55, top_cover=0.8, sleeves=0.5, skirt=0.8, bottom=0.05, bottom_shade=0.55, shoes=0.45, hat=0.2),
               "warm, early to rise, proud of her bread, hums while she works", "bakes the village's bread every dawn in her brick oven, burning Will's charcoal, and apple pies from Owen's apples; sends loaves to the inn and the tavern",
               "warm and musical", ["Clara Ames: daughter", "Owen Fairbanks: sells her apples", "Will Larsen: sells her charcoal", "Harriet Wells: baking for her supper"], T(0.8, 0.5, 0.4, 0.6, 0.9, 0.6),
               aims=[{"text": "bake apple pies for Harriet's harvest supper", "want": HAS("Harriet Wells", "an apple pie")}, {"text": "get apples from Owen", "want": HAS("Rosie Ames", "an apple")}]),
        person("Clara Ames", 16, "the rooms behind the bakery",
               "a quiet girl with dark brown skin and her hair in two puffs, a mustard sweater, a sketchbook under her arm",
               F(height=1.64, hair=0.4, hair_style=0.6, hair_color=0.05, skin=0.85, undertone=0.7, top=0.15, top_shade=0.6, top_cover=0.8, sleeves=1.0, skirt=0.5, bottom=0.6, bottom_shade=0.3, shoes=0.45),
               "dreamy, observant, shy until she isn't", "helps her mother at the oven before dawn, then sketches the cove and the boats; likes to watch the sunset from the pier head; is sweet on Finn",
               "soft, thoughtful", ["Rosie Ames: mother", "Finn O'Leary: sweet on him", "June Pruitt: like a grandmother", "Ivy Crane: teaches her to paint"], T(0.5, 0.5, 0.9, 0.4, 0.7, 0.8),
               aims=[{"text": "win Finn's heart", "want": {"kind": "affection", "with": "Finn O'Leary", "at_least": 0.8}}, {"text": "sketch the sunset from the pier head"}]),
        # The Haskells, the fishing family.
        person("Tom Haskell", 45, "a family cottage on Orchard Lane",
               "a broad, weathered lobsterman with a short brown beard, yellow oilskin overalls over a grey sweater, rubber boots",
               M(height=1.8, build=1.2, hair=0.25, hair_color=0.35, beard=0.4, skin=0.15, top=0.08, top_shade=0.5, top_cover=0.9, sleeves=1.0, bottom=0.15, bottom_shade=0.85, legs=1.0, shoes=0.8),
               "hard-working, quiet, steady as a rock", "hauls lobster pots and fishes the cove from his boat at dawn, landing the catch at the fish shed for Ruth to sell; has asked Eli to build him a new boat of Pine Hollow oak",
               "plain and slow", ["Ruth Haskell: wife", "Ben Haskell: son", "Abe Haskell: father", "Finn O'Leary: his deckhand", "Eli Crane: building his boat"], T(0.6, 0.4, 0.3, 0.8, 0.7, 0.5),
               aims=[{"text": "get a new boat from Eli", "want": HOLD("a new boat")}]),
        person("Ruth Haskell", 42, "a family cottage on Orchard Lane",
               "a tall, quick woman with fair hair tied back, a navy fisherman's sweater, a long canvas apron and rubber boots",
               F(height=1.72, hair=0.75, hair_style=0.8, hair_color=0.6, skin=0.08, top=0.66, top_shade=0.15, top_cover=0.9, sleeves=1.0, skirt=0.6, bottom=0.66, bottom_shade=0.2, shoes=0.8),
               "practical, sharp at a bargain, kind to children", "sells the catch in the fish shed on the landing and trades fish to Kate for firewood; keeps the family's accounts",
               "brisk, a market voice", ["Tom Haskell: husband", "Ben Haskell: son", "Molly Brennan: buys her fish", "Kate Tierney: trades with her"], T(0.8, 0.3, 0.4, 0.5, 0.6, 0.7),
               aims=[{"text": "trade a basket of fish to Kate for firewood", "want": HAS("Kate Tierney", "a basket of fish")}]),
        person("Ben Haskell", 10, "a family cottage on Orchard Lane",
               "a small, sandy-haired boy in a red knitted sweater and shorts too short for him, scabbed knees, always running",
               M(height=1.36, build=0.8, hair=0.35, hair_style=0.5, hair_color=0.6, skin=0.08, top=0.0, top_shade=0.8, top_cover=0.7, sleeves=1.0, bottom=0.66, bottom_shade=0.35, legs=0.3, shoes=0.45),
               "cheeky, fearless, curious about everything", "helps his grandfather with the nets and plays on the green; wants to catch a fish himself off the end of the pier",
               "fast and excited", ["Tom Haskell: father", "Ruth Haskell: mother", "Abe Haskell: grandfather", "Sam Hartley: best friend", "Charlie Tierney: friend"], T(0.9, 1.0, 1.0, 0.8, 0.6, 0.6),
               aims=[{"text": "catch his own fish off the pier", "want": HOLD("a fish")}]),
        person("Abe Haskell", 76, "a cottage on Bluff Lane",
               "a small, brown, wrinkled old fisherman with a white stubble beard, a faded blue cap and a patched sweater, always mending a net",
               M(height=1.66, build=0.9, hair=0.15, hair_color=1.0, beard=0.25, skin=0.25, top=0.62, top_shade=0.55, top_cover=0.8, sleeves=1.0, bottom=1.2, bottom_shade=0.7, legs=0.6, shoes=0.2, hat=0.4),
               "patient, dry-humoured, full of weather sayings", "mends the village's nets on the landing and fishes off the pier with his rod; is teaching his grandson Ben to fish before his own hands give out",
               "slow, salty, half in proverbs", ["Tom Haskell: son", "Ben Haskell: grandson", "Walter Wells: old shipmate", "Amos Reed: his doctor"], T(0.7, 0.4, 0.4, 0.6, 0.8, 0.5),
               aims=[{"text": "see Ben catch his first fish", "want": HAS("Ben Haskell", "a fish")}]),
        person("Finn O'Leary", 24, "a cottage on Bluff Lane",
               "a lanky young man with black curls and a ready grin, a green wool cap, a canvas jacket, a fiddle case on his back",
               M(height=1.84, build=0.9, hair=0.45, hair_style=0.5, hair_color=0.05, skin=0.06, top=0.36, top_shade=0.35, top_cover=0.9, sleeves=1.0, bottom=0.62, bottom_shade=0.3, legs=1.0, shoes=0.7, hat=0.5),
               "funny, easygoing, a romantic", "crews on Tom's lobster boat in the mornings and plays the fiddle at the Anchor in the evenings; wants to give Clara something made for her, and has asked Hazel in Pine Hollow to carve it",
               "lilting and quick", ["Tom Haskell: skipper", "Gus Brennan: plays at his tavern", "Clara Ames: sweet on her", "Hazel Larsen: carving for him"], T(0.9, 0.9, 0.6, 0.6, 0.7, 0.6),
               aims=[{"text": "give Clara a carved gift", "want": HAS("Clara Ames", "a carving")}]),
        # The boat shop's family.
        person("Eli Crane", 38, "a family cottage on Orchard Lane",
               "a wiry, sawdusty man with close-cropped hair and a short beard, wire spectacles, a canvas work apron full of tools",
               M(height=1.78, hair=0.15, hair_color=0.3, beard=0.3, skin=0.55, undertone=0.6, top=1.2, top_shade=0.6, top_cover=0.8, sleeves=0.6, bottom=0.08, bottom_shade=0.4, legs=1.0, shoes=0.7),
               "careful, patient, proud of a tight seam", "builds and mends the village's boats in the boat shop on the landing; needs good oak logs from John in Pine Hollow to build Tom's new boat",
               "measured and quiet", ["Ivy Crane: wife", "Lily Crane: daughter", "Tom Haskell: building his boat", "John Larsen: his oak"], T(0.5, 0.4, 0.6, 0.5, 0.7, 0.9),
               aims=[{"text": "get oak logs from John for Tom's boat", "want": HAS("Eli Crane", "a log")}]),
        person("Ivy Crane", 35, "a family cottage on Orchard Lane",
               "a cheerful woman with short black hair, paint on her hands and her overalls, a red bandana",
               F(height=1.62, hair=0.25, hair_color=0.05, skin=0.5, undertone=0.55, top=0.05, top_shade=0.5, top_cover=0.8, sleeves=0.5, bottom=0.62, bottom_shade=0.4, legs=1.0, shoes=0.45, hat=0.2),
               "bright, playful, a little untidy, sees colour everywhere", "paints the village's signs and the boats' names, and paints the cove at sunset at her easel on the beach, the painting still half done; teaches Clara to paint",
               "bright and quick", ["Eli Crane: husband", "Lily Crane: daughter", "Clara Ames: her pupil"], T(0.8, 0.8, 0.8, 0.5, 0.7, 0.8),
               aims=[{"text": "paint the sunset from the pier head"}]),
        person("Lily Crane", 8, "a family cottage on Orchard Lane",
               "a tiny girl with black pigtails, a yellow raincoat and red boots, always carrying a jar for treasures",
               F(height=1.26, build=0.8, hair=0.6, hair_style=0.6, hair_color=0.05, skin=0.52, undertone=0.58, top=0.15, top_shade=0.8, top_cover=0.9, sleeves=1.0, bottom=0.15, bottom_shade=0.8, legs=0.4, shoes=0.8),
               "sweet, bold, collects leaves and shells", "plays on the green and the beach and feeds every cat in the village; wants a puppy, and has heard John's dog Bear is a good dog",
               "piping and earnest", ["Eli Crane: father", "Ivy Crane: mother", "Mae Collins: gives her flowers", "Lottie Pierce: teacher"], T(0.9, 1.0, 1.0, 0.7, 0.8, 0.5),
               aims=[{"text": "have a dog of her own"}]),
        # The smith, the teacher, the doctor.
        person("Hank Doyle", 47, "a cottage on Wood Road",
               "a huge, gentle, soot-streaked smith with a bald head and a thick black beard, a leather apron over bare forearms",
               M(height=1.9, build=1.4, hair=0.05, beard=0.7, hair_color=0.05, skin=0.7, undertone=0.6, top=0.62, top_shade=0.2, top_cover=0.8, sleeves=0.3, bottom=0.08, bottom_shade=0.25, legs=1.0, shoes=0.8),
               "slow to speak, strong, tender with animals", "works the forge in the smithy by the green: shoes the hamlet's horses, mends hinges, hooks and anchors; needs Will's charcoal for the forge and is forging a new bell for the schoolhouse",
               "low rumble", ["Will Larsen: sells him charcoal", "Kate Tierney: shoes her horses", "Lottie Pierce: making her a bell"], T(0.5, 0.4, 0.4, 0.8, 0.8, 0.8),
               aims=[{"text": "get charcoal from Will for the forge", "want": HAS("Hank Doyle", "charcoal")}, {"text": "forge a new bell for the schoolhouse"}]),
        person("Lottie Pierce", 29, "a cottage on Wood Road",
               "a brisk young teacher with brown hair under a felt hat, a long tweed skirt, a red scarf, a satchel of books",
               F(height=1.66, hair=0.7, hair_style=0.8, hair_color=0.3, skin=0.12, top=0.08, top_shade=0.35, top_cover=0.9, sleeves=1.0, skirt=1.0, bottom=0.1, bottom_shade=0.4, shoes=0.45, hat=0.6),
               "lively, patient, loves the woods and their names", "teaches the village's children in the red schoolhouse; takes them out to learn the trees and the birds; wants every child to know the names of the trees",
               "clear, encouraging", ["Sam Hartley: pupil", "Ben Haskell: pupil", "Lily Crane: pupil", "Hank Doyle: making her a bell"], T(0.8, 0.6, 0.9, 0.5, 0.8, 0.6),
               aims=[{"text": "teach the children the names of the trees"}]),
        person("Amos Reed", 66, "a cottage at the end of Bluff Lane",
               "a thin, kindly old doctor with a white moustache, a brown tweed suit, a black bag and a walking stick",
               M(height=1.8, build=0.85, hair=0.3, hair_color=0.95, beard=0.15, skin=0.65, undertone=0.6, top=0.08, top_shade=0.3, top_cover=1.0, sleeves=1.0, bottom=0.08, bottom_shade=0.3, legs=1.0, shoes=0.45),
               "gentle, wise, fond of chess and long walks", "is the village doctor: walks his rounds every morning, looks in on Abe's cough and Elsie's knees, and plays chess with Walter",
               "soft and unhurried", ["Abe Haskell: patient", "Walter Wells: chess partner", "Elsie Larsen: patient"], T(0.7, 0.4, 0.6, 0.5, 0.9, 0.5),
               aims=[{"text": "look in on Abe and his cough"}]),
        # The lighthouse, and the orchard.
        person("Agnes Moore", 55, "the keeper's house at the lighthouse",
               "a weathered, strong woman with cropped grey hair, a heavy cable-knit sweater, wool trousers and boots, a brass lantern",
               F(height=1.7, hair=0.2, hair_color=0.8, skin=0.14, top=0.66, top_shade=0.85, top_cover=1.0, sleeves=1.0, bottom=0.08, bottom_shade=0.25, legs=1.0, shoes=0.8),
               "solitary, wry, steady, kinder than she looks", "keeps the lighthouse on Lantern Point: trims and lights the lamp each dusk and watches for boats; wants someone young to learn the light from her",
               "low and wry", ["Sam Hartley: wants to teach him", "Walter Wells: old friend", "Tom Haskell: watches for his boat"], T(0.4, 0.4, 0.6, 0.9, 0.7, 0.7),
               aims=[{"text": "teach Sam to keep the light"}]),
        person("Mae Collins", 70, "a cottage on Orchard Lane",
               "a tiny, smiling old woman with a white braid, a straw hat with a ribbon, a flowered dress and a long green cardigan, a basket of flowers on her arm",
               F(height=1.52, build=0.9, hair=0.85, hair_style=0.8, hair_color=1.0, skin=0.1, top=0.33, top_shade=0.6, top_cover=0.9, sleeves=1.0, skirt=1.0, bottom=0.33, bottom_shade=0.55, shoes=0.45, hat=0.8),
               "sunny, generous, a little forgetful, talks to her plants", "grows pumpkins, asters and mums in her garden and gives them away; wants a pumpkin on every doorstep for the harvest supper",
               "sing-song and kind", ["Lily Crane: gives her flowers", "Harriet Wells: friend", "Owen Fairbanks: like a grandson"], T(0.8, 0.6, 0.6, 0.4, 1.0, 0.5),
               aims=[{"text": "give pumpkins to the children for the harvest supper"}]),
        person("Owen Fairbanks", 34, "a family cottage on Bluff Lane",
               "a sunburnt, sandy-haired orchardist in a plaid flannel shirt, a quilted vest and work boots, a ladder over his shoulder",
               M(height=1.82, build=1.05, hair=0.3, hair_color=0.6, beard=0.2, skin=0.1, top=0.0, top_shade=0.15, top_cover=0.8, sleeves=1.0, bottom=0.66, bottom_shade=0.3, legs=1.0, shoes=0.8),
               "easygoing, cheerful, loves the trees", "keeps the apple orchards on the hills: picks the apples, presses cider for the tavern, and sells baskets to Rosie and Dale",
               "easy and drawling", ["Grace Fairbanks: wife", "Rosie Ames: sells her apples", "Dale Pruitt: sells him apples", "Mae Collins: like a grandmother"], T(0.8, 0.6, 0.5, 0.6, 0.8, 0.5),
               aims=[{"text": "bring a basket of apples to Rosie", "want": HAS("Rosie Ames", "a basket of apples")}]),
        person("Grace Fairbanks", 32, "a family cottage on Bluff Lane",
               "a calm woman with long chestnut hair, a hand-knitted cream sweater, a long brown skirt, knitting needles in her pocket",
               F(height=1.66, hair=0.9, hair_style=0.8, hair_color=0.4, skin=0.1, top=0.8, top_shade=0.95, top_cover=0.9, sleeves=1.0, skirt=1.0, bottom=0.08, bottom_shade=0.35, shoes=0.45),
               "calm, patient, a good friend", "knits warm hats and scarves for the whole village and helps Owen in the orchard; is knitting Walter a scarf for the winter",
               "soft and steady", ["Owen Fairbanks: husband", "Walter Wells: knitting for him", "Nell Hartley: friend"], T(0.6, 0.4, 0.5, 0.5, 0.9, 0.7),
               aims=[{"text": "knit Walter a scarf for the winter"}]),
    ]


def woodcutters():
    W = {"skin": 0.12}
    return [
        person("John Larsen", 52, "the big cabin in Pine Hollow",
               "a huge, gentle woodcutter with a bushy brown beard, a red buffalo-check shirt, leather braces and boots",
               M(height=1.92, build=1.35, hair=0.3, hair_style=0.5, beard=0.8, hair_color=0.4, top=0.0, top_shade=0.45, top_cover=0.8, sleeves=1.0, bottom=0.08, bottom_shade=0.3, legs=1.0, shoes=0.8, trim=0.08, **W),
               "slow to speak, deeply kind, never fells a tree without thanks",
               "fells only the old trees in the maple woods, splits and stacks them, and leads the hamlet; has promised Eli his best oak for Tom's new boat",
               "deep and slow", ["Martha Larsen: wife", "Will Larsen: son", "Hazel Larsen: daughter", "Elsie Larsen: mother", "Eli Crane: promised him oak"], T(0.6, 0.4, 0.4, 0.8, 0.9, 0.5),
               aims=[{"text": "fell an old oak for Eli's boat", "want": HOLD("a log")}, {"text": "plant a new grove for his grandchildren"}]),
        person("Martha Larsen", 49, "the big cabin in Pine Hollow",
               "a strong woman with long grey-brown hair braided over one shoulder, a green dress and a straw hat, a bee veil pushed back",
               F(height=1.66, hair=0.85, hair_style=0.8, hair_color=0.55, top=0.33, top_shade=0.35, top_cover=0.8, sleeves=0.6, skirt=0.8, bottom=0.33, bottom_shade=0.35, shoes=0.45, hat=0.8, **W),
               "calm, knows every plant and herb", "keeps the bees and the vegetable gardens and makes wildflower honey; sells her honey to Dale's store and Gus's tavern",
               "soft, unhurried", ["John Larsen: husband", "Will Larsen: son", "Hazel Larsen: daughter", "Dale Pruitt: sells him honey", "Gus Brennan: sells him honey"], T(0.6, 0.4, 0.7, 0.5, 0.8, 0.6),
               aims=[{"text": "take honey down to Dale's store", "want": HAS("Dale Pruitt", "honey")}]),
        person("Will Larsen", 22, "the big cabin in Pine Hollow",
               "a soot-smudged young man with fair hair to his shoulders, a grey thermal shirt and patched trousers",
               M(height=1.8, hair=0.7, hair_color=0.65, top=1.2, top_shade=0.4, top_cover=0.8, sleeves=1.0, bottom=0.08, bottom_shade=0.25, legs=1.0, shoes=0.7, **W),
               "quiet, proud, competitive", "burns charcoal in the kiln and sells it to Hank for the forge and Rosie for her oven; wants to prove he can fell a tree faster than Jack",
               "short and plain", ["John Larsen: father", "Martha Larsen: mother", "Hazel Larsen: sister", "Jack Tierney: rival", "Hank Doyle: buys his charcoal"], T(0.4, 0.4, 0.5, 0.8, 0.5, 0.6),
               aims=[{"text": "fell a tree before Jack does"}, {"text": "bring charcoal to Hank", "want": HAS("Hank Doyle", "charcoal")}]),
        person("Hazel Larsen", 16, "the big cabin in Pine Hollow",
               "a freckled girl with a long copper braid, a blue flannel shirt over trousers and boots, wood shavings in her hair",
               F(height=1.62, hair=0.85, hair_style=0.8, hair_color=0.7, top=0.62, top_shade=0.55, top_cover=1.0, sleeves=1.0, bottom=0.08, bottom_shade=0.35, legs=1.0, shoes=0.7, **W),
               "quiet, patient, wonderful with animals", "looks after the horses and whittles little animals from birch; Finn asked her to carve something for Clara",
               "quiet and earnest", ["John Larsen: father", "Martha Larsen: mother", "Will Larsen: brother", "Finn O'Leary: carving for him"], T(0.5, 0.6, 0.8, 0.6, 0.8, 0.9),
               aims=[{"text": "carve a gift for Finn to give Clara", "want": HAS("Finn O'Leary", "a carving")}, {"text": "ride all the way to the lighthouse"}]),
        person("Elsie Larsen", 81, "the oldest cabin, by the great oak",
               "a tiny, bent old woman with white hair under a knitted shawl, a long brown dress, a walking stick",
               F(height=1.5, build=0.85, hair=0.7, hair_style=0.8, hair_color=1.0, top=0.08, top_shade=0.2, top_cover=1.0, sleeves=1.0, skirt=1.0, bottom=0.08, bottom_shade=0.2, shoes=0.45, hat=0.2, **W),
               "wise, mischievous, tells tales of the woods", "keeps the lanterns on the great oak, ties its ribbons, and tells the old stories; wants every child to learn the old songs of the woods",
               "a cracked, sing-song voice", ["John Larsen: son", "Amos Reed: her doctor", "Charlie Tierney: favourite"], T(0.7, 0.6, 0.6, 0.5, 0.9, 0.7),
               aims=[{"text": "teach Charlie and Ben the song of the woods"}]),
        person("Jack Tierney", 35, "the cabin by the woodpile",
               "a wiry, quick woodcutter with a short black beard, a brown canvas vest over a white shirt, a felt hat",
               M(height=1.76, hair=0.25, beard=0.4, hair_color=0.05, top=0.08, top_shade=0.3, top_cover=0.8, sleeves=1.0, bottom=0.08, bottom_shade=0.2, legs=1.0, shoes=0.8, hat=0.6, trim=0.08, **W),
               "boastful, funny, works harder than anyone", "saws and splits the logs and builds things of wood; promised Kate a new woodshed beside their cabin before the snow",
               "quick and boastful", ["Kate Tierney: wife", "Charlie Tierney: son", "Will Larsen: rival", "John Larsen: master"], T(0.8, 0.7, 0.5, 0.8, 0.5, 0.8),
               aims=[{"text": "build Kate a woodshed", "want": {"kind": "be", "place": "the woodshed"}}]),
        person("Charlie Tierney", 9, "the cabin by the woodpile",
               "a round-faced boy with a brown fringe, a green jacket and short trousers, a wooden sword in his belt",
               M(height=1.33, build=0.85, hair=0.35, hair_color=0.3, top=0.33, top_shade=0.4, top_cover=0.9, sleeves=1.0, bottom=0.08, bottom_shade=0.4, legs=0.4, shoes=0.45, **W),
               "dreamy, adventurous, loves Elsie's stories", "gathers kindling and acorns; wants to find the white stag of the woods that Elsie tells of",
               "excited, a storyteller", ["Jack Tierney: father", "Kate Tierney: mother", "Elsie Larsen: favourite", "Ben Haskell: friend"], T(0.8, 1.0, 1.0, 0.7, 0.7, 0.6),
               aims=[{"text": "find the white stag of the woods"}]),
        person("Kate Tierney", 33, "the cabin by the woodpile",
               "a tall, sunburnt carter with a red kerchief over cropped hair, a leather waistcoat, trousers and boots",
               F(height=1.74, hair=0.2, hair_color=0.35, top=0.05, top_shade=0.45, top_cover=0.8, sleeves=1.0, bottom=0.1, bottom_shade=0.3, legs=1.0, shoes=0.8, hat=0.2, **W),
               "brave, blunt, laughs loudly", "drives the firewood and charcoal down to the village with the horses and brings back fish and bread; wants a second wagon",
               "loud and plain", ["Jack Tierney: husband", "Charlie Tierney: son", "Ruth Haskell: trades with her", "Hank Doyle: shoes her horses"], T(0.7, 0.6, 0.5, 0.9, 0.6, 0.5),
               aims=[{"text": "trade firewood to Ruth for fish"}, {"text": "get a second wagon built"}]),
    ]


# Who lives where: building key -> names.
HOUSEHOLDS = {
    "mayor": ["Harriet Wells", "Walter Wells"],
    "w1": ["Gus Brennan", "Molly Brennan"],
    "inn": ["Nell Hartley", "Sam Hartley"],
    "store": ["Dale Pruitt", "June Pruitt"],
    "bakery": ["Rosie Ames", "Clara Ames"],
    "e3": ["Tom Haskell", "Ruth Haskell", "Ben Haskell"],
    "w3": ["Abe Haskell"],
    "w4": ["Finn O'Leary"],
    "e6": ["Eli Crane", "Ivy Crane", "Lily Crane"],
    "n1": ["Hank Doyle"],
    "n2": ["Lottie Pierce"],
    "w6": ["Amos Reed"],
    "e5": ["Mae Collins"],
    "w2": ["Owen Fairbanks", "Grace Fairbanks"],
}
CHILDREN = {"Sam Hartley", "Ben Haskell", "Lily Crane", "Clara Ames"}
COUPLES = {"mayor", "w1", "store", "e3", "e6", "w2"}
# Something of a household's own in its home.
HOBBY = {"mayor": "bookshelf", "w3": "armchair", "w4": "armchair", "e6": "bookshelf", "n2": "bookshelf", "w6": "desk", "e5": "armchair", "w2": "upright piano"}

ITEMS = [("fishing rod", "Abe Haskell"), ("fishing rod", "Ben Haskell"), ("fishing rod", "Tom Haskell"), ("fishing rod", "Sam Hartley"), ("fishing net", "Abe Haskell"),
         ("basket of fish", "Tom Haskell"), ("basket of fish", "Ruth Haskell"), ("leather ball", "Ben Haskell"), ("basket of bread", "Rosie Ames"), ("apple pie", "Rosie Ames"),
         ("fiddle", "Finn O'Leary"), ("mug of beer", "Gus Brennan"), ("basket of apples", "Owen Fairbanks"), ("pumpkin", "Mae Collins"), ("bunch of flowers", "Mae Collins"),
         ("newspaper", "June Pruitt"), ("book", "Lottie Pierce"), ("book", "Amos Reed"), ("broom", "Nell Hartley"), ("watering can", "Mae Collins"), ("hand lantern", "Agnes Moore"),
         ("woodcutter's axe", "John Larsen"), ("woodcutter's axe", "Jack Tierney"), ("woodcutter's axe", "Will Larsen"), ("jar of honey", "Martha Larsen"), ("leather ball", "Charlie Tierney"),
         ("bucket", "Molly Brennan")]
RIDES = [("bicycle", "June Pruitt"), ("bicycle", "Amos Reed"), ("bicycle", "Sam Hartley")]


def creatures(h):
    return [
        # In the village: cats about the landing and the green, a few dogs.
        {"species": "cat", "count": 3, "x": 10.0, "z": -22.0, "names": ["Pickles", "Mackerel", "Smudge"], "owner": "", "description": "landing cats begging at the fish crates"},
        {"species": "cat", "count": 2, "x": -6.0, "z": 30.0, "names": ["Pumpkin", "Maple"], "owner": "Lily Crane", "description": "two ginger cats who sleep in the gazebo, fed by Lily"},
        {"species": "cat", "count": 1, "x": -28.0, "z": 40.0, "names": ["Biscuit"], "owner": "June Pruitt", "description": "the store's fat tabby, asleep on the porch"},
        {"species": "cat", "count": 1, "x": 2.0, "z": 50.0, "names": ["Duchess"], "owner": "Nell Hartley", "description": "the inn's grey cat, who greets every guest"},
        {"species": "dog", "count": 1, "x": 18.0, "z": -24.0, "names": ["Skipper"], "owner": "Tom Haskell", "description": "a scruffy black-and-white boat dog"},
        {"species": "dog", "count": 1, "x": 28.0, "z": 46.0, "names": ["Admiral"], "owner": "Walter Wells", "description": "a dignified old golden retriever who walks the pier with Walter"},
        {"species": "dog", "count": 1, "x": -22.0, "z": -22.0, "names": ["Barley"], "owner": "Gus Brennan", "description": "a big friendly brown dog who lies by the tavern's porch"},
        {"species": "dog", "count": 1, "x": -74.0, "z": 6.0, "names": ["Pip"], "owner": "Owen Fairbanks", "description": "a bouncy little terrier"},
        # In the cove: harbor seals round the pier.
        {"species": "harbor seal", "count": 4, "x": 14.0, "z": -70.0, "names": [], "owner": "", "description": "seals bobbing round the pier head"},
        {"species": "harbor seal", "count": 3, "x": -120.0, "z": -80.0, "names": [], "owner": "", "description": "seals off the beach"},
        # Pine Hollow: the horses and the dog.
        {"species": "horse", "count": 3, "x": 154.0, "z": 492.0, "names": ["Nutmeg", "Smoke", "Star"], "owner": "Kate Tierney", "description": "the hamlet's sturdy work horses"},
        {"species": "dog", "count": 1, "x": 126.0, "z": 507.0, "names": ["Bear"], "owner": "John Larsen", "description": "a huge shaggy brown dog"},
        # Wild: white-tailed deer everywhere the meadows meet the woods.
        {"species": "deer", "count": 5, "x": 70.0, "z": 120.0, "names": [], "owner": "", "description": "white-tailed deer grazing in the meadow above the village"},
        {"species": "deer", "count": 4, "x": -70.0, "z": 100.0, "names": [], "owner": "", "description": "deer among the asters behind Bluff Lane"},
        {"species": "deer", "count": 6, "x": 60.0, "z": 600.0, "names": [], "owner": "", "description": "deer at the edge of the maple woods"},
        {"species": "deer", "count": 5, "x": 205.0, "z": 470.0, "names": [], "owner": "", "description": "deer in the clearings below the woods"},
        {"species": "deer", "count": 5, "x": -40.0, "z": 330.0, "names": [], "owner": "", "description": "deer in the tall grass by the coast road"},
        {"species": "deer", "count": 4, "x": 170.0, "z": 120.0, "names": [], "owner": "", "description": "a doe and her fawns in the meadows toward the point"},
        {"species": "deer", "count": 4, "x": -230.0, "z": 220.0, "names": [], "owner": "", "description": "deer in the orchards"},
        {"species": "horse", "count": 3, "x": -160.0, "z": 160.0, "names": [], "owner": "", "description": "Owen's horses out in the meadow"},
    ]


def lay(p, cape_h, clearing):
    import interiors
    import village
    folk = villagers()
    by_name = {c["name"]: c for c in folk}
    # Everyone stands by day at their own door, the household side by side.
    for key, names in HOUSEHOLDS.items():
        x, z, h = village.door_of(key)
        for i, n in enumerate(names):
            c = by_name[n]
            c["home_x"], c["home_z"], c["y"] = round(x + (i - (len(names) - 1) / 2) * 1.2, 2), round(z - 0.5 * (i % 2), 2), h
    agnes = by_name["Agnes Moore"]
    agnes["home_x"], agnes["home_z"], agnes["y"] = 242.0, -176.0, cape_h
    import hamlet
    woods = woodcutters()
    for c, (dx, dz) in zip(woods, ((-6, -9), (21, 9), (-18, 13), (17, -21), (1, 9), (-9, 9), (1, -5), (11, -21))):
        c["home_x"], c["home_z"], c["y"] = hamlet.C[0] + dx, hamlet.C[1] + dz, clearing
    p.people += folk + woods
    homes = {c["name"]: (c["home_x"], c["home_z"]) for c in p.people}
    for ty, who in ITEMS:
        x, z = homes[who]
        p.item(ty, who, x, z)
    # Bikes lean by their owners' doors, on the level yard.
    for ty, who in RIDES:
        key = next(k for k, names in HOUSEHOLDS.items() if who in names)
        x, z, b, h, kind = village.PLACED[key]
        W, D, dx, porch = village.SHAPE[kind]
        side = 1 if dx < 0 else -1
        p.put(ty, *village.local(key, dx + side * (W * 0.5 + 0.8), D + porch + 1.0), b + 90, y=h)
    p.creatures += creatures(clearing)
    households = {key: (key in COUPLES, sum(1 for n in names if n in CHILDREN), HOBBY.get(key)) for key, names in HOUSEHOLDS.items()}
    interiors.lay(p, households)
