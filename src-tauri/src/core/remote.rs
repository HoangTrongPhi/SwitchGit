//! Remote URLs and the patterns accounts claim them with.
//!
//! Everything is compared in one normal form, `host/owner/repo` in lower case, so
//! `https://me@github.com/Owner/Repo.git` and `git@github.com:Owner/Repo` are the same remote.

/// `host/owner/repo` (lower case, no scheme, user, port or `.git`). Empty when `url` is not a remote
/// on a host (a local path).
pub fn normalize(url: &str) -> String {
    let mut s = url.trim().trim_end_matches('/').to_string();
    let had_scheme = s.contains("://");
    if let Some(i) = s.find("://") {
        s = s[i + 3..].to_string();
    }
    // user@ (and user:password@) in front of the host.
    let first_slash = s.find('/').unwrap_or(s.len());
    if let Some(at) = s[..first_slash].rfind('@') {
        s = s[at + 1..].to_string();
    }
    // scp form `host:owner/repo`; with a scheme the colon is a port.
    let host_end = s.find('/').unwrap_or(s.len());
    if let Some(colon) = s[..host_end].find(':') {
        let after = &s[colon + 1..host_end];
        let is_port = had_scheme && !after.is_empty() && after.chars().all(|c| c.is_ascii_digit());
        s = if is_port { format!("{}{}", &s[..colon], &s[host_end..]) } else { format!("{}/{}", &s[..colon], &s[colon + 1..]) };
    }
    let s = s.trim_end_matches('/');
    let s = s.strip_suffix(".git").unwrap_or(s);
    // A drive letter or a bare word is a local path, not a host.
    let host = s.split('/').next().unwrap_or("");
    if host.starts_with('.') || host.contains('\\') || (!host.contains('.') && host != "localhost") {
        return String::new();
    }
    s.to_lowercase()
}

/// A pattern as the user typed it (`github.com/Owner`, `github.com/Owner/*`, a whole URL) in the
/// same normal form, without the trailing wildcard.
pub fn normalize_pattern(pattern: &str) -> String {
    let p = pattern.trim().trim_end_matches("**").trim_end_matches('*').trim_end_matches('/');
    normalize(p)
}

/// How specific the match is (length of the pattern); `None` when `url` is not under `pattern`.
pub fn match_len(pattern: &str, url: &str) -> Option<usize> {
    let (p, u) = (normalize_pattern(pattern), normalize(url));
    if p.is_empty() || u.is_empty() {
        return None;
    }
    (u == p || u.starts_with(&format!("{p}/"))).then_some(p.len())
}

/// Host of a pattern (`github.com`), for per-host credential settings.
pub fn host_of(pattern: &str) -> Option<String> {
    let p = normalize_pattern(pattern);
    p.split('/').next().filter(|h| !h.is_empty()).map(str::to_string)
}

/// The `hasconfig:remote.*.url:` globs that select repositories under `pattern`, one per URL form
/// git remotes are written in. Git matches these case-sensitively, so the owner keeps the case the
/// user typed.
pub fn include_globs(pattern: &str) -> Vec<String> {
    let lower = normalize_pattern(pattern);
    if lower.is_empty() {
        return Vec::new();
    }
    // Keep the user's case for the path part: take it from the typed pattern when it lines up.
    let typed = pattern.trim().trim_end_matches("**").trim_end_matches('*').trim_end_matches('/');
    let typed = typed.strip_suffix(".git").unwrap_or(typed);
    let cased = if typed.to_lowercase().ends_with(&lower) { typed[typed.len() - lower.len()..].to_string() } else { lower.clone() };
    let (host, path) = match cased.split_once('/') {
        Some((h, p)) => (h.to_lowercase(), format!("{p}/")),
        None => (cased.to_lowercase(), String::new()),
    };
    vec![
        format!("https://{host}/{path}**"),
        format!("https://*@{host}/{path}**"),
        format!("git@{host}:{path}**"),
        format!("ssh://git@{host}/{path}**"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_url_form_normalizes_alike() {
        for url in [
            "https://github.com/HoangTrongPhi/Abyss.git",
            "https://phi@github.com/HoangTrongPhi/Abyss",
            "https://phi:token@github.com/HoangTrongPhi/Abyss/",
            "git@github.com:HoangTrongPhi/Abyss.git",
            "ssh://git@github.com/HoangTrongPhi/Abyss.git",
            "ssh://git@github.com:22/HoangTrongPhi/Abyss.git",
        ] {
            assert_eq!(normalize(url), "github.com/hoangtrongphi/abyss", "{url}");
        }
        assert_eq!(normalize("D:/repos/local.git"), "");
        assert_eq!(normalize("../sibling"), "");
        assert_eq!(normalize("https://gitlab.example.com:8443/team/sub/repo.git"), "gitlab.example.com/team/sub/repo");
    }

    #[test]
    fn patterns_match_by_prefix_on_whole_segments() {
        let url = "git@github.com:MageTech/game-client.git";
        assert_eq!(match_len("github.com/MageTech", url), Some("github.com/magetech".len()));
        assert_eq!(match_len("github.com/magetech/*", url), Some("github.com/magetech".len()));
        assert_eq!(match_len("https://github.com/MageTech/game-client.git", url), Some("github.com/magetech/game-client".len()));
        assert!(match_len("github.com", url).is_some(), "a whole host");
        assert_eq!(match_len("github.com/Mage", url), None, "not a whole segment");
        assert_eq!(match_len("gitlab.com/MageTech", url), None);
        assert_eq!(match_len("", url), None);
        assert_eq!(match_len("github.com/MageTech", "D:/local"), None);
    }

    #[test]
    fn include_globs_cover_https_and_ssh() {
        assert_eq!(
            include_globs("github.com/HoangTrongPhi/*"),
            ["https://github.com/HoangTrongPhi/**", "https://*@github.com/HoangTrongPhi/**", "git@github.com:HoangTrongPhi/**", "ssh://git@github.com/HoangTrongPhi/**"]
        );
        assert_eq!(include_globs("https://GitHub.com")[0], "https://github.com/**");
        assert!(include_globs("not a remote").is_empty());
        assert_eq!(host_of("git@github.com:MageTech/x.git").as_deref(), Some("github.com"));
    }
}
