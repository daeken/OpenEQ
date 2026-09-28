# Offline zone atmosphere snapshot

`peq-zone-environment.txt` contains 620 zone/version records exported from the
public ProjectEQ world database dump dated **2026-09-26**, schema **9328**. The
dump was downloaded from `https://db.eqemu.dev/api/v1/dump/latest` while setting
up the server documented in `docs/SERVER_SETUP.md`. Content originates with
ProjectEQ; it is not extracted from or redistributed with proprietary game assets.

This is an offline-viewer fallback. Connected clients use the server's `NewZone`
packet as the authority, so custom server atmosphere is retained. Sky art is not
included: `Resources/sky/sky.ini`, `weather.ini`, and referenced DDS files are read
from the user's existing EverQuest installation.

Rows are whitespace separated. The file header lists columns. RGB values are
integer sRGB bytes; distances are EQ world units. Fog sets deliberately follow
EQEmu's `zone/zone.cpp` NewZone mapping: unsuffixed fields, then suffixes `2`, `3`,
and `4`. Suffix `1` exists in PEQ but is not sent as a separate fog set.

Re-export using this query, retaining the header and stable ordering:

```sql
SELECT short_name,version,zoneidnumber,
       fog_red,fog_green,fog_blue,fog_minclip,fog_maxclip,
       fog_red2,fog_green2,fog_blue2,fog_minclip2,fog_maxclip2,
       fog_red3,fog_green3,fog_blue3,fog_minclip3,fog_maxclip3,
       fog_red4,fog_green4,fog_blue4,fog_minclip4,fog_maxclip4,
       fog_density,minclip,maxclip,sky,ztype,time_type,castoutdoor,timezone
FROM zone WHERE short_name IS NOT NULL ORDER BY short_name,version;
```

Current sky rendering resolves a weather pattern and selects the color-map frame
for a supplied day fraction, then renders the first authored cloud layer. It does
not yet simulate transitions, every cloud population, celestial satellites, or
zone backdrop geometry. The day fraction can be supplied by a future server time
integration without changing how sky assets are loaded.
