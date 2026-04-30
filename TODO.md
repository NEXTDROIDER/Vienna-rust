# Vienna TODO

## Completed

- Make updates preserve user data, mods, logs, and local databases.
- Move logs next to the launcher/run scripts and expose log controls in the launcher.
- Shop catalog API backed by loaded static data.
- Player items API backed by earth.db.
- Encounter/tappable generation API scaffold backed by active tiles.
- Maximum tile cache setting is exposed in portable settings and API server config.
- Support importing Vienna data with warn-and-merge behavior.
- Custom-login-only server option and auth config endpoint.
- Log listing, reading, clearing, and optional log-secret auth.
- Show/store roles through a player roles API.
- Level reward lookup API for reward buildplates/items.
- Windows portable launcher settings panel for slower/config-heavy flows.

## In progress

## Gameplay and content

- Launcher buildplate preview: store preview in the launcher's DB instead of earthdb/object store.
- Launcher buildplate preview: add liquid rendering.
- Full AR encounter flow beyond server-side encounter generation.
- Use real map/biome tile data when spawning tappables: avoid water/forbidden areas and tune biome-based spawns.
- Wire level reward buildplates into real level-up mutation flow.
- NFC mini figures.
- Find tokens for first time tutorial, daily login, and related flows.

## Tools and imports

- Custom Java resource pack conversion tool for Earth/Bedrock resource packs.
- Export buildplates in both formats.
- Launch/connect to remote components, for example running buildplate launcher on another PC.
- View the player buildplate template if it exists.

## Auth, profiles, and permissions

- Clear logs should remain a separate permission.
- Associate a player profile with a user and allow permissions scoped to that associated player.

## UX and diagnostics

- View old logs in launcher UI, not only through API/explorer.
- Investigate Windows slowness deeper and add more spinners/progress states.

## Refactoring

- Get rid of LinkedList.
- Use Guid instead of string where IDs are expected.
- Load static data types only when needed.
