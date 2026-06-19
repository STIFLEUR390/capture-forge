# Docs | Oxichrome

## Getting Started

oxichrome lets you write browser extensions entirely in Rust. Three commands and you have your first extension running in Chrome or Firefox.

### Install the CLI

terminal

```
cargo install cargo-oxichrome
```

### Create a new extension

terminal

```
cargo oxichrome new my-extension
cd my-extension
```

### Build and load

terminal

```
cargo oxichrome build
```

**Chrome/Edge:** Open `chrome://extensions`, enable **Developer mode**, click **Load unpacked**, and select the `dist/chromium/` folder.

**Firefox:** Build with `cargo oxichrome build --target firefox`, then open `about:debugging#/runtime/this-firefox`, click **Load Temporary Add-on**, and select `dist/firefox/manifest.json`.

## Project Structure

After scaffolding, your project looks like this:

```
my-extension/
├── Cargo.toml
├── src/
│   └── lib.rs          # Your extension code
└── static/             # Optional static assets (icons, CSS)
    └── icon-128.png
```

### Cargo.toml requirements

Cargo.toml

```
[lib]
crate-type = ["cdylib"]    # Required for WebAssembly output

[dependencies]
oxichrome = "0.2"
leptos = { version = "0.7", features = ["csr"] }
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
```

The `cdylib` crate type tells Cargo to produce a dynamic library suitable for WebAssembly compilation. The `oxichrome` facade crate re-exports everything you need.

## Proc Macros

oxichrome provides five attribute macros. Each maps directly to a browser extension concept. Together they replace manifest.json, background.js, popup.html, and all the JS glue code.

### `#[oxichrome::extension]`

Defines your extension's identity. This macro extracts the metadata at build time to generate `manifest.json`.

src/lib.rs

```
#[oxichrome::extension(
    name = "My Extension",
    version = "1.0.0",
    description = "A short description",
    permissions = ["storage", "tabs"]
)]
struct MyExtension;
```

| Attribute | Type | Required | Description |
| --- | --- | --- | --- |
| `name` | String | Yes | Display name shown in Chrome |
| `version` | String | Yes | Semantic version (e.g. "1.0.0") |
| `description` | String | No | Short description for Chrome Web Store |
| `permissions` | Array | No | Chrome permissions to request |

At build time, the macro also generates a hidden `__oxichrome_meta` module with constants that the build tool reads to produce manifest.json.

### `#[oxichrome::background]`

Marks an async function as the service worker entry point. The macro generates a `#[wasm_bindgen]` export that wraps your function in `spawn_local`.

src/lib.rs

```
#[oxichrome::background]
async fn start() {
    oxichrome::log!("Service worker alive!");
}
```

Expands to:

expanded

```
async fn start() {
    oxichrome::log!("Service worker alive!");
}

#[wasm_bindgen]
pub fn __oxichrome_bg_start() {
    spawn_local(async { start().await; });
}
```

**Named exports, not `#[wasm_bindgen(start)]`.** The same Wasm binary is loaded by background, popup, and options pages. Named exports prevent background init from running everywhere.

### `#[oxichrome::on]`

Registers an async function as a Chrome event handler. The build tool generates the JS that wires the Wasm export to the correct `chrome.*` listener.

src/lib.rs

```
#[oxichrome::on(runtime::on_installed)]
async fn handle_install(details: JsValue) {
    oxichrome::log!("Extension installed!");
    oxichrome::storage::set("count", &0i32).await.ok();
}
```

#### Supported events

| Event | Chrome API | Description |
| --- | --- | --- |
| `runtime::on_installed` | `chrome.runtime.onInstalled` | Extension installed or updated |
| `runtime::on_message` | `chrome.runtime.onMessage` | Message received from another part of the extension |
| `storage::on_changed` | `chrome.storage.onChanged` | Storage value changed |
| `tabs::on_updated` | `chrome.tabs.onUpdated` | Tab URL, title, or status changed |
| `tabs::on_activated` | `chrome.tabs.onActivated` | Active tab changed in a window |

The macro generates a `Closure` that wraps your function and calls `.forget()` to keep it alive for the lifetime of the service worker.

### `#[oxichrome::options_page]`

Same as `#[popup]` but for the options/settings page. Generates `options.html` and `options.js`.

src/lib.rs

```
#[oxichrome::options_page]
fn Options() -> impl IntoView {
    view! {
        <h1>"Settings"</h1>
    }
}
```

## Chrome APIs

oxichrome wraps Chrome's JavaScript APIs in type-safe, async Rust interfaces. All functions return `Result<T, OxichromeError>` and use `serde` for automatic serialization.

### Storage

Async, generic wrappers around `chrome.storage.local`.

#### Get a value

```
let count: Option<i32> =
    oxichrome::storage::get("count").await?;

// With a custom type
#[derive(Deserialize)]
struct Settings { theme: String }

let settings: Option<Settings> =
    oxichrome::storage::get("settings").await?;
```

#### Set a value

```
oxichrome::storage::set("count", &42).await?;
oxichrome::storage::set("settings", &my_settings).await?;
```

#### Remove a value

```
oxichrome::storage::remove("count").await?;
```

Any type that implements `Serialize` + `DeserializeOwned` works. Conversion is handled by `serde_wasm_bindgen`.

### Runtime

#### Get extension URL

```
let url = oxichrome::runtime::get_url("icon.png");  // Synchronous
```

#### Send message

```
#[derive(Serialize)]
struct Msg { text: String }

let response = oxichrome::runtime::send_message(&Msg {
    text: "hello".into()
}).await?;
```

### Tabs

#### Query tabs

```
#[derive(Serialize)]
struct Query { active: bool }

#[derive(Deserialize)]
struct Tab { id: i32, url: String }

let tabs: Vec<Tab> =
    oxichrome::tabs::query(&Query { active: true }).await?;
```

#### Create a tab

```
#[derive(Serialize)]
struct NewTab { url: String }

let tab: Tab = oxichrome::tabs::create(&NewTab {
    url: "https://example.com".into()
}).await?;
```

#### Send message to a tab

```
oxichrome::tabs::send_message(tab_id, &my_message).await?;
```

## Leptos UI

oxichrome uses [Leptos](https://leptos.dev/) for reactive UI in popup and options pages. Leptos uses fine-grained reactivity with no virtual DOM, just direct DOM updates when signals change.

### Reactive signals

```
let count = RwSignal::new(0);

count.set(42);                     // Set value
count.update(|c| *c += 1);        // Mutate in place
let val = count.get();             // Read (creates dependency)
let val = count.get_untracked();  // Read without subscribing
```

### Effects

```
Effect::new(move || {
    // Runs whenever count changes
    let current = count.get();
    oxichrome::log!("Count: {}", current);
});
```

### Async in UI

```
let on_click = move |_| {
    spawn_local(async move {
        let val = oxichrome::storage::get::<i32>("key").await;
        if let Ok(Some(n)) = val {
            count.set(n);
        }
    });
};

view! {
    <button on:click=on_click>"Load"</button>
}
```

### Dynamic content

```
view! {
    // Reactive text
    <div>{move || count.get()}</div>

    // List rendering
    <For
        each=move || items.get()
        key=|item| item.id
        children=move |item| view! { <div>{item.name}</div> }
    />
}
```

## Build Pipeline

`cargo oxichrome build` runs a 10-step pipeline:

1.  **Read Cargo.toml** to extract the crate name for Wasm file naming
2.  **Ensure wasm32-unknown-unknown target** is installed, running `rustup target add` if missing
3.  **Match wasm-bindgen-cli version** by parsing Cargo.lock and installing the matching CLI version
4.  **Compile to Wasm** via `cargo build --lib --target wasm32-unknown-unknown`
5.  **Run wasm-bindgen** to generate JS bindings and processed Wasm in `dist/wasm/`
6.  **Parse source** by walking the AST with `syn` to discover annotated functions
7.  **Generate manifest.json** with Manifest V3, correct permissions, CSP, and entry points
8.  **Generate background.js** as an ES module service worker that imports and calls Wasm exports
9.  **Generate popup/options files** with HTML shells and JS loaders if components are detected
10.  **Run wasm-opt** for optional size optimization with the `-Oz` flag

### Debug vs Release

terminal

```


# Debug (faster compile, larger Wasm ~1-2MB)
cargo oxichrome build

# Release (slower compile, optimized Wasm ~200-500KB)
cargo oxichrome build --release
```

### Browser Target

terminal

```


# Chromium (default) — Chrome, Edge, Brave, Opera
cargo oxichrome build

# Firefox
cargo oxichrome build --target firefox

# Both
cargo oxichrome build && cargo oxichrome build --target firefox
```

Each target outputs to its own directory: `dist/chromium/` or `dist/firefox/`. Both can coexist.

### Clean

terminal

```


# Remove the entire dist/ directory
cargo oxichrome clean
```

### Build Output

```
dist/
├── chromium/                   # --target chromium (default)
│   ├── manifest.json           # Chrome Manifest V3
│   ├── background.js           # Service worker (ES module)
│   ├── popup.html / popup.js   # If #[popup] exists
│   ├── options.html / options.js
│   ├── wasm/
│   │   ├── {crate_name}.js     # wasm-bindgen ES module
│   │   └── {crate_name}_bg.wasm
│   └── [static/ contents]
└── firefox/                    # --target firefox
    ├── manifest.json           # Firefox Manifest V3 (background scripts, gecko ID)
    └── ...                     # Same files as chromium/
```

The generated `background.js` imports Wasm exports by name, registers event listeners first, then calls background functions:

background.js (generated)

```
import init, {
    __oxichrome_bg_start,
    __oxichrome_register_handle_install
} from './wasm/my_extension.js';

async function start() {
    await init();
    __oxichrome_register_handle_install();  // Events first
    __oxichrome_bg_start();                 // Background second
}

start();
```

### Browser Targets

By default, `cargo oxichrome build` targets Chromium (Chrome, Edge, Brave, Opera). Use `--target firefox` to build for Firefox.

The Firefox manifest differs from Chromium in three ways:

| Field | Chromium | Firefox |
| --- | --- | --- |
| `background` | `{ "service_worker": "background.js" }` | `{ "scripts": ["background.js"] }` |
| `content_security_policy` | Same object format on both (`{ "extension_pages": "..." }`) |
| `browser_specific_settings` | Not present | `{ "gecko": { "id": "name@oxichrome.dev" } }` |

The gecko ID is derived from the extension name: `my-extension@oxichrome.dev`. All other files (JS shims, WASM, HTML) are identical between targets.

### CLI Commands

| Command | Description |
| --- | --- |
| `cargo oxichrome build` | Build the extension (default: Chromium) |
| `cargo oxichrome build --release` | Optimized release build |
| `cargo oxichrome build --target firefox` | Build for Firefox |
| `cargo oxichrome clean` | Remove the `dist/` directory |
| `cargo oxichrome new <name>` | Scaffold a new extension project |

## Architecture

oxichrome is a workspace of five crates with clear responsibilities:

| Crate | Phase | Role |
| --- | --- | --- |
| `oxichrome-macros` | Compile | Proc macros that generate `#[wasm_bindgen]` exports and metadata |
| `oxichrome-core` | Runtime | Async wrappers around Chrome JS APIs (storage, tabs, runtime) |
| `oxichrome` | Facade | Re-exports macros + core. The one crate users depend on. |
| `oxichrome-build` | Build | Source analysis with `syn`, manifest/shim generation |
| `oxichrome-cli` | CLI | The `cargo oxichrome` binary that orchestrates the full pipeline |

### Key design decisions

**Two-pass build.** First, `cargo build` compiles Rust to Wasm (macros generate exports). Then `oxichrome-build` parses source with `syn` to extract metadata for manifest and JS shims. This avoids config files or extracting info from binaries.

**`__private` module pattern.** Generated code uses `oxichrome::__private::wasm_bindgen` and similar paths. Users don't need to directly depend on `wasm-bindgen`, `leptos`, or worry about version mismatches.

**ES modules everywhere.** The manifest uses `"type": "module"` for the service worker. `wasm-bindgen --target web` produces ES modules. This enables top-level `await` and clean imports.

## Examples

Full working examples are in the `examples/` directory. Each demonstrates real patterns you'll use in your own extensions.

### Counter Extension

A minimal popup extension with persistent state. The count survives browser restarts via `chrome.storage.local`.

src/lib.rs

```
use oxichrome::prelude::*;
use leptos::*;

#[oxichrome::extension(
    name = "Counter Extension",
    version = "0.1.0",
    description = "A simple counter stored in chrome.storage.local",
    permissions = ["storage"]
)]
struct CounterExtension;

#[oxichrome::background]
async fn start() {
    oxichrome::log!("Counter service worker started!");
}

#[oxichrome::on(runtime::on_installed)]
async fn handle_install(_details: JsValue) {
    oxichrome::storage::set("counter", &0i32).await.ok();
}

#[oxichrome::popup]
fn Popup() -> impl IntoView {
    let count = RwSignal::new(0i32);

    // Load persisted count on mount
    Effect::new(move || {
        spawn_local(async move {
            if let Ok(Some(val)) = oxichrome::storage::get::<i32>("counter").await {
                count.set(val);
            }
        });
    });

    let increment = move |_| {
        count.update(|c| *c += 1);
        let val = count.get_untracked();
        spawn_local(async move {
            let _ = oxichrome::storage::set("counter", &val).await;
        });
    };

    view! {
        <div class="popup">
            <h1>"Counter"</h1>
            <div class="count">{move || count.get()}</div>
            <button on:click=increment>"+"</button>
        </div>
    }
}
```

### Color Picker Extension

Uses the EyeDropper Web API to pick colors from any page. Demonstrates custom FFI bindings, messaging, and color history stored in `chrome.storage.local`.

src/lib.rs

```
use oxichrome::prelude::*;
use leptos::*;

#[oxichrome::extension(
    name = "Color Picker",
    version = "0.1.0",
    description = "Pick colors from any page",
    permissions = ["activeTab", "storage"]
)]
struct ColorPickerExt;

// Custom FFI for EyeDropper Web API
#[wasm_bindgen]
extern "C" {
    type EyeDropper;

    #[wasm_bindgen(constructor)]
    fn new() -> EyeDropper;

    #[wasm_bindgen(method)]
    fn open(this: &EyeDropper) -> js_sys::Promise;
}

#[oxichrome::popup]
fn Popup() -> impl IntoView {
    let color = RwSignal::new("#000000".to_string());
    let history = RwSignal::new(Vec::<String>::new());

    // Load saved history
    Effect::new(move || {
        spawn_local(async move {
            if let Ok(Some(h)) =
                oxichrome::storage::get::<Vec<String>>("history").await
            {
                history.set(h);
            }
        });
    });

    let pick = move |_| {
        spawn_local(async move {
            let dropper = EyeDropper::new();
            if let Ok(result) = JsFuture::from(dropper.open()).await {
                let hex = js_sys::Reflect::get(
                    &result, &JsValue::from_str("sRGBHex")
                ).ok()
                    .and_then(|v| v.as_string())
                    .unwrap_or_else(|| "#000000".into());

                color.set(hex.clone());
                history.update(|h| {
                    h.retain(|c| c != &hex);
                    h.insert(0, hex);
                    h.truncate(20);
                });

                // Persist
                let _ = oxichrome::storage::set(
                    "history", &history.get_untracked()
                ).await;
            }
        });
    };

    view! {
        <div class="picker">
            <div
                class="swatch"
                style=move || format!("background:{}", color.get())
            />
            <p>{move || color.get()}</p>
            <button on:click=pick>"Pick from page"</button>
        </div>
    }
}
```

## Common Patterns

### Load on mount, save on change

```
let data = RwSignal::new(MyData::default());

// Load once
Effect::new(move || {
    spawn_local(async move {
        if let Ok(Some(loaded)) = oxichrome::storage::get("data").await {
            data.set(loaded);
        }
    });
});

// Save on change
let on_change = move |new_value| {
    data.set(new_value);
    let val = data.get_untracked();
    spawn_local(async move {
        let _ = oxichrome::storage::set("data", &val).await;
    });
};
```

### Custom Web API bindings

```
#[wasm_bindgen]
extern "C" {
    type MyApi;

    #[wasm_bindgen(constructor)]
    fn new() -> MyApi;

    #[wasm_bindgen(method)]
    fn do_thing(this: &MyApi) -> js_sys::Promise;
}

// Usage:
let api = MyApi::new();
let result = JsFuture::from(api.do_thing()).await?;
```

### Message passing between components

```
// In background:
#[oxichrome::on(runtime::on_message)]
async fn handle_message(msg: JsValue) -> JsValue {
    let request: MyRequest = serde_wasm_bindgen::from_value(msg).unwrap();
    // Process and respond
    serde_wasm_bindgen::to_value(&response).unwrap()
}

// In popup:
let response = oxichrome::runtime::send_message(&my_request).await?;
```

## Permissions

Add to `permissions = [...]` in your `#[extension]` attribute:

| Permission | Enables |
| --- | --- |
| `"storage"` | `chrome.storage.local` / `chrome.storage.sync` |
| `"tabs"` | `chrome.tabs.*` (read tab URLs and metadata) |
| `"activeTab"` | Access current tab on user action (click, keyboard shortcut) |
| `"scripting"` | Inject scripts into pages |
| `"notifications"` | Show desktop notifications |
| `"contextMenus"` | Add context menu items |
| `"cookies"` | Read/write cookies |
| `"webRequest"` | Intercept network requests |
| `"clipboardWrite"` | Write to clipboard |

Host permissions (for specific sites):

```
#[oxichrome::extension(
    name = "My Ext",
    version = "1.0.0",
    permissions = ["storage", "https://example.com/*"]
)]
```

## Troubleshooting

### WASM initialization failed

**Cause:** Content Security Policy blocking Wasm.

**Fix:** oxichrome automatically sets `wasm-unsafe-eval` in the generated manifest. If you're overriding CSP manually, ensure it includes `"script-src 'self' 'wasm-unsafe-eval'"`.

### wasm-bindgen version mismatch

**Cause:** The wasm-bindgen CLI version doesn't match the library version in Cargo.lock.

**Fix:** Delete `Cargo.lock`, run `cargo update`, then rebuild. The CLI auto-installs a matching version.

### Extension service worker inactive

**Cause:** Chrome suspends service workers after inactivity.

**Normal behavior.** Chrome wakes them on registered events. Use `oxichrome::log!` to verify event handlers fire.

### Closure has been dropped

**Cause:** An event listener `Closure` was dropped before the event fired.

**Fix:** The proc macros handle `.forget()` automatically. If you're writing custom bindings, make sure to call `closure.forget()` for long-lived callbacks.

### Cannot find module './wasm/...'

**Cause:** Wasm files weren't generated in `dist/wasm/`.

**Fix:** Run the full `cargo oxichrome build` and check that the wasm-bindgen step succeeded.

### Firefox: EyeDropper or other Chromium-only APIs

**Cause:** Some Web APIs (e.g. `EyeDropper`) only exist in Chromium.

**Fix:** Check [MDN](https://developer.mozilla.org/en-US/docs/Web/API) for browser compatibility. For Chromium-only APIs, either skip Firefox or implement a fallback.

### Logging

```
oxichrome::log!("Debug: {} = {}", key, value);
```

Output appears in:

-   **Background (Chrome):** DevTools → Extensions → Your Extension → Service Worker → Console

-   **Background (Firefox):** about:debugging → Your Extension → Inspect
-   **Popup/Options:** Right-click popup → Inspect → Console

---
Source: [Docs | Oxichrome](https://oxichrome.dev/docs#macro-extension)
