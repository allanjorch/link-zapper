# link-zapper — Specification

## 1. Overview

`link-zapper` is a lightweight CLI utility that takes share links from social media platforms and produces clean, tracking-free versions. It unwraps redirect wrappers, resolves shortened URLs, strips tracking parameters, and normalizes hosts.

## 2. Core Purpose

- Remove tracking and analytics parameters from URLs
- Unwrap redirect-wrapper URLs to reveal the real destination
- Resolve shortened URLs to their final destination
- Canonicalize links to their simplest reliable form
- Provide a fast, clipboard-centric workflow

## 3. Architecture

### 3.1 Processing pipeline (in order)

```
Input URL
  │
  ├─ 1. Redirect unwrapping (structural detection)
  │     path is /redirect, /url, or /l.php, on any host
  │     → extract q, u, or url when the value is an http(s) URL
  │     → recursively clean the extracted URL
  │     → works for any platform, no config needed
  │
  ├─ 2. Shortener resolution (HTTP redirect follow)
  │     known shortener host, or a share path such as facebook.com/share/
  │     → follow redirect chain via reqwest blocking client
  │     → recursively clean the resolved URL
  │     → fallback: return original if network unavailable or the result is a login page
  │
  ├─ 3. YouTube URL reconstruction
  │     Gate: host in is_youtube_host()
  │     → watch?v=ID → youtu.be/ID
  │     → shorts/ID  → youtu.be/ID
  │     → embed/ID   → youtu.be/ID
  │     → youtu.be/ID (pass through)
  │     → Preserves timestamp (t=, start=) and playlist (list=, index=)
  │
  ├─ 4. General tracking removal (config-driven)
  │     → utm_source, fbclid, gclid, dclid, msclkid, etc.
  │     → any param starting with utm_
  │
  ├─ 5. Platform-specific tracking removal (config-driven)
  │     → matched via find_platform() by host
  │
  ├─ 6. Fragment removal
  │
  └─ 7. Host normalization
        → www. / m. prefix stripped
        → twitter.com → x.com (if normalize_host set)
        → http → https upgrade
```

### 3.2 Detectable behavior (no config needed)

| Feature | Detection | Implementation |
|---------|-----------|---------------|
| Redirect unwrapping | path `/redirect`, `/url`, or `/l.php` plus an http(s) `q`, `u`, or `url` | `clean_url()` early return |
| Shortener resolution | known shortener host, or a share path such as `/share/` | `resolve_redirect()` HTTP client |
| YouTube video reconstruction | host in `is_youtube_host()` | `clean_youtube()` URL builder |

### 3.3 Config-driven behavior

| Feature | Config mechanism |
|---------|-----------------|
| Tracking param removal | `tracking_params` / `tracking_prefixes` (general + per-platform) |
| Host normalization | `normalize_host` per-platform |
| Platform domain matching | `domains` per-platform |

## 4. Configuration

```toml
[general]
tracking_params = ["utm_source", "fbclid", ...]
tracking_prefixes = ["utm_"]

[platforms.<name>]
domains = ["domain.com", "www.domain.com"]
tracking_params = ["si", "is"]
tracking_prefixes = []
normalize_host = "x.com"
```

## 5. Implementation details

### 5.1 Redirect unwrapping

Located in `clean_url()` as an early return:

```
Detect: path is /redirect, /url, or /l.php, on any host
Extract: query param q, u, or url, when the value is an http(s) URL
Action: return clean_url(extracted_url, config)
```

The url crate's `query_pairs()` automatically percent-decodes values, so `q=https%3A%2F%2Fexample.com` yields `"https://example.com"`.

### 5.2 Shortener resolution

```
Client: reqwest::blocking::Client
Method: HEAD request, fallback to GET if HEAD fails
Policy: follow up to 10 redirects
Timeout: 10 seconds
Action: return clean_url(final_url, config)
```

### 5.3 Config loading

```
Config::load():
  1. Look for ~/.config/link-zapper/config.toml
  2. If found and valid TOML → return parsed config (missing fields default via serde)
  3. If not found → write default config file, then return Config::default()
  4. If found but invalid TOML → fall through to Config::default()

NOTE: No merging. Config::default() is only used if no valid TOML file exists.
```

### 5.4 Platform matching

```rust
find_platform(host, config) -> Option<(&str, &PlatformConfig)>
  → exact host match against each platform's domain list
  → returns (section_name, config) on first match
```

## 6. Platform-specific behavior

### 6.1 YouTube

| Format | Output |
|--------|--------|
| `youtube.com/watch?v=ID&list=PL…&index=2&t=123` | `youtu.be/ID?list=PL…&index=2&t=123` |
| `youtube.com/watch?v=ID&t=123` | `youtu.be/ID?t=123` |
| `youtu.be/ID` | `youtu.be/ID` (pass through) |
| `youtube.com/shorts/ID` | `youtu.be/ID` |
| `youtube.com/embed/ID` | `youtu.be/ID` |
| `music.youtube.com/watch?v=ID` | `youtu.be/ID` |
| `youtube.com/redirect?q=URL&v=ID` | `clean(URL)` |
| `m.youtube.com/redirect?q=URL` | `clean(URL)` |
| `youtube-nocookie.com/*` | Same as youtube.com/* |

Share tokens `si` and `is` are removed. On a video link the rewrite also keeps `list` and `index` when the video belongs to a playlist. On other YouTube pages, such as a channel or a playlist page, those tokens are removed as tracking parameters and the rest of the URL stays.

### 6.2 X (Twitter)

- Normalizes host to `x.com`
- Removes the `s` and `t` tracking parameters. `t` on an X link is a share token, not a timestamp
- Supports `x.com`, `twitter.com`, `m.x.com`, `m.twitter.com`

### 6.3 Instagram

- Removes `igshid=` and `igsh=` tracking parameters
- Strips `www.` and `m.` prefixes
- Preserves full path (`/p/...`, `/reel/...`, etc.)

### 6.4 Facebook

- Removes `mibextid=` and `__tn__=` tracking parameters
- Strips `m.` prefix
- Supports `facebook.com` and `fb.com`

## 7. CLI interface

```
link-zapper [OPTIONS] [URL]

ARGS:
    <URL>    URL to zap (reads from clipboard if omitted)

OPTIONS:
    -c, --copy    Copy the zapped URL to clipboard
    -h, --help    Print help information

Return codes:
    0 — success
    1 — no input and clipboard empty/inaccessible
```

## 8. Key design decisions

1. **Clipboard-first workflow** — no args reads clipboard, no `--copy` flag needed because clipboard is the primary input channel
2. **Hardcoded fallbacks** — YouTube rewriting and shortener resolution work even without a config file, so the tool is useful out of the box
3. **YouTube is recognized by host** — `youtube.com`, `youtu.be`, `music.youtube.com`, and `youtube-nocookie.com` are rewritten in code. A video link keeps its timestamp and, when it belongs to a playlist, `list` and `index`. The YouTube config block lists the share tokens `si` and `is` so they are also removed from pages that are not a single video
4. **Structural detection** — redirect unwrapping detects `/redirect`, `/url`, or `/l.php` plus an http(s) destination, not a platform match
5. **No external network in the main path** — only shortener resolution uses the network. It times out after 10 seconds and keeps the original link if the lookup fails

Built with [Allan Jorch](https://github.com/allanjorch), [Claude Code](https://claude.ai) (opencode), and [Grok](https://x.ai).
