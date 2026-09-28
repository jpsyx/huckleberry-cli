//! The client: a session, a timezone, and the Huckleberry surface.
//!
//! [`Huckleberry`] is the type a caller holds. It owns the session and renews
//! it when it has to, so no method on it ever fails for want of a fresh token
//! while a valid refresh token is in hand.
//!
//! The operations themselves are in [`crate::ops`], split by tracker, and they
//! are `impl Huckleberry` blocks rather than free functions so that the whole
//! surface reads as one type. This file holds only what they all share: the
//! session, the HTTP client, the Firestore handle, the timezone, and the
//! clock.

use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::Mutex;

use crate::auth::{self, Session};
use crate::constants;
use crate::error::{Error, Result};
use crate::firestore::Firestore;
use crate::timezone::Zone;

/// What a sign-in needs.
#[derive(Debug, Clone)]
pub struct Credentials {
    /// The account's email address.
    pub email: String,
    /// The account's password.
    pub password: String,
}

impl Credentials {
    /// An email and password pair.
    #[must_use]
    pub fn new(email: &str, password: &str) -> Self {
        Self {
            email: email.to_owned(),
            password: password.to_owned(),
        }
    }
}

/// A Huckleberry client.
///
/// Build one with [`Huckleberry::new`] from credentials, or with
/// [`Huckleberry::resume`] from a [`Session`] saved earlier. A resumed client
/// can renew itself from its refresh token; it cannot sign in again from
/// nothing, and says so with [`Error::NotAuthenticated`] if the refresh token
/// is rejected.
///
/// ```no_run
/// # async fn example() -> Result<(), huckleberry_api::Error> {
/// use huckleberry_api::{Credentials, Huckleberry};
///
/// let client = Huckleberry::new(
///     Credentials::new("parent@example.com", "hunter2"),
///     "America/New_York",
/// )?;
/// let user = client.user().await?;
/// let child = user.first_child().expect("a child on the account");
/// println!("{}", child.nickname.as_deref().unwrap_or(&child.cid));
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Huckleberry {
    http: reqwest::Client,
    firestore: Firestore,
    credentials: Option<Credentials>,
    zone: Zone,
    session: Mutex<Option<Session>>,
}

impl Huckleberry {
    /// A client that signs in with these credentials when it first needs to.
    ///
    /// Nothing happens on the network here: the first call that needs a token
    /// is the one that signs in. Call [`Huckleberry::authenticate`] to do it
    /// up front, which is what a `login` command wants.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownTimezone`] when the timezone name is not one the IANA
    /// database knows.
    pub fn new(credentials: Credentials, timezone: &str) -> Result<Self> {
        Ok(Self::with_parts(
            reqwest::Client::new(),
            constants::firestore_documents_url(),
            Some(credentials),
            Zone::new(timezone)?,
            None,
        ))
    }

    /// A client that picks up a session saved earlier.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownTimezone`] when the timezone name is not one the IANA
    /// database knows.
    pub fn resume(session: Session, timezone: &str) -> Result<Self> {
        Ok(Self::with_parts(
            reqwest::Client::new(),
            constants::firestore_documents_url(),
            None,
            Zone::new(timezone)?,
            Some(session),
        ))
    }

    /// A client from every part spelled out. The way to point the client at a
    /// stub server in a test, or to share one `reqwest::Client` across a
    /// program.
    #[must_use]
    pub fn with_parts(
        http: reqwest::Client,
        documents_url: String,
        credentials: Option<Credentials>,
        zone: Zone,
        session: Option<Session>,
    ) -> Self {
        Self {
            firestore: Firestore::new(http.clone(), documents_url),
            http,
            credentials,
            zone,
            session: Mutex::new(session),
        }
    }

    /// Adds credentials to a client that was resumed from a session, so it can
    /// sign in again rather than failing when the refresh token lapses.
    #[must_use]
    pub fn with_credentials(mut self, credentials: Credentials) -> Self {
        self.credentials = Some(credentials);
        self
    }

    /// Signs in now, rather than on the first call that needs a token.
    ///
    /// # Errors
    ///
    /// [`Error::SignIn`] when the credentials are refused,
    /// [`Error::NotAuthenticated`] when the client was built without any.
    pub async fn authenticate(&self) -> Result<Session> {
        let credentials = self.credentials.as_ref().ok_or(Error::NotAuthenticated)?;
        let session = auth::sign_in(
            &self.http,
            &credentials.email,
            &credentials.password,
            now_seconds(),
        )
        .await?;
        *self.session.lock().await = Some(session.clone());
        Ok(session)
    }

    /// The current session, if there is one.
    ///
    /// This is what a caller persists between runs so a person is not asked
    /// for their password every time. It contains a refresh token, so it is a
    /// credential and should be stored like one.
    pub async fn session(&self) -> Option<Session> {
        self.session.lock().await.clone()
    }

    /// Forgets the session. The next call signs in again, if there are
    /// credentials to sign in with.
    pub async fn sign_out(&self) {
        *self.session.lock().await = None;
    }

    /// The signed-in user's Firebase uid.
    ///
    /// # Errors
    ///
    /// Whatever signing in would fail with.
    pub async fn user_uid(&self) -> Result<String> {
        self.ensure_session().await.map(|session| session.user_uid)
    }

    /// The timezone this client records offsets in.
    #[must_use]
    pub const fn zone(&self) -> &Zone {
        &self.zone
    }

    /// The HTTP client, for the one read that is not Firestore: the curated
    /// food database in Firebase Storage.
    pub(crate) const fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// The Firestore handle the operations use.
    pub(crate) const fn firestore(&self) -> &Firestore {
        &self.firestore
    }

    /// A bearer token that will still be valid when the request arrives.
    ///
    /// Signs in when there is no session, renews when the one there is falls
    /// inside its last five minutes, and hands back what it has otherwise.
    ///
    /// # Errors
    ///
    /// [`Error::NotAuthenticated`] when there is neither a session nor
    /// credentials, and whatever the sign-in or the renewal failed with.
    pub(crate) async fn token(&self) -> Result<String> {
        self.ensure_session().await.map(|session| session.id_token)
    }

    /// The session, signing in or renewing first if it needs to.
    #[expect(
        clippy::significant_drop_tightening,
        reason = "the lock is held across the await deliberately: two callers finding an \
                  expired token must produce one sign-in between them, not one each"
    )]
    async fn ensure_session(&self) -> Result<Session> {
        // The lock is held across the sign-in and the renewal on purpose: two
        // concurrent calls finding an expired token should produce one
        // sign-in, not two.
        let mut held = self.session.lock().await;
        let now = now_seconds();

        let live = held
            .as_ref()
            .filter(|session| !auth::needs_refresh(session.expires_at, now))
            .cloned();
        if let Some(session) = live {
            return Ok(session);
        }

        // A refresh token that is still accepted is the cheap path, and the
        // only one available to a client resumed from a saved session.
        if let Some(existing) = held.as_ref() {
            match auth::refresh(&self.http, &existing.refresh_token, now).await {
                Ok(renewed) => {
                    *held = Some(renewed.clone());
                    return Ok(renewed);
                }
                Err(failure) if self.credentials.is_none() => return Err(failure),
                // A rejected refresh token with a password in hand is not a
                // failure: it is the moment to sign in again.
                Err(_) => {}
            }
        }

        let credentials = self.credentials.as_ref().ok_or(Error::NotAuthenticated)?;
        let session =
            auth::sign_in(&self.http, &credentials.email, &credentials.password, now).await?;
        *held = Some(session.clone());
        Ok(session)
    }
}

/// The wall clock, in seconds since the epoch.
///
/// Every operation that writes a timestamp takes `now` as an argument so the
/// value can be pinned in a test; this is what the public methods pass.
#[must_use]
pub fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(expires_at: i64) -> Session {
        Session {
            id_token: "id-token".to_owned(),
            refresh_token: "refresh-token".to_owned(),
            user_uid: "user-uid".to_owned(),
            expires_at,
        }
    }

    fn client(credentials: Option<Credentials>, session: Option<Session>) -> Huckleberry {
        Huckleberry::with_parts(
            reqwest::Client::new(),
            "http://127.0.0.1:1/documents".to_owned(),
            credentials,
            Zone::utc(),
            session,
        )
    }

    #[tokio::test]
    async fn a_live_session_is_handed_back_without_touching_the_network() {
        let far_future = now_seconds() as i64 + 3600;
        let client = client(None, Some(session(far_future)));
        assert_eq!(client.token().await.expect("a token"), "id-token");
    }

    #[tokio::test]
    async fn a_client_with_neither_a_session_nor_credentials_says_so() {
        assert!(matches!(
            client(None, None).token().await,
            Err(Error::NotAuthenticated)
        ));
    }

    #[tokio::test]
    async fn signing_out_forgets_the_session() {
        let far_future = now_seconds() as i64 + 3600;
        let client = client(None, Some(session(far_future)));
        assert!(client.session().await.is_some());
        client.sign_out().await;
        assert!(client.session().await.is_none());
    }

    #[tokio::test]
    async fn an_expired_session_with_no_way_to_renew_fails_rather_than_being_used() {
        // The refresh endpoint is unreachable at this address, so this also
        // pins that an expired token is never sent as-is.
        let client = client(None, Some(session(0)));
        assert!(client.token().await.is_err());
    }

    #[test]
    fn a_timezone_the_database_does_not_know_is_refused_at_construction() {
        let refused = Huckleberry::new(Credentials::new("a@b.c", "x"), "Mars/Olympus_Mons");
        assert!(matches!(refused, Err(Error::UnknownTimezone(_))));
    }

    #[test]
    fn the_clock_is_after_the_epoch_and_before_the_heat_death() {
        let now = now_seconds();
        assert!(now > 1_700_000_000.0 && now < 4_000_000_000.0);
    }
}
