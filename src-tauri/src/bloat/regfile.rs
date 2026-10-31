//! `.reg` files, the way Win11Debloat uses them: parse, check whether a file is already in
//! effect (every operation matches the live registry), back up exactly what it would
//! replace, import it with `reg import`, and restore the backup for undo.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::executor::ps;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum RegData {
    DWord { value: u32 },
    QWord { value: u64 },
    String { value: String },
    /// Raw bytes of `hex:`, `hex(2):`, `hex(7):` etc., base64 encoded, with the kind name.
    Raw { kind: String, base64: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum RegOp {
    /// `name` is empty for the key's default value (`@`).
    Set { key: String, name: String, data: RegData },
    DeleteValue { key: String, name: String },
    DeleteKey { key: String },
}

impl RegOp {
    pub fn key(&self) -> &str {
        match self {
            RegOp::Set { key, .. } | RegOp::DeleteValue { key, .. } | RegOp::DeleteKey { key } => key,
        }
    }
}

/// UTF-16 (with BOM) or UTF-8 text.
pub fn decode(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)).to_string()
    }
}

/// Expands `HKCU` style prefixes to the full hive names `reg import` writes.
pub fn full_key(key: &str) -> String {
    let k = key.trim();
    for (short, long) in [
        ("HKCU\\", "HKEY_CURRENT_USER\\"),
        ("HKLM\\", "HKEY_LOCAL_MACHINE\\"),
        ("HKCR\\", "HKEY_CLASSES_ROOT\\"),
        ("HKU\\", "HKEY_USERS\\"),
    ] {
        if let Some(rest) = k.strip_prefix(short) {
            return format!("{}{}", long, rest);
        }
    }
    k.to_string()
}

fn unquote(s: &str) -> Option<(String, &str)> {
    let rest = s.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                if let Some((_, n)) = chars.next() {
                    out.push(n);
                }
            }
            '"' => return Some((out, &rest[i + 1..])),
            c => out.push(c),
        }
    }
    None
}

fn hex_bytes(s: &str) -> Vec<u8> {
    s.split(',')
        .filter_map(|b| u8::from_str_radix(b.trim(), 16).ok())
        .collect()
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn parse_data(raw: &str) -> Option<RegData> {
    let raw = raw.trim();
    if raw.starts_with('"') {
        return unquote(raw).map(|(s, _)| RegData::String { value: s });
    }
    if let Some(h) = raw.strip_prefix("dword:") {
        return u32::from_str_radix(h.trim(), 16).ok().map(|v| RegData::DWord { value: v });
    }
    if let Some(h) = raw.strip_prefix("hex(b):") {
        let b = hex_bytes(h);
        if b.len() == 8 {
            return Some(RegData::QWord { value: u64::from_le_bytes(b.try_into().ok()?) });
        }
    }
    let (kind, h) = if let Some(h) = raw.strip_prefix("hex:") {
        ("Binary", h)
    } else if let Some(h) = raw.strip_prefix("hex(2):") {
        ("ExpandString", h)
    } else if let Some(h) = raw.strip_prefix("hex(7):") {
        ("MultiString", h)
    } else {
        return None;
    };
    Some(RegData::Raw { kind: kind.into(), base64: base64(&hex_bytes(h)) })
}

/// Parses a `.reg` file into its operations, in order.
pub fn parse(text: &str) -> Vec<RegOp> {
    // Join continuation lines (a trailing backslash).
    let mut lines: Vec<String> = Vec::new();
    let mut pending = String::new();
    for line in text.lines() {
        let l = line.trim_end();
        if let Some(stripped) = l.strip_suffix('\\') {
            pending.push_str(stripped.trim_start());
            continue;
        }
        pending.push_str(if pending.is_empty() { l } else { l.trim_start() });
        lines.push(std::mem::take(&mut pending));
    }
    if !pending.is_empty() {
        lines.push(pending);
    }

    let mut ops = Vec::new();
    let mut key: Option<String> = None;
    for line in lines {
        let l = line.trim();
        if l.is_empty() || l.starts_with(';') || l.starts_with("Windows Registry Editor") || l.starts_with("REGEDIT") {
            continue;
        }
        if let Some(inner) = l.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
            if let Some(del) = inner.strip_prefix('-') {
                ops.push(RegOp::DeleteKey { key: full_key(del) });
                key = None;
            } else {
                key = Some(full_key(inner));
            }
            continue;
        }
        let Some(k) = key.clone() else { continue };
        let (name, rest) = if let Some(rest) = l.strip_prefix('@') {
            (String::new(), rest)
        } else if let Some((n, rest)) = unquote(l) {
            (n, rest)
        } else {
            continue;
        };
        let Some(value) = rest.trim_start().strip_prefix('=') else { continue };
        if value.trim() == "-" {
            ops.push(RegOp::DeleteValue { key: k, name });
        } else if let Some(data) = parse_data(value) {
            ops.push(RegOp::Set { key: k, name, data });
        }
    }
    ops
}

pub fn touches_machine(ops: &[RegOp]) -> bool {
    ops.iter().any(|o| o.key().starts_with("HKEY_LOCAL_MACHINE") || o.key().starts_with("HKEY_CLASSES_ROOT"))
}

/// One value (or key) as it is now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveValue {
    pub key_exists: bool,
    pub exists: bool,
    /// .NET RegistryValueKind: "DWord", "String", "Binary", ...
    pub kind: Option<String>,
    /// DWord as a number; QWord as a string; Binary base64; MultiString an array; others a string.
    pub value: Option<Value>,
}

fn b64_arg(json: &str) -> String {
    base64(json.as_bytes())
}

/// Queries per PowerShell call: the script carries them on its command line, which Windows
/// limits to 32,767 characters.
const READ_CHUNK: usize = 60;

/// Reads many values, a chunk per PowerShell call. `name` `None` only checks the key.
pub fn read_live(queries: &[(String, Option<String>)]) -> Result<Vec<LiveValue>, String> {
    let mut out = Vec::with_capacity(queries.len());
    for chunk in queries.chunks(READ_CHUNK) {
        let part = read_chunk(chunk)?;
        if part.len() != chunk.len() {
            return Err("The registry read returned an unexpected number of values.".into());
        }
        out.extend(part);
    }
    Ok(out)
}

fn read_chunk(queries: &[(String, Option<String>)]) -> Result<Vec<LiveValue>, String> {
    if queries.is_empty() {
        return Ok(Vec::new());
    }
    let q: Vec<Value> = queries
        .iter()
        .map(|(k, n)| serde_json::json!({ "k": k, "n": n }))
        .collect();
    let script = format!(
        r#"
$q = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{b64}')) | ConvertFrom-Json
$out = foreach ($x in @($q)) {{
  $r = [ordered]@{{ keyExists = $false; exists = $false; kind = $null; value = $null }}
  $item = Get-Item -LiteralPath ('Registry::' + $x.k) -ErrorAction SilentlyContinue
  if ($item) {{
    $r.keyExists = $true
    if ($null -ne $x.n -and (@($item.GetValueNames()) -contains [string]$x.n)) {{
      $nm = [string]$x.n
      $r.exists = $true
      $r.kind = [string]$item.GetValueKind($nm)
      $v = $item.GetValue($nm, $null, 'DoNotExpandEnvironmentNames')
      switch ($r.kind) {{
        'DWord' {{ $r.value = [int64][BitConverter]::ToUInt32([BitConverter]::GetBytes([int32]$v), 0) }}
        'QWord' {{ $r.value = [string][BitConverter]::ToUInt64([BitConverter]::GetBytes([int64]$v), 0) }}
        'Binary' {{ $r.value = [Convert]::ToBase64String([byte[]]$v) }}
        'MultiString' {{ $r.value = @($v) }}
        default {{ $r.value = [string]$v }}
      }}
    }}
  }}
  [pscustomobject]$r
}}
ConvertTo-Json -InputObject @($out) -Depth 4 -Compress
"#,
        b64 = b64_arg(&serde_json::to_string(&q).unwrap_or_default())
    );
    let out = ps(script.trim())?;
    let v: Value = serde_json::from_str(&out).map_err(|e| format!("Could not read the registry: {}", e))?;
    let arr = match v {
        Value::Array(a) => a,
        other => vec![other],
    };
    Ok(arr
        .into_iter()
        .map(|x| serde_json::from_value(x).unwrap_or_default())
        .collect())
}

/// The queries needed to judge or back up a file's operations.
pub fn queries_for(ops: &[RegOp]) -> Vec<(String, Option<String>)> {
    ops.iter()
        .map(|o| match o {
            RegOp::Set { key, name, .. } | RegOp::DeleteValue { key, name } => (key.clone(), Some(name.clone())),
            RegOp::DeleteKey { key } => (key.clone(), None),
        })
        .collect()
}

/// Does the live value already match this operation?
pub fn op_matches(op: &RegOp, live: &LiveValue) -> bool {
    match op {
        RegOp::DeleteKey { .. } => !live.key_exists,
        RegOp::DeleteValue { .. } => !live.exists,
        RegOp::Set { data, .. } => {
            if !live.exists {
                return false;
            }
            let kind = live.kind.as_deref().unwrap_or("");
            match data {
                RegData::DWord { value } => kind == "DWord" && live.value.as_ref().and_then(|v| v.as_u64()) == Some(*value as u64),
                RegData::QWord { value } => {
                    kind == "QWord" && live.value.as_ref().and_then(|v| v.as_str()).map(|s| s == value.to_string()).unwrap_or(false)
                }
                RegData::String { value } => {
                    kind == "String" && live.value.as_ref().and_then(|v| v.as_str()) == Some(value.as_str())
                }
                RegData::Raw { kind: k, base64: b } => {
                    k == "Binary" && kind == "Binary" && live.value.as_ref().and_then(|v| v.as_str()) == Some(b.as_str())
                }
            }
        }
    }
}

/// Every operation already in effect (a file with no operations is never "applied").
pub fn all_match(ops: &[RegOp], live: &[LiveValue]) -> bool {
    !ops.is_empty() && ops.len() == live.len() && ops.iter().zip(live).all(|(o, l)| op_matches(o, l))
}

/// Exactly what a file would replace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum RegBackup {
    Value { key: String, name: String, live: LiveValue },
    /// A key the file deletes, exported with `reg export` (UTF-16 text, base64).
    Key { key: String, export_base64: Option<String> },
}

fn reg_cli_key(key: &str) -> String {
    key.replacen("HKEY_CURRENT_USER", "HKCU", 1)
        .replacen("HKEY_LOCAL_MACHINE", "HKLM", 1)
        .replacen("HKEY_CLASSES_ROOT", "HKCR", 1)
        .replacen("HKEY_USERS", "HKU", 1)
}

fn export_key(key: &str, scratch: &Path) -> Result<Option<String>, String> {
    let file = scratch.join(format!("export-{}.reg", crate::residual::now_ms()));
    let _ = std::fs::create_dir_all(scratch);
    let status = std::process::Command::new("reg")
        .args(["export", &reg_cli_key(key), &file.to_string_lossy(), "/y"])
        .output()
        .map_err(|e| format!("Could not run reg export: {}", e))?;
    if !status.status.success() {
        return Ok(None); // the key does not exist
    }
    let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&file);
    Ok(Some(base64(&bytes)))
}

pub fn backup(ops: &[RegOp], scratch: &Path) -> Result<Vec<RegBackup>, String> {
    let live = read_live(&queries_for(ops))?;
    let mut out = Vec::new();
    for (op, l) in ops.iter().zip(live) {
        match op {
            RegOp::Set { key, name, .. } | RegOp::DeleteValue { key, name } => {
                out.push(RegBackup::Value { key: key.clone(), name: name.clone(), live: l })
            }
            RegOp::DeleteKey { key } => out.push(RegBackup::Key {
                key: key.clone(),
                export_base64: if l.key_exists { export_key(key, scratch)? } else { None },
            }),
        }
    }
    Ok(out)
}

/// Runs `reg import` on a file's bytes.
pub fn import(bytes: &[u8], scratch: &Path) -> Result<(), String> {
    let _ = std::fs::create_dir_all(scratch);
    let file = scratch.join(format!("import-{}.reg", crate::residual::now_ms()));
    std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
    let out = std::process::Command::new("reg")
        .args(["import", &file.to_string_lossy()])
        .output()
        .map_err(|e| format!("Could not run reg import: {}", e));
    let _ = std::fs::remove_file(&file);
    let out = out?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { "Windows refused the registry change.".into() } else { err })
    }
}

/// Puts back exactly what `backup` recorded, last operation first.
pub fn restore(backups: &[RegBackup], scratch: &Path) -> Result<(), String> {
    let values: Vec<Value> = backups
        .iter()
        .rev()
        .filter_map(|b| match b {
            RegBackup::Value { key, name, live } => Some(serde_json::json!({
                "k": key, "n": name, "e": live.exists, "t": live.kind, "v": live.value
            })),
            RegBackup::Key { .. } => None,
        })
        .collect();
    for values in values.chunks(READ_CHUNK) {
        let script = format!(
            r#"
$b = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{b64}')) | ConvertFrom-Json
function Open-Writable($k, $create) {{
  $hive, $rest = $k -split '\\', 2
  $root = switch ($hive) {{
    'HKEY_CURRENT_USER' {{ [Microsoft.Win32.Registry]::CurrentUser }}
    'HKEY_LOCAL_MACHINE' {{ [Microsoft.Win32.Registry]::LocalMachine }}
    'HKEY_CLASSES_ROOT' {{ [Microsoft.Win32.Registry]::ClassesRoot }}
    default {{ [Microsoft.Win32.Registry]::Users }}
  }}
  if ($create) {{ return $root.CreateSubKey($rest, $true) }} else {{ return $root.OpenSubKey($rest, $true) }}
}}
foreach ($x in @($b)) {{
  if ($x.e) {{
    switch ([string]$x.t) {{
      'DWord' {{ $v = [BitConverter]::ToInt32([BitConverter]::GetBytes([uint32]$x.v), 0) }}
      'QWord' {{ $v = [BitConverter]::ToInt64([BitConverter]::GetBytes([uint64]$x.v), 0) }}
      'Binary' {{ $v = [Convert]::FromBase64String([string]$x.v) }}
      'MultiString' {{ $v = [string[]]@($x.v) }}
      default {{ $v = [string]$x.v }}
    }}
    $w = Open-Writable $x.k $true
    $w.SetValue([string]$x.n, $v, [Microsoft.Win32.RegistryValueKind]([string]$x.t)); $w.Close()
  }} else {{
    $w = Open-Writable $x.k $false
    if ($w) {{ $w.DeleteValue([string]$x.n, $false); $w.Close() }}
  }}
}}
"#,
            b64 = b64_arg(&serde_json::to_string(&values).unwrap_or_default())
        );
        ps(script.trim())?;
    }
    for b in backups.iter().rev() {
        if let RegBackup::Key { export_base64: Some(b64), .. } = b {
            import(&base64_decode(b64), scratch)?;
        }
    }
    Ok(())
}

fn base64_decode(s: &str) -> Vec<u8> {
    let val = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut out = Vec::new();
    let clean: Vec<u8> = s.bytes().filter(|c| *c != b'=').collect();
    for chunk in clean.chunks(4) {
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= val(*c).unwrap_or(0) << (18 - 6 * i);
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..chunk.len().saturating_sub(1)]);
    }
    out
}

#[cfg(test)]
pub fn test_base64(bytes: &[u8]) -> String {
    base64(bytes)
}
#[cfg(test)]
pub fn test_base64_decode(s: &str) -> Vec<u8> {
    base64_decode(s)
}
