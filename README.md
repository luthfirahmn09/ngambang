# ngambang 🫧

**A tiny floating chat window for the Claude already living on your Mac, for people who are tired of alt-tabbing to ask one question.**

You know the moment. You're deep in something. You need to ask Claude *one thing*. So you leave your app, find the terminal, lose your place, forget the question, and return forty minutes later having reorganized your Downloads folder.

That, friends, is why *ngambang* exists. (It means "floating" in Indonesian. It's also how your focus feels most days.)

One tiny Mac app, about 7 MB. (That's megabytes. Not gigabytes. Go ahead, check again, we'll wait.)

No Electron. No subscription. No login. No onboarding carousel. No "Introducing a fresh new look!" modal. Just a bubble that floats above everything and talks to the `claude` you already have installed.

Written in Rust with [Tauri](https://tauri.app) and a single HTML file, because we have self-control. Sometimes.

---

## What it does

- **Floats on top of everything.** Built to hover above your other windows, across all Spaces, including fullscreen apps, via a non-activating panel. (Whether macOS lets it is between you and macOS.)
- **Minimizes into a bubble.** Hit `–` and it shrinks into a small orange circle. Click the circle, the chat comes back. Peekaboo.
- **`⌥Space` from anywhere** shows or hides it. No Dock icon, just a quiet little menu bar item.
- **Talks to your local Claude.** It runs one long-lived `claude -p` process in streaming mode, so replies appear word by word and the second message isn't paying the startup tax again. Same session, same brain, no new account.
- **Bring your own brain.** Click ⚙ and pick the provider: your local **Claude Code** (default), the **Anthropic API**, or any **OpenAI-compatible** endpoint (OpenAI, OpenRouter, Groq, Gemini, DeepSeek, Ollama, LM Studio...). Presets fill in the base URL; you type the model name, because model names go stale faster than milk.
- **Typing dots.** Because staring at a blank box and wondering whether it's thinking or dead is a lifestyle we decided to retire.
- **Screen mode.** Two buttons next to the input:
  - **▣** drag a region of your screen and it's attached to your next message.
  - **◉** turns on *auto-screen*: every message you send includes a screenshot of the whole screen. Handy for "what does this error mean" or "answer the question I'm looking at".

---

## Install

Needs macOS and [Rust](https://rustup.rs). For the default provider, [Claude Code](https://claude.com/claude-code) must be installed and logged in. If you'd rather use an API key (or a local model), skip that and set it up in ⚙ Settings.

```sh
git clone https://github.com/luthfirahmn09/ngambang.git
cd ngambang
./build_app.sh            # builds and installs to ~/Applications
./build_app.sh --system   # ...or to /Applications
```

First build installs `tauri-cli` and compiles half the internet, so go refill your coffee.

If macOS gets dramatic on first launch (the app is not blessed by Apple), right-click → Open, once. Peace follows.

For screenshots, allow **Screen Recording** in System Settings → Privacy & Security, then reopen the app. The permission is tied to the installed app, so granting it to a `cargo run` build won't carry over.

---

## Your screen is your business (the important part, seriously)

Screenshots go to whichever provider you picked, the same as anything else you'd paste into their chat box. **Auto-screen sends your *entire* screen with every message.** Your banking tab, your 47 unread DMs, the tab you swore you'd close. Leave ◉ off unless you mean it. Use ▣ when you only want to share one rectangle.

Images are downscaled to 1600 px on the longest side before sending. That's for payload size, not privacy.

---

## Honesty section: what it can't do

- **API keys live in a plain file** (`~/Library/Application Support/dev.luthfi.ngambang/settings.json`, mode 0600), not the Keychain. Fine for a toy, not for your production master key. Use a scoped key.
- **API providers are plain chat.** No tools, no file access, no web. Only the Claude Code provider can touch your machine, and even then it can't ask permission (see below).
- **Non-Claude providers need vision-capable models** if you want screenshots to work. A text-only model will politely complain.
This is a **minimal** chat bubble, not a replacement for your terminal.

- **No permission prompts.** With the Claude Code provider it runs `claude -p`, which can't ask you interactively. Tools that need approval will simply be denied. It's a chatty bubble, not a coding agent with the keys to your disk.
- **Runs from your home folder**, not from a project. It doesn't know which repo you're staring at.
- **Markdown is tiny**: code blocks, inline code, and bold. No tables, no lists rendering, no syntax highlighting.
- **One conversation at a time.** `✎` starts a new one and kills the old process. History isn't saved between launches.
- **Resizing** a borderless window by its edges may or may not work on your macOS version.
- **Fullscreen overlay** relies on macOS window-level behavior that Apple is free to change on a whim. Reports from real-world setups welcome.
- **macOS only.** Linux and Windows are somebody else's problem.

If you need more than that, well, your terminal is still out there. It isn't going anywhere. Believe us, we checked.

---

## For the tinkerers

```sh
cargo tauri build     # release bundle
cargo run             # quick dev run (no bundle)
```

| File | What it does |
|---|---|
| `src/api.rs` | Anthropic and OpenAI-compatible streaming, so Claude isn't the only voice in the room |
| `src/main.rs` | Window magic (NSPanel conversion, all Spaces, fullscreen), the long-lived `claude` process, screenshots, tray, `⌥Space` |
| `ui/index.html` | The whole UI. One file, no framework, no `node_modules` black hole |
| `tauri.conf.json` | Window flags: borderless, transparent, always on top |
| `build_app.sh` | Builds the `.app` and installs it |

---

*ngambang: because sometimes you just want to ask one question, not rearrange your entire desktop.*
