# Testing Unused apps (Storage → Unused apps)

The Unused apps tab lists apps you have not used for a while and offers the right way to remove
each one. ASAO never uninstalls anything without your click: it opens the app's own uninstaller,
asks Windows to do it, or (for apps Windows never registered) moves the program folder to the
Recycle Bin.

## How "last used" is found

| Source | What it covers | Notes |
| --- | --- | --- |
| UserAssist | Programs and Store apps launched from Explorer, Start or the taskbar | Readable without admin rights. Misses programs started by other programs or at login. |
| Windows Prefetch | Every program start | Only read when ASAO runs as administrator. |
| ASAO's own monitor | Every program ASAO sees running (checked every 30 seconds) | Starts empty on the first launch and improves over time. Saved to `%APPDATA%\com.asao.app\residual\app_usage.json`. |

The newest date from any source wins. Apps running right now are not listed. Apps with no
record from any source go under a collapsed **No record of use** section.

Left out on purpose:
- runtimes, drivers, SDKs, redistributables, plugins and services
- hardware vendor tools (NVIDIA, Intel, AMD, Realtek, Dolby and similar)
- Microsoft components, except Microsoft apps people open (VS Code, Office, OneDrive, Teams, Visual Studio)
- command-line tools: any app with a folder on PATH
- Store frameworks and Store apps that cannot be removed

## How each app is removed

| App | Button | What happens |
| --- | --- | --- |
| Registered, uninstaller present | Uninstall | The app's own uninstaller opens. ASAO watches Windows' list for up to 20 minutes and reports when the app is gone. |
| Registered with Windows Installer | Uninstall | `msiexec /x {product code}` runs, which also works when the uninstall entry is broken. |
| Registered, uninstaller missing | Open Windows Settings, plus Remove program folder | Settings opens on Apps. If that cannot remove it either, ASAO can move its program folder to the Recycle Bin. |
| Microsoft Store app | Remove | The package is removed for your account (`Remove-AppxPackage`). No admin prompt. |
| Never registered (portable app, copied game) | Remove program folder | The folder and your Start menu and desktop shortcuts to it go to the Recycle Bin. |
| Steam game | Uninstall in Steam | Steam's own uninstall opens (`steam://uninstall/<id>`). |
| Epic game | Open Epic Games | The launcher opens. Uninstall it from your library there. |

Before removing a program folder, ASAO checks that nothing in it is running, that no service,
scheduled task or startup entry starts it, and that no program has files open in it. A folder
that needs administrator rights is not removed. Open it in Explorer to remove it yourself.

After any removal, ASAO records the app as uninstalled, so the **Leftovers** tab shows the folders
it left behind with high confidence. The note on the app's row has a **Review leftovers** button.

"Don't suggest this app again" (the eye icon) hides an app. Settings, Leftover Cleanup,
Ignored items lists hidden apps, and you can bring them back there.

## Testing it yourself

Run the automated tests (51 tests, none touch real apps):

```powershell
cd src-tauri
cargo test --lib
```

Print what the tab would list on this machine (read-only):

```powershell
cargo test --lib live_unused_apps_smoke -- --ignored --nocapture
```

### Safe end-to-end test with a fake app

1. Create a fake installed app whose "uninstaller" only deletes its own registry entry:

   ```powershell
   $dir = "$env:LOCALAPPDATA\Programs\UnusedTestApp"
   New-Item -ItemType Directory -Force $dir | Out-Null
   Set-Content "$dir\uninstall.cmd" "@echo off`r`nreg delete `"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\UnusedTestApp`" /f" -Encoding ASCII
   $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\UnusedTestApp'
   New-Item $key -Force | Out-Null
   Set-ItemProperty $key DisplayName 'Unused Test App'
   Set-ItemProperty $key Publisher 'Test Vendor'
   Set-ItemProperty $key InstallLocation $dir
   Set-ItemProperty $key UninstallString "`"$dir\uninstall.cmd`""
   ```

2. Open Storage, **Unused apps**, and click **Find unused apps**. "Unused Test App" appears under
   **No record of use**, because it has never been run.
3. Click **Uninstall**, then confirm. A console window flashes and closes. Within a few seconds the
   row says it was uninstalled and offers **Review leftovers**.
4. In Settings, Leftover Cleanup, choose **0 days (dev)**. Then scan on the Leftovers tab: the
   `UnusedTestApp` folder shows as **High**, pre-ticked, with "Was its install folder (+8)".
5. Clean up: `Remove-Item -Recurse "$env:LOCALAPPDATA\Programs\UnusedTestApp"` and set the age
   back to 30 days.

### Admin prompt

Uninstallers that need administrator rights show the Windows prompt themselves. If you click
**No**, the row says permission was declined and nothing changes.

## Known limitations

- **Early days:** right after the first launch, many apps you use daily show "No record of use" until ASAO's monitor has seen them run. Apps launched from the Start menu or taskbar usually have a UserAssist record already.
- **Background and terminal programs:** apps started only from a terminal, or only at login and never opened, may look unused. Check before removing.
- **Portable apps:** portable apps in Downloads, Desktop or Documents are not listed. Those folders are mostly installers and personal files.
- **Admin folders:** removing a program folder that needs administrator rights is not supported yet.
- **Epic games:** the Epic Games Launcher cannot be asked to uninstall a specific game, so ASAO only opens it.
