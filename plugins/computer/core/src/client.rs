//! wisp as the driver's MCP client over stdio: newline-delimited JSON-RPC (MCP's stdio transport). The host
//! writes what [`Client`] gives it and feeds it what the driver prints; [`Client::feed`] says which of
//! wisp's requests each line answers. The driver may ask wisp something too (`ping`, `roots/list`): wisp
//! answers so it never waits, with nothing it doesn't have.

use serde_json::{Value, json};

use crate::codes;

/// What a line from the driver means.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// the answer to wisp's request `id`: its `result`, or its `error` as `Err`
    Reply { id: u64, outcome: Result<Value, Value> },
    /// write this back to the driver (an answer to its own request)
    Send(Vec<u8>),
}

/// Ids, framing, and the half line still waiting for its end.
#[derive(Debug, Default)]
pub struct Client {
    next: u64,
    buf: Vec<u8>,
}

/// One JSON-RPC message as a line.
pub fn line(v: &Value) -> Vec<u8> {
    let mut b = serde_json::to_vec(v).unwrap_or_default();
    b.push(b'\n');
    b
}

impl Client {
    pub fn new() -> Self {
        Self::default()
    }

    /// A request: (its id, the line to write).
    pub fn request(&mut self, method: &str, params: &Value) -> (u64, Vec<u8>) {
        self.next += 1;
        let id = self.next;
        (id, line(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })))
    }

    /// `initialize`, as wisp's own client.
    pub fn initialize(&mut self) -> (u64, Vec<u8>) {
        self.request(
            "initialize",
            &json!({ "protocolVersion": crate::PROTOCOL_VERSIONS[0], "capabilities": {}, "clientInfo": { "name": "wisp", "version": wisp_computer_api::DRIVER_VERSION } }),
        )
    }

    /// The notification that follows `initialize`'s answer.
    pub fn initialized() -> Vec<u8> {
        line(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
    }

    /// What the driver printed (any chunk: a line may come in pieces, or several at once).
    pub fn feed(&mut self, data: &[u8]) -> Vec<Event> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        while let Some(at) = self.buf.iter().position(|b| *b == b'\n') {
            let raw: Vec<u8> = self.buf.drain(..=at).collect();
            // a line that isn't JSON (a log the driver printed on stdout by mistake) is skipped, never fatal
            if let Ok(v) = serde_json::from_slice::<Value>(&raw[..raw.len() - 1]) {
                out.extend(event(&v));
            }
        }
        out
    }
}

fn event(v: &Value) -> Option<Event> {
    let id = v.get("id");
    if let Some(method) = v.get("method").and_then(Value::as_str) {
        // the driver's own request (a notification has no id: nothing to answer)
        let id = id?;
        let reply = if method == "ping" {
            json!({ "jsonrpc": "2.0", "id": id, "result": {} })
        } else if method == "roots/list" {
            json!({ "jsonrpc": "2.0", "id": id, "result": { "roots": [] } })
        } else {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": codes::METHOD_NOT_FOUND, "message": format!("wisp doesn't answer {method}") } })
        };
        return Some(Event::Send(line(&reply)));
    }
    let id = id?.as_u64()?;
    let outcome = match v.get("error") {
        Some(e) => Err(e.clone()),
        None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
    };
    Some(Event::Reply { id, outcome })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_count_up_and_are_lines() {
        let mut c = Client::new();
        let (a, init) = c.initialize();
        let (b, _) = c.request("tools/list", &json!({}));
        assert_eq!((a, b), (1, 2));
        assert_eq!(init.last(), Some(&b'\n'));
        let v: Value = serde_json::from_slice(&init).expect("json");
        assert_eq!(v["params"]["clientInfo"]["name"], "wisp");
        assert!(Client::initialized().ends_with(b"}\n"));
    }

    #[test]
    fn a_reply_split_across_chunks_or_sharing_one_arrives_whole() {
        let mut c = Client::new();
        assert!(c.feed(br#"{"jsonrpc":"2.0","id":1,"res"#).is_empty(), "half a line waits");
        let got = c.feed(b"ult\":{\"ok\":true}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{\"code\":-1,\"message\":\"no\"}}\n");
        assert_eq!(
            got,
            vec![
                Event::Reply { id: 1, outcome: Ok(json!({ "ok": true })) },
                Event::Reply { id: 2, outcome: Err(json!({ "code": -1, "message": "no" })) },
            ]
        );
    }

    #[test]
    fn the_drivers_own_requests_are_answered_and_noise_is_skipped() {
        let mut c = Client::new();
        let got = c.feed(
            b"not json\n{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"ping\"}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\"}\n",
        );
        assert_eq!(got.len(), 1);
        let Event::Send(b) = &got[0] else { panic!("an answer to send") };
        let v: Value = serde_json::from_slice(b).expect("json");
        assert_eq!(v, json!({ "jsonrpc": "2.0", "id": 9, "result": {} }));
        let got = c.feed(b"{\"jsonrpc\":\"2.0\",\"id\":\"x\",\"method\":\"sampling/createMessage\"}\n");
        let Event::Send(b) = &got[0] else { panic!() };
        let v: Value = serde_json::from_slice(b).expect("json");
        assert_eq!(v["error"]["code"], codes::METHOD_NOT_FOUND);
    }
}
