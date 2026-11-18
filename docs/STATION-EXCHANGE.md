# Moving saved stations between the Mac and Windows apps

Josh's ask (2026-11-17): import the stations saved in one Shortwave app into the other. Both apps
get **Import Stations…** and **Export Stations…**. Both read and write the same file. The Mac
implementation is the reference: `Sources/ShortwaveCore/Exchange.swift` (format, reading, merging)
and `ExchangeTests` in `Tests/ShortwaveTests/CoreTests.swift`. Translate both 1:1, as with the rest
of the core.

## The file

UTF-8 JSON, suggested name `Shortwave Stations.json`, pretty-printed with sorted keys:

```json
{
  "app" : "Shortwave for macOS 1.0 (nightly 09fd9a9)",
  "covers" : { "431DD15A-….png" : "<base64 PNG or JPEG>" },
  "exported" : "2026-11-18T02:46:33Z",
  "format" : "shortwave-stations",
  "stations" : [ { …station, exactly as in library.json… } ],
  "version" : 1
}
```

- **`stations`:** the whole library, sorted the way the library is sorted. Each station has the same
  keys and types as in `library.json`. Optional values are left out.
- **`covers`:** one entry for each `customCover` file name used by a station. The value is the image
  file's bytes in base64. Only PNG and JPEG are accepted (checked by their signatures).
- **`app`, `exported` and `format`:** informational. A reader doesn't require them.
- An exchange file is also a valid `library.json`, because it has `stations` and `version`.

## Reading

- Accept an exchange file, a plain `library.json`, or a bare JSON array of stations. Anything else
  is an error: "This file isn't a Shortwave station file." (not JSON) or "This file doesn't contain
  a list of stations." (JSON without a station list).
- Read stations leniently. `id`, `name` and `streamURL` must be strings, and `streamURL` must not be
  blank. Every other key falls back to its default when it's missing or the wrong type: numbers may
  arrive as strings, booleans as 0 or 1, and nulls are dropped from `tags`. Count the entries that
  fail as **unreadable**; don't fail the whole file over them. The Mac's `library.json` loader now
  uses the same rules.

## Importing (merge rules)

Import only adds stations. It never changes or removes anything already in the library.

1. Skip a station whose `id` is already in the library, and count it as **already present**. Database
   stations have the same radio-browser UUID on every machine, so this catches them.
2. Skip a local station (`isLocal`) when the library already has a local station with the same
   stream address (trimmed, case-insensitive). Count it as already present.
3. Apply rules 1 and 2 to stations added earlier from the same file too, so duplicates inside one file
   are skipped.
4. If a station has a `customCover` and the file carries that image, save the image to the covers
   folder under a new name (`<new UUID>.png`, or `.jpg` for JPEG) and point the station at it. If the
   image isn't in the file, clear `customCover` so the station falls back to its favicon.
5. Append the new stations, save the library, then refresh it from radio-browser, which also marks
   stations the database no longer knows as orphaned.
6. Tell the user what happened: "N stations were added to your library. M were already there. K
   couldn't be read." If nothing was added, title it "Nothing Imported". After a successful import,
   go to the Library page.

## Exporting

Export the library with its custom covers. Refuse with "Your library is empty." when there's nothing
to export. After writing, say how many stations were saved and where.

## Where it is in the apps

| | Mac | Windows |
|---|---|---|
| Menu | File > Import Stations… (⇧⌘I), File > Export Stations… (⇧⌘E) | Ctrl+Shift+I / Ctrl+Shift+E |
| Library page | "More" menu next to Sort: Import Stations…, Export Stations… | Same, in the page header |
| Empty library | "Import Stations…" button next to "Add Local Station" | Same |

## Test flags (test runs only)

| Flag | Effect |
|---|---|
| `-importStations <file>` | At launch, import the file into the test-run library. Log `imported added=N present=M unreadable=K covers=C` (or `import failed: <reason>`) |
| `-exportStations <file>` | After that, export the test-run library. Log `exported N stations to <file>` |
| `-quitAfter <seconds>` | Quit after that many seconds (the Windows app already had it) |

## Test fixtures (`fixtures/exchange/`)

| File | Written by | Holds |
|---|---|---|
| `seed.json` | Hand-written | Three stations: SomaFM Groove Salad (database station with tags and location), "Exchange Test Local" (local, with a custom cover), "Exchange Test Orphan" (orphaned, with a homepage) |
| `mac-export.json` | The Mac app: `scripts/exchange-test.sh --update-fixture` imports `seed.json` into an empty test-run library and exports it | The same three stations |
| `windows-export.json` | **The Windows app**, the same way: import `seed.json` into an empty test-run library, export, and commit the result | The same three stations |

Each app's unit tests read **both** exports. They expect three stations, no unreadable ones, and one
cover, with the field values from `seed.json`. Then they merge each export into an empty library.
The Mac's `readsWindowsExport` test is skipped until `windows-export.json` is committed.
`scripts/exchange-test.sh` (Mac) also imports the Windows export into the real app in a test run.
The Windows side needs the same checks.
