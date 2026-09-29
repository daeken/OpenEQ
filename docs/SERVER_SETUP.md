# Storage2 EQEmu development server

Deployed 2026-09-28 on `storage2.daeken.dev` (`104.194.9.203`, Ubuntu 25.04).
Runtime: `/srv/eqemu`, running as the dedicated `eqemu` service user. Existing
Beyond, nginx, and file services remain operational.

## Connect

- RoF2/SoD+ login: `storage2.daeken.dev:5999` UDP.
- Titanium login: port `5998` UDP.
- World: port `9000` UDP; advertised name **OpenEQ Storage2**, server ID `1`.
- Zones: UDP `7000–7040`, assigned by world; use the address returned during zoning.
- Chat/mail: UDP `7778`.

The private connection JSON is `/Users/daeken/.config/openeq/storage2-credentials.json`
(mode `0600`). It contains the login password; do not commit or print it. The
`openeq` account has the `Explorer` character, a level 65 human warrior with
GM status 250 and invulnerability for client development. Explorer starts in
Plane of Knowledge, EQ coordinates `(-285, -148, -159)`. A short sword, backpack,
food, and water are seeded. Passwords are random, with login passwords hashed
using EQEmu's mode 14. Account auto-creation is disabled.

```sh
cargo run -p openeq -- --connect "$HOME/.config/openeq/storage2-credentials.json"
```

For a headless connection and NPC movement probe, close the graphical client and
run:

```sh
cargo run -p openeq-net --bin eqlogin -- \
  --config "$HOME/.config/openeq/storage2-credentials.json" --seconds 30
```

For the original RoF2 client, use `Host=storage2.daeken.dev:5999` in its
`eqhost.txt`. The database is bound to localhost only. A dedicated nftables
`inet eqemu` table rejects external access to EQEmu TCP control ports; it does
not change policies or ports for unrelated services. Zone WebSocket APIs bind
to loopback. No public EQEmulator login service is involved.

## Operate

```sh
ssh storage2.daeken.dev
sudo systemctl status eqemu.target eqemu-world eqemu-login
sudo systemctl restart eqemu.target
sudo systemctl stop eqemu.target
sudo journalctl -u eqemu-world -u eqemu-zone@poknowledge -f
sudo /srv/eqemu/private/backup.sh
```

`eqemu.target` is enabled at boot and owns login, world, chat/mail, static
`poknowledge` and `anguish`, six dynamic zone workers, and the control-port
network guard. Each server restarts after failure. Configuration and credentials
are under `/srv/eqemu`; binaries are in `bin/`, zone data in `maps/` and `quests/`,
and logs in `logs/`. MariaDB stores the `peq` database in `/var/lib/mysql/peq`.
Manual backups include the database and private configuration in
`/srv/eqemu/backups`. Treat backups as sensitive.

The dynamic pool was expanded from two to six on 2026-09-28 after natural border
travel exhausted the two occupied workers (Greater Faydark and Arena). The
additional `dynamic_03`–`dynamic_06` services are persistent Wants in
`/etc/systemd/system/eqemu.target.d/capacity.conf`; the original service template
and active zones were not restarted. Keep idle workers available: an empty
recently visited zone can occupy a process until its shutdown timer expires.

Zone services also restart after a successful exit: the override at
`/etc/systemd/system/eqemu-zone@.service.d/restart.conf` sets `Restart=always`.
Dynamic workers can exit normally when an empty zone shuts down; without this
override the spare worker pool eventually disappears and travel fails with
“zone is unavailable.” Explicit service stops still leave them stopped.

The reproducible provisioning, service-generation, test-account, and backup
scripts are saved in `/home/daeken/eqemu-bootstrap`. Do **not** re-import the
initial PEQ dump into a server containing player progress: its scripts recreate
tables. Take a backup before upgrades, apply EQEmu migrations, and restart the
target. `sudo -u eqemu` commands must run with `/srv/eqemu` as their current
directory.

Additional login accounts can be created with the loginserver CLI:

```sh
sudo -u eqemu sh -c 'cd /srv/eqemu && bin/loginserver login-user:create USERNAME PASSWORD'
```

Avoid putting real passwords in shell history; the existing account seeder reads
its secret from `/srv/eqemu/private/credentials.json` and redirects CLI output to
a private log. Original clients can create characters normally. New characters
start in Plane of Knowledge; the tutorial is disabled. Additional administrator
privileges can be assigned through EQEmu's account-status CLI or the database.

## Sources and content

- EQEmu source: commit `4aceae18b94ffaafc08e2b17bc41cd72c77f795d`.
- Build: CMake/Ninja Release, Perl and Lua enabled, loginserver/client-file tools
  enabled. Build tree: `/home/daeken/eqemu-bootstrap/build`; source tree:
  `/home/daeken/eqemu-bootstrap/source`.
- PEQ full dump: `https://db.eqemu.dev/api/v1/dump/latest`, dated 2026-09-26.
  The obsolete `db.projecteq.net` hostname does not resolve.
- Database schema `9328`, matching the binary; 221 imported tables, 67,530 NPC
  types, 165,711 spawn points, and 859,842 path-grid entries.
- Quests: `ProjectEQ/projecteqquests`, commit
  `2124cc0f069f05f5b90916162fb2ffa430a531bf`.
- Maps/navmeshes: `EQEmu/maps`, commit
  `e8efa8ed4f4ea4c434c03c40d2a758388a54f4d6`.
- Content expansion remains PEQ's Dragons of Norrath setting; content flags and
  seasonal restrictions are preserved.

Build command, after installing the dependencies recorded in the provisioning
logs:

```sh
cmake -S ~/eqemu-bootstrap/source -B ~/eqemu-bootstrap/build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DEQEMU_BUILD_LOGIN=ON \
  -DEQEMU_BUILD_LUA=ON -DEQEMU_BUILD_PERL=ON -DEQEMU_BUILD_CLIENT_FILES=ON
cmake --build ~/eqemu-bootstrap/build --parallel 8
```

The latest downloadable release was older than the checked-out source and linked
to an incompatible Perl ABI; the deployed binaries are built from source against
this host's libraries instead.

## Verified

- Login/world/zone UDP session handshakes succeeded from the development Mac,
  around 55 ms round-trip.
- Login credentials accepted; world registered as server `1`.
- OpenEQ completed login, character selection, world-to-zone handoff, and entered
  Plane of Knowledge with NPC spawns and server fog data. A 35-second headless
  probe received **322 entities, 321 NPCs, and 47 NPC position changes**. A
  final 40-second live GPU probe stayed connected, rendered **271 nearby NPCs**,
  and observed **48 position changes**. NPC motion continued between server
  packets on **1,757 actor frames**. Distance culling and unsupported models
  account for the smaller rendered count. The captured frame showed textured,
  animated classic NPCs, the original XML player/target/status windows, and the
  zone sky and fog.
- PoK boot loaded its collision/water/navmesh assets, 34 active movement grids
  with 1,139 waypoints, and 323 spawn groups/530 spawn entries. The unfiltered
  database has 535 PoK spawn points and 43 pathing spawn points.
- PoK and Anguish static zones, six dynamic workers, world, login, and chat/mail
  are active; the target is enabled on boot.

One upstream log message says `Loaded [0] spawn2 entries` on a fresh boot: it
mistakenly reports the respawn-timer vector's size instead of the spawn vector.
This does not mean the zone has no NPCs; live OpenEQ packets confirm spawns.
