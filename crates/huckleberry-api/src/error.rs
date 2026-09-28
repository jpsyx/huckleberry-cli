//! What can go wrong, as a type rather than as a string.
//!
//! A library that returns `anyhow::Error` forces every caller to match on
//! message text, so this crate returns one enum instead. The variants are the
//! distinctions a caller acts on differently: bad credentials are worth
//! re-prompting for, a permission denial on one collection is worth carrying
//! on past (several Huckleberry collections are simply not readable on some
//! accounts), and a network failure is worth retrying.

use std::fmt;

/// The result of anything in this crate that can fail.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything this crate can fail at.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The email and password were not accepted.
    #[error("Huckleberry rejected the sign-in: {0}")]
    SignIn(String),

    /// A call needed a session and there was not one.
    #[error("not signed in to Huckleberry")]
    NotAuthenticated,

    /// The session expired and could not be renewed.
    #[error("the Huckleberry session could not be refreshed: {0}")]
    Refresh(String),

    /// Firestore answered, and the answer was a failure.
    #[error("{operation} failed ({status}): {message}")]
    Api {
        /// What was being attempted, as a person would say it.
        operation: String,
        /// The HTTP status Firestore replied with.
        status: u16,
        /// Firestore's own message.
        message: String,
    },

    /// The request never got an answer.
    #[error("{operation} could not reach Huckleberry")]
    Network {
        /// What was being attempted.
        operation: String,
        /// The transport failure underneath.
        #[source]
        source: reqwest::Error,
    },

    /// The answer arrived in a shape this crate does not understand.
    #[error("{operation} returned something unexpected: {detail}")]
    Decode {
        /// What was being attempted.
        operation: String,
        /// What about the payload did not fit.
        detail: String,
    },

    /// The caller asked for something that cannot be written.
    #[error("{0}")]
    Invalid(String),

    /// The timezone name is not one the IANA database knows.
    #[error("unknown timezone `{0}`")]
    UnknownTimezone(String),
}

impl Error {
    /// Whether this is the account simply not being allowed to read something.
    ///
    /// Several Huckleberry collections (`activities`, `growth`, `medicine`)
    /// answer with a permission denial on accounts where the tracker was never
    /// enabled. A snapshot that aborts on the first one loses everything else,
    /// so callers pulling many collections test this and carry on.
    #[must_use]
    pub const fn is_permission_denied(&self) -> bool {
        matches!(self, Self::Api { status: 403, .. })
    }

    /// Whether the session is the problem, which is worth a fresh sign-in.
    #[must_use]
    pub const fn is_authentication(&self) -> bool {
        matches!(
            self,
            Self::SignIn(_)
                | Self::NotAuthenticated
                | Self::Refresh(_)
                | Self::Api { status: 401, .. }
        )
    }

    /// A failure while talking to Firestore, built from a status and a body.
    pub(crate) fn api(operation: impl fmt::Display, status: u16, body: &str) -> Self {
        Self::Api {
            operation: operation.to_string(),
            status,
            message: describe_failure(body),
        }
    }

    /// A transport failure, tagged with what it was trying to do.
    pub(crate) fn network(operation: impl fmt::Display, source: reqwest::Error) -> Self {
        Self::Network {
            operation: operation.to_string(),
            source,
        }
    }

    /// A payload this crate could not make sense of.
    pub(crate) fn decode(operation: impl fmt::Display, detail: impl fmt::Display) -> Self {
        Self::Decode {
            operation: operation.to_string(),
            detail: detail.to_string(),
        }
    }
}

/// Pulls the human part out of a Google error body.
///
/// Both Firestore and Identity Toolkit answer with
/// `{"error": {"message": "..."}}`, and the message is the only part worth
/// showing: the rest is a status code the caller already has. A body that is
/// not that shape is returned as it arrived.
#[must_use]
pub fn describe_failure(body: &str) -> String {
    let trimmed = body.trim();
    serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .and_then(|payload| {
            payload
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(|message| message.as_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| {
            if trimmed.is_empty() {
                "no message".to_owned()
            } else {
                trimmed.to_owned()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_google_error_body_is_reduced_to_its_message() {
        let body = r#"{"error":{"code":403,"message":"Missing or insufficient permissions.","status":"PERMISSION_DENIED"}}"#;
        assert_eq!(
            describe_failure(body),
            "Missing or insufficient permissions."
        );
    }

    #[test]
    fn a_body_that_is_not_json_is_shown_as_it_arrived() {
        assert_eq!(describe_failure("  502 Bad Gateway \n"), "502 Bad Gateway");
    }

    #[test]
    fn an_empty_body_says_so_rather_than_showing_nothing() {
        assert_eq!(describe_failure(""), "no message");
    }

    #[test]
    fn a_denial_is_recognised_so_a_snapshot_can_carry_on() {
        let denied = Error::api("reading activities", 403, "{}");
        assert!(denied.is_permission_denied());
        assert!(!denied.is_authentication());
    }

    #[test]
    fn an_expired_session_is_recognised_as_an_authentication_problem() {
        assert!(Error::api("reading sleep", 401, "{}").is_authentication());
        assert!(Error::NotAuthenticated.is_authentication());
    }

    #[test]
    fn a_server_failure_is_neither_a_denial_nor_an_authentication_problem() {
        let failure = Error::api("reading sleep", 503, "{}");
        assert!(!failure.is_permission_denied() && !failure.is_authentication());
    }
}
