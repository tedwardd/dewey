use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

pub fn encode_request(id: u64, method: &str, params: &Value) -> String {
    let req = Request {
        jsonrpc: "2.0".into(),
        id,
        method: method.into(),
        params: Some(params.clone()),
    };
    format!("{}\n", serde_json::to_string(&req).expect("request serializes"))
}

pub fn decode_response(line: &str) -> Result<Response, serde_json::Error> {
    serde_json::from_str(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_request_is_single_ndjson_line() {
        let line = encode_request(7, "search", &json!({"query": "dune"}));
        assert!(line.ends_with('\n'));
        let req: Request = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, 7);
        assert_eq!(req.method, "search");
        assert_eq!(req.params, Some(json!({"query": "dune"})));
    }

    #[test]
    fn decode_response_result() {
        let resp = decode_response(r#"{"jsonrpc":"2.0","id":7,"result":{"ok":true}}"#).unwrap();
        assert_eq!(resp.id, 7);
        assert_eq!(resp.result, Some(json!({"ok": true})));
        assert!(resp.error.is_none());
    }

    #[test]
    fn decode_response_error() {
        let resp = decode_response(r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32000,"message":"boom"}}"#).unwrap();
        assert_eq!(resp.error.unwrap().code, -32000);
        assert!(resp.result.is_none());
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(decode_response("not json\n").is_err());
    }
}
