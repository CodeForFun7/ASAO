# Testing residual file cleanup (Storage → Leftovers)

This guide walks through testing the leftover cleanup feature by hand on Windows. Start with
dry run on, which is the default, and only turn it off for the tests that ask you to.

## 1. Build, run, and find the feature

```powershell
pnpm install
pnpm tauri dev
```

`pnpm tauri dev` also starts the optional Nyra Python service. If Python or its packages are
missing, the launcher stops. Either install them (see the README) or run the app without Nyra:

```powershell
pnpm tauri dev --config '{\"build\":{\"beforeDevCommand\":\"pnpm dev\"}}'
```

Where to find it:

- **Storage, Leftovers tab.** This is the review screen: scan, review, tick, remove.
- **Storage, REC button.** A "Leftovers from uninstalled apps" card shows the total. Its "Review Leftovers" button opens the tab.
- **Desktop widget.** The same card rotates in the widget. Clicking it opens the tab.
- **Settings, Leftover Cleanup.** This holds the age threshold, the ignored items and the log locations.

Automated tests (30 tests, all against fake folders in a temp directory):

```powershell
cd src-tauri
cargo test --lib
```

A read-only scan of this machine that prints what would be flagged, for review:

```powershell
cargo test --lib live_scan_smoke -- --ignored --nocapture
```

## 2. Dry run, the age override, and the dry-run log

- **Dry run** is the toggle at the top right of the Leftovers tab. It is on every time ASAO starts. With it on, the whole flow runs, including the confirm dialog, every safety check and the report. Nothing is deleted and no administrator prompt appears.
- **"Modified in the last 30 days" rule:** Settings, Leftover Cleanup, Skip recently used folders. Choices are 7, 14, 30, 60 or 90 days.
- **Developer override (0 days):** the same row shows "0 days (dev)" in debug builds (`pnpm tauri dev`), or when ASAO runs with `ASAO_DEV=1`. With 0, folders changed today can be flagged too.
- **Dry-run log:** `%APPDATA%\com.asao.app\residual\logs\dry-run.jsonl`, one JSON line per folder.

## 3. Safe first test: dry-run scan with nothing uninstalled

1. Launch ASAO and open Storage, Leftovers. Leave dry run on.
2. Click **Scan for leftovers**.
3. Check the following:
   - No folder of an app you have installed appears as High or Medium. Expand every group to check.
   - On a first run, nothing is High: ASAO has no record yet of apps being uninstalled. The info banner says so.
   - Medium items are Program Files folders with no uninstall entry at all. They start unticked.
   - Everything else is under the collapsed **Uncertain items** section, unticked.
4. Tick one item, click **Dry run selected**, then **Run dry run**. The report should say "Dry run finished", the folder must still exist, and the dry-run log gains a `would-delete` line.

If an installed app's folder shows up, note its path and the app's name in Settings, Apps. That is a bug in the ownership checks and should be reported.

## 4. Real uninstall test (Notepad++ or VLC)

ASAO keeps a running record of installed apps: first-seen, last-seen and removed-on dates. It is
updated quietly a few seconds after every launch and on every scan.

1. Install Notepad++ (or VLC). Run it once so it creates its settings, then close it.
2. Launch ASAO (or click **Scan again**). The record now includes the app.
3. Uninstall the app from Windows Settings, Apps.
4. In ASAO, open Settings, Leftover Cleanup, and choose **0 days (dev)**. You just ran the app, so its settings are newer than 30 days and would otherwise be skipped on purpose.
5. Open Storage, Leftovers and click **Scan again**.
6. Check the following:
   - A group named after the app, for example "Notepad++", shows its folders as High, pre-ticked. Typical paths are `%APPDATA%\Notepad++` and sometimes `C:\Program Files\Notepad++`.
   - Each row lists its reasons, for example "Notepad++ was uninstalled (+8) · Name matches Notepad++ (+6)".
7. Turn **dry run off**, click **Remove selected**, read the dialog, and keep **Move to Recycle Bin**.
8. Program Files items trigger a Windows administrator prompt. AppData items do not.
9. Check the following:
   - The report says "Leftovers removed" and lists each folder.
   - **Open Recycle Bin** shows the folders. Right-click one and choose **Restore** to put it back.
10. Turn dry run back on and set the age back to 30 days.

## 5. Fake leftover test

1. In Settings, Leftover Cleanup, choose **0 days (dev)**.
2. Create a test folder:

   ```powershell
   New-Item -ItemType Directory "$env:APPDATA\AsaoTestLeftover" | Out-Null
   1..3 | ForEach-Object { Set-Content "$env:APPDATA\AsaoTestLeftover\file$_.dat" ("x" * 4096) }
   ```

3. Scan again. `AsaoTestLeftover` must appear under **Uncertain items**, in the **Unknown leftovers** group, labelled Uncertain and **unticked**. No uninstalled app matches it.
4. Optional: click the eye icon on the row ("Don't show this folder again"). It disappears, and Settings, Leftover Cleanup, Ignored items lists it. Remove it there to see it again.
5. Clean up with `Remove-Item -Recurse "$env:APPDATA\AsaoTestLeftover"` and set the age back to 30 days.

Without the 0-day override the new folder is skipped. That is the "modified in the last 30 days" rule working.

## 6. Elevation test

1. Open PowerShell **as administrator** and create a Program Files test folder:

   ```powershell
   New-Item -ItemType Directory "C:\Program Files\AsaoElevationTest" | Out-Null
   Set-Content "C:\Program Files\AsaoElevationTest\readme.txt" "test"
   ```

2. In ASAO (running normally, not as administrator), choose **0 days (dev)** and scan. The folder appears as Medium, unticked, with a shield icon ("Needs administrator permission").
3. Tick it, turn dry run off, and click **Remove selected**. The dialog states that Windows will ask for administrator permission for it only.
4. Confirm. The Windows permission prompt (UAC) appears. Click **No**. The report lists the folder as skipped with "Administrator permission was declined", and the folder is still there.
5. Repeat with an AppData folder, for example the fake leftover from step 5. No permission prompt appears.
6. Clean up from the administrator PowerShell: `Remove-Item -Recurse "C:\Program Files\AsaoElevationTest"`. Turn dry run back on and set the age back to 30 days.

If you click **Yes** instead, a second, hidden copy of ASAO runs as administrator. It re-checks the folder and moves it to the Recycle Bin.

## 7. The deletion log

Real removals are logged in `%APPDATA%\com.asao.app\residual\logs\deletions.jsonl`, one JSON object per line:

| Field | Meaning |
| --- | --- |
| `timestampMs` | When it happened (Unix milliseconds) |
| `action` | `deleted`, `skipped` or `failed` (`would-delete` in the dry-run log) |
| `path` | The folder |
| `sizeBytes` | Size on disk at scan time |
| `category` | `install-remnant`, `cache`, `logs` or `app-data` |
| `recycled` | `true` when it went to the Recycle Bin |
| `reason` | Why it was skipped or failed |
| `elevated` | `true` when the administrator helper handled it |

Read it as a table:

```powershell
Get-Content "$env:APPDATA\com.asao.app\residual\logs\deletions.jsonl" | ConvertFrom-Json |
  Select-Object @{n='When';e={[DateTimeOffset]::FromUnixTimeMilliseconds($_.timestampMs).LocalDateTime}}, action, path, sizeBytes, recycled, reason |
  Format-Table -AutoSize
```

Other files in `%APPDATA%\com.asao.app\residual\`: `app_history.json` (the installed-app record),
`ignore_list.json`, and `logs\scan.log` (folders that could not be read, and history notes).

## 8. Known limitations of this version

- **Not covered yet:** game saves in Documents, `Documents\My Games`, `Saved Games`, Steam `userdata`, and leftover registry entries.
- **No history before ASAO:** apps uninstalled before ASAO first ran are never "recently uninstalled". Their folders can only appear as Medium (Program Files) or Uncertain.
- **Microsoft folders:** folders named "Microsoft…" or "Windows…", and folders holding Microsoft-published `.exe` or `.sys` files, are never flagged. Windows components often have no uninstall entry. Leftovers of Microsoft apps are therefore not found.
- **Vendor folders:** inside a folder whose publisher still has an installed app (for example `AppData\Roaming\Adobe`), only sub-folders that match an uninstalled app are shown.
- **Very large folders:** folders with more than 250,000 entries are skipped. They cannot be checked for recent files safely.
- **Locked folders:** a running scan without administrator rights cannot read a few ProgramData folders. They are skipped and listed in `scan.log`.
- **Recycle Bin size limit:** if a folder is too large for the Recycle Bin, Windows asks before deleting it permanently.
- **Scheduled task and service check:** if PowerShell cannot list services and scheduled tasks, scans still run but removal is refused until it works.
- **Background scan:** a background scan runs once per launch, about 50 seconds after start. It only feeds the Storage suggestion and never deletes anything.
