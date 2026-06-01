# link-zapper — Specification

## 1. Overview

`link-zapper` (formerly `clean-link`) is a lightweight CLI utility that takes "share links" from social media platforms and produces clean, tracking-free versions. It unwraps redirect wrappers (YouTube `/redirect`), resolves shortened URLs (`t.co`), strips tracking parameters, and normalizes hosts.

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
  │     path == "/redirect" && has param "q"
  │     → extract `q`, recursively clean the extracted URL
  │     → works for any platform, no config needed
  │
  ├─ 2. t.co resolution (HTTP redirect follow)
  │     host == "t.co"
  │     → follow redirect chain via reqwest blocking client
  │     → recursively clean the resolved URL
  │     → fallback: return original if network unavailable
  │
  ├─ 3. YouTube URL reconstruction
  │     Gate: config has cleaner = "youtube" OR host in is_youtube_host()
  │     → watch?v=ID → youtu.be/ID
  │     → shorts/ID  → youtu.be/ID
  │     → embed/ID   → youtu.be/ID
  │     → youtu.be/ID (pass through, preserve t=)
  │     → Preserves timestamp (t=, start=)
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
| Redirect unwrapping | `path == "/redirect"` + has `q` param | `clean_url()` early return |
| t.co resolution | `host == "t.co"` | `resolve_redirect()` HTTP client |
| YouTube video reconstruction | host in `is_youtube_host()` | `clean_youtube()` URL builder |

### 3.3 Config-driven behavior (extensible)

| Feature | Config mechanism |
|---------|-----------------|
| Tracking param removal | `tracking_params` / `tracking_prefixes` (general + per-platform) |
| Host normalization | `normalize_host` per-platform |
| Platform domain matching | `domains` per-platform |
| Cleaner activation | `cleaner = "youtube"` (with hardcoded `is_youtube_host()` fallback) |

## 4. Config design

### 4.1 Current schema

```toml
[general]
tracking_params = ["utm_source", "fbclid", ...]
tracking_prefixes = ["utm_"]

[platforms.<name>]
domains = ["domain.com", "www.domain.com"]
tracking_params = ["si", "igshid"]
tracking_prefixes = []
normalize_host = "x.com"
cleaner = "youtube"        # optional, activates built-in handler
```

### 4.2 Planned redesign

Under discussion: replacing the current deny-list approach with an allow-list model:

```toml
[general]
force_https = true
keep_params = []                     # global allow-list (currently empty = deny-all)

[platforms.youtube]
force_https = false
domains = ["youtube.com", "youtu.be", ...]
normalized_domain = "youtube.com"
keep_params = ["v", "t"]             # allow-list: drop everything else
redirect_params = ["redirect"]       # path or query param indicating redirect
redirect_services = []               # shortener domains requiring HTTP resolution

[platforms.x]
domains = ["x.com", "twitter.com"]
normalized_domain = "x.com"
redirect_services = ["t.co"]
```

Key design considerations:
- `keep_params` is an allow-list — anything not in it is removed
- `redirect_params` identifies redirect wrappers (e.g. path `/redirect` with param `q`)
- `redirect_services` lists shortener domains that need HTTP resolution
- Per-platform settings override their global counterpart
- Redirect unwrapping happens *before* param filtering so `q` isn't lost

## 5. Implementation details

### 5.1 YouTube redirect unwrapping

Located in `clean_url()` as an early return:

```
Detect: host in is_youtube_host() && path == "/redirect"
Extract: query param "q" (URL-decoded by the url crate)
Action: return clean_url(extracted_url, config)
```

The url crate's `query_pairs()` automatically percent-decodes values, so `q=https%3A%2F%2Fexample.com` yields `"https://example.com"`.

### 5.2 t.co resolution

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
| `youtube.com/watch?v=ID&t=123` | `youtu.be/ID?t=123` |
| `youtu.be/ID` | `youtu.be/ID` (pass through) |
| `youtube.com/shorts/ID` | `youtu.be/ID` |
| `youtube.com/embed/ID` | `youtu.be/ID` |
| `music.youtube.com/watch?v=ID` | `youtu.be/ID` |
| `youtube.com/redirect?q=URL&v=ID` | `clean(URL)` |
| `m.youtube.com/redirect?q=URL` | `clean(URL)` |
| `youtube-nocookie.com/*` | Same as youtube.com/* |

### 6.2 X (Twitter)

- Normalizes host to `x.com`
- Removes `s=` tracking parameter
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
2. **Hardcoded fallbacks** — YouTube redirect and t.co resolution work even without a config file, ensuring the tool is useful out of the box
3. **Config-first, then fallback** — `cleaner = "youtube"` in config takes precedence; if absent, `is_youtube_host()` provides a safety net
4. **Structural detection** — redirect unwrapping detects `/redirect` + `q` purely by URL structure, not by platform match
5. **No external network in main path** — t.co resolution is the only network call; it's isolated and forgiving (10s timeout, graceful failure)

## 9. Future items

See also "Future Considerations" in this section and the evolving config design (section 4.2).

- [ ] Config allow-list (`keep_params`) replacing deny-list
- [ ] `redirect_params` / `redirect_services` config fields
- [ ] Per-platform `force_https` override
- [ ] Support for additional platforms (TikTok, Reddit, LinkedIn, Threads, Bluesky)
- [ ] Multiple URL processing
- [ ] JSON output mode
