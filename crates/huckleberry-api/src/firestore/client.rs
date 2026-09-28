//! Firestore over REST.
//!
//! The Python client this crate ports uses `google-cloud-firestore`, which
//! talks gRPC and expects Google credentials. There is no equivalent in Rust
//! that takes a bare Firebase ID token, and pulling a gRPC stack in to make
//! six kinds of request would be the largest thing in the dependency tree. So
//! this module speaks the Firestore REST API directly, which wants nothing but
//! an HTTP client and a bearer token.
//!
//! What that costs is real-time listeners: `Listen` is a bidirectional gRPC
//! stream with no REST equivalent, so `setup_*_listener` in the Python client
//! becomes polling here. See [`crate::watch`].
//!
//! Everything below is one of five requests:
//!
//! | Method | Firestore | Used for |
//! | --- | --- | --- |
//! | [`Firestore::get`] | `GET .../{path}` | one document |
//! | [`Firestore::query`] | `POST .../{parent}:runQuery` | a filtered subcollection |
//! | [`Firestore::list`] | `GET .../{parent}/{collection}` | a whole subcollection, paged |
//! | [`Firestore::set`] | `PATCH .../{path}` | write or overwrite |
//! | [`Firestore::update`] | `PATCH .../{path}` with a mask | change named fields |

use serde_json::{Map, Value as Json};

use super::field_path::{self, FieldUpdate};
use super::query::{self, Query};
use super::value;
use crate::error::{Error, Result, describe_failure};

/// One document, as this crate hands it on: its id and its contents as plain
/// JSON, with the Firestore tagging already gone.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    /// The last segment of the document's Firestore name.
    pub id: String,
    /// The contents, untagged.
    pub fields: Map<String, Json>,
}

impl Document {
    /// The contents as a JSON value, for handing to `serde_json::from_value`.
    #[must_use]
    pub fn into_json(self) -> Json {
        Json::Object(self.fields)
    }
}

/// The Firestore REST surface, given a base URL and an HTTP client.
///
/// It holds no session. Every call takes the bearer token as its first
/// argument, because renewing that token is [`crate::auth`]'s job and this
/// module has no opinion on when it happens.
#[derive(Debug, Clone)]
pub struct Firestore {
    http: reqwest::Client,
    documents_url: String,
}

impl Firestore {
    /// A client against a documents root, for example the URL
    /// [`crate::constants::firestore_documents_url`] builds.
    #[must_use]
    pub const fn new(http: reqwest::Client, documents_url: String) -> Self {
        Self {
            http,
            documents_url,
        }
    }

    /// One document, or `None` when it does not exist.
    ///
    /// A missing document is not a failure: `sleep/{child}` does not exist
    /// until the first sleep is logged, and a caller asking "is a timer
    /// running" wants `None`, not an error.
    ///
    /// # Errors
    ///
    /// [`Error::Api`] when Firestore refuses the request (403 for a tracker
    /// this account cannot read, 401 for an expired token),
    /// [`Error::Network`] when it cannot be reached, and [`Error::Decode`]
    /// when the reply is not JSON.
    pub async fn get(&self, token: &str, path: &str, operation: &str) -> Result<Option<Document>> {
        let response = self
            .http
            .get(self.url(path))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|source| Error::network(operation, source))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let payload = read_json(response, operation).await?;
        Ok(Some(decode_document(&payload)))
    }

    /// The documents a structured query matches.
    ///
    /// # Errors
    ///
    /// [`Error::Api`] when Firestore refuses the request (403 for a tracker
    /// this account cannot read, 401 for an expired token),
    /// [`Error::Network`] when it cannot be reached, and [`Error::Decode`]
    /// when the reply is not JSON.
    pub async fn query(
        &self,
        token: &str,
        parent: &str,
        query: &Query,
        operation: &str,
    ) -> Result<Vec<Document>> {
        let response = self
            .http
            .post(format!("{}:runQuery", self.url(parent)))
            .json(&query::body(query))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|source| Error::network(operation, source))?;
        let payload = read_json(response, operation).await?;
        Ok(decode_query_results(&payload))
    }

    /// Every document in a subcollection, following the pages to the end.
    ///
    /// `query` would answer the same question in one request, but a
    /// `runQuery` response has to fit in one reply. This is the safe read for
    /// a collection whose size nobody has promised anything about.
    ///
    /// # Errors
    ///
    /// [`Error::Api`] when Firestore refuses the request (403 for a tracker
    /// this account cannot read, 401 for an expired token),
    /// [`Error::Network`] when it cannot be reached, and [`Error::Decode`]
    /// when the reply is not JSON.
    pub async fn list(
        &self,
        token: &str,
        parent: &str,
        collection: &str,
        operation: &str,
    ) -> Result<Vec<Document>> {
        let mut documents = Vec::new();
        let mut page_token: Option<String> = None;
        loop {
            let mut request = self
                .http
                .get(format!("{}/{}", self.url(parent), encode_path(collection)))
                .query(&[("pageSize", "300")]);
            if let Some(token) = &page_token {
                request = request.query(&[("pageToken", token.as_str())]);
            }
            let response = request
                .bearer_auth(token)
                .send()
                .await
                .map_err(|source| Error::network(operation, source))?;
            let payload = read_json(response, operation).await?;
            if let Some(page) = payload.get("documents").and_then(Json::as_array) {
                documents.extend(page.iter().map(decode_document));
            }
            page_token = payload
                .get("nextPageToken")
                .and_then(Json::as_str)
                .filter(|token| !token.is_empty())
                .map(ToOwned::to_owned);
            if page_token.is_none() {
                return Ok(documents);
            }
        }
    }

    /// Writes a document, replacing whatever was there.
    ///
    /// # Errors
    ///
    /// [`Error::Api`] when Firestore refuses the request (403 for a tracker
    /// this account cannot read, 401 for an expired token),
    /// [`Error::Network`] when it cannot be reached, and [`Error::Decode`]
    /// when the reply is not JSON.
    pub async fn set(
        &self,
        token: &str,
        path: &str,
        fields: &Map<String, Json>,
        operation: &str,
    ) -> Result<()> {
        self.patch(token, path, fields, None, false, operation)
            .await
    }

    /// Writes a document, leaving fields the payload does not mention alone.
    ///
    /// This is `set(..., merge=True)`: the mask names every leaf of the
    /// payload, so a sibling key that is not in `fields` survives the write.
    ///
    /// # Errors
    ///
    /// [`Error::Api`] when Firestore refuses the request (403 for a tracker
    /// this account cannot read, 401 for an expired token),
    /// [`Error::Network`] when it cannot be reached, and [`Error::Decode`]
    /// when the reply is not JSON.
    pub async fn merge(
        &self,
        token: &str,
        path: &str,
        fields: &Map<String, Json>,
        operation: &str,
    ) -> Result<()> {
        let plain: Map<String, Json> = value::fields_to_json(fields);
        let mask = field_path::leaves(&plain);
        self.patch(token, path, fields, Some(mask), false, operation)
            .await
    }

    /// Changes named fields of a document that already exists.
    ///
    /// A [`FieldUpdate`] carrying `None` deletes its field. The document is
    /// required to exist, which is what makes "pause the running sleep" fail
    /// loudly rather than quietly creating a sleep document with one field in
    /// it.
    ///
    /// # Errors
    ///
    /// [`Error::Api`] when Firestore refuses the request (403 for a tracker
    /// this account cannot read, 401 for an expired token),
    /// [`Error::Network`] when it cannot be reached, and [`Error::Decode`]
    /// when the reply is not JSON.
    pub async fn update(
        &self,
        token: &str,
        path: &str,
        updates: &[FieldUpdate],
        operation: &str,
    ) -> Result<()> {
        let (fields, mask) = field_path::document_and_mask(updates);
        self.patch(token, path, &fields, Some(mask), true, operation)
            .await
    }

    /// The one write request the four methods above differ only in the
    /// arguments to.
    #[allow(
        clippy::too_many_arguments,
        reason = "one private request builder, not a public surface"
    )]
    async fn patch(
        &self,
        token: &str,
        path: &str,
        fields: &Map<String, Json>,
        mask: Option<Vec<String>>,
        require_existing: bool,
        operation: &str,
    ) -> Result<()> {
        let mut request = self.http.patch(self.url(path));
        for entry in mask.unwrap_or_default() {
            request = request.query(&[("updateMask.fieldPaths", entry)]);
        }
        if require_existing {
            request = request.query(&[("currentDocument.exists", "true")]);
        }
        let response = request
            .json(&serde_json::json!({ "fields": fields }))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|source| Error::network(operation, source))?;
        read_json(response, operation).await.map(|_| ())
    }

    /// The absolute URL of a document or collection path.
    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.documents_url, encode_path(path))
    }
}

/// Percent-encodes a path, leaving the separators alone.
///
/// Document ids are Huckleberry's own (`1758572400000-3f2a...`) and need no
/// encoding in practice, but a path segment is user data the moment a custom
/// food id or a collection name comes from somewhere else.
#[must_use]
pub fn encode_path(path: &str) -> String {
    path.split('/')
        .map(encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

/// Percent-encodes one path segment.
#[must_use]
pub fn encode_segment(segment: &str) -> String {
    use core::fmt::Write as _;

    let mut encoded = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            // Writing into a `String` cannot fail, so the result is dropped
            // rather than turned into an error nobody could act on.
            other => {
                let _ = write!(encoded, "%{other:02X}");
            }
        }
    }
    encoded
}

/// The id of a document, from its full Firestore name.
#[must_use]
pub fn document_id(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).to_owned()
}

/// Turns one document payload into a [`Document`].
#[must_use]
pub fn decode_document(payload: &Json) -> Document {
    Document {
        id: payload
            .get("name")
            .and_then(Json::as_str)
            .map(document_id)
            .unwrap_or_default(),
        fields: payload
            .get("fields")
            .and_then(Json::as_object)
            .map(value::fields_to_json)
            .unwrap_or_default(),
    }
}

/// Turns a `runQuery` reply into the documents it found.
///
/// The reply is a list, and not every element carries a document: an empty
/// result is one element with only a `readTime`, and a transactional read
/// leads with a `transaction`. Both are skipped rather than decoded into a
/// document with no fields.
#[must_use]
pub fn decode_query_results(payload: &Json) -> Vec<Document> {
    payload
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("document"))
                .map(decode_document)
                .collect()
        })
        .unwrap_or_default()
}

/// Reads a reply, turning a failing status into an [`Error::Api`].
async fn read_json(response: reqwest::Response, operation: &str) -> Result<Json> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|source| Error::network(operation, source))?;
    if !status.is_success() {
        return Err(Error::api(operation, status.as_u16(), &body));
    }
    if body.trim().is_empty() {
        return Ok(Json::Null);
    }
    serde_json::from_str(&body)
        .map_err(|error| Error::decode(operation, format!("{error}: {}", describe_failure(&body))))
}

#[cfg(test)]
mod paths {
    use super::*;

    #[test]
    fn an_ordinary_path_passes_through_unchanged() {
        assert_eq!(
            encode_path("sleep/abc123/intervals"),
            "sleep/abc123/intervals"
        );
    }

    #[test]
    fn a_separator_inside_a_segment_would_be_encoded_rather_than_split() {
        assert_eq!(encode_segment("a/b"), "a%2Fb");
    }

    #[test]
    fn an_interval_id_needs_no_encoding() {
        let id = "1758572400000-3f2a9b8c7d6e5f4a3b2c";
        assert_eq!(encode_segment(id), id);
    }

    #[test]
    fn a_space_is_encoded() {
        assert_eq!(encode_segment("Breast Milk"), "Breast%20Milk");
    }

    #[test]
    fn the_id_is_the_last_segment_of_the_name() {
        let name = "projects/simpleintervals/databases/(default)/documents/sleep/abc/intervals/xyz";
        assert_eq!(document_id(name), "xyz");
    }
}

#[cfg(test)]
mod decoding {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_document_arrives_untagged() {
        let payload = json!({
            "name": "projects/p/databases/(default)/documents/sleep/abc/intervals/i1",
            "fields": {
                "start": { "integerValue": "1758572400" },
                "duration": { "doubleValue": 3600.0 },
            },
        });
        let document = decode_document(&payload);
        assert_eq!(document.id, "i1");
        assert_eq!(
            document.into_json(),
            json!({ "start": 1_758_572_400, "duration": 3600.0 })
        );
    }

    #[test]
    fn a_query_reply_yields_one_document_per_entry() {
        let payload = json!([
            { "document": { "name": "a/b/c/i1", "fields": { "multi": { "booleanValue": true } } } },
            { "document": { "name": "a/b/c/i2", "fields": {} } },
        ]);
        let documents = decode_query_results(&payload);
        assert_eq!(documents.len(), 2);
        assert_eq!(documents[0].id, "i1");
    }

    #[test]
    fn an_empty_result_is_no_documents_rather_than_one_empty_one() {
        let payload = json!([{ "readTime": "2025-09-22T18:00:00Z" }]);
        assert!(decode_query_results(&payload).is_empty());
    }

    #[test]
    fn a_document_with_no_fields_decodes_to_an_empty_map() {
        assert!(decode_document(&json!({ "name": "a/b" })).fields.is_empty());
    }
}
