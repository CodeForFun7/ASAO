# Testing the Debloat page

The Debloat page works the way [Win11Debloat](https://github.com/Raphire/Win11Debloat) does:

- **Apps:** what can be removed, how safe each one is, what is ticked by default and the presets all come from Win11Debloat's `Config/Apps.json`.
- **Windows settings:** these come from its `Config/Features.json` and the `.reg` files it points to.

ASAO adds its own layer on top:

- safety checks
- impact scores
- background services and startup entries
- a restore point
- Preview only
- undo

## 1. Build, run, and find the feature

```powershell
pnpm install
pnpm tauri dev --config '{\"build\":{\"beforeDevCommand\":\"pnpm dev\"}}'
```

Open **Debloat** in the sidebar, below Storage, and click **Scan for bloatware**. A scan takes
about 40 seconds:

1. ASAO reads apps, services, startup entries, scheduled tasks and the current Windows settings.
2. It measures background CPU and memory for 30 seconds.
3. It scores every item.

Scanning never changes anything.

| Tab | Shows |
| --- | --- |
| Recommended | Apps Win11Debloat rates safe to remove |
| All apps | Every app ASAO knows or that is visible in Settings, Apps, with **Quick select** buttons (Xbox gaming apps, PC maker apps) that tick a whole group from Win11Debloat's presets |
| Services | Apps with services, plus standalone services |
| Windows tweaks | Win11Debloat's settings, grouped by category, with their current state |
| Recent changes | Every real batch, with Undo per change |

Tick apps and settings, then click **Apply selected** (**Preview changes** while Preview only is on).

Automated tests:

```powershell
cd src-tauri
cargo test --lib bloat
```

## 2. What is offered and ticked

### Apps

| Win11Debloat rating | ASAO shows | Can be ticked |
| --- | --- | --- |
| safe | Safe to remove | Yes |
| optional | Usually safe | Yes |
| unsafe | Not recommended | No |

- **Ticked by default:** exactly what Win11Debloat's `SelectedByDefault` selects.
- **Matching:** a Store package is matched the way Win11Debloat matches it, when its name contains the app id. Its winget apps (Copilot, OneDrive, Edge) are matched to the installed app.
- **ASAO's checks still apply on top**, and can only make an app more cautious. An app that fails a check becomes Caution, which can't be ticked even if Win11Debloat selects it:
  - running now, or used in the last 30 days
  - another service depends on it
  - it is the active antivirus
  - the PC is managed by an organisation (for PC maker tools)
  - it is always kept: Store, App Installer, Windows Security, runtimes, drivers
- **Apps Win11Debloat doesn't list:** desktop PC maker software, trialware such as McAfee and Norton, Windows services and updaters come from ASAO's own list (`resources/bloatware-db.json`). They are never ticked by default.

### Windows tweaks

Each tweak imports one of Win11Debloat's `.reg` files.

**Not offered:**

- the Windows Update, System and Optional Windows Features categories
- tweaks Win11Debloat does with a script instead of a registry file (for example Disable widgets, Clear Start menu, Store search suggestions)
- tweaks for other Windows versions

The tab lists them under "Win11Debloat settings are not offered here", with the reason.

**Ticked by default:** the tweaks in Win11Debloat's `DefaultSettings.json` that are not in effect yet.

**State:** a tweak counts as "Already set" when every value in its registry file already matches. Pick-one settings (for example taskbar search style, Alt+Tab tabs, taskbar combining, the folder File Explorer opens to) show as a choice. The option in effect is marked "current".

**Labels:** "Admin" means the tweak changes a machine-wide value. "Restart needed" comes from Win11Debloat's `RequiresReboot`.

## 3. Preview only (dry run)

| Build | Preview only | Switch |
| --- | --- | --- |
| Development (`pnpm tauri dev`) | On by default | **Preview only** at the top right |
| Release | Off | Hidden |

To force it in a release build, use any one of:

- `"bloatDryRun": true` in `%APPDATA%\com.asao.app\settings.json`
- start ASAO with `--bloat-dry-run`
- set `ASAO_BLOAT_DRY_RUN=1`

Preview reads the current state of everything that would change, including the exact registry
values each tweak would replace. It writes one line per change to
`%APPDATA%\com.asao.app\bloat\logs\dry-run.jsonl`. Nothing changes, and no administrator prompt
appears.

## 4. Read-only first test

1. Scan. Check that the ticked apps match Win11Debloat's default selection, except apps you used this month, which show Caution with the reason.
2. Open **Windows tweaks**. Settings you already have (for example dark mode or showing file extensions) show "Already set". Compare a few with Windows Settings.
3. Check the Windows Update and System tweaks are not offered.
4. In **All apps**, filter by Safe to remove and Usually safe. None of these must appear:
   - drivers
   - graphics or audio control panels
   - antivirus, the Microsoft Store or App Installer
   - runtimes
5. Click **Preview changes**, then **Show preview**. The report says "Preview finished: nothing was changed", and the preview log has one line per change, with the registry values each tweak would replace.

To print every tweak and its state from the command line (read-only):

```powershell
cd src-tauri
cargo test --lib live_tweak_state -- --ignored --nocapture
```

## 5. Real test (virtual machine or spare PC)

### Apps

1. Install a free promoted game from the Store (for example Solitaire & Casual Games). Scan: it is ticked.
2. Turn Preview only off. Click **Apply selected**, then **Make changes**, and accept the administrator prompt.
3. The report shows the restore point and the uninstall. **Find their leftovers** opens Storage, Leftovers. In Recent changes, **Open in Store** opens its Store page.

### Tweaks

1. Choose a visible one, for example **Taskbar search style: Show search icon only**, or tick **Show file extensions** if it is not already set. Apply it.
2. Click **Restart File Explorer** in the report (open File Explorer windows close). The taskbar changes.
3. In **Recent changes**, click **Undo**. ASAO puts back the exact values it backed up. Restart File Explorer again, and the old setting is back.
4. Machine-wide tweaks (marked Admin) need the administrator prompt for both apply and undo.

### Services and startup entries

These work as before:

- **Services:** stopped and set to Manual or Disabled, never deleted. Undo restores the exact start type.
- **Startup entries:** switched off the same way Task Manager does it.

## 6. Restore point

Before any real change, ASAO asks Windows for a restore point named "ASAO: before removing
bloatware". Windows makes at most one a day: if one was made in the last 24 hours, the report
says that one is your fallback.

To check: press Win + R, run `rstrui`, and choose **Choose a different restore point**.

If System Protection is off, no restore point can be made. By default ASAO then stops before
changing anything. The confirm dialog has **continue anyway** for that case.

## 7. Logs

| File | What it holds |
| --- | --- |
| `%APPDATA%\com.asao.app\bloat\logs\actions.jsonl` | Real changes |
| `%APPDATA%\com.asao.app\bloat\logs\dry-run.jsonl` | Previews |
| `%APPDATA%\com.asao.app\bloat\history.json` | Undo records |

Each log line has these fields:

- `timestampMs`
- `batchId`
- `dryRun`
- `elevated`
- `item`
- `action`: for a tweak, `apply-tweak` with the feature id and registry file
- `status`
- `message`
- `original`: the backed-up state

Read it as a table:

```powershell
Get-Content "$env:APPDATA\com.asao.app\bloat\logs\actions.jsonl" | ConvertFrom-Json |
  Select-Object @{n='When';e={[DateTimeOffset]::FromUnixTimeMilliseconds($_.timestampMs).LocalDateTime}}, item, @{n='Action';e={$_.action.type}}, status, message |
  Format-Table -AutoSize
```

## 8. Updating the data

**Win11Debloat's files** are a reviewed copy in `src-tauri/resources/win11debloat/`. It contains
`Config/Apps.json`, `Config/Features.json`, `Config/DefaultSettings.json`,
`Config/Languages/en-US/Apps.json`, the `Regfiles/` folder and the MIT `LICENSE`.
`SOURCE.md` names the upstream commit. To update:

- **With a rebuild:** copy newer files from the Win11Debloat repository over these.
- **Without a rebuild:** put the same layout in `%APPDATA%\com.asao.app\bloat\win11debloat\`. Registry files missing there fall back to the bundled ones. If the copy is not valid, ASAO ignores it, uses the bundled copy, and notes it in the scan log.

ASAO never downloads these files. Registry files from the internet are never applied with
administrator rights.

**ASAO's own list** is `src-tauri/resources/bloatware-db.json`. Override it with
`%APPDATA%\com.asao.app\bloat\bloatware-db.json`. Its rules are the same as before:

- **Fields:** each entry needs `id`, `match`, `displayName`, `category`, `whatItDoes`, `safety`, `safetyReason`, `whatBreaks`, `howToRestore`, `recommendation` and `source`.
- **Safety and recommendation:** `safety` is one of `safe`, `usually-safe`, `caution` or `not-recommended`, and `recommendation` must fit it.
- **Conditions:** they can only make an entry more cautious.

Where an app is in both lists, Win11Debloat's rating wins.

## 9. Known limitations

- **Missing Win11Debloat features:** its script-only features are not available, nor is its removal for all users at once. ASAO removes Store apps for your account, with an optional "also for new users".
- **Tweak undo:** it restores the values ASAO backed up. It does not use Win11Debloat's undo files.
- **Policy settings:** a tweak whose values are overridden by a policy shows "Not confirmed" after applying.
- **Elevated helper:** it runs as the account that approves the prompt. Use an administrator account, so that your own settings and apps are the ones changed.
- **Coverage and measurement gaps:**
  - Store apps' own startup tasks and `\Microsoft\` scheduled tasks are not listed.
  - CPU of Windows services sharing `svchost.exe` is not measured on its own.
- **Desktop app uninstallers:** some only finish after a restart. They show "Not confirmed" until you scan again.
