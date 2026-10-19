"""Marinella's other families: the people who keep its shops, bars and
trades, each household living over its own door."""
from people import person

T = lambda s, p, c, b, g, cr: {"sociable": s, "playful": p, "curious": c, "brave": b, "generous": g, "crafty": cr}
M = lambda **k: dict({"shape": 0.0}, **k)
F = lambda **k: dict({"shape": 1.0}, **k)


def newcomers():
    P = lambda *a, **k: person(a[0], a[1], 0.0, 0.0, None, *a[2:], **k)
    return [
        # The osteria on the quay.
        P("Salvatore", 61, "the rooms over the Osteria del Porto", "a big, bald, laughing innkeeper with a grey moustache, braces over a white shirt, an apron, an accordion never far away",
          M(height=1.8, build=1.35, hair=0.05, hair_color=0.9, beard=0.3, top=1.2, top_shade=0.95, top_cover=0.8, sleeves=0.5, bottom=0.66, bottom_shade=0.2, legs=1.0, shoes=0.45, trim=0.0),
          "jovial, loud, sentimental, never lets a guest leave hungry", "keeps the Osteria del Porto: the house wine from the barrels, Concetta's cooking, music in the evening; wants to play at Giulia's summer dance",
          "booming and warm, sings when he can", ["Concetta: wife", "Gino: oldest friend", "Pietro: plays with him"], T(1.0, 0.8, 0.4, 0.6, 0.9, 0.4),
          aims=[{"text": "play the accordion at the summer dance in the piazza"}]),
        P("Concetta", 58, "the rooms over the Osteria del Porto", "a small, quick woman with grey hair in a bun, black dress and flowered apron, a wooden spoon in her pocket",
          F(height=1.55, build=1.1, hair=0.7, hair_style=0.8, hair_color=0.85, top=0.08, top_shade=0.1, top_cover=0.8, sleeves=0.5, skirt=0.8, bottom=0.08, bottom_shade=0.1, shoes=0.45),
          "fierce in her kitchen, tender out of it", "cooks the osteria's fish soup and pasta, buys from Lucia and Aldo every morning; wants her recipes written down before she forgets them",
          "sharp and fast", ["Salvatore: husband", "Lucia: buys her fish", "Aldo: buys his meat"], T(0.7, 0.4, 0.4, 0.7, 0.8, 0.6)),
        # The gelateria.
        P("Chiara", 29, "over the gelateria on the quay", "a sunny young woman with a dark ponytail, a pink-striped shirt and a white apron dusted with sugar",
          F(height=1.65, hair=0.8, hair_style=0.8, top=0.92, top_shade=0.8, top_cover=0.7, sleeves=0.2, skirt=0.5, bottom=1.2, bottom_shade=0.9, shoes=0.2),
          "cheerful, inventive, a little scatterbrained", "makes the gelato every morning from Carla's lemons, Marta's honey and the hamlet's chestnuts; wants to invent a flavour named after Marinella",
          "bright and quick", ["Davide: husband", "Leo: son", "Carla: buys her lemons", "Marta: buys her honey"], T(0.9, 0.8, 0.8, 0.5, 0.8, 0.8),
          aims=[{"text": "invent a new gelato flavour named for Marinella"}]),
        P("Davide", 31, "over the gelateria on the quay", "a lanky man in a short-sleeved blue shirt and sunglasses pushed up on his curls, always on his mint Vespa",
          M(height=1.82, hair=0.35, hair_style=0.5, top=0.6, top_shade=0.6, top_cover=0.8, sleeves=0.3, bottom=1.2, bottom_shade=0.8, legs=1.0, shoes=0.45),
          "easy-going, a show-off, kind", "delivers gelato, bread and the post's parcels round the village and up the road on his Vespa; wants to race Enzo's Vespa to the gate",
          "laid-back, joking", ["Chiara: wife", "Leo: son", "Enzo: rival on the road"], T(0.9, 0.9, 0.5, 0.8, 0.6, 0.5)),
        P("Leo", 11, "over the gelateria on the quay", "a wiry boy with a buzz cut, a red T-shirt, shorts, scabbed knees and a skateboard",
          M(height=1.42, build=0.8, hair=0.12, top=0.0, top_shade=0.6, top_cover=0.6, sleeves=0.2, bottom=0.66, bottom_shade=0.4, legs=0.3, shoes=0.45),
          "daring, restless, funny", "rides his skateboard down every ramp in the village and helps at the gelateria for a free cone; wants to ride down the whole Via del Ponente without falling",
          "fast, cheeky", ["Chiara: mother", "Davide: father", "Tomi: best friend", "Sofia: friend"], T(0.9, 1.0, 0.8, 1.0, 0.6, 0.6)),
        # The bar.
        P("Gino", 57, "over the Bar Tabacchi on the quay", "a dapper widower with slicked silver hair, a waistcoat and bow tie, a cloth over his shoulder",
          M(height=1.72, hair=0.3, hair_color=0.9, top=1.2, top_shade=0.95, top_cover=0.8, sleeves=1.0, bottom=0.08, bottom_shade=0.1, legs=1.0, shoes=0.45, trim=0.0),
          "polite, wry, a keeper of secrets", "runs the Bar Tabacchi: espresso at dawn for the fishermen, newspapers, panini and cards in the evening; wants his nephew Pietro to take over the bar one day",
          "dry and courteous", ["Pietro: nephew", "Giulia: daughter", "Salvatore: oldest friend", "Nino: plays cards with him"], T(0.8, 0.4, 0.5, 0.4, 0.7, 0.7)),
        # The pharmacy.
        P("Valeria", 46, "over the farmacia", "a calm woman with short grey-streaked hair, reading glasses on a chain and a crisp white coat",
          F(height=1.68, hair=0.3, hair_color=0.5, top=1.2, top_shade=0.97, top_cover=1.0, sleeves=1.0, skirt=0.5, bottom=0.66, bottom_shade=0.3, shoes=0.45),
          "careful, kind, sees everything", "keeps the pharmacy, mixes remedies and treats the village's cuts and coughs; wants Martina to study medicine in the city",
          "measured, gentle", ["Paolo: husband", "Martina: daughter", "Sara: friend"], T(0.6, 0.3, 0.7, 0.6, 0.8, 0.6)),
        P("Paolo", 48, "over the farmacia", "the postman: a tanned man in a grey-blue uniform and cap with a leather satchel, always on his black bicycle",
          M(height=1.76, hair=0.2, top=0.62, top_shade=0.5, top_cover=0.9, sleeves=0.5, bottom=0.62, bottom_shade=0.35, legs=1.0, shoes=0.7, hat=0.4),
          "chatty, punctual, knows every door", "brings the post round the village on his bicycle and up to Boscoverde once a week; wants to win the bicycle race at the feast of San Pietro",
          "chatty and warm", ["Valeria: wife", "Martina: daughter", "Davide: friend"], T(1.0, 0.6, 0.6, 0.5, 0.7, 0.4)),
        P("Martina", 15, "over the farmacia", "a tall, studious girl with long straight hair, glasses and a school satchel",
          F(height=1.66, hair=0.9, top=0.55, top_shade=0.7, top_cover=0.8, sleeves=0.5, skirt=0.5, bottom=0.66, bottom_shade=0.3, shoes=0.45),
          "shy, clever, dreamy", "studies, reads Ottavio's books and helps at the pharmacy; wants to see the stars through Franco's telescope",
          "soft-spoken", ["Valeria: mother", "Paolo: father", "Alice: best friend", "Ottavio: lends her books"], T(0.4, 0.4, 1.0, 0.4, 0.7, 0.6)),
        # The butcher.
        P("Aldo", 52, "over the macelleria", "a broad, red-faced butcher with forearms like hams, a striped apron and a pencil behind his ear",
          M(height=1.78, build=1.3, hair=0.1, beard=0.2, top=1.2, top_shade=0.95, top_cover=0.8, sleeves=0.3, bottom=0.62, bottom_shade=0.2, legs=1.0, shoes=0.7),
          "gruff, generous, a terrible singer", "cuts meat and makes salami and sausages; wants the hamlet to raise pigs for him",
          "gruff and loud", ["Teresa: wife", "Concetta: buys his meat", "Bruno: wants to trade with him"], T(0.6, 0.5, 0.3, 0.7, 0.8, 0.5)),
        P("Teresa", 49, "over the macelleria", "a neat woman in a blue dress and cardigan, her dark hair in a bob, keeping the shop's books",
          F(height=1.6, hair=0.4, top=0.62, top_shade=0.4, top_cover=0.8, sleeves=0.8, skirt=0.6, bottom=0.62, bottom_shade=0.4, shoes=0.45),
          "orderly, devout, warm with children", "keeps the butcher's accounts and sings in the church choir; wants a new altar cloth sewn by Mirella for the feast",
          "precise and polite", ["Aldo: husband", "Mirella: friend", "Don Anselmo: priest"], T(0.7, 0.3, 0.4, 0.4, 0.8, 0.4)),
        # The pizzeria.
        P("Luca", 36, "over the pizzeria", "a stocky pizzaiolo with flour up to his elbows, a white T-shirt, a red neckerchief and a paddle",
          M(height=1.74, build=1.15, hair=0.3, hair_style=0.5, beard=0.4, top=1.2, top_shade=0.97, top_cover=0.6, sleeves=0.2, bottom=0.62, bottom_shade=0.3, legs=1.0, shoes=0.45, trim=0.0),
          "proud, playful, fiery at the oven", "bakes pizza in the wood oven with Ivo's charcoal and Concetta's sauce recipe; wants the best pizza on the coast",
          "proud and teasing", ["Anna: wife", "Nico: son", "Gabri: daughter", "Ivo: buys his charcoal"], T(0.9, 0.8, 0.5, 0.7, 0.7, 0.8)),
        P("Anna", 34, "over the pizzeria", "a cheerful waitress with curly auburn hair tied up, a green blouse and a black skirt, a notepad",
          F(height=1.62, hair=0.75, hair_style=0.8, hair_color=0.6, top=0.33, top_shade=0.5, top_cover=0.7, sleeves=0.3, skirt=0.5, bottom=0.08, bottom_shade=0.1, shoes=0.45),
          "warm, organised, the calm next to Luca", "serves the tables and raises the twins; wants a free evening to dance at the summer dance",
          "kind and brisk", ["Luca: husband", "Nico: son", "Gabri: daughter"], T(0.9, 0.6, 0.5, 0.5, 0.8, 0.6)),
        P("Nico", 7, "over the pizzeria", "a little boy with a pudding-bowl haircut and a striped T-shirt, a toy boat in his hand",
          M(height=1.2, build=0.8, hair=0.3, top=0.6, top_shade=0.6, top_cover=0.6, sleeves=0.2, bottom=0.62, bottom_shade=0.4, legs=0.3, shoes=0.45),
          "curious, follows the big boys", "plays with his twin Gabri and wants to go out on a fishing boat", "eager", ["Gabri: twin sister", "Luca: father", "Anna: mother", "Tomi: hero"], T(0.8, 1.0, 1.0, 0.6, 0.6, 0.5)),
        P("Gabri", 7, "over the pizzeria", "a little girl with two bunches of curls, a yellow dress and sandals",
          F(height=1.2, build=0.8, hair=0.5, hair_style=0.5, top=0.15, top_shade=0.8, top_cover=0.8, sleeves=0.1, skirt=0.4, bottom=0.15, bottom_shade=0.8, shoes=0.2),
          "bossy, funny, loves cats", "plays with her twin Nico and feeds the cats with Sofia; wants a kitten", "loud and sure", ["Nico: twin brother", "Sofia: idol", "Luca: father", "Anna: mother"], T(0.9, 1.0, 0.8, 0.7, 0.7, 0.5)),
        # The barber.
        P("Nino", 70, "over the barber's shop", "a dapper little old barber with a white moustache, a short white jacket, scissors in his top pocket",
          M(height=1.62, build=0.9, hair=0.2, hair_color=1.0, beard=0.2, top=1.2, top_shade=0.97, top_cover=0.8, sleeves=0.8, bottom=0.08, bottom_shade=0.2, legs=1.0, shoes=0.45),
          "the village's great gossip, sentimental", "shaves the fishermen and cuts the children's hair, and knows everyone's news first; wants to see Elena and Pietro married",
          "a whispering confidant", ["Gino: plays cards with him", "Pietro: customer", "Salvatore: old friend"], T(1.0, 0.6, 0.9, 0.3, 0.7, 0.6)),
        # The vet.
        P("Sara", 38, "over the vet's surgery", "a brisk woman with a long red braid, rolled shirt sleeves, green trousers and boots, a bag of instruments",
          F(height=1.7, hair=0.85, hair_style=0.8, hair_color=0.6, top=0.33, top_shade=0.4, top_cover=0.8, sleeves=0.4, bottom=0.33, bottom_shade=0.3, legs=1.0, shoes=0.8),
          "practical, fearless with animals, impatient with people", "tends the village's cats and dogs and rides up to Boscoverde on her bicycle for the horses; wants to find homes for the stray cats",
          "direct and dry", ["Valeria: friend", "Silvia: looks after her horses", "Sofia: helps her with the cats"], T(0.5, 0.4, 0.8, 0.9, 0.8, 0.6),
          aims=[{"text": "find homes for the harbour cats"}]),
        # The bookshop.
        P("Ottavio", 73, "over the bookshop", "a tall, stooped retired professor in a linen suit and a panama hat, a book under his arm",
          M(height=1.8, build=0.85, hair=0.3, hair_color=1.0, beard=0.5, top=1.2, top_shade=0.8, top_cover=1.0, sleeves=1.0, bottom=1.2, bottom_shade=0.8, legs=1.0, shoes=0.45, hat=0.6),
          "learned, absent-minded, gentle", "sells books and newspapers and writes a history of Marinella; wants to find the old pirate wall's foundation stone",
          "slow and wordy", ["Ines: wife", "Martina: lends her books", "Don Anselmo: chess partner"], T(0.6, 0.4, 1.0, 0.4, 0.8, 0.5),
          aims=[{"text": "finish his history of Marinella"}]),
        P("Ines", 71, "over the bookshop", "a bright-eyed old woman with a white bun, a lilac cardigan and a watering can",
          F(height=1.56, hair=0.7, hair_style=0.8, hair_color=1.0, top=0.8, top_shade=0.7, top_cover=0.9, sleeves=1.0, skirt=0.8, bottom=0.8, bottom_shade=0.5, shoes=0.45),
          "cheerful, nosy, green-fingered", "keeps the geraniums of half the village and minds the bookshop when Ottavio forgets; wants the prettiest balcony at the feast",
          "lively and teasing", ["Ottavio: husband", "Rita: buys her flowers", "Nino: gossips with him"], T(0.9, 0.6, 0.8, 0.4, 0.8, 0.5)),
        # The tailor.
        P("Mirella", 41, "over the sartoria", "an elegant seamstress with dark hair pinned up, a tape measure round her neck, a fitted purple dress",
          F(height=1.66, hair=0.75, hair_style=0.8, top=0.8, top_shade=0.4, top_cover=0.8, sleeves=0.5, neckline=0.5, skirt=0.6, bottom=0.8, bottom_shade=0.4, shoes=0.45),
          "stylish, exacting, generous with her time", "makes and mends the village's clothes, the hotel's curtains and the church's cloths; wants to dress Elena for the summer dance",
          "precise and amused", ["Alice: daughter", "Teresa: friend", "Elena: making her a dress"], T(0.7, 0.4, 0.6, 0.5, 0.8, 0.9)),
        P("Alice", 17, "over the sartoria", "a lively girl with a short dark bob, a striped top and a long skirt she made herself",
          F(height=1.64, hair=0.35, top=0.66, top_shade=0.9, top_cover=0.7, sleeves=0.5, skirt=0.9, bottom=0.08, bottom_shade=0.3, shoes=0.2),
          "bold, artistic, impatient with the village", "helps her mother sew and sketches dresses; wants to go to the city to study fashion, and to get Davide's Vespa for a ride",
          "quick and witty", ["Mirella: mother", "Martina: best friend", "Elena: admires her"], T(0.9, 0.8, 0.8, 0.7, 0.6, 0.8)),
        # The hotel.
        P("Paola", 56, "the owner's flat in the Albergo Marinella", "a handsome woman with silver-streaked hair swept up, pearls, a navy dress and a ring of keys",
          F(height=1.7, hair=0.7, hair_style=0.8, hair_color=0.7, top=0.66, top_shade=0.2, top_cover=0.8, sleeves=0.5, skirt=0.6, bottom=0.66, bottom_shade=0.2, shoes=0.45),
          "gracious, shrewd, proud of her hotel", "runs the Albergo Marinella and gives every traveler the room with the sea view; wants the hotel full for the feast of San Pietro",
          "gracious and smooth", ["Enzo: husband", "Giulia: sends her guests to the café", "Rita: flowers for the rooms"], T(0.9, 0.4, 0.6, 0.5, 0.7, 0.8),
          aims=[{"text": "make the traveler welcome at the hotel"}]),
        P("Enzo", 59, "the owner's flat in the Albergo Marinella", "a porter with a bushy black moustache, a maroon waistcoat with brass buttons, who rides a red Vespa",
          M(height=1.77, build=1.1, hair=0.25, beard=0.3, top=0.0, top_shade=0.3, top_cover=0.8, sleeves=1.0, bottom=0.08, bottom_shade=0.1, legs=1.0, shoes=0.45, trim=0.15),
          "proud, competitive, a romantic", "carries the guests' bags, fixes everything in the hotel and fetches guests from the coast road on his Vespa; wants to beat Davide in a race to the gate",
          "theatrical", ["Paola: wife", "Davide: rival on the road"], T(0.8, 0.8, 0.4, 0.8, 0.6, 0.6)),
        # The florist.
        P("Rita", 26, "over the flower shop", "a freckled young woman with wild curly hair under a straw hat, a floral dress and a basket of flowers on her bicycle",
          F(height=1.64, hair=0.85, hair_style=0.5, hair_color=0.6, top=0.92, top_shade=0.7, top_cover=0.8, sleeves=0.1, skirt=0.6, bottom=0.33, bottom_shade=0.6, shoes=0.2, hat=0.8),
          "romantic, impulsive, kind", "grows and sells flowers, decorates the church and the hotel, and gathers wildflowers in the meadows; wants to fill the piazza with flowers for the dance",
          "breathless and warm", ["Elena: best friend", "Paola: buys flowers", "Ines: customer"], T(0.9, 0.8, 0.7, 0.5, 0.9, 0.6)),
        # The school.
        P("Lidia", 51, "over the school", "the schoolteacher: a slim woman in a grey skirt and white blouse, spectacles, chalk on her fingers",
          F(height=1.65, hair=0.7, hair_style=0.8, hair_color=0.4, top=1.2, top_shade=0.95, top_cover=0.9, sleeves=1.0, skirt=0.7, bottom=1.2, bottom_shade=0.4, shoes=0.45),
          "strict, fair, secretly hilarious", "teaches the village's children their letters, sums and the history of the coast; wants every child to read by the summer",
          "clear and firm", ["Tomi: pupil", "Sofia: pupil", "Leo: pupil", "Ottavio: lends books"], T(0.6, 0.5, 0.8, 0.6, 0.8, 0.6)),
        # The boatyard.
        P("Rina", 29, "over the boatyard", "a sturdy young woman in paint-spattered overalls with a red scarf over her hair, a paintbrush behind her ear",
          F(height=1.66, build=1.05, hair=0.6, hair_style=0.8, top=0.62, top_shade=0.6, top_cover=1.0, sleeves=0.3, bottom=0.62, bottom_shade=0.6, legs=1.0, shoes=0.7, hat=0.2),
          "patient, funny, a fine painter", "paints the eyes and names on the boats Matteo builds; wants to paint a mural on the harbour wall",
          "easy-going", ["Matteo: husband", "Elena: paints with her"], T(0.7, 0.8, 0.7, 0.6, 0.7, 0.9)),
        # The priest's house.
        P("Assunta", 72, "the priest's house", "a tiny, fierce housekeeper in black with a starched apron and a broom",
          F(height=1.5, build=0.9, hair=0.7, hair_style=0.8, hair_color=1.0, top=0.08, top_shade=0.05, top_cover=1.0, sleeves=1.0, skirt=1.0, bottom=0.08, bottom_shade=0.05, shoes=0.45, hat=0.2),
          "devout, bossy, a wonderful cook", "keeps Don Anselmo's house and the church spotless and feeds him far too well; wants the bell tower clock to chime again",
          "scolding and fond", ["Don Anselmo: looks after him", "Teresa: choir"], T(0.6, 0.3, 0.5, 0.7, 0.8, 0.5)),
    ]


# Who lives where: building key -> names (the first keeps the business).
HOUSEHOLDS = {
    "cantiere": ["Matteo", "Rina"],
    "pescheria": ["Lucia", "Marco", "Tomi", "Nonno Beppe"],
    "osteria": ["Salvatore", "Concetta"],
    "gelateria": ["Chiara", "Davide", "Leo"],
    "capitaneria": ["Franco"],
    "bar": ["Gino", "Pietro"],
    "farmacia": ["Valeria", "Paolo", "Martina"],
    "macelleria": ["Aldo", "Teresa"],
    "alimentari": ["Carla", "Sofia"],
    "pizzeria": ["Luca", "Anna", "Nico", "Gabri"],
    "barbiere": ["Nino"],
    "veterinario": ["Sara"],
    "libreria": ["Ottavio", "Ines"],
    "panetteria": ["Rosa", "Elena"],
    "sartoria": ["Mirella", "Alice"],
    "albergo": ["Paola", "Enzo"],
    "caffe": ["Giulia"],
    "fioraio": ["Rita"],
    "scuola": ["Lidia"],
    "canonica": ["Don Anselmo", "Assunta"],
}
CHILDREN = {"Tomi", "Sofia", "Leo", "Martina", "Nico", "Gabri", "Alice"}
COUPLES = {"cantiere", "pescheria", "osteria", "gelateria", "farmacia", "macelleria", "pizzeria", "libreria", "albergo"}
# Something of a person's own in their home: (type, floor).
HOBBY = {
    "cantiere": ("easel", 1), "capitaneria": ("telescope", 2), "osteria": ("upright piano", 1), "libreria": ("desk", 1),
    "sartoria": ("sewing table", 1), "panetteria": ("easel", 2), "farmacia": ("bookshelf", 1), "barbiere": ("armchair", 1),
    "veterinario": ("desk", 1), "canonica": ("bookshelf", 2), "bar": ("upright piano", 1), "fioraio": ("easel", 1),
}
ITEMS = [("pizza", "Luca"), ("panino", "Gino"), ("gelato", "Chiara"), ("salami", "Aldo"), ("bottle of medicine", "Valeria"), ("newspaper", "Ottavio"),
         ("book", "Lidia"), ("bunch of flowers", "Rita"), ("broom", "Concetta"), ("broom", "Assunta"), ("watering can", "Ines"), ("accordion", "Salvatore"),
         ("cup of espresso", "Giulia"), ("bucket", "Nino"), ("bottle of wine", "Salvatore"), ("book", "Martina")]
# Things to ride, by their owner.
RIDES = [("vespa", "Davide"), ("vespa", "Enzo"), ("bicycle", "Paolo"), ("bicycle", "Sara"), ("bicycle", "Rita"), ("skateboard", "Leo")]
