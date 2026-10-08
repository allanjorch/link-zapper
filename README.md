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

1. **Redirect unwrapping** — a URL whose path is `/redirect`, `/url`, or `/l.php` and whose `q`, `u`, or `url` parameter is an http(s) link is unwrapped, then cleaned again. This runs for every host. No config needed.

2. **Shortener resolution** — known shorteners (`t.co`, `bit.ly`, `vm.tiktok.com`, `lnkd.in`, `facebook.com/share/…`, and others) are followed over HTTP. The destination is then cleaned. Requires network access; if the lookup fails, or lands on a login page, the original link is kept and still stripped of tracking.

3. **YouTube URL reconstruction** — converts to `youtu.be/ID`:
   - `youtube.com/watch?v=ID`
   - `youtube.com/shorts/ID`
   - `youtube.com/embed/ID`
   - `music.youtube.com/watch?v=ID`
   - Preserves a timestamp (`t=` / `start=`) and, when the video is in a playlist, `list` and `index`

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

YouTube video links are rewritten in code for `youtube.com`, `youtu.be`, `music.youtube.com`, and `youtube-nocookie.com`. That rewrite keeps a timestamp and playlist position (`list`, `index`), and drops every other parameter, including the share tokens `si` and `is`. The `[platforms.youtube]` block lists those same tokens so they are also removed from pages that are not a single video, such as a channel or a playlist.

## Adding a platform

If the platform only needs tracking-parameter removal and host normalization, add it to the config:

```toml
[platforms.reddit]
domains = ["reddit.com", "www.reddit.com", "old.reddit.com"]
tracking_params = ["utm_source", "share_id"]
normalize_host = "reddit.com"
```

YouTube's `youtu.be` form is built in. A platform that needs its own URL shape needs a change in the program.

## License

MIT

---

Built with [Allan Jorch](https://github.com/allanjorch), [Claude Code](https://claude.ai) (opencode), and [Grok](https://x.ai).
