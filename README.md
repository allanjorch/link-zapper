# FINAL DESCRIPTION NOT DONE!

# link-zapper

A lightweight CLI tool that **zaps** tracking parameters, unwraps redirect wrappers, and resolves shortened URLs from social media share links. Works with YouTube, X (Twitter), Instagram, Facebook, and any URL with common tracking tags.

## Usage

```bash
# Zap a URL from the clipboard (auto-copies result back)
link-zapper

# Zap a specific URL
link-zapper "https://www.youtube.com/watch?v=dQw4w9WgXcQ&si=abc123"
# → https://youtu.be/dQw4w9WgXcQ

# Zap a URL and copy the result to clipboard
link-zapper -c "https://twitter.com/user/status/123?s=20"
```

### Clipboard workflow (recommended)

1. Copy a share link from any platform
2. Press your keyboard shortcut bound to `link-zapper`
3. Paste the clean URL — no shell quoting needed

## Installation

### From source

```bash
git clone https://github.com/allanjorch/link-zapper.git
cd link-zapper
cargo build --release
cp target/release/link-zapper ~/.local/bin/
```

### Dependencies

Requires a clipboard utility for the clipboard-first workflow:

- **Wayland**: `wl-clipboard` (`wl-paste` / `wl-copy`)
- **X11**: `xclip`

Works fine without either — pass a URL as an argument and read stdout.

## How it works

### Input

- **No argument**: reads from the system clipboard
- **URL argument**: cleans the given URL
- **`--copy` / `-c`**: forces clipboard copy (useful with a URL argument)

### Zap phases (in order)

1. **Redirect unwrapping** — any URL with path `/redirect` and a `q` parameter is unwrapped to reveal the real destination URL, then re-cleaned. Automatic for all URLs, no config needed.

2. **t.co resolution** — `t.co` shortened URLs are resolved via HTTP redirect. The destination URL is then cleaned through the full pipeline. Requires network access; falls back gracefully.

3. **YouTube URL reconstruction** — converts to `youtu.be/ID`:
   - `youtube.com/watch?v=ID`
   - `youtube.com/shorts/ID`
   - `youtube.com/embed/ID`
   - `music.youtube.com/watch?v=ID`
   - Preserves timestamp (`t=` / `start=`)

4. **General tracking removal** — parameters removed from every URL:
   - `utm_source`, `utm_medium`, `utm_campaign`, `utm_term`, `utm_content`
   - `fbclid`, `gclid`, `dclid`, `msclkid`

5. **Platform-specific removal** — parameters removed when host matches a known platform

6. **Normalization** — upgrades `http://` → `https://`, strips `www.` / `m.`, normalizes `twitter.com` → `x.com`, removes fragments

### Output

- Always prints the clean URL to stdout
- Auto-copies to clipboard when reading from clipboard
- Only copies with `--copy` when a URL is given as argument

## Configuration

On first run, `link-zapper` creates `~/.config/link-zapper/config.toml` with documented defaults:

```toml
[general]
tracking_params = ["utm_source", "fbclid", "gclid"]
tracking_prefixes = ["utm_"]

[platforms.tiktok]
domains = ["tiktok.com", "www.tiktok.com", "m.tiktok.com"]
tracking_params = ["_t"]
```

For YouTube, the config also includes `cleaner = "youtube"` to activate built-in redirect unwrapping and video URL reconstruction for all domains in that section. No rebuild needed — add a new domain and it works immediately.

## Adding a platform

If the platform only needs tracking-parameter removal and host normalization, add it to the config:

```toml
[platforms.reddit]
domains = ["reddit.com", "www.reddit.com", "old.reddit.com"]
tracking_params = ["utm_source", "share_id"]
normalize_host = "reddit.com"
```

If the platform needs custom URL reconstruction (like YouTube), set `cleaner = "youtube"` in its section — or open an issue for a new built-in handler.

## License

MIT

---

Built with [Allan Jorch](https://github.com/allanjorch) and [Claude Code](https://claude.ai) (opencode).
