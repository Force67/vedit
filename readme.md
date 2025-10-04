# Minimal Text Editor (Rust) — with first-class debugging

> Fast. Native. No jank. A tiny editor where the **debugger is a first-class citizen**—not a bolted-on afterthought.

## Why?!?

I want the Visual Studio vibe (hit F5, set breakpoints, hover to inspect, just debug) **without** the VS bloat or separate processes. On Linux especially, I’m tired of juggling terminals and glue scripts just to step through code. I want to click “run,” start debugging from the editor, and get on with it.

Also: editors should be **fast**. This one is written in **Rust** with **iced** for the GUI, because latency kills flow. My day-to-day languages are **C++** and **Rust**, so the editor should feel great for those—**no fuss**.

## Goal?

- Start and control debugging **from the editor**.
- Set breakpoints inline.
- Hover variables to see values and state.
- Keep the UI snappy and minimal.
- Be a good time for C++ and Rust projects.

> TL;DR: VS-style debugging UX, Linux-friendly, tiny and quick.

## Features (List in progress)
- Workspace specific settings

## Getting Started

### The easy way (Nix)

```bash
# Enter the dev shell
nix develop

# Run the editor
cargo run
````

If you hit system-lib issues, the Nix dev shell is the smooth path for now.

## Performance notes

* Rust + iced for predictable latency.
* Minimal UI, no electron weight.
* Focused feature set so the hot path stays hot.

## Roadmap (rough cut)

* [ ] Launch/debug sessions from the editor (Rust & C++)
* [ ] Inline breakpoints + gutter markers
* [ ] Hover-to-inspect variables / scopes view
* [ ] Call stack + watch panes
* [ ] Rust and C++ build/run presets
* [ ] Keybindings that don’t fight you

## Contributing

Bug reports, ideas, and PRs welcome—especially around debugging backends and Linux packaging. Keep it small and focused.

## License

TBD
