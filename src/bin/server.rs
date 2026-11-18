//! Untrusted sync server for gpui-todo.
//!
//! It stores one opaque blob per vault id, in memory. It never receives the
//! passphrase or key, so it cannot read the todos: the log below prints what
//! the server actually sees.
//!
//! API:
//! - `GET /v1/vaults/{id}` -> 200 + blob + `X-Version`, or 404
//! - `PUT /v1/vaults/{id}` with `If-Match: <version>` (0 for a new vault)
//!   -> 200 + new `X-Version`, or 409 if the version is stale

use std::{collections::HashMap, io::Read};

use tiny_http::{Header, Method, Response, Server};

struct Vault {
    version: u64,
    blob: Vec<u8>,
}

fn main() {
    let addr = std::env::var("TODO_SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let server = Server::http(&addr).expect("failed to bind");
    eprintln!("sync server listening on http://{addr} (stores ciphertext only)");

    let mut vaults: HashMap<String, Vault> = HashMap::new();

    for mut request in server.incoming_requests() {
        let Some(id) = request
            .url()
            .strip_prefix("/v1/vaults/")
            .filter(|id| id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()))
            .map(str::to_owned)
        else {
            let _ = request.respond(Response::empty(404));
            continue;
        };

        let response = match request.method() {
            Method::Get => match vaults.get(&id) {
                Some(vault) => Response::from_data(vault.blob.clone())
                    .with_header(version_header(vault.version)),
                None => Response::from_data(Vec::new()).with_status_code(404),
            },
            Method::Put => {
                let expected = request
                    .headers()
                    .iter()
                    .find(|h| h.field.equiv("If-Match"))
                    .and_then(|h| h.value.as_str().parse::<u64>().ok());
                let current = vaults.get(&id).map_or(0, |v| v.version);
                let mut blob = Vec::new();
                let read_ok = request
                    .as_reader()
                    .take(1024 * 1024)
                    .read_to_end(&mut blob)
                    .is_ok();

                if !read_ok || expected.is_none() {
                    Response::from_data(Vec::new()).with_status_code(400)
                } else if expected != Some(current) {
                    eprintln!(
                        "PUT vault={id} rejected: stale version {expected:?}, current {current}"
                    );
                    Response::from_data(Vec::new())
                        .with_status_code(409)
                        .with_header(version_header(current))
                } else {
                    let version = current + 1;
                    eprintln!(
                        "PUT vault={id} v{version} {} bytes: {}…",
                        blob.len(),
                        hex_preview(&blob, 32)
                    );
                    vaults.insert(id, Vault { version, blob });
                    Response::from_data(Vec::new()).with_header(version_header(version))
                }
            }
            _ => Response::from_data(Vec::new()).with_status_code(405),
        };
        let _ = request.respond(response);
    }
}

fn version_header(version: u64) -> Header {
    Header::from_bytes("X-Version", version.to_string()).expect("valid header")
}

fn hex_preview(bytes: &[u8], n: usize) -> String {
    bytes.iter().take(n).map(|b| format!("{b:02x}")).collect()
}
