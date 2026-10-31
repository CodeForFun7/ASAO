# Win11Debloat data (bundled copy)

These files are copied unchanged from Win11Debloat by Raphire,
https://github.com/Raphire/Win11Debloat, commit 5d88bfe9ef2be5a8a643166a6400eb78a30c46ec
(2026-10-31). They are used under the MIT licence in `LICENSE`.

- `Config/Apps.json`: removable apps, default selection, rating (safe, optional, unsafe), removal method, presets
- `Config/Features.json`: tweaks, each pointing to an apply and an undo file in `Regfiles/`
- `Config/DefaultSettings.json`: the tweaks Win11Debloat's default mode selects
- `Config/Languages/en-US/`: app names and descriptions
- `Regfiles/`: the registry files the tweaks import

To update, replace these files with a newer copy from the repository and rebuild, or put the
same layout in `%APPDATA%\com.asao.app\bloat\win11debloat\` to override without a rebuild.
