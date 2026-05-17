# Theme system; type-less ships no third-party intellectual property

The Speechcraft Level-Up Overlay is rendered through a Theme system — a folder containing a `theme.json` manifest plus image, sound, and font assets. type-less ships exactly one CC0-licensed, original-art default Theme. All other Themes — including the Skyrim-styled one that motivated the product — are installed by the user via folder drop into the Themes directory; type-less itself never hosts, bundles, or links to third-party-owned assets.

The product is heavily inspired by Skyrim's speechcraft level-up, but Bethesda/ZeniMax owns the specific audio chime, parchment frame, and visual treatment. Shipping those assets — or even close "homage" recreations — would invite DMCA action and reputationally tag the project as the open-source tool that got taken down. Decoupling expression from mechanic also expands the surface: any user-made theme (Zelda, Halo, FFVII, Mario power-up) works through the same code path with zero engineering cost. A separate community tutorial ("Skyrim your local dictation") will walk users through installing their own assets if they own the game.

## Considered alternatives

- **Ship close "homage" Skyrim assets** — rejected: legally grey, trade-dress exposure remains, and a takedown against an open-source project carries reputational damage even when ultimately defensible.
- **Ship no overlay at all and let users build it themselves** — rejected: the overlay is the differentiator; first-run users must see *something* delightful out of the box, hence the CC0 default Theme.
