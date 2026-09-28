//! Signing in, and keeping the session alive.
//!
//! Huckleberry authenticates through Firebase Identity Toolkit: an email and
//! password are exchanged for a short-lived ID token, a long-lived refresh
//! token, and the user's Firebase uid. Every Firestore request afterwards
//! carries the ID token as a bearer token.
//!
//! The ID token lasts an hour. This module renews it five minutes early rather
//! than on expiry, because a request that takes the last of that hour would
//! otherwise be rejected by the time it arrived. The decision of *when* to
//! renew is [`needs_refresh`], a pure function, so the awkward boundary cases
//! are tested rather than waited for.
//!
//! [`Session`] is serializable on purpose: a command-line tool that signs in
//! on every invocation would be asking for a password on every invocation. A
//! caller that stores one is storing a credential and should say so with the
//! file permissions it uses.

use serde::{Deserialize, Serialize};

use crate::constants::{FIREBASE_API_KEY, REFRESH_URL, SIGN_IN_URL};
use crate::error::{Error, Result, describe_failure};

/// How long before expiry a token is renewed.
///
/// Matches the Python client: long enough that an in-flight request cannot
/// outlive its own token, short enough that a session is not renewed on every
/// other call.
pub const RENEW_BEFORE_EXPIRY_SECONDS: f64 = 300.0;

/// An authenticated Huckleberry session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// The bearer token every Firestore request carries.
    pub id_token: String,
    /// The long-lived token that buys a new `id_token`.
    pub refresh_token: String,
    /// The signed-in user's Firebase uid, which is their `users/{uid}` id.
    pub user_uid: String,
    /// When `id_token` stops being accepted, as a Unix timestamp in seconds.
    /// Stored as a whole second so the session survives a round trip through
    /// a configuration file without drifting.
    pub expires_at: i64,
}

/// Whether the session should be renewed before it is used again.
#[must_use]
pub fn needs_refresh(expires_at: i64, now: f64) -> bool {
    now >= expires_at as f64 - RENEW_BEFORE_EXPIRY_SECONDS
}

/// How long a session has left, in seconds, floored at zero.
#[must_use]
pub fn seconds_remaining(expires_at: i64, now: f64) -> i64 {
    let remaining = expires_at - now as i64;
    remaining.max(0)
}

/// Reads the sign-in reply.
///
/// `expiresIn` arrives as a string of seconds, which is why this is not a
/// plain `Deserialize`.
///
/// # Errors
///
/// [`Error::Decode`] when the reply is missing a token or the user id.
pub fn parse_sign_in(payload: &serde_json::Value, now: f64) -> Result<Session> {
    let field = |name: &str| {
        payload
            .get(name)
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .ok_or_else(|| Error::decode("signing in", format!("no `{name}` in the reply")))
    };
    Ok(Session {
        id_token: field("idToken")?,
        refresh_token: field("refreshToken")?,
        user_uid: field("localId")?,
        expires_at: expiry_from(payload.get("expiresIn"), now),
    })
}

/// Reads the refresh reply, which names the same values in snake case.
///
/// # Errors
///
/// [`Error::Decode`] when the reply is missing a token or the user id.
pub fn parse_refresh(payload: &serde_json::Value, now: f64) -> Result<Session> {
    let field = |name: &str| {
        payload
            .get(name)
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .ok_or_else(|| {
                Error::decode(
                    "refreshing the session",
                    format!("no `{name}` in the reply"),
                )
            })
    };
    Ok(Session {
        id_token: field("id_token")?,
        refresh_token: field("refresh_token")?,
        user_uid: field("user_id")?,
        expires_at: expiry_from(payload.get("expires_in"), now),
    })
}

/// Turns the reply's lifetime into an absolute expiry. A lifetime that is
/// missing or unreadable is treated as already expired, so the next call
/// renews rather than sending a token of unknown age.
fn expiry_from(lifetime: Option<&serde_json::Value>, now: f64) -> i64 {
    let seconds = lifetime
        .and_then(|value| {
            value
                .as_str()
                .and_then(|text| text.parse::<i64>().ok())
                .or_else(|| value.as_i64())
        })
        .unwrap_or(0);
    now as i64 + seconds
}

/// Exchanges an email and password for a session.
///
/// # Errors
///
/// [`Error::SignIn`] when the credentials are refused, [`Error::Network`]
/// when Huckleberry cannot be reached, [`Error::Decode`] when the reply is
/// not the shape Identity Toolkit documents.
pub async fn sign_in(
    http: &reqwest::Client,
    email: &str,
    password: &str,
    now: f64,
) -> Result<Session> {
    let response = http
        .post(format!("{SIGN_IN_URL}?key={FIREBASE_API_KEY}"))
        .json(&serde_json::json!({
            "email": email,
            "password": password,
            "returnSecureToken": true,
        }))
        .send()
        .await
        .map_err(|source| Error::network("signing in", source))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|source| Error::network("signing in", source))?;
    if !status.is_success() {
        return Err(Error::SignIn(describe_failure(&body)));
    }
    let payload =
        serde_json::from_str(&body).map_err(|error| Error::decode("signing in", error))?;
    parse_sign_in(&payload, now)
}

/// Exchanges a refresh token for a fresh session.
///
/// # Errors
///
/// [`Error::Refresh`] when the token is no longer accepted, which means the
/// password has to be entered again. [`Error::Network`] and
/// [`Error::Decode`] as for [`sign_in`].
pub async fn refresh(http: &reqwest::Client, refresh_token: &str, now: f64) -> Result<Session> {
    let response = http
        .post(format!("{REFRESH_URL}?key={FIREBASE_API_KEY}"))
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
        }))
        .send()
        .await
        .map_err(|source| Error::network("refreshing the session", source))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|source| Error::network("refreshing the session", source))?;
    if !status.is_success() {
        return Err(Error::Refresh(describe_failure(&body)));
    }
    let payload = serde_json::from_str(&body)
        .map_err(|error| Error::decode("refreshing the session", error))?;
    parse_refresh(&payload, now)
}

#[cfg(test)]
mod renewal {
    use super::*;

    const EXPIRES_AT: i64 = 10_000;

    #[test]
    fn a_fresh_session_is_left_alone() {
        assert!(!needs_refresh(EXPIRES_AT, 1_000.0));
    }

    #[test]
    fn a_session_inside_the_last_five_minutes_is_renewed_early() {
        assert!(needs_refresh(EXPIRES_AT, 9_701.0));
    }

    #[test]
    fn the_five_minute_boundary_itself_renews() {
        assert!(needs_refresh(EXPIRES_AT, 9_700.0));
    }

    #[test]
    fn a_second_before_the_boundary_does_not() {
        assert!(!needs_refresh(EXPIRES_AT, 9_699.0));
    }

    #[test]
    fn an_expired_session_is_renewed() {
        assert!(needs_refresh(EXPIRES_AT, 20_000.0));
    }

    #[test]
    fn time_left_never_goes_negative() {
        assert_eq!(seconds_remaining(EXPIRES_AT, 9_000.0), 1_000);
        assert_eq!(seconds_remaining(EXPIRES_AT, 20_000.0), 0);
    }
}

#[cfg(test)]
mod replies {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_sign_in_reply_becomes_a_session_that_expires_when_it_says() {
        let payload = json!({
            "idToken": "id-token",
            "refreshToken": "refresh-token",
            "localId": "user-uid",
            "expiresIn": "3600",
        });
        let session = parse_sign_in(&payload, 1_000.0).expect("a complete reply");
        assert_eq!(session.user_uid, "user-uid");
        assert_eq!(session.expires_at, 4_600);
    }

    #[test]
    fn a_refresh_reply_names_the_same_values_in_snake_case() {
        let payload = json!({
            "id_token": "id-token",
            "refresh_token": "refresh-token",
            "user_id": "user-uid",
            "expires_in": "3600",
        });
        let session = parse_refresh(&payload, 1_000.0).expect("a complete reply");
        assert_eq!(session.id_token, "id-token");
        assert_eq!(session.expires_at, 4_600);
    }

    #[test]
    fn a_reply_missing_a_token_is_a_decode_failure_rather_than_a_panic() {
        let payload = json!({ "refreshToken": "refresh-token", "localId": "uid" });
        assert!(matches!(
            parse_sign_in(&payload, 0.0),
            Err(Error::Decode { .. })
        ));
    }

    #[test]
    fn a_reply_with_no_lifetime_is_treated_as_already_expired() {
        let payload = json!({
            "idToken": "id-token",
            "refreshToken": "refresh-token",
            "localId": "user-uid",
        });
        let session = parse_sign_in(&payload, 1_000.0).expect("a reply without a lifetime");
        assert!(needs_refresh(session.expires_at, 1_000.0));
    }

    #[test]
    fn a_session_survives_a_round_trip_through_a_configuration_file() {
        let session = Session {
            id_token: "id-token".to_owned(),
            refresh_token: "refresh-token".to_owned(),
            user_uid: "user-uid".to_owned(),
            expires_at: 4_600,
        };
        let text = serde_json::to_string(&session).expect("serializing");
        assert_eq!(
            serde_json::from_str::<Session>(&text).expect("parsing"),
            session
        );
    }
}
