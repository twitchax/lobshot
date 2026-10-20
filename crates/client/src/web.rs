//! Browser glue: the page URL, `localStorage`, creating a room, the clipboard, and the thin DOM
//! text input (chat and names), which handles IME, paste and phone keyboards better than a canvas.

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use lobshot_protocol::RoomCode;
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};

/// Where the page is: the title screen (`/`), or a room (`/r/{code}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page {
    Title,
    Room(RoomCode),
}

/// Reads the page from the URL.
#[must_use]
pub fn page() -> Page {
    let path = web_sys::window().and_then(|w| w.location().pathname().ok()).unwrap_or_default();
    path.strip_prefix("/r/")
        .and_then(|code| RoomCode::parse(code.trim_end_matches('/')).ok())
        .map_or(Page::Title, Page::Room)
}

/// The room's WebSocket URL, on this page's origin (the WebSocket API needs it absolute).
#[must_use]
pub fn socket_url(code: &RoomCode) -> String {
    let location = web_sys::window().map(|w| w.location());
    let secure = location.as_ref().and_then(|l| l.protocol().ok()).is_some_and(|p| p == "https:");
    let host = location.and_then(|l| l.host().ok()).unwrap_or_default();
    format!("{}//{host}/ws/{}", if secure { "wss:" } else { "ws:" }, code.as_str())
}

/// The page's own URL, for sharing.
#[must_use]
pub fn href() -> String {
    web_sys::window().and_then(|w| w.location().href().ok()).unwrap_or_default()
}

fn storage() -> Option<web_sys::Storage> {
    // Private windows and blocked site data can make localStorage throw; then nothing persists.
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

/// A remembered value.
#[must_use]
pub fn load(key: &str) -> Option<String> {
    storage().and_then(|s| s.get_item(&format!("lobshot.{key}")).ok().flatten())
}

/// Remembers a value (best effort).
pub fn save(key: &str, value: &str) {
    if let Some(storage) = storage() {
        let _ = storage.set_item(&format!("lobshot.{key}"), value);
    }
}

/// Creates a room on the server and goes to it (a full page load, so the client starts fresh in
/// the room). Failures land in `errors`.
pub fn create_room(errors: Arc<Mutex<Vec<String>>>) {
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = try_create_room().await
            && let Ok(mut errors) = errors.lock()
        {
            errors.push(format!("couldn't create a room: {error:?}"));
        }
    });
}

async fn try_create_room() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    let response: web_sys::Response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str_and_init("/api/rooms", &init)).await?.dyn_into()?;
    if !response.ok() {
        return Err(format!("the server said {}", response.status()).into());
    }
    let body = wasm_bindgen_futures::JsFuture::from(response.json()?).await?;
    let code = js_sys::Reflect::get(&body, &"code".into())?.as_string().ok_or("no code in the answer")?;
    window.location().set_href(&format!("/r/{code}"))
}

/// Copies text to the clipboard (best effort; browsers allow it after a click).
pub fn copy(text: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.navigator().clipboard().write_text(text);
    }
}

/// What the DOM input produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Typed {
    /// Enter, with the text.
    Submitted(String),
    /// Escape, or the input lost focus.
    Cancelled,
}

/// The DOM text input laid over the canvas. One at a time: `open` shows it, and whatever the
/// player types arrives through `take`.
#[derive(Resource, Clone, Default)]
pub struct TextInput {
    typed: Arc<Mutex<Vec<Typed>>>,
    open: Arc<Mutex<bool>>,
}

impl TextInput {
    const ID: &'static str = "lobshot-input";

    fn element() -> Option<web_sys::HtmlInputElement> {
        let document = web_sys::window()?.document()?;
        if let Some(existing) = document.get_element_by_id(Self::ID) {
            return existing.dyn_into().ok();
        }
        let input: web_sys::HtmlInputElement = document.create_element("input").ok()?.dyn_into().ok()?;
        input.set_id(Self::ID);
        input.set_class_name("lobshot-input");
        document.body()?.append_child(&input).ok()?;
        Some(input)
    }

    /// Shows the input with a placeholder and a length limit, focused.
    pub fn open(&self, placeholder: &str, max_length: usize, value: &str) {
        let Some(input) = Self::element() else { return };
        input.set_placeholder(placeholder);
        input.set_max_length(i32::try_from(max_length).unwrap_or(i32::MAX));
        input.set_value(value);
        let _ = input.style().set_property("display", "block");
        let _ = input.focus();
        if let Ok(mut open) = self.open.lock() {
            *open = true;
        }

        let typed = Arc::clone(&self.typed);
        let open = Arc::clone(&self.open);
        let field = input.clone();
        let on_key = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
            let result = match event.key().as_str() {
                "Enter" => Typed::Submitted(field.value()),
                "Escape" => Typed::Cancelled,
                _ => return,
            };
            event.prevent_default();
            let _ = field.style().set_property("display", "none");
            field.set_onkeydown(None);
            if let Ok(mut typed) = typed.lock() {
                typed.push(result);
            }
            if let Ok(mut open) = open.lock() {
                *open = false;
            }
            // Give the keyboard back to the game.
            if let Some(canvas) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id("lobshot"))
                && let Ok(canvas) = canvas.dyn_into::<web_sys::HtmlElement>()
            {
                let _ = canvas.focus();
            }
        });
        input.set_onkeydown(Some(on_key.as_ref().unchecked_ref()));
        // The handler lives as long as the page; it is replaced on the next open.
        on_key.forget();
    }

    /// Whether the input is showing (the game should ignore keys then).
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.lock().is_ok_and(|open| *open)
    }

    /// Everything typed since the last call.
    #[must_use]
    pub fn take(&self) -> Vec<Typed> {
        self.typed.lock().map_or_default(|mut typed| std::mem::take(&mut *typed))
    }
}

/// Publishes what the client is doing as `window.lobshot` (`{ screen, turns, seat }`), for the
/// browser smoke test to watch. Cheap enough to do every frame.
pub fn publish(screen: &str, turns: Option<i64>, seat: Option<u8>) {
    let Some(window) = web_sys::window() else { return };
    let state = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&state, &"screen".into(), &screen.into());
    // Turn counts stay far below 2^53, exact as a JS number.
    #[allow(clippy::cast_precision_loss)]
    let turns = turns.map_or(JsValue::NULL, |turns| JsValue::from_f64(turns as f64));
    let _ = js_sys::Reflect::set(&state, &"turns".into(), &turns);
    let _ = js_sys::Reflect::set(&state, &"seat".into(), &seat.map_or(JsValue::NULL, JsValue::from));
    let _ = js_sys::Reflect::set(&window, &"lobshot".into(), &state);
}
