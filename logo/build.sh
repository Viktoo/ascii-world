#!/bin/sh
# Builds logo/scene.pocket: Maple Cove (worlds/starter) with the logo's own
# types swapped in and the cast moved into place. Run from the repo root.
set -e
cd "$(dirname "$0")/.."
OUT=logo/scene.pocket
gunzip -c worlds/starter/starter.pocket.gz > $OUT

sqlite3 $OUT <<'SQL'
-- Detailed maples and petalled flowers in place of the starter's own.
-- A type's stored meta keeps its probed bottom and top; the flowers' are set
-- by hand (the probe misses their thin stems and would sink them 0.8 m).
UPDATE types SET code = CAST(readfile('logo/types/sugar-maple.js') AS TEXT) WHERE name = 'sugar maple';
UPDATE types SET code = CAST(readfile('logo/types/autumn-wildflowers.js') AS TEXT), meta_json = '{"bottom":0.03,"top":1.25}' WHERE name = 'autumn wildflowers';

-- The title and the farmer's body ('{}' meta: probed on load).
INSERT INTO types(version_id, name, code, meta_json, status, errors)
  VALUES ((SELECT max(id) FROM versions), 'infinite pocket letters', CAST(readfile('logo/types/letters.js') AS TEXT), '{}', 'ok', '');
INSERT INTO types(version_id, name, code, meta_json, status, errors)
  VALUES ((SELECT max(id) FROM versions), 'farmer figure', CAST(readfile('logo/types/farmer-figure.js') AS TEXT), '{}', 'ok', '');

-- The letters stand in the meadow east of the village, face to the west.
INSERT INTO instances(type_id, version_id, chunk_x, chunk_z, x, y, z, rot_y, scale, params_json)
  VALUES ((SELECT id FROM types WHERE name = 'infinite pocket letters'), (SELECT max(id) FROM versions),
          2, 0, 167.0, 8.3, 0.0, -1.5708, 1.0, '[0.5,0.5,0.5,0.5,0.5,0.5,0.5,0.5]');

-- Owen Fairbanks, the orchardist, in the farmer body beside the T.
UPDATE characters SET
  persona_json = json_set(persona_json, '$.body', 'farmer figure', '$.look.height', 1.78),
  state_json = json_set(state_json, '$.x', 165.6, '$.z', -6.2, '$.y', 8.4, '$.yaw', -1.2)
  WHERE json_extract(persona_json, '$.name') = 'Owen Fairbanks';

-- Pumpkin the cat, walking left to right under the CK.
UPDATE characters SET
  state_json = json_set(state_json, '$.x', 163.5, '$.z', -0.45, '$.y', 8.4, '$.yaw', 3.1416)
  WHERE json_extract(persona_json, '$.name') = 'Pumpkin';
SQL
echo "wrote $OUT"
