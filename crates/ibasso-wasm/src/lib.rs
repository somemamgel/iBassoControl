//! Thin WebAssembly bindings. All wire bytes and validation come from ibasso-protocol.
use ibasso_protocol::{
    Filter, Gain, Output, PID, PRODUCT, Query, Report, Setting, VID, VOLUME_TABLE,
};
use serde::Deserialize;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Input {
    Volume { level: u8, balance: i8 },
    Filter { value: FilterName },
    Gain { value: GainName },
    Output { value: OutputName },
    Eco { value: bool },
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum FilterName {
    D1,
    D2,
    D3,
    D4,
    D5,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum GainName {
    Low,
    Medium,
    High,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum OutputName {
    Normal,
    Turbo,
}

impl From<Input> for Setting {
    fn from(input: Input) -> Self {
        match input {
            Input::Volume { level, balance } => Setting::Volume { level, balance },
            Input::Filter { value } => Setting::Filter(match value {
                FilterName::D1 => Filter::D1,
                FilterName::D2 => Filter::D2,
                FilterName::D3 => Filter::D3,
                FilterName::D4 => Filter::D4,
                FilterName::D5 => Filter::D5,
            }),
            Input::Gain { value } => Setting::Gain(match value {
                GainName::Low => Gain::Low,
                GainName::Medium => Gain::Medium,
                GainName::High => Gain::High,
            }),
            Input::Output { value } => Setting::Output(match value {
                OutputName::Normal => Output::Normal,
                OutputName::Turbo => Output::Turbo,
            }),
            Input::Eco { value } => Setting::Eco(value),
        }
    }
}

fn checked_report(bytes: &[u8]) -> Result<Report, String> {
    bytes
        .try_into()
        .map(Report)
        .map_err(|_| "a report must contain exactly 16 bytes".to_owned())
}

fn plan_value(input: &str) -> Result<Value, String> {
    let input: Input = serde_json::from_str(input).map_err(|e| e.to_string())?;
    let steps = Setting::from(input).plan().map_err(|e| e.to_string())?;
    Ok(Value::Array(
        steps
            .iter()
            .map(|step| {
                json!({
                    "write": step.write.0, "readback": step.readback.0, "expected": step.expected
                })
            })
            .collect(),
    ))
}

#[wasm_bindgen]
pub fn metadata_json() -> String {
    json!({
        "deviceFilter": {"vendorId":VID,"productId":PID,"usagePage":12,"usage":1},
        "product": PRODUCT,
        "volumeTable": VOLUME_TABLE.to_vec(),
        "filters": Filter::ALL.into_iter().map(|f| (f.name(),f.raw())).collect::<std::collections::BTreeMap<_,_>>(),
        "gains": {"low":Gain::Low.raw(),"medium":Gain::Medium.raw(),"high":Gain::High.raw()},
        "outputs": {"normal":Output::Normal.raw(),"turbo":Output::Turbo.raw()},
    }).to_string()
}

#[wasm_bindgen]
pub fn queries_json() -> String {
    let queries = [
        ("firmware", Query::Firmware),
        ("volumeLeft", Query::VolumeLeft),
        ("volumeRight", Query::VolumeRight),
        ("filterLeft", Query::FilterLeft),
        ("filterRight", Query::FilterRight),
        ("gainLeft", Query::GainLeft),
        ("gainRight", Query::GainRight),
        ("outputLeft", Query::OutputLeft),
        ("outputRight", Query::OutputRight),
        ("eco", Query::Eco),
    ];
    serde_json::to_string(&queries.map(|(name, query)| (name, query.report().0)))
        .expect("query serialization")
}

#[wasm_bindgen]
pub fn plan_json(input: &str) -> Result<String, JsValue> {
    plan_value(input).map(|v| v.to_string()).map_err(js_error)
}

#[wasm_bindgen]
pub fn preamble(bytes: &[u8]) -> Result<Option<Vec<u8>>, JsValue> {
    Ok(checked_report(bytes)
        .map_err(js_error)?
        .read_preamble()
        .map(|r| r.0.to_vec()))
}

#[wasm_bindgen]
pub fn decode_response(bytes: &[u8], expected_id: u32) -> Result<Option<Vec<u8>>, JsValue> {
    let expected = u8::try_from(expected_id).map_err(js_error)?;
    ibasso_protocol::response_data(bytes, expected)
        .map(|v| v.map(<[u8]>::to_vec))
        .map_err(js_error)
}

#[wasm_bindgen]
pub fn read_value(report: &[u8], data: &[u8]) -> Result<u8, JsValue> {
    ibasso_protocol::response_value(checked_report(report).map_err(js_error)?, data)
        .map_err(js_error)
}

#[wasm_bindgen]
pub fn volume_level(raw: u32) -> Option<u8> {
    u8::try_from(raw)
        .ok()
        .and_then(ibasso_protocol::volume_level)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_json_without_truncating_numbers() {
        for input in [
            r#"{"kind":"volume","level":256,"balance":0}"#,
            r#"{"kind":"volume","level":30.5,"balance":0}"#,
            r#"{"kind":"volume","level":101,"balance":0}"#,
            r#"{"kind":"volume","level":30,"balance":-11}"#,
            r#"{"kind":"filter","value":"d6"}"#,
            r#"{"kind":"eco","value":1}"#,
            r#"{"kind":"eco","value":true,"extra":1}"#,
        ] {
            assert!(plan_value(input).is_err(), "{input}");
        }
    }
    #[test]
    fn bindings_use_core_plan_and_metadata() {
        let value = plan_value(r#"{"kind":"filter","value":"d1"}"#).unwrap();
        assert_eq!(
            value[0]["write"],
            json!(Setting::Filter(Filter::D1).plan().unwrap()[0].write.0)
        );
        let meta: Value = serde_json::from_str(&metadata_json()).unwrap();
        assert_eq!(meta["volumeTable"], json!(VOLUME_TABLE.to_vec()));
        assert_eq!(meta["deviceFilter"]["vendorId"], VID);
        assert!(checked_report(&[0; 15]).is_err());
    }
}
