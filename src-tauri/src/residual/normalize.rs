//! Name and path normalisation shared by the ownership checker and the scorer.

/// Words dropped from the end of a product name ("Foo Setup" and "Foo" are the same app).
const TRAILING_WORDS: &[&str] = &["setup", "launcher", "installer"];

/// Architecture markers dropped anywhere in a name.
const ARCH_TOKENS: &[&str] = &[
    "x64", "x86", "amd64", "arm64", "win64", "win32", "64bit", "32bit", "bit",
];

/// Legal suffixes dropped from publisher names ("Mozilla Corporation" becomes "mozilla").
const LEGAL_SUFFIXES: &[&str] = &[
    "inc", "llc", "ltd", "limited", "corp", "corporation", "co", "company", "gmbh", "ag", "bv",
    "sro", "sa", "srl", "pty", "plc", "oy", "ab", "kg", "the",
];

/// Folder names too generic to be matched to an app on their own.
const GENERIC_WORDS: &[&str] = &[
    "data", "cache", "logs", "log", "temp", "tmp", "config", "settings",
];

/// Canonical form of a Windows path for comparisons: lowercase, backslashes,
/// no `\\?\` prefix and no trailing separator (except on a drive root).
pub fn path_key(p: &str) -> String {
    let mut s = p.trim().replace('/', "\\");
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        s = rest.to_string();
    }
    if let Some(rest) = s.strip_prefix(r"\??\") {
        s = rest.to_string();
    }
    let mut s = s.to_lowercase();
    while s.len() > 3 && s.ends_with('\\') {
        s.pop();
    }
    s
}

/// True when `child` is `parent` itself or sits somewhere below it.
pub fn is_same_or_inside(child_key: &str, parent_key: &str) -> bool {
    if parent_key.is_empty() {
        return false;
    }
    if child_key == parent_key {
        return true;
    }
    let prefix = if parent_key.ends_with('\\') {
        parent_key.to_string()
    } else {
        format!("{}\\", parent_key)
    };
    child_key.starts_with(&prefix)
}

fn is_version_token(token: &str, index: usize) -> bool {
    let body = token.strip_prefix('v').unwrap_or(token);
    let Some(first) = body.chars().next() else {
        return false;
    };
    if !first.is_ascii_digit() {
        return false;
    }
    if !body.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return false;
    }
    // "1.2.3" and "v12" are versions. A bare number counts only after the first word,
    // so "7-Zip" keeps its 7 but "Office 2016" loses the 2016.
    body.contains('.') || token.starts_with('v') || index > 0
}

/// Lowercase, `_` and `-` as spaces, bracketed text, versions, architecture markers,
/// trademark signs and trailing words like "Setup" removed.
pub fn normalize_name(input: &str) -> String {
    let mut s = String::with_capacity(input.len());
    let mut depth = 0i32;
    for ch in input.chars() {
        match ch {
            '(' | '[' | '{' => {
                depth += 1;
                s.push(' ');
            }
            ')' | ']' | '}' => {
                if depth > 0 {
                    depth -= 1;
                }
                s.push(' ');
            }
            _ if depth > 0 => {}
            '\u{2122}' | '\u{00AE}' | '\u{00A9}' => {}
            '_' | '-' => s.push(' '),
            c => s.extend(c.to_lowercase()),
        }
    }

    let mut tokens: Vec<String> = Vec::new();
    for (i, raw) in s.split_whitespace().enumerate() {
        let t = raw.trim_matches(|c: char| matches!(c, ',' | ':' | ';' | '.' | '"' | '\''));
        if t.is_empty() || is_version_token(t, i) || ARCH_TOKENS.contains(&t) {
            continue;
        }
        tokens.push(t.to_string());
    }
    while tokens.len() > 1
        && tokens
            .last()
            .map(|t| TRAILING_WORDS.contains(&t.as_str()))
            .unwrap_or(false)
    {
        tokens.pop();
    }
    tokens.join(" ")
}

/// Publisher form: dots dropped ("s.r.o." becomes "sro"), legal suffixes removed.
pub fn normalize_publisher(input: &str) -> String {
    let pre = input.replace('.', "").replace(',', " ");
    let norm = normalize_name(&pre);
    let kept: Vec<&str> = norm
        .split_whitespace()
        .filter(|t| !LEGAL_SUFFIXES.contains(t))
        .collect();
    if kept.is_empty() {
        norm
    } else {
        kept.join(" ")
    }
}

/// Letters and digits only (plus `+` and `#`, so "Notepad++" stays distinct).
pub fn compact(norm: &str) -> String {
    norm.chars()
        .filter(|c| c.is_alphanumeric() || *c == '+' || *c == '#')
        .collect()
}

/// "mozilla firefox" with publisher "mozilla" becomes "firefox".
pub fn strip_publisher(name_norm: &str, publisher_norm: &str) -> Option<String> {
    let name: Vec<&str> = name_norm.split_whitespace().collect();
    let publ: Vec<&str> = publisher_norm.split_whitespace().collect();
    if name.len() < 2 || publ.is_empty() {
        return None;
    }
    if name.len() > publ.len() && name[..publ.len()] == publ[..] {
        return Some(name[publ.len()..].join(" "));
    }
    if publ[0].chars().count() >= 3 && name[0] == publ[0] {
        return Some(name[1..].join(" "));
    }
    None
}

pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameMatch {
    None,
    Fuzzy,
    Exact,
}

/// Exact when the compact forms are equal; fuzzy within edit distance 2 when both
/// names are longer than 4 characters.
pub fn match_names(a_norm: &str, b_norm: &str) -> NameMatch {
    let ca = compact(a_norm);
    let cb = compact(b_norm);
    if ca.is_empty() || cb.is_empty() {
        return NameMatch::None;
    }
    if ca == cb {
        return NameMatch::Exact;
    }
    if ca.chars().count() > 4 && cb.chars().count() > 4 && levenshtein(&ca, &cb) <= 2 {
        return NameMatch::Fuzzy;
    }
    NameMatch::None
}

/// Stricter fuzzy match used for scoring: names under 8 characters may differ by one edit,
/// longer names by two. ("packer" and "docker" differ by two and do not match.)
/// The ownership checker keeps the looser `match_names`, since a false match there only
/// protects a folder.
pub fn match_names_for_scoring(a_norm: &str, b_norm: &str) -> NameMatch {
    match match_names(a_norm, b_norm) {
        NameMatch::Fuzzy => {
            let ca = compact(a_norm);
            let cb = compact(b_norm);
            let shortest = ca.chars().count().min(cb.chars().count());
            let allowed = if shortest >= 8 { 2 } else { 1 };
            if levenshtein(&ca, &cb) <= allowed {
                NameMatch::Fuzzy
            } else {
                NameMatch::None
            }
        }
        other => other,
    }
}

/// Similar-name check: does `hay` contain `needle` as whole words, or (for needles of
/// 5+ characters) as a substring of the compact form? "chrome remote desktop" contains "chrome".
pub fn token_contains(hay_norm: &str, needle_norm: &str) -> bool {
    let hay: Vec<&str> = hay_norm.split_whitespace().collect();
    let needle: Vec<&str> = needle_norm.split_whitespace().collect();
    if needle.is_empty() || hay.is_empty() || hay == needle {
        return false;
    }
    if needle.len() < hay.len() && hay.windows(needle.len()).any(|w| w == needle.as_slice()) {
        return true;
    }
    let ch = compact(hay_norm);
    let cn = compact(needle_norm);
    cn.chars().count() >= 5 && ch != cn && ch.contains(&cn)
}

pub fn is_generic_or_short(norm: &str) -> bool {
    compact(norm).chars().count() <= 4 || GENERIC_WORDS.contains(&norm)
}

/// Expands `%NAME%` references using the current environment.
pub fn expand_env(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if let Some(end) = after.find('%') {
            let name = &after[..end];
            match std::env::var(name) {
                Ok(val) if !name.is_empty() => out.push_str(&val),
                _ => {
                    out.push('%');
                    out.push_str(name);
                    out.push('%');
                }
            }
            rest = &after[end + 1..];
        } else {
            out.push('%');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

fn is_absolute_windows(p: &str) -> bool {
    let b = p.as_bytes();
    (b.len() >= 3 && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')) || p.starts_with(r"\\")
}

/// Executable path from a command line: quotes stripped, arguments dropped,
/// environment variables expanded. Returns a `path_key`.
pub fn exe_path_from_command(cmd: &str) -> Option<String> {
    let s = cmd.trim();
    let s = s.strip_prefix(r"\??\").unwrap_or(s);
    if s.is_empty() {
        return None;
    }
    let raw = if let Some(inner) = s.strip_prefix('"') {
        inner.split('"').next().unwrap_or("").to_string()
    } else {
        let lower = s.to_ascii_lowercase();
        if let Some(i) = lower.find(".exe") {
            s[..i + 4].to_string()
        } else {
            s.split_whitespace().next().unwrap_or("").to_string()
        }
    };
    let expanded = expand_env(raw.trim());
    if is_absolute_windows(&expanded) {
        Some(path_key(&expanded))
    } else {
        None
    }
}

/// Parent folder of an executable referenced by a command (an uninstall string).
pub fn dir_from_command(cmd: &str) -> Option<String> {
    let exe = exe_path_from_command(cmd)?;
    parent_key(&exe)
}

/// Folder of a `DisplayIcon` value like `"C:\App\app.exe",0`.
pub fn dir_from_icon(icon: &str) -> Option<String> {
    let mut s = icon.trim().trim_matches('"').to_string();
    if let Some((head, tail)) = s.rsplit_once(',') {
        if tail.trim().parse::<i32>().is_ok() {
            s = head.trim().trim_matches('"').to_string();
        }
    }
    let expanded = expand_env(&s);
    if !is_absolute_windows(&expanded) {
        return None;
    }
    let key = path_key(&expanded);
    let last = key.rsplit('\\').next().unwrap_or("");
    if last.contains('.') {
        parent_key(&key)
    } else {
        Some(key)
    }
}

pub fn parent_key(key: &str) -> Option<String> {
    let idx = key.rfind('\\')?;
    if idx < 2 {
        return None;
    }
    let parent = &key[..idx];
    if parent.len() == 2 && parent.ends_with(':') {
        return Some(format!("{}\\", parent));
    }
    Some(parent.to_string())
}

/// Stable short id for a path or name (FNV-1a).
pub fn stable_id(input: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in input.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_versions_arch_and_trademarks() {
        assert_eq!(normalize_name("Mozilla Firefox (x64 en-US)"), "mozilla firefox");
        assert_eq!(normalize_name("Python 3.12.1 (64-bit)"), "python");
        assert_eq!(normalize_name("7-Zip 23.01 (x64)"), "7 zip");
        assert_eq!(normalize_name("Notepad++ (64-bit x64)"), "notepad++");
        assert_eq!(normalize_name("Contoso\u{2122} Studio\u{00AE} v12"), "contoso studio");
        assert_eq!(normalize_name("Foo_Bar-Baz Setup"), "foo bar baz");
        assert_eq!(normalize_name("Game Launcher"), "game");
        assert_eq!(normalize_name("Microsoft Office 2016"), "microsoft office");
        assert_eq!(normalize_name("Setup"), "setup");
    }

    #[test]
    fn normalises_publishers() {
        assert_eq!(normalize_publisher("Mozilla Corporation"), "mozilla");
        assert_eq!(normalize_publisher("JetBrains s.r.o."), "jetbrains");
        assert_eq!(normalize_publisher("Google LLC"), "google");
        assert_eq!(normalize_publisher("VideoLAN"), "videolan");
    }

    #[test]
    fn strips_publisher_from_product() {
        assert_eq!(
            strip_publisher("mozilla firefox", "mozilla").as_deref(),
            Some("firefox")
        );
        assert_eq!(strip_publisher("firefox", "mozilla"), None);
        assert_eq!(
            strip_publisher("microsoft visual studio code", "microsoft").as_deref(),
            Some("visual studio code")
        );
    }

    #[test]
    fn fuzzy_matching_rules() {
        assert_eq!(match_names("notepad++", "notepad++"), NameMatch::Exact);
        assert_eq!(match_names("7 zip", "7zip"), NameMatch::Exact);
        assert_eq!(match_names("audacity", "audacty"), NameMatch::Fuzzy);
        assert_eq!(match_names("blender", "blendr2"), NameMatch::Fuzzy);
        // Names of 4 characters or fewer never match fuzzily.
        assert_eq!(match_names("zoom", "zoon"), NameMatch::None);
        assert_eq!(match_names("audacity", "gimp"), NameMatch::None);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        // Scoring is stricter for short names
        assert_eq!(match_names("packer", "docker"), NameMatch::Fuzzy);
        assert_eq!(match_names_for_scoring("packer", "docker"), NameMatch::None);
        assert_eq!(match_names_for_scoring("audacity", "audacty"), NameMatch::Fuzzy);
        assert_eq!(match_names_for_scoring("blender", "blendr2"), NameMatch::None);
        assert_eq!(match_names_for_scoring("photoshop", "photoshp2"), NameMatch::Fuzzy);
    }

    #[test]
    fn similar_name_check() {
        assert!(token_contains("chrome remote desktop", "chrome"));
        assert!(token_contains("google chrome", "chrome"));
        assert!(!token_contains("chrome", "chrome"));
        assert!(!token_contains("firefox", "chrome"));
        assert!(token_contains("visualstudiocode", "studio"));
    }

    #[test]
    fn generic_names() {
        assert!(is_generic_or_short("data"));
        assert!(is_generic_or_short("abc"));
        assert!(is_generic_or_short("settings"));
        assert!(!is_generic_or_short("audacity"));
    }

    #[test]
    fn paths_and_commands() {
        assert_eq!(path_key("C:/Program Files/App/"), r"c:\program files\app");
        assert_eq!(path_key(r"\\?\C:\Temp"), r"c:\temp");
        assert!(is_same_or_inside(r"c:\a\b", r"c:\a"));
        assert!(!is_same_or_inside(r"c:\ab", r"c:\a"));
        assert_eq!(
            exe_path_from_command(r#""C:\Program Files\App\uninst.exe" /S"#).as_deref(),
            Some(r"c:\program files\app\uninst.exe")
        );
        assert_eq!(
            exe_path_from_command(r"C:\Program Files\App\app.exe --flag").as_deref(),
            Some(r"c:\program files\app\app.exe")
        );
        assert_eq!(exe_path_from_command("MsiExec.exe /X{GUID}"), None);
        assert_eq!(
            dir_from_command(r#""C:\Program Files\App\uninst.exe" /S"#).as_deref(),
            Some(r"c:\program files\app")
        );
        assert_eq!(
            dir_from_icon(r#""C:\Program Files\App\app.exe",0"#).as_deref(),
            Some(r"c:\program files\app")
        );
        assert_eq!(parent_key(r"c:\app.exe").as_deref(), Some(r"c:\"));
    }
}
