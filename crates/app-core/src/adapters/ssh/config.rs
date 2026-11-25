use crate::api::{ConfigImport, Error, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub struct Manifest {
    pub fingerprint: String,
    pub aliases: Vec<String>,
    pub disclosures: Vec<String>,
    pub text: String,
}
struct Importer {
    hash: Sha256,
    bytes: usize,
    files: usize,
    stack: BTreeSet<PathBuf>,
    aliases: BTreeSet<String>,
    disclosures: BTreeSet<String>,
    text: String,
}
impl Importer {
    fn read(&mut self, path: &Path, depth: usize) -> Result<()> {
        if !path.exists() {
            self.hash.update(path.as_os_str().as_encoded_bytes());
            self.hash.update(b"missing");
            return Ok(());
        }
        if depth > 8 || self.files >= 100 {
            return Err(Error::Limit("SSH Include depth/files"));
        }
        let canonical = fs::canonicalize(path)?;
        if !self.stack.insert(canonical.clone()) {
            return Err(Error::Invalid("SSH Include cycle".into()));
        }
        let meta = fs::metadata(&canonical)?;
        if !meta.is_file() || meta.len() > (1024 * 1024 - self.bytes) as u64 {
            return Err(Error::Limit("SSH config 1 MiB"));
        }
        let bytes = fs::read(&canonical)?;
        self.bytes += bytes.len();
        self.files += 1;
        if self.bytes > 1024 * 1024 {
            return Err(Error::Limit("SSH config 1 MiB"));
        }
        self.hash.update(canonical.as_os_str().as_encoded_bytes());
        self.hash.update(&bytes);
        let content = String::from_utf8(bytes)
            .map_err(|_| Error::Invalid("SSH config must be UTF-8".into()))?;
        for line in content.lines() {
            let words = tokenize(line)?;
            if words.is_empty() {
                continue;
            }
            let key = words[0].to_ascii_lowercase();
            if key == "include" {
                for pattern in &words[1..] {
                    if pattern.contains(['%', '$', '`']) {
                        self.disclosures.insert(
                            "Dynamic Include: native SSH execution requires session consent".into(),
                        );
                        self.text.push_str(line);
                        self.text.push('\n');
                        continue;
                    }
                    let expanded = expand_home(pattern)?;
                    let pattern_path = if expanded.is_absolute() {
                        expanded
                    } else {
                        // OpenSSH resolves user includes relative to ~/.ssh, system includes to /etc/ssh.
                        let base = if canonical.starts_with("/etc/ssh") {
                            PathBuf::from("/etc/ssh")
                        } else {
                            home()?.join(".ssh")
                        };
                        base.join(expanded)
                    };
                    let mut matches = bounded_glob(&pattern_path, 100 - self.files)?;
                    matches.sort();
                    self.hash
                        .update(pattern_path.as_os_str().as_encoded_bytes());
                    for p in matches {
                        self.hash.update(p.as_os_str().as_encoded_bytes());
                        self.read(&p, depth + 1)?;
                    }
                }
                continue;
            }
            if key == "host" {
                for alias in &words[1..] {
                    if valid_target(alias) {
                        self.aliases.insert(alias.clone());
                    }
                }
            }
            // Native dynamic execution is never silently invoked during import/connect.
            if [
                "match",
                "proxycommand",
                "localcommand",
                "knownhostscommand",
                "canonicalizehostname",
                "remotecommand",
                "permitlocalcommand",
                "pkcs11provider",
                "securitykeyprovider",
            ]
            .contains(&key.as_str())
            {
                self.disclosures.insert(format!(
                    "{key}: dynamic/executable configuration requires explicit session consent"
                ));
            }
            self.text.push_str(line);
            self.text.push('\n');
        }
        self.stack.remove(&canonical);
        Ok(())
    }
}
pub fn home() -> Result<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or_else(|| Error::Invalid("HOME unavailable".into()))
}
pub fn expand_home(value: &str) -> Result<PathBuf> {
    if let Some(p) = value.strip_prefix("~/") {
        Ok(home()?.join(p))
    } else {
        Ok(PathBuf::from(value))
    }
}
pub fn valid_target(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && !s.starts_with('-')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-:@[]".contains(&b))
}
pub fn load() -> Result<Manifest> {
    let mut i = Importer {
        hash: Sha256::new(),
        bytes: 0,
        files: 0,
        stack: BTreeSet::new(),
        aliases: BTreeSet::new(),
        disclosures: BTreeSet::new(),
        text: String::new(),
    };
    i.read(&home()?.join(".ssh/config"), 0)?;
    i.text.push_str("Host *\n");
    #[cfg(unix)]
    i.read(Path::new("/etc/ssh/ssh_config"), 0)?;
    // Binary identity, trust mode and inherited agent participate in mux generation.
    let binary = fs::read(ssh_binary())?;
    i.hash.update(binary);
    i.hash.update(b"strict-host-key/no-forwarding/v1");
    if let Some(socket) = std::env::var_os("SSH_AUTH_SOCK") {
        i.hash.update(socket.as_encoded_bytes());
    }
    let mut trust_files = BTreeSet::from([
        home()?.join(".ssh/known_hosts"),
        home()?.join(".ssh/known_hosts2"),
        PathBuf::from("/etc/ssh/ssh_known_hosts"),
        PathBuf::from("/etc/ssh/ssh_known_hosts2"),
    ]);
    for line in i.text.lines() {
        let words = tokenize(line)?;
        if words.first().is_some_and(|w| {
            ["userknownhostsfile", "globalknownhostsfile"]
                .contains(&w.to_ascii_lowercase().as_str())
        }) {
            for path in words.iter().skip(1) {
                if path.contains(['%', '$', '`']) {
                    i.disclosures
                        .insert("Dynamic trust-file path requires session consent".into());
                    continue;
                }
                if path != "none" && path != "/dev/null" {
                    trust_files.insert(expand_home(path)?);
                }
            }
        }
    }
    if trust_files.len() > 100 {
        return Err(Error::Limit("trust files"));
    }
    let mut trust_bytes = 0;
    for path in trust_files {
        use std::io::Read;
        i.hash.update(path.as_os_str().as_encoded_bytes());
        let file = match fs::File::open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                i.hash.update(b"missing");
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        if !file.metadata()?.is_file() {
            return Err(Error::Invalid("trust file must be regular".into()));
        }
        let mut bytes = vec![];
        file.take((4 * 1024 * 1024 - trust_bytes + 1) as u64)
            .read_to_end(&mut bytes)?;
        trust_bytes += bytes.len();
        if trust_bytes > 4 * 1024 * 1024 {
            return Err(Error::Limit("known_hosts 4 MiB"));
        }
        i.hash.update(bytes);
    }
    Ok(Manifest {
        fingerprint: format!("{:x}", i.hash.finalize()),
        aliases: i.aliases.into_iter().collect(),
        disclosures: i.disclosures.into_iter().collect(),
        text: i.text,
    })
}
impl Manifest {
    pub fn user_trust_paths(&self, target: &str) -> Result<Vec<PathBuf>> {
        let target = target.rsplit('@').next().unwrap_or(target);
        let mut active = true;
        let mut dynamic_scope = false;
        for line in self.text.lines() {
            let words = tokenize(line)?;
            let Some(key) = words.first() else {
                continue;
            };
            let key = key.to_ascii_lowercase();
            if key == "host" {
                dynamic_scope = false;
                let mut positive = false;
                let mut negative = false;
                for word in &words[1..] {
                    let (negated, pattern) = word
                        .strip_prefix('!')
                        .map_or((false, word.as_str()), |p| (true, p));
                    if glob::Pattern::new(pattern)
                        .map_err(|_| Error::Invalid("Host pattern".into()))?
                        .matches(target)
                    {
                        if negated {
                            negative = true;
                        } else {
                            positive = true;
                        }
                    }
                }
                active = positive && !negative;
            } else if key == "match" {
                dynamic_scope = true;
            } else if key == "userknownhostsfile" && (active || dynamic_scope) {
                if dynamic_scope || words.iter().skip(1).any(|p| p.contains(['%', '$', '`'])) {
                    return Err(Error::NeedsAction(
                        "Use static UserKnownHostsFile paths for in-app fingerprint verification"
                            .into(),
                    ));
                }
                return words.iter().skip(1).filter(|p| p.as_str() != "none").map(|p| {
                    let path = expand_home(p)?;
                    if !path.is_absolute() { return Err(Error::NeedsAction("Use absolute known_hosts paths for in-app fingerprint verification".into())); }
                    Ok(path)
                }).collect();
            }
        }
        Ok(vec![
            home()?.join(".ssh/known_hosts"),
            home()?.join(".ssh/known_hosts2"),
        ])
    }
    pub fn needs_consent(&self, target: &str) -> Result<bool> {
        let mut pending = vec![target.to_string()];
        let mut visited = BTreeSet::new();
        while let Some(target) = pending.pop() {
            if !visited.insert(target.clone()) {
                continue;
            }
            if visited.len() > 8 {
                return Err(Error::Limit("ProxyJump depth"));
            }
            let target = target.rsplit('@').next().unwrap_or(&target);
            let mut active = true;
            for line in self.text.lines() {
                let words = tokenize(line)?;
                if words.is_empty() {
                    continue;
                }
                let key = words[0].to_ascii_lowercase();
                if key == "host" {
                    let mut positive = false;
                    let mut negative = false;
                    for pattern in &words[1..] {
                        let (negated, pattern) = pattern
                            .strip_prefix('!')
                            .map_or((false, pattern.as_str()), |p| (true, p));
                        if glob::Pattern::new(pattern)
                            .map_err(|_| Error::Invalid("Host pattern".into()))?
                            .matches(target)
                        {
                            if negated {
                                negative = true;
                            } else {
                                positive = true;
                            }
                        }
                    }
                    active = positive && !negative;
                    continue;
                }
                // Match scope cannot safely be resolved statically, including its jump closure.
                if key == "match" {
                    return Ok(true);
                }
                if !active {
                    continue;
                }
                if key == "proxyjump" {
                    for jump in words.iter().skip(1).flat_map(|w| w.split(',')) {
                        if jump != "none" {
                            pending.push(
                                jump.rsplit_once(':')
                                    .map_or(jump, |(host, port)| {
                                        if port.bytes().all(|b| b.is_ascii_digit()) {
                                            host
                                        } else {
                                            jump
                                        }
                                    })
                                    .into(),
                            );
                        }
                    }
                }
                if [
                    "proxycommand",
                    "localcommand",
                    "knownhostscommand",
                    "remotecommand",
                    "pkcs11provider",
                    "securitykeyprovider",
                ]
                .contains(&key.as_str())
                    && words.get(1).is_none_or(|v| v != "none")
                {
                    return Ok(true);
                }
                if key == "include" && words.iter().skip(1).any(|w| w.contains(['%', '$', '`'])) {
                    return Ok(true);
                }
                if ["userknownhostsfile", "globalknownhostsfile"].contains(&key.as_str())
                    && words.iter().skip(1).any(|w| w.contains(['%', '$', '`']))
                {
                    return Ok(true);
                }
                if key == "canonicalizehostname" && words.get(1).is_some_and(|v| v != "no") {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    pub fn dto(&self) -> ConfigImport {
        ConfigImport {
            aliases: self.aliases.clone(),
            disclosures: self.disclosures.clone(),
            fingerprint: self.fingerprint.clone(),
        }
    }
}
fn tokenize(line: &str) -> Result<Vec<String>> {
    let mut words = vec![];
    let mut word = String::new();
    let mut quote = false;
    let mut escaped = false;
    for c in line.chars() {
        if escaped {
            word.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            quote = !quote;
        } else if c == '#' && !quote {
            break;
        } else if (c.is_whitespace() || c == '=') && !quote {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        } else {
            word.push(c);
        }
    }
    if quote || escaped {
        return Err(Error::Invalid("unsupported SSH config quoting".into()));
    }
    if !word.is_empty() {
        words.push(word);
    }
    Ok(words)
}

fn bounded_glob(pattern: &Path, cap: usize) -> Result<Vec<PathBuf>> {
    let mut paths = vec![PathBuf::new()];
    for component in pattern.components() {
        use std::path::Component;
        let name = match component {
            Component::Normal(name) => name,
            Component::Prefix(prefix) => {
                for path in &mut paths {
                    path.push(prefix.as_os_str());
                }
                continue;
            }
            Component::RootDir => {
                for path in &mut paths {
                    path.push(std::path::MAIN_SEPARATOR.to_string());
                }
                continue;
            }
            _ => return Err(Error::Invalid("Absolute Include path required".into())),
        };
        let name = name.to_string_lossy();
        let mut next = vec![];
        for base in paths {
            if name.contains(['*', '?', '[']) {
                let pattern = glob::Pattern::new(&name)
                    .map_err(|_| Error::Invalid("Include pattern".into()))?;
                let entries = match fs::read_dir(&base) {
                    Ok(e) => e,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(e.into()),
                };
                for (n, entry) in entries.enumerate() {
                    if n >= 1000 {
                        return Err(Error::Limit("SSH Include directory entries"));
                    }
                    let entry = entry?;
                    if pattern.matches(&entry.file_name().to_string_lossy()) {
                        if next.len() >= cap {
                            return Err(Error::Limit("SSH Include membership"));
                        }
                        next.push(entry.path());
                    }
                }
            } else {
                next.push(base.join(name.as_ref()));
            }
        }
        paths = next;
    }
    paths.retain(|p| p.exists());
    Ok(paths)
}

/// Use the operating system SSH client, never a shell or an untrusted working-directory executable.
pub fn ssh_binary() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
            .join("System32/OpenSSH/ssh.exe")
    }
    #[cfg(unix)]
    {
        PathBuf::from("/usr/bin/ssh")
    }
}
