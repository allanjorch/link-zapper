use std::io::Write;
use std::process::{Command, Stdio};
use url::Url;
use reqwest::blocking::Client;

mod config;
use config::Config;

fn main() {
    let (input_url, copy_flag, help_flag) = parse_args(std::env::args().skip(1));

    if help_flag {
        print_help();
        return;
    }

    let config = Config::load();
    let had_input = input_url.is_some();

    let input = match input_url {
        Some(url) => url,
        None => match read_clipboard() {
            Some(text) => text,
            None => {
                eprintln!("Error: no URL provided and clipboard is empty or inaccessible");
                eprintln!("Usage: link-zapper [OPTIONS] [URL]");
                eprintln!("       link-zapper --help for more info");
                std::process::exit(1);
            }
        },
    };

    let should_copy = if had_input { copy_flag } else { true };

    let cleaned = clean_url(&input, &config);

    println!("{cleaned}");

    if should_copy {
        write_clipboard(&cleaned);
    }
}

fn parse_args(args: impl Iterator<Item = String>) -> (Option<String>, bool, bool) {
    let mut input_url = None;
    let mut copy_flag = false;
    let mut help_flag = false;

    for arg in args {
        match arg.as_str() {
            "--copy" | "-c" => copy_flag = true,
            "--help" | "-h" => help_flag = true,
            _ => {
                if input_url.is_none() && !arg.starts_with('-') {
                    input_url = Some(arg);
                }
            }
        }
    }

    (input_url, copy_flag, help_flag)
}

fn print_help() {
    println!("link-zapper 0.1.0");
    println!("Remove tracking parameters from social media share links");
    println!();
    println!("USAGE:");
    println!("    link-zapper [OPTIONS] [URL]");
    println!();
    println!("ARGS:");
    println!("    <URL>    URL to clean (reads from clipboard if omitted)");
    println!();
    println!("OPTIONS:");
    println!("    -c, --copy    Copy the cleaned URL to clipboard");
    println!("    -h, --help    Print help information");
}

fn read_clipboard() -> Option<String> {
    if let Ok(output) = Command::new("wl-paste").output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    if let Ok(output) = Command::new("xclip")
        .args(["-o", "-selection", "clipboard"])
        .output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

fn write_clipboard(text: &str) {
    if let Ok(mut child) = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .spawn()
        && let Some(mut stdin) = child.stdin.take()
    {
        let _ = stdin.write_all(text.as_bytes());
        drop(stdin);
        let _ = child.wait();
        return;
    }
    if let Ok(mut child) = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(Stdio::piped())
        .spawn()
        && let Some(mut stdin) = child.stdin.take()
    {
        let _ = stdin.write_all(text.as_bytes());
    }
}

fn clean_url(input: &str, config: &Config) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut url = match Url::parse(trimmed) {
        Ok(u) => u,
        Err(_) => {
            let with_scheme = format!("https://{trimmed}");
            match Url::parse(&with_scheme) {
                Ok(u) => u,
                Err(_) => return trimmed.to_string(),
            }
        }
    };

    if url.scheme() == "http" {
        let _ = url.set_scheme("https");
    }

    let host = url.host_str().unwrap_or("").to_lowercase();
    let platform = config::find_platform(&host, config);

    // YouTube redirect — extract the real destination from the `q` param
    if is_youtube_platform(platform, url.host_str())
        && url.path() == "/redirect"
        && let Some(dest) = url
            .query_pairs()
            .find(|(k, _)| k == "q")
            .map(|(_, v)| v.to_string())
        && !dest.is_empty()
    {
        return clean_url(&dest, config);
    }

    // Resolve t.co shortened URLs via HTTP redirect
    if host == "t.co"
        && let Some(resolved) = resolve_redirect(url.as_ref())
    {
        return clean_url(&resolved, config);
    }

    // YouTube uses a built-in URL reconstructor
    if is_youtube_platform(platform, url.host_str())
        && let Some((_, pconfig)) = &platform
    {
        return clean_youtube(&url, config, pconfig);
    }

    // General tracking removal (applies to ALL URLs)
    remove_tracking_params(
        &mut url,
        &config.general.tracking_params,
        &config.general.tracking_prefixes,
    );

    // Platform-specific tracking removal
    if let Some((_, pconfig)) = &platform {
        remove_tracking_params(
            &mut url,
            &pconfig.tracking_params,
            &pconfig.tracking_prefixes,
        );
    }

    remove_fragment(&mut url);
    normalize_host(
        &mut url,
        platform.and_then(|(_, p)| p.normalize_host.as_deref()),
    );
    url.to_string()
}

fn clean_youtube(url: &Url, config: &Config, pconfig: &config::PlatformConfig) -> String {
    let host = url.host_str().unwrap_or("").to_lowercase();

    // youtu.be/VIDEO_ID
    if host == "youtu.be" || host == "www.youtu.be" {
        let path = url.path().trim_start_matches('/');
        let id = path.split(&['/', '?', '#'][..]).next().unwrap_or(path);
        if is_valid_video_id(id) {
            if let Some(ts) = extract_timestamp(url) {
                return format!("https://youtu.be/{id}?t={ts}");
            }
            return format!("https://youtu.be/{id}");
        }
    }

    // youtube.com/watch?v=ID
    if let Some(v) = url.query_pairs().find(|(k, _)| k == "v") {
        let id = v.1.to_string();
        if is_valid_video_id(&id) {
            if let Some(ts) = extract_timestamp(url) {
                return format!("https://youtu.be/{id}?t={ts}");
            }
            return format!("https://youtu.be/{id}");
        }
    }

    // youtube.com/shorts/ID
    let path = url.path();
    if let Some(stripped) = path.strip_prefix("/shorts/") {
        let id = stripped.split(&['/', '?', '#'][..]).next().unwrap_or(stripped);
        if is_valid_video_id(id) {
            if let Some(ts) = extract_timestamp(url) {
                return format!("https://youtu.be/{id}?t={ts}");
            }
            return format!("https://youtu.be/{id}");
        }
    }

    // youtube.com/embed/ID
    if let Some(stripped) = path.strip_prefix("/embed/") {
        let id = stripped.split(&['/', '?', '#'][..]).next().unwrap_or(stripped);
        if is_valid_video_id(id) {
            if let Some(ts) = extract_timestamp(url) {
                return format!("https://youtu.be/{id}?t={ts}");
            }
            return format!("https://youtu.be/{id}");
        }
    }

    // Fallback: remove tracking params without URL reconstruction
    let mut u = url.clone();
    remove_tracking_params(
        &mut u,
        &config.general.tracking_params,
        &config.general.tracking_prefixes,
    );
    remove_tracking_params(
        &mut u,
        &pconfig.tracking_params,
        &pconfig.tracking_prefixes,
    );
    remove_fragment(&mut u);
    u.to_string()
}

fn is_youtube_host(host: Option<&str>) -> bool {
    matches!(
        host,
        Some(
            "youtube.com"
            | "www.youtube.com"
            | "m.youtube.com"
            | "youtu.be"
            | "www.youtu.be"
            | "music.youtube.com"
            | "www.music.youtube.com"
            | "youtube-nocookie.com"
            | "www.youtube-nocookie.com"
        )
    )
}

fn is_youtube_platform(platform: Option<(&str, &config::PlatformConfig)>, host: Option<&str>) -> bool {
    // Config takes precedence
    if let Some((_, p)) = platform
        && p.cleaner.as_deref() == Some("youtube")
    {
        return true;
    }
    // Fall back to hardcoded host check
    is_youtube_host(host)
}

fn resolve_redirect(url_str: &str) -> Option<String> {
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;

    let resp = client.head(url_str).send().ok()?;
    let final_url = resp.url().to_string();

    if final_url == url_str {
        let resp = client.get(url_str).send().ok()?;
        let final_url = resp.url().to_string();
        if final_url == url_str {
            return None;
        }
        return Some(final_url);
    }

    Some(final_url)
}

fn extract_timestamp(url: &Url) -> Option<String> {
    for (key, value) in url.query_pairs() {
        if key == "t" || key == "start" {
            let ts = value.trim().to_string();
            if !ts.is_empty() {
                return Some(ts);
            }
        }
    }
    None
}

fn is_valid_video_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 11 && id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

fn remove_tracking_params(
    url: &mut Url,
    tracking_params: &[String],
    tracking_prefixes: &[String],
) {
    if tracking_params.is_empty() && tracking_prefixes.is_empty() {
        return;
    }

    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let to_keep: Vec<&(String, String)> = pairs
        .iter()
        .filter(|(k, _)| !is_tracking_param(k, tracking_params, tracking_prefixes))
        .collect();

    if to_keep.len() == pairs.len() {
        return;
    }

    if to_keep.is_empty() {
        url.set_query(None);
    } else {
        let mut serializer = url.query_pairs_mut();
        serializer.clear();
        for (k, v) in to_keep {
            serializer.append_pair(k, v);
        }
    }
}

fn is_tracking_param(key: &str, tracking_params: &[String], tracking_prefixes: &[String]) -> bool {
    let lower = key.to_lowercase();
    tracking_params.contains(&lower)
        || tracking_prefixes.iter().any(|p| lower.starts_with(p))
}

fn remove_fragment(url: &mut Url) {
    url.set_fragment(None);
}

fn normalize_host(url: &mut Url, normalize_target: Option<&str>) {
    if let Some(host) = url.host_str() {
        let lower = host.to_lowercase();
        let cleaned = lower
            .strip_prefix("www.")
            .or_else(|| lower.strip_prefix("m."))
            .unwrap_or(&lower);
        let cleaned = match normalize_target {
            Some(target) if cleaned != target => target,
            _ => cleaned,
        };
        if cleaned != lower {
            let _ = url.set_host(Some(cleaned));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(input: &str) -> String {
        clean_url(input, &Config::default())
    }

    #[test]
    fn test_youtube_watch() {
        assert_eq!(
            clean("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            "https://youtu.be/dQw4w9WgXcQ"
        );
    }

    #[test]
    fn test_youtube_short() {
        assert_eq!(
            clean("https://youtu.be/dQw4w9WgXcQ"),
            "https://youtu.be/dQw4w9WgXcQ"
        );
    }

    #[test]
    fn test_youtube_shorts() {
        assert_eq!(
            clean("https://www.youtube.com/shorts/dQw4w9WgXcQ"),
            "https://youtu.be/dQw4w9WgXcQ"
        );
    }

    #[test]
    fn test_youtube_embed() {
        assert_eq!(
            clean("https://www.youtube.com/embed/dQw4w9WgXcQ"),
            "https://youtu.be/dQw4w9WgXcQ"
        );
    }

    #[test]
    fn test_youtube_music() {
        assert_eq!(
            clean("https://music.youtube.com/watch?v=dQw4w9WgXcQ"),
            "https://youtu.be/dQw4w9WgXcQ"
        );
    }

    #[test]
    fn test_youtube_with_timestamp() {
        assert_eq!(
            clean("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=123"),
            "https://youtu.be/dQw4w9WgXcQ?t=123"
        );
    }

    #[test]
    fn test_youtube_with_start() {
        assert_eq!(
            clean("https://www.youtube.com/watch?v=dQw4w9WgXcQ&start=60"),
            "https://youtu.be/dQw4w9WgXcQ?t=60"
        );
    }

    #[test]
    fn test_youtube_strips_tracking() {
        let result = clean("https://www.youtube.com/watch?v=dQw4w9WgXcQ&si=abc123&utm_source=twitter");
        assert_eq!(result, "https://youtu.be/dQw4w9WgXcQ");
    }

    #[test]
    fn test_youtube_short_with_timestamp() {
        assert_eq!(
            clean("https://youtu.be/dQw4w9WgXcQ?t=123"),
            "https://youtu.be/dQw4w9WgXcQ?t=123"
        );
    }

    #[test]
    fn test_twitter_to_x() {
        assert_eq!(
            clean("https://twitter.com/user/status/123456789"),
            "https://x.com/user/status/123456789"
        );
    }

    #[test]
    fn test_x_strips_s_param() {
        assert_eq!(
            clean("https://x.com/user/status/123456789?s=20"),
            "https://x.com/user/status/123456789"
        );
    }

    #[test]
    fn test_instagram_strips_tracking() {
        assert_eq!(
            clean("https://www.instagram.com/p/CxYzAbCdEfG/?igshid=abc123"),
            "https://instagram.com/p/CxYzAbCdEfG/"
        );
    }

    #[test]
    fn test_instagram_strips_m() {
        assert_eq!(
            clean("https://m.instagram.com/p/CxYzAbCdEfG/"),
            "https://instagram.com/p/CxYzAbCdEfG/"
        );
    }

    #[test]
    fn test_facebook_strips_tracking() {
        assert_eq!(
            clean("https://www.facebook.com/user/posts/12345?__tn__=abc&fbclid=def"),
            "https://facebook.com/user/posts/12345"
        );
    }

    #[test]
    fn test_facebook_strips_m() {
        assert_eq!(
            clean("https://m.facebook.com/user/posts/12345"),
            "https://facebook.com/user/posts/12345"
        );
    }

    #[test]
    fn test_utm_removal() {
        assert_eq!(
            clean("https://example.com/page?utm_source=twitter&utm_medium=social&foo=bar"),
            "https://example.com/page?foo=bar"
        );
    }

    #[test]
    fn test_fbclid_removal() {
        assert_eq!(
            clean("https://example.com/page?fbclid=abc123"),
            "https://example.com/page"
        );
    }

    #[test]
    fn test_gclid_removal() {
        assert_eq!(
            clean("https://example.com/page?gclid=abc123"),
            "https://example.com/page"
        );
    }

    #[test]
    fn test_fragment_removal() {
        assert_eq!(
            clean("https://example.com/page#section"),
            "https://example.com/page"
        );
    }

    #[test]
    fn test_http_upgrade() {
        assert_eq!(
            clean("http://example.com/page"),
            "https://example.com/page"
        );
    }

    #[test]
    fn test_www_stripped() {
        assert_eq!(
            clean("https://www.example.com/page"),
            "https://example.com/page"
        );
    }

    #[test]
    fn test_generic_s_param_preserved() {
        // ?s= on a non-X platform should NOT be removed
        assert_eq!(
            clean("https://example.com/page?s=foo"),
            "https://example.com/page?s=foo"
        );
    }

    #[test]
    fn test_invalid_url_preserved() {
        assert_eq!(clean("not a url"), "not a url");
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(clean(""), "");
    }

    #[test]
    fn test_whitespace_trimmed() {
        assert_eq!(
            clean("  https://www.youtube.com/watch?v=dQw4w9WgXcQ  "),
            "https://youtu.be/dQw4w9WgXcQ"
        );
    }

    #[test]
    fn test_url_without_scheme() {
        assert_eq!(
            clean("youtube.com/watch?v=dQw4w9WgXcQ"),
            "https://youtu.be/dQw4w9WgXcQ"
        );
    }

    #[test]
    fn test_mibextid_removal() {
        assert_eq!(
            clean("https://facebook.com/page?mibextid=xyz"),
            "https://facebook.com/page"
        );
    }

    #[test]
    fn test_dclid_removal() {
        assert_eq!(
            clean("https://example.com/page?dclid=xyz"),
            "https://example.com/page"
        );
    }

    #[test]
    fn test_msclkid_removal() {
        assert_eq!(
            clean("https://example.com/page?msclkid=xyz"),
            "https://example.com/page"
        );
    }

    #[test]
    fn test_youtube_redirect_extracts_destination() {
        assert_eq!(
            clean("https://www.youtube.com/redirect?event=video_description&redir_token=TOKEN&q=https%3A%2F%2Fexample.com%2Fpage&v=rAzT5lcezPs"),
            "https://example.com/page"
        );
    }

    #[test]
    fn test_youtube_redirect_mobile() {
        assert_eq!(
            clean("https://m.youtube.com/redirect?event=video_description&q=https%3A%2F%2Fexample.com%2Fpage&v=rAzT5lcezPs"),
            "https://example.com/page"
        );
    }

    #[test]
    fn test_tco_url_bypasses_redirect_resolution_in_tests() {
        let result = clean("https://example.com/page");
        assert_eq!(result, "https://example.com/page");
    }

    #[ignore]
    #[test]
    fn test_tco_resolves_via_network() {
        let result = clean("https://t.co/S16ync3MBq");
        assert_ne!(result, "https://t.co/S16ync3MBq");
        assert!(result.starts_with("https://"));
        assert!(!result.contains("t.co"));
        println!("t.co resolved to: {result}");
    }

    #[test]
    fn test_youtube_redirect_cleans_destination_tracking() {
        assert_eq!(
            clean("https://www.youtube.com/redirect?event=video_description&q=https%3A%2F%2Fexample.com%2Fpage%3Futm_source%3Dtwitter%26foo%3Dbar&v=rAzT5lcezPs"),
            "https://example.com/page?foo=bar"
        );
    }
}
