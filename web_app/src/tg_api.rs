//! Telegram JS API bindings.

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// Telegram Web App object initialized by a Telegram JS script: <https://telegram.org/js/telegram-web-app.js>.
    ///
    /// For all possible methods and fields see <https://core.telegram.org/bots/webapps#initializing-mini-apps>.
    pub type WebApp;

    /// A method that expands the Mini App to the maximum available height.
    #[wasm_bindgen(method)]
    pub fn expand(this: &WebApp);

    /// A method that enables a confirmation dialog while the user is trying to close the Mini App.
    #[wasm_bindgen(method)]
    pub fn enableClosingConfirmation(this: &WebApp);

    /// A method used to send data to the bot.
    /// When this method is called, a service message is sent to the bot containing the data `data`
    /// of the length up to 4096 bytes, and the Mini App is closed.
    #[wasm_bindgen(method, catch)]
    pub fn sendData(this: &WebApp, data: JsValue) -> Result<(), JsValue>;
}
