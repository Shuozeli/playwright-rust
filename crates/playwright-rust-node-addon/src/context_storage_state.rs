//! Node conversion boundary for Rust-owned BrowserContext storage-state reads.
//!
//! Browser traversal and storage extraction stay in the host. The addon
//! serializes the bounded typed snapshot off the JavaScript thread; the public
//! facade performs only the final JSON language-value conversion.

use napi::{Env, JsString, Unknown, bindgen_prelude::PromiseRaw};
use napi_derive::napi;
use playwright_rust_node_host::{
    HostError,
    context_cookies::BrowserContextCookieInput,
    context_storage_state::{
        ContextStorageOrigin, ContextStorageState, ContextStorageStateSnapshot,
        MAX_STORAGE_STATE_BYTES,
    },
    ffi_budget::FfiLane,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{NativeSession, binding_error, boundary};

const FFI_ALLOWANCE: u64 = 2 * MAX_STORAGE_STATE_BYTES as u64 + 2 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StorageStateCookieInput {
    name: String,
    value: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    expires: Option<f64>,
    #[serde(default)]
    http_only: Option<bool>,
    #[serde(default)]
    secure: Option<bool>,
    #[serde(default)]
    same_site: Option<String>,
    #[serde(default)]
    partition_key: Option<String>,
    #[serde(rename = "_crHasCrossSiteAncestor", default)]
    partition_has_cross_site_ancestor: Option<bool>,
}

impl From<StorageStateCookieInput> for BrowserContextCookieInput {
    fn from(value: StorageStateCookieInput) -> Self {
        Self {
            name: value.name,
            value: value.value,
            url: value.url,
            domain: value.domain,
            path: value.path,
            expires: value.expires,
            http_only: value.http_only,
            secure: value.secure,
            same_site: value.same_site,
            partition_key: value.partition_key,
            partition_has_cross_site_ancestor: value.partition_has_cross_site_ancestor,
        }
    }
}

#[derive(Deserialize)]
struct StorageStateInput {
    #[serde(default)]
    cookies: Vec<StorageStateCookieInput>,
    #[serde(default)]
    origins: Vec<ContextStorageOrigin>,
}

fn parse_state_json(input: &str) -> Result<ContextStorageState, HostError> {
    let input: StorageStateInput = serde_json::from_str(input).map_err(|error| {
        HostError::InvalidArgument(format!("invalid BrowserContext storage state: {error}"))
    })?;
    Ok(ContextStorageState {
        cookies: input.cookies.into_iter().map(Into::into).collect(),
        origins: input.origins,
    })
}

fn snapshot_json(
    snapshot: ContextStorageStateSnapshot,
    indexed_db: bool,
) -> Result<String, HostError> {
    let cookies = snapshot
        .cookies
        .into_iter()
        .map(|cookie| {
            let mut value = Map::new();
            value.insert("name".into(), Value::String(cookie.name));
            value.insert("value".into(), Value::String(cookie.value));
            value.insert("domain".into(), Value::String(cookie.domain));
            value.insert("path".into(), Value::String(cookie.path));
            value.insert("expires".into(), json!(cookie.expires));
            value.insert("httpOnly".into(), Value::Bool(cookie.http_only));
            value.insert("secure".into(), Value::Bool(cookie.secure));
            value.insert("sameSite".into(), Value::String(cookie.same_site));
            if let Some(partition_key) = cookie.partition_key {
                value.insert("partitionKey".into(), Value::String(partition_key));
            }
            if let Some(ancestor) = cookie.partition_has_cross_site_ancestor {
                value.insert("_crHasCrossSiteAncestor".into(), Value::Bool(ancestor));
            }
            Value::Object(value)
        })
        .collect::<Vec<_>>();
    let origins = snapshot
        .origins
        .into_iter()
        .map(|origin| {
            let mut value = Map::new();
            value.insert("origin".into(), Value::String(origin.origin));
            value.insert(
                "localStorage".into(),
                Value::Array(
                    origin
                        .local_storage
                        .into_iter()
                        .map(|item| json!({ "name": item.name, "value": item.value }))
                        .collect(),
                ),
            );
            if indexed_db && let Some(databases) = origin.indexed_db {
                value.insert("indexedDB".into(), Value::Array(databases));
            }
            Value::Object(value)
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&json!({ "cookies": cookies, "origins": origins }))
        .map_err(|error| HostError::Backend(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_snapshot_keeps_the_playwright_storage_state_shape() {
        // Arrange
        let snapshot = ContextStorageStateSnapshot {
            cookies: Vec::new(),
            origins: Vec::new(),
        };

        // Act
        let output = snapshot_json(snapshot, false).expect("snapshot should serialize");

        // Assert
        assert_eq!(output, r#"{"cookies":[],"origins":[]}"#);
    }

    #[test]
    fn input_preserves_partitioned_cookie_fields() {
        // Arrange
        let input = r#"{"cookies":[{"name":"session","value":"value","domain":"example.test","path":"/","sameSite":"None","partitionKey":"https://top.test","_crHasCrossSiteAncestor":true}],"origins":[]}"#;

        // Act
        let state = parse_state_json(input).expect("storage state should parse");

        // Assert
        assert_eq!(state.cookies.len(), 1);
        assert_eq!(
            state.cookies[0].partition_key.as_deref(),
            Some("https://top.test")
        );
        assert_eq!(
            state.cookies[0].partition_has_cross_site_ancestor,
            Some(true)
        );
    }
}

#[napi]
impl NativeSession {
    #[napi]
    pub fn context_storage_state<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
        indexed_db: bool,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, FFI_ALLOWANCE)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            let snapshot = host.context_storage_state(&context_id, indexed_db).await?;
            snapshot_json(snapshot, indexed_db)
        })
    }

    #[napi]
    pub fn context_set_storage_state<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
        state_json: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, FFI_ALLOWANCE)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        let state_json = boundary::bounded_string(
            state_json,
            MAX_STORAGE_STATE_BYTES,
            "BrowserContext storage state",
        )?;
        let state = parse_state_json(&state_json).map_err(binding_error)?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.context_set_storage_state(&context_id, state).await
        })
    }
}
