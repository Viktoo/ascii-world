"""Who lives in Marinella and Boscoverde, their things, and the animals."""

# Looks shared by the people of the coast: olive skin, dark hair and eyes.
COAST = {"skin": 0.36, "undertone": 0.78, "eyes": 0.25, "eye_shape": 0.3, "nose": 0.35, "hair_color": 0.12}
WOODS = {"skin": 0.3, "undertone": 0.7, "eyes": 0.4, "eye_shape": 0.3, "nose": 0.4, "hair_color": 0.3}


def person(name, age, x, z, y, home, appearance, look, personality, goals, voice, rel, traits, base=COAST, aims=None):
    lk = dict(base)
    lk.update(look)
    c = {"name": name, "age": age, "appearance": appearance, "look": lk, "personality": personality, "goals": goals, "voice": voice,
         "tone": {}, "traits": traits, "home": home, "home_x": x, "home_z": z, "relationships": rel, "species": "human", "variety": "", "layers": []}
    if aims:
        c["aims"] = aims
    if y is not None:
        c["y"] = y
    return c


def villagers(cape_h):
    T = lambda s, p, c, b, g, cr: {"sociable": s, "playful": p, "curious": c, "brave": b, "generous": g, "crafty": cr}
    return [
        person("Nonno Beppe", 74, -30.0, -17.0, 1.8, "a blue house on the quay",
               "a small, brown, wrinkled old fisherman with a white stubble beard, a faded blue cap and rolled trousers, always mending a net",
               {"height": 1.66, "build": 0.9, "shape": 0.0, "hair": 0.15, "hair_color": 1.0, "beard": 0.25, "top": 0.62, "top_shade": 0.55, "top_cover": 0.8, "sleeves": 0.5,
                "bottom": 1.2, "bottom_shade": 0.7, "legs": 0.6, "shoes": 0.2, "hat": 0.4, "trim": 0.62, "skin": 0.48},
               "patient, dry-humoured, full of sea stories and weather sayings",
               "mends the village's fishing nets on the quay; wants to see his grandson Tomi take a boat out before he dies",
               "slow, salty, half in proverbs", ["Marco: son", "Lucia: daughter-in-law", "Tomi: grandson", "Pietro: apprentice"], T(0.7, 0.4, 0.4, 0.6, 0.8, 0.5)),
        person("Marco", 41, -6.0, -21.0, 1.8, "a blue house on the quay",
               "a broad, sunburnt fisherman with black curly hair and a short beard, in a striped shirt with sleeves rolled",
               {"height": 1.8, "build": 1.15, "shape": 0.0, "hair": 0.25, "hair_style": 0.5, "beard": 0.35, "top": 0.62, "top_shade": 0.8, "top_cover": 0.8, "sleeves": 0.45,
                "bottom": 0.62, "bottom_shade": 0.3, "legs": 0.9, "shoes": 0.7, "trim": 1.2},
               "hard-working, proud of his boat, loud and warm", "fishes the bay in the blue gozzo before dawn and sells the catch on the quay; wants a bigger boat with a proper engine",
               "loud and quick, laughs easily", ["Lucia: wife", "Tomi: son", "Nonno Beppe: father", "Matteo: friend"], T(0.8, 0.5, 0.3, 0.8, 0.6, 0.4),
               aims=[{"text": "own a bigger boat", "want": {"kind": "hold", "what": "a bigger boat"}}]),
        person("Lucia", 38, 12.0, -19.0, 1.8, "a blue house on the quay",
               "a quick, strong woman with her dark hair tied back under a red kerchief, a white blouse and a long blue apron, selling fish",
               {"height": 1.64, "shape": 1.0, "hair": 0.75, "hair_style": 0.8, "top": 1.2, "top_shade": 0.95, "top_cover": 0.7, "sleeves": 0.4, "skirt": 0.8, "bottom": 0.62,
                "bottom_shade": 0.4, "shoes": 0.2, "hat": 0.2, "trim": 0.0},
               "practical, sharp at a bargain, kind to children", "sells the morning's fish from the crates on the quay and keeps the family's accounts; wants Tomi to go to school in the city",
               "brisk, with a market voice", ["Marco: husband", "Tomi: son", "Nonno Beppe: father-in-law", "Rosa: friend"], T(0.8, 0.3, 0.4, 0.5, 0.6, 0.7)),
        person("Tomi", 10, 4.0, 39.0, 10.6, "a blue house on the quay",
               "a skinny, barefoot boy with a mop of black hair, in shorts and a yellow shirt, always running",
               {"height": 1.38, "build": 0.8, "shape": 0.0, "hair": 0.35, "hair_style": 0.5, "top": 0.15, "top_shade": 0.8, "top_cover": 0.6, "sleeves": 0.2, "bottom": 0.62,
                "bottom_shade": 0.35, "legs": 0.25, "shoes": 0.0},
               "cheeky, fearless, curious about everything", "plays ball in the piazza and helps his grandfather with the nets; wants to steer the boat himself",
               "fast and excited, full of questions", ["Marco: father", "Lucia: mother", "Nonno Beppe: grandfather", "Sofia: best friend", "Teo: friend"], T(0.9, 1.0, 1.0, 0.8, 0.6, 0.6)),
        person("Giulia", 34, 31.5, 46.0, 10.6, "rooms over the Caffè del Porto",
               "a tall, smiling woman with short dark hair, gold earrings, a black dress and a white apron, a tray always in her hand",
               {"height": 1.72, "shape": 1.0, "hair": 0.3, "top": 0.0, "top_shade": 0.05, "top_cover": 0.8, "sleeves": 0.3, "neckline": 0.4, "skirt": 0.5, "bottom": 0.0, "bottom_shade": 0.05,
                "shoes": 0.45, "trim": 1.2},
               "warm, talkative, knows everyone's news first", "runs the Caffè del Porto under the arcade on the piazza, coffee and lemon granita; wants to put on a summer dance in the piazza",
               "bright and chatty", ["Rosa: friend", "Don Anselmo: teases him", "Pietro: cousin"], T(1.0, 0.7, 0.6, 0.5, 0.7, 0.5),
               aims=[{"text": "hold a summer dance in the piazza"}]),
        person("Don Anselmo", 61, -6.0, 59.5, 13.6, "the house behind San Pietro",
               "a round, bald priest with spectacles and a kindly face, in a long black cassock",
               {"height": 1.68, "build": 1.25, "shape": 0.0, "hair": 0.1, "hair_color": 0.9, "top": 0.0, "top_shade": 0.05, "top_cover": 1.0, "sleeves": 1.0, "skirt": 1.0,
                "bottom": 0.0, "bottom_shade": 0.05, "shoes": 0.45, "trim": 1.2},
               "gentle, fond of food and gossip, wiser than he lets on", "rings San Pietro's bell, keeps the church and blesses the boats; wants the bell tower's old clock to run again",
               "soft, slow, a little Latin now and then", ["Giulia: friend", "Rosa: friend", "Grazia: old friend"], T(0.8, 0.4, 0.5, 0.4, 0.9, 0.4)),
        person("Rosa", 55, 20.0, 46.0, 10.6, "over the bakery on the piazza",
               "a plump, rosy-cheeked baker with flour on her arms, grey-streaked hair in a bun, a pink dress and a white apron",
               {"height": 1.6, "build": 1.25, "shape": 1.0, "hair": 0.7, "hair_style": 0.8, "hair_color": 0.55, "top": 0.92, "top_shade": 0.75, "top_cover": 0.8, "sleeves": 0.35,
                "skirt": 0.8, "bottom": 0.92, "bottom_shade": 0.75, "shoes": 0.45, "trim": 1.2},
               "motherly, bossy, generous with bread", "bakes the village's bread and focaccia before dawn; wants Elena to take over the bakery one day",
               "warm and scolding", ["Elena: daughter", "Lucia: friend", "Giulia: friend"], T(0.8, 0.4, 0.3, 0.5, 1.0, 0.4)),
        person("Elena", 19, 17.0, 41.0, 10.6, "over the bakery on the piazza",
               "a slim young woman with long dark wavy hair, a sky-blue summer dress and sandals",
               {"height": 1.66, "shape": 1.0, "hair": 0.85, "hair_style": 0.5, "top": 0.55, "top_shade": 0.75, "top_cover": 0.8, "sleeves": 0.0, "neckline": 0.4, "skirt": 0.6,
                "bottom": 0.55, "bottom_shade": 0.75, "shoes": 0.2, "trim": 1.2},
               "dreamy, clever, restless", "helps at the bakery and draws the harbour; wants to see the cities across the sea",
               "soft and thoughtful", ["Rosa: mother", "Pietro: sweetheart", "Giulia: friend"], T(0.6, 0.6, 0.9, 0.5, 0.6, 0.6),
               aims=[{"text": "win Pietro's heart", "want": {"kind": "affection", "with": "Pietro", "at_least": 0.7}}]),
        person("Pietro", 24, -40.0, -16.5, 1.8, "a room in the yellow house by the ramp",
               "a lean young fisherman with tousled black hair, a white shirt open at the neck and dark trousers, a mandolin on his back",
               {"height": 1.78, "shape": 0.0, "hair": 0.4, "hair_style": 0.5, "top": 1.2, "top_shade": 0.95, "top_cover": 0.8, "sleeves": 0.6, "neckline": 0.6, "bottom": 0.62,
                "bottom_shade": 0.2, "legs": 1.0, "shoes": 0.45},
               "romantic, easy-going, a little shy sober", "fishes with Nonno Beppe and plays the mandolin on the quay in the evenings; wants to marry Elena",
               "gentle, sings rather than speaks when happy", ["Nonno Beppe: master", "Elena: sweetheart", "Giulia: cousin", "Matteo: friend"], T(0.7, 0.8, 0.5, 0.4, 0.7, 0.4)),
        person("Carla", 46, -33.0, 44.0, 10.6, "a green house on the second terrace",
               "a sturdy woman with a sun hat over her curly hair, a green dress with an apron full of change",
               {"height": 1.63, "build": 1.1, "shape": 1.0, "hair": 0.55, "hair_style": 0.5, "top": 0.33, "top_shade": 0.5, "top_cover": 0.8, "sleeves": 0.3, "skirt": 0.8,
                "bottom": 0.33, "bottom_shade": 0.5, "shoes": 0.2, "hat": 0.8, "trim": 0.12},
               "cheerful, haggles for fun", "sells lemons, tomatoes and greens from her market stall on the piazza; wants to grow the biggest lemons on the coast",
               "sing-song, calls out her wares", ["Sofia: daughter", "Marta: buys honey from her"], T(0.9, 0.6, 0.4, 0.5, 0.6, 0.6)),
        person("Sofia", 8, -20.0, 42.0, 10.6, "a green house on the second terrace",
               "a small girl with two black braids, a red dress and bare feet",
               {"height": 1.28, "build": 0.8, "shape": 1.0, "hair": 0.7, "hair_style": 0.8, "top": 0.0, "top_shade": 0.6, "top_cover": 0.8, "sleeves": 0.1, "skirt": 0.4,
                "bottom": 0.0, "bottom_shade": 0.6, "shoes": 0.0},
               "bold, bossy, loves animals", "helps her mother at the stall and feeds every cat in the village; wants a dog of her own",
               "quick and decided", ["Carla: mother", "Tomi: best friend", "Teo: friend"], T(0.9, 1.0, 0.9, 0.9, 0.8, 0.5),
               aims=[{"text": "have a dog of her own"}]),
        person("Franco", 58, 86.0, 100.0, 16.6, "the house below the watchtower",
               "a tall, weathered man with a grey moustache, a navy cap and jacket with brass buttons, a spyglass at his belt",
               {"height": 1.84, "shape": 0.0, "hair": 0.2, "hair_color": 0.85, "beard": 0.15, "top": 0.66, "top_shade": 0.15, "top_cover": 0.9, "sleeves": 1.0, "bottom": 0.66,
                "bottom_shade": 0.15, "legs": 1.0, "shoes": 0.7, "hat": 0.4, "trim": 0.15},
               "watchful, orderly, secretly sentimental", "is harbourmaster and keeps the watch from the tower for storms and boats coming home; wants a new bell for the tower",
               "clipped, precise, a sailor's words", ["Ada: sister", "Marco: worries about him at sea"], T(0.4, 0.2, 0.6, 0.8, 0.5, 0.6)),
        person("Ada", 67, 242.0, -176.0, cape_h, "the keeper's house at the lighthouse",
               "a wiry old woman with cropped white hair, a thick grey sweater and trousers, a lantern in her hand",
               {"height": 1.62, "shape": 1.0, "hair": 0.2, "hair_color": 1.0, "top": 1.2, "top_shade": 0.5, "top_cover": 0.9, "sleeves": 1.0, "bottom": 0.66, "bottom_shade": 0.25,
                "legs": 1.0, "shoes": 0.7},
               "solitary, sharp, kind to visitors who climb the cape", "keeps the lighthouse on Capo Faro, trims and lights its lamp each dusk; wants someone young to learn the light from her",
               "dry and few words", ["Franco: brother", "Nonno Beppe: old friend"], T(0.3, 0.3, 0.6, 0.9, 0.6, 0.5)),
        person("Matteo", 30, 50.0, -18.5, 1.8, "a pink house on the first terrace",
               "a stocky boat painter with paint on his hands and face, cropped hair, a blue work shirt and canvas trousers",
               {"height": 1.75, "build": 1.1, "shape": 0.0, "hair": 0.15, "top": 0.6, "top_shade": 0.45, "top_cover": 0.8, "sleeves": 0.3, "bottom": 1.2, "bottom_shade": 0.75,
                "legs": 1.0, "shoes": 0.45},
               "cheerful, artistic, a joker", "builds and paints the village's boats, each in its own bright colours; wants to paint the eye of the sea on every boat in the harbour",
               "playful and teasing", ["Marco: friend", "Pietro: friend"], T(0.8, 0.9, 0.6, 0.6, 0.7, 0.9)),
    ]


def woodcutters(h):
    T = lambda s, p, c, b, g, cr: {"sociable": s, "playful": p, "curious": c, "brave": b, "generous": g, "crafty": cr}
    W = WOODS
    return [
        person("Bruno", 52, 124.0, 511.0, h, "the big cabin in the clearing",
               "a huge, gentle woodcutter with a bushy brown beard, a red checked shirt, leather braces and boots",
               {"height": 1.92, "build": 1.35, "shape": 0.0, "hair": 0.3, "hair_style": 0.5, "beard": 0.8, "top": 0.0, "top_shade": 0.45, "top_cover": 0.8, "sleeves": 1.0,
                "bottom": 0.08, "bottom_shade": 0.3, "legs": 1.0, "shoes": 0.8, "trim": 0.08}, "slow to speak, deeply kind, never fells a tree without thanks",
               "fells and splits chestnut, choosing only old trees, and leads the hamlet; wants to plant a new grove for his grandchildren",
               "deep and slow", ["Marta: wife", "Ivo: son", "Lia: daughter", "Grazia: mother"], T(0.6, 0.4, 0.4, 0.8, 0.9, 0.5), base=W),
        person("Marta", 49, 151.0, 529.0, h, "the big cabin in the clearing",
               "a strong woman with long grey-brown hair braided over one shoulder, a green dress and a straw hat, a bee veil pushed back",
               {"height": 1.66, "shape": 1.0, "hair": 0.85, "hair_style": 0.8, "hair_color": 0.55, "top": 0.33, "top_shade": 0.35, "top_cover": 0.8, "sleeves": 0.6, "skirt": 0.8,
                "bottom": 0.33, "bottom_shade": 0.35, "shoes": 0.45, "hat": 0.8}, "calm, knows every plant and herb",
               "keeps the bees and the vegetable gardens and makes chestnut honey; wants to sell her honey in the village market",
               "soft, unhurried", ["Bruno: husband", "Ivo: son", "Lia: daughter", "Carla: sells honey to her"], T(0.6, 0.4, 0.7, 0.5, 0.8, 0.6), base=W),
        person("Ivo", 22, 112.0, 533.0, h, "the big cabin in the clearing",
               "a soot-smudged young man with dark hair to his shoulders, a grey shirt and patched trousers",
               {"height": 1.8, "shape": 0.0, "hair": 0.7, "top": 1.2, "top_shade": 0.4, "top_cover": 0.8, "sleeves": 0.8, "bottom": 0.08, "bottom_shade": 0.25, "legs": 1.0, "shoes": 0.7},
               "brooding, proud, competitive", "burns charcoal in the kiln and watches it day and night; wants to prove he can fell a tree faster than Dario",
               "short, a bit sullen", ["Bruno: father", "Marta: mother", "Lia: sister", "Dario: rival"], T(0.4, 0.4, 0.5, 0.8, 0.4, 0.6), base=W),
        person("Lia", 16, 147.0, 499.0, h, "the big cabin in the clearing",
               "a freckled girl with a long auburn braid, a blue tunic over trousers and boots, wood shavings in her hair",
               {"height": 1.62, "shape": 1.0, "hair": 0.85, "hair_style": 0.8, "hair_color": 0.6, "top": 0.62, "top_shade": 0.55, "top_cover": 1.0, "sleeves": 0.7, "bottom": 0.08,
                "bottom_shade": 0.35, "legs": 1.0, "shoes": 0.7}, "quiet, patient, wonderful with animals",
               "looks after the horses and carves little animals from chestnut wood; wants to ride all the way to the cape", "quiet and earnest",
               ["Bruno: father", "Marta: mother", "Ivo: brother", "Silvia: teacher"], T(0.5, 0.6, 0.8, 0.6, 0.8, 0.9), base=W),
        person("Grazia", 81, 131.0, 529.0, h, "the oldest cabin, by the great oak",
               "a tiny, bent old woman with white hair under a black shawl, a long brown dress, a walking stick",
               {"height": 1.5, "build": 0.85, "shape": 1.0, "hair": 0.7, "hair_style": 0.8, "hair_color": 1.0, "top": 0.08, "top_shade": 0.2, "top_cover": 1.0, "sleeves": 1.0,
                "skirt": 1.0, "bottom": 0.08, "bottom_shade": 0.2, "shoes": 0.45, "hat": 0.2}, "wise, mischievous, tells tales of the wood's spirits",
               "keeps the shrine at the great oak, lights its lanterns and ties the ribbons; wants every child to learn the old songs of the wood",
               "a cracked, sing-song voice", ["Bruno: son", "Don Anselmo: old friend", "Teo: favourite"], T(0.7, 0.6, 0.6, 0.5, 0.9, 0.7), base=W),
        person("Dario", 35, 121.0, 529.0, h, "the cabin by the woodpile",
               "a wiry, quick woodcutter with a short black beard, a brown vest over a white shirt, a felt hat",
               {"height": 1.76, "shape": 0.0, "hair": 0.25, "beard": 0.4, "top": 0.08, "top_shade": 0.3, "top_cover": 0.8, "sleeves": 1.0, "bottom": 0.08, "bottom_shade": 0.2,
                "legs": 1.0, "shoes": 0.8, "hat": 0.6, "trim": 0.08}, "boastful, funny, works harder than anyone",
               "saws and splits the logs and builds the sledges; wants to build a cart road straight to the village", "quick and boastful",
               ["Silvia: wife", "Teo: son", "Ivo: rival", "Bruno: master"], T(0.8, 0.7, 0.5, 0.8, 0.5, 0.8), base=W),
        person("Teo", 9, 131.0, 515.0, h, "the cabin by the woodpile",
               "a round-faced boy with a brown fringe, a green jerkin and short trousers, a wooden sword in his belt",
               {"height": 1.33, "build": 0.85, "shape": 0.0, "hair": 0.35, "top": 0.33, "top_shade": 0.4, "top_cover": 0.9, "sleeves": 0.4, "bottom": 0.08, "bottom_shade": 0.4,
                "legs": 0.4, "shoes": 0.45}, "dreamy, adventurous, loves Grazia's stories",
               "gathers kindling and chestnuts; wants to find the spirit of the wood", "excited, a storyteller", ["Dario: father", "Silvia: mother", "Grazia: favourite", "Tomi: friend"],
               T(0.8, 1.0, 1.0, 0.7, 0.7, 0.6), base=W),
        person("Silvia", 33, 141.0, 499.0, h, "the cabin by the woodpile",
               "a tall, sunburnt carter with a red kerchief over cropped hair, a leather waistcoat, trousers and boots",
               {"height": 1.74, "shape": 1.0, "hair": 0.2, "top": 0.05, "top_shade": 0.45, "top_cover": 0.8, "sleeves": 0.4, "bottom": 0.1, "bottom_shade": 0.3, "legs": 1.0,
                "shoes": 0.8, "hat": 0.2, "trim": 0.0}, "brave, blunt, laughs loudly",
               "drives the wood and charcoal down to the village with the horses and brings back fish and salt; wants a second wagon", "loud and plain",
               ["Dario: husband", "Teo: son", "Lia: pupil", "Lucia: trades with her"], T(0.7, 0.6, 0.5, 0.9, 0.6, 0.5), base=W),
    ]


# Each person's work and what they are after, written so that one's aim
# needs another's work: wood from the hamlet for the village's boats and
# ovens, fish and bread up the road, gifts and favours between them.
GOALS = {
    "Nonno Beppe": ("mends the village's nets on the quay and takes the blue gozzo out at dawn with his rod and net; is teaching his grandson Tomi to fish before his own hands give out",
                    [{"text": "see Tomi catch his first fish", "want": {"kind": "has", "who": "Tomi", "what": "a fish"}}]),
    "Marco": ("fishes the bay in the blue gozzo and lands the catch on the quay for Lucia to sell; has asked Matteo to build him a bigger boat of chestnut wood from Boscoverde",
              [{"text": "get Matteo to build him a bigger boat", "want": {"kind": "hold", "what": "a bigger boat"}}]),
    "Lucia": ("sells the fish from the crates on the quay and trades fish to Silvia for charcoal and firewood; keeps the family's accounts",
              [{"text": "trade a basket of fish to Silvia for charcoal", "want": {"kind": "has", "who": "Silvia", "what": "a basket of fish"}}]),
    "Tomi": ("plays ball in the piazza and helps his grandfather with the nets; wants to catch a fish himself from the end of the breakwater",
             [{"text": "catch his own fish", "want": {"kind": "hold", "what": "a fish"}}]),
    "Giulia": ("runs the Caffè del Porto under the arcade: coffee, Rosa's bread, and lemon granita from Carla's lemons; is planning a summer dance in the piazza with Pietro playing",
               [{"text": "get Pietro to play the mandolin for her dance"}, {"text": "buy lemons from Carla for granita", "want": {"kind": "has", "who": "Giulia", "what": "a lemon"}}]),
    "Don Anselmo": ("keeps San Pietro and rings its bell; wants a new bell rope and the old clock mended, and has asked Franco and Matteo for help",
                    [{"text": "have the bell tower clock mended"}]),
    "Rosa": ("bakes the village's bread in her wood-fired oven under the arcade every morning, burning Ivo's charcoal; sends loaves to Giulia's café and up the road to Boscoverde",
             [{"text": "bake bread in her oven for the café", "want": {"kind": "has", "who": "Giulia", "what": "bread"}}, {"text": "get charcoal from Ivo for the oven"}]),
    "Elena": ("helps her mother at the oven and draws the harbour; wants to learn the lighthouse from Ada and see the cities across the sea",
              [{"text": "win Pietro's heart", "want": {"kind": "affection", "with": "Pietro", "at_least": 0.8}}, {"text": "visit Ada at the lighthouse"}]),
    "Pietro": ("fishes with Nonno Beppe and plays the mandolin on the quay in the evenings; wants to give Elena something made for her, and has asked Lia in Boscoverde to carve it",
               [{"text": "give Elena a carved gift", "want": {"kind": "has", "who": "Elena", "what": "a carving"}}]),
    "Carla": ("picks lemons from the potted lemon trees and sells them with tomatoes and greens at her stall; buys Marta's honey to sell",
              [{"text": "get a jar of honey from Marta for the stall", "want": {"kind": "has", "who": "Carla", "what": "honey"}}]),
    "Sofia": ("helps her mother at the stall and feeds every cat in the village; wants a puppy, and has heard Bruno's dog Orso is a good dog",
              [{"text": "have a dog of her own"}]),
    "Franco": ("is harbourmaster and keeps the watch from the tower for storms and boats coming home; is mending the tower ladder and wants a new bell for the watch",
               [{"text": "get a bell hung in the watchtower"}]),
    "Ada": ("keeps the lighthouse on Capo Faro, trims and lights its lamp each dusk; wants someone young to learn the light from her",
            [{"text": "teach Elena to keep the light"}]),
    "Matteo": ("builds and paints the village's boats; needs good chestnut logs from Boscoverde to build Marco's new boat, and has asked Bruno for them",
               [{"text": "get chestnut logs from Bruno for Marco's boat", "want": {"kind": "has", "who": "Matteo", "what": "a log"}}]),
    "Bruno": ("fells old chestnuts with his axe, only the ones that are ready, splits and stacks them; promised Matteo his best logs for Marco's boat and Dario timber for the new woodshed",
              [{"text": "fell an old chestnut for Matteo's boat", "want": {"kind": "hold", "what": "a log"}}, {"text": "plant a new grove for his grandchildren"}]),
    "Marta": ("keeps the bees and the vegetable gardens and makes chestnut honey; sells honey to Carla in the village market",
              [{"text": "take honey down to Carla", "want": {"kind": "has", "who": "Carla", "what": "honey"}}]),
    "Ivo": ("burns charcoal in the kiln and sells it to Rosa for her oven; wants to prove he can fell a tree faster than Dario",
            [{"text": "fell a tree before Dario does"}, {"text": "bring charcoal to Rosa"}]),
    "Lia": ("looks after the horses and carves little animals from chestnut wood; Pietro asked her to carve something for Elena",
            [{"text": "carve a gift for Pietro to give Elena", "want": {"kind": "has", "who": "Pietro", "what": "a carving"}}, {"text": "ride all the way to the cape"}]),
    "Grazia": ("keeps the shrine at the great oak, lights its lanterns and ties the ribbons; wants every child to learn the old songs of the wood",
               [{"text": "teach Teo and Tomi the song of the wood"}]),
    "Dario": ("saws and splits logs and builds things of wood; promised Silvia a new woodshed beside their cabin before the rains, and needs logs and a partner to carry them",
              [{"text": "build Silvia a woodshed", "want": {"kind": "be", "place": "the woodshed"}}]),
    "Teo": ("gathers kindling and chestnuts; wants to find the spirit of the wood that Grazia tells of", [{"text": "find the spirit of the wood"}]),
    "Silvia": ("drives the wood and charcoal down to the village with the horses and brings back fish and bread; wants a second cart",
               [{"text": "trade charcoal to Lucia for fish"}, {"text": "get a second cart built"}]),
}

ITEMS = [("fishing rod", "Nonno Beppe"), ("fishing rod", "Tomi"), ("fishing rod", "Marco"), ("woodcutter's axe", "Ivo"), ("fishing net", "Nonno Beppe"), ("basket of fish", "Marco"), ("basket of fish", "Lucia"), ("leather ball", "Tomi"), ("basket of bread", "Rosa"),
         ("mandolin", "Pietro"), ("woodcutter's axe", "Bruno"), ("woodcutter's axe", "Dario"), ("leather ball", "Teo")]


def creatures(h):
    return [
        # In Marinella: cats everywhere, a dog or two.
        {"species": "cat", "count": 3, "x": -18.0, "z": -15.0, "names": ["Sardina", "Moro", "Bianca"], "owner": "", "description": "harbour cats begging at the fish crates"},
        {"species": "cat", "count": 3, "x": 6.0, "z": 50.0, "names": ["Pepe", "Tigre", "Nuvola"], "owner": "Sofia", "description": "the piazza's cats, fed by Sofia"},
        {"species": "cat", "count": 2, "x": -50.0, "z": 0.0, "names": ["Ombra", "Fiocco"], "owner": "", "description": "cats asleep on the warm steps"},
        {"species": "dog", "count": 1, "x": -2.0, "z": -20.0, "names": ["Capitano"], "owner": "Marco", "description": "a scruffy black-and-white boat dog"},
        {"species": "dog", "count": 1, "x": 82.0, "z": 96.0, "names": ["Vento"], "owner": "Franco", "description": "a lean grey hound who climbs the tower with Franco"},
        # In the bay: dolphins following the boats.
        {"species": "bottlenose dolphin", "count": 6, "x": 10.0, "z": -150.0, "names": [], "owner": "", "description": "a pod playing round the harbour mouth"},
        {"species": "bottlenose dolphin", "count": 4, "x": -170.0, "z": -130.0, "names": [], "owner": "", "description": "dolphins off the sandy cove"},
        # Boscoverde: the horses, the dog.
        {"species": "horse", "count": 3, "x": 154.0, "z": 492.0, "names": ["Castagna", "Fumo", "Stella"], "owner": "Silvia", "description": "the hamlet's sturdy chestnut work horses"},
        {"species": "dog", "count": 1, "x": 126.0, "z": 507.0, "names": ["Orso"], "owner": "Bruno", "description": "a huge shaggy brown dog"},
        # Wild: deer at the wood's edges, horses in the meadows.
        {"species": "deer", "count": 6, "x": 60.0, "z": 600.0, "names": [], "owner": "", "description": "red deer at the edge of the chestnut wood"},
        {"species": "deer", "count": 5, "x": 205.0, "z": 470.0, "names": [], "owner": "", "description": "deer grazing in the clearings below the wood"},
        {"species": "deer", "count": 5, "x": -40.0, "z": 430.0, "names": [], "owner": "", "description": "roe deer in the tall grass"},
        {"species": "horse", "count": 4, "x": -150.0, "z": 200.0, "names": [], "owner": "", "description": "half-wild horses roaming the flowering meadows"},
        {"species": "deer", "count": 4, "x": 160.0, "z": 120.0, "names": [], "owner": "", "description": "deer in the meadows east of the wall"},
    ]


def lay(p, cape_h, clearing):
    p.people += villagers(cape_h) + woodcutters(clearing)
    for c in p.people:
        if c["name"] in GOALS:
            c["goals"], c["aims"] = GOALS[c["name"]]
    homes = {c["name"]: (c["home_x"], c["home_z"]) for c in p.people}
    for ty, who in ITEMS:
        x, z = homes[who]
        p.item(ty, who, x, z)
    p.creatures += creatures(clearing)
