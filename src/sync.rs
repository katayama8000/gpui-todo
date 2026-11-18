//! End-to-end encrypted sync against an untrusted server (see `src/bin/server.rs`).
//!
//! The whole todo list is serialized, encrypted with [`VaultKey`] and stored as
//! one opaque blob. Each todo carries an `updated_at` timestamp and deletions
//! are kept as tombstones, so two clients that edited concurrently can be
//! merged per item (last writer wins) without the server's help.

use std::{collections::BTreeMap, time::Duration};

use serde::{Deserialize, Serialize};

use crate::crypto::VaultKey;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Todo {
    pub id: u64,
    pub title: String,
    pub done: bool,
    /// Tombstone: kept so the deletion wins over older copies on other clients.
    pub deleted: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

impl Todo {
    /// Total order used to pick the winner when two copies of the same todo differ.
    fn rank(&self) -> (u64, bool, bool, &str) {
        (self.updated_at, self.deleted, self.done, &self.title)
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Merges two lists item by item, keeping the most recently updated copy.
/// Commutative and idempotent, so it is safe to apply repeatedly.
pub fn merge(a: &[Todo], b: &[Todo]) -> Vec<Todo> {
    let mut by_id: BTreeMap<u64, Todo> = BTreeMap::new();
    for todo in a.iter().chain(b) {
        match by_id.get(&todo.id) {
            Some(existing) if existing.rank() >= todo.rank() => {}
            _ => {
                by_id.insert(todo.id, todo.clone());
            }
        }
    }
    let mut merged: Vec<Todo> = by_id.into_values().collect();
    merged.sort_by_key(|t| (t.created_at, t.id));
    merged
}

#[derive(Serialize, Deserialize)]
struct Document {
    todos: Vec<Todo>,
}

pub struct SyncOutcome {
    pub todos: Vec<Todo>,
    pub version: u64,
}

pub struct SyncClient {
    agent: ureq::Agent,
    url: String,
    key: VaultKey,
}

type Error = Box<dyn std::error::Error + Send + Sync>;

impl SyncClient {
    pub fn new(server: &str, key: VaultKey) -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(5)))
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
            url: format!(
                "{}/v1/vaults/{}",
                server.trim_end_matches('/'),
                key.vault_id()
            ),
            key,
        }
    }

    pub fn vault_id(&self) -> &str {
        self.key.vault_id()
    }

    /// Pulls the remote state, merges it with `local` and pushes the result if
    /// the server is missing anything. Blocking; run it off the UI thread.
    pub fn sync(&self, local: &[Todo]) -> Result<SyncOutcome, Error> {
        for _ in 0..5 {
            let (version, remote) = self.pull()?;
            let merged = merge(local, &remote);
            if merged == remote {
                return Ok(SyncOutcome {
                    todos: merged,
                    version,
                });
            }
            if let Some(version) = self.push(version, &merged)? {
                return Ok(SyncOutcome {
                    todos: merged,
                    version,
                });
            }
            // Someone else pushed in between: pull again and re-merge.
        }
        Err("too many concurrent updates, will retry".into())
    }

    fn pull(&self) -> Result<(u64, Vec<Todo>), Error> {
        let mut resp = self.agent.get(&self.url).call()?;
        match resp.status().as_u16() {
            404 => Ok((0, Vec::new())),
            200 => {
                let version = header_version(&resp)?;
                let blob = resp.body_mut().read_to_vec()?;
                let plaintext = self.key.open(&blob)?;
                let doc: Document = serde_json::from_slice(&plaintext)?;
                Ok((version, doc.todos))
            }
            status => Err(format!("GET failed: HTTP {status}").into()),
        }
    }

    /// Returns the new version, or `None` if `expected` was stale (HTTP 409).
    fn push(&self, expected: u64, todos: &[Todo]) -> Result<Option<u64>, Error> {
        let plaintext = serde_json::to_vec(&Document {
            todos: todos.to_vec(),
        })?;
        let blob = self.key.seal(&plaintext);
        let resp = self
            .agent
            .put(&self.url)
            .header("If-Match", expected.to_string())
            .header("Content-Type", "application/octet-stream")
            .send(&blob[..])?;
        match resp.status().as_u16() {
            200 => Ok(Some(header_version(&resp)?)),
            409 => Ok(None),
            status => Err(format!("PUT failed: HTTP {status}").into()),
        }
    }
}

fn header_version(resp: &ureq::http::Response<ureq::Body>) -> Result<u64, Error> {
    Ok(resp
        .headers()
        .get("X-Version")
        .ok_or("missing X-Version header")?
        .to_str()?
        .parse()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn todo(id: u64, title: &str, done: bool, deleted: bool, updated_at: u64) -> Todo {
        Todo {
            id,
            title: title.into(),
            done,
            deleted,
            created_at: id,
            updated_at,
        }
    }

    #[test]
    fn union_of_disjoint_edits() {
        let a = vec![todo(1, "a", false, false, 1)];
        let b = vec![todo(2, "b", false, false, 1)];
        let merged = merge(&a, &b);
        assert_eq!(merged.iter().map(|t| t.id).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn newer_edit_wins_and_tombstones_stick() {
        let old = vec![todo(1, "a", false, false, 1)];
        let toggled = vec![todo(1, "a", true, false, 2)];
        let deleted = vec![todo(1, "a", false, true, 3)];
        assert!(merge(&old, &toggled)[0].done);
        assert!(merge(&toggled, &deleted)[0].deleted);
        assert!(merge(&deleted, &old)[0].deleted);
    }

    /// Needs a running server: `cargo run --bin server`, then
    /// `cargo test -- --ignored two_clients_converge`.
    #[test]
    #[ignore]
    fn two_clients_converge_through_server() {
        let server =
            std::env::var("TODO_SYNC_URL").unwrap_or_else(|_| "http://127.0.0.1:8787".into());
        let vault = format!("test-{}", now_ms());
        let a = SyncClient::new(&server, VaultKey::derive(&vault, "pw"));
        let b = SyncClient::new(&server, VaultKey::derive(&vault, "pw"));
        let eve = SyncClient::new(&server, VaultKey::derive(&vault, "wrong"));

        let a_state = a.sync(&[todo(1, "from a", false, false, 1)]).unwrap().todos;
        let b_state = b.sync(&[todo(2, "from b", false, false, 1)]).unwrap().todos;
        assert_eq!(b_state.len(), 2);

        // A deletes its todo while B's copy is stale; the tombstone must win.
        let mut a_edit = merge(&a_state, &b_state);
        a_edit[0].deleted = true;
        a_edit[0].updated_at = 2;
        a.sync(&a_edit).unwrap();
        let b_final = b.sync(&b_state).unwrap().todos;
        assert!(b_final.iter().find(|t| t.id == 1).unwrap().deleted);

        // A wrong passphrase maps to a different, empty vault.
        assert!(eve.sync(&[]).unwrap().todos.is_empty());
    }

    #[test]
    fn merge_is_commutative() {
        let a = vec![todo(1, "a", true, false, 5), todo(2, "b", false, false, 1)];
        let b = vec![todo(1, "a", false, false, 5), todo(3, "c", false, true, 2)];
        assert_eq!(merge(&a, &b), merge(&b, &a));
    }
}
