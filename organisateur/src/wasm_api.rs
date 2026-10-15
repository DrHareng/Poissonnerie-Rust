use wasm_bindgen::prelude::*;

use crate::engine::{apply, inspect};
use crate::export::export_xlsx;
use crate::model::{Action, Tournament};
use crate::scoring::match_points;
use crate::sectorials;

#[wasm_bindgen]
pub fn sectorials_json() -> String {
    serde_json::to_string(sectorials::all()).unwrap_or_else(|_| "[]".into())
}

#[wasm_bindgen]
pub fn preview_points(p1_op: u8, p2_op: u8) -> String {
    serde_json::json!({
        "p1": match_points(p1_op, p2_op),
        "p2": match_points(p2_op, p1_op),
    })
    .to_string()
}

#[wasm_bindgen]
pub fn inspect_tournament(state: &str) -> String {
    console_error_panic_hook::set_once();
    match serde_json::from_str::<Tournament>(state) {
        Ok(tournament) => serde_json::json!({
            "ok": true,
            "view": inspect(&tournament),
        })
        .to_string(),
        Err(error) => serde_json::json!({
            "ok": false,
            "error": format!("état illisible : {error}"),
        })
        .to_string(),
    }
}

#[wasm_bindgen]
pub fn dispatch(state: Option<String>, action: &str) -> String {
    console_error_panic_hook::set_once();
    let result = (|| -> Result<serde_json::Value, String> {
        let action: Action =
            serde_json::from_str(action).map_err(|error| format!("action illisible : {error}"))?;
        let current = match state {
            Some(raw) => Some(
                serde_json::from_str::<Tournament>(&raw)
                    .map_err(|error| format!("état illisible : {error}"))?,
            ),
            None => None,
        };
        let tournament = apply(current, action)?;
        Ok(serde_json::json!({
            "ok": true,
            "tournament": tournament,
            "view": inspect(&tournament),
        }))
    })();
    match result {
        Ok(value) => value.to_string(),
        Err(error) => serde_json::json!({ "ok": false, "error": error }).to_string(),
    }
}

#[wasm_bindgen]
pub fn export_workbook(state: &str) -> Result<Vec<u8>, JsValue> {
    console_error_panic_hook::set_once();
    let tournament: Tournament = serde_json::from_str(state)
        .map_err(|error| JsValue::from_str(&format!("état illisible : {error}")))?;
    export_xlsx(&tournament).map_err(|error| JsValue::from_str(&error))
}
