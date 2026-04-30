# Vienna TODO

## Completed

- Make updates preserve user data, mods, logs, and local databases.
- Move logs next to the launcher/run scripts and expose log controls in the launcher.

## In progress

## Gameplay and content

- Launcher buildplate preview: store preview in the launcher's DB instead of earthdb/object store.
- Launcher buildplate preview: add liquid rendering.
- Shop management.
- Player items management.
- Encounter generation and AR.
- Use tiles when spawning tappables: avoid water/forbidden areas and tune biome-based spawns.
- Allow setting maximum cache size for tiles.
- Add level reward buildplates and wire them into level ups.
- NFC mini figures.
- Find tokens for first time tutorial, daily login, and related flows.

## Tools and imports

- Custom Java resource pack conversion tool for Earth/Bedrock resource packs.
- Support importing Vienna data with warn-and-merge behavior when data already exists.
- Export buildplates in both formats.
- Launch/connect to remote components, for example running buildplate launcher on another PC.
- View the player buildplate template if it exists.

## Auth, profiles, and permissions

- Option to only allow custom login because Microsoft accounts cannot be verified here.
- Add auth for logs, possibly via random secret passed through CLI args and verified by the controller.
- Clear logs should remain a separate permission.
- Show roles on profile page.
- Associate a player profile with a user and allow permissions scoped to that associated player.

## UX and diagnostics

- View old logs in launcher.
- Investigate Windows slowness and add spinners for slower flows.

## Refactoring

- Get rid of LinkedList.
- Use Guid instead of string where IDs are expected.
- Load static data types only when needed.
