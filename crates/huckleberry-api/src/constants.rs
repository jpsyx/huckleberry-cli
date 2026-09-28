//! The Firebase project Huckleberry runs on.
//!
//! These are the values the Huckleberry mobile app ships, and they identify
//! the project rather than the person: a Firebase web API key is a public
//! identifier, not a secret, and every request still has to carry a signed-in
//! user's token. They are constants here for the same reason they are
//! constants in the app: there is one Huckleberry.

/// The Firebase web API key the Huckleberry app uses.
pub const FIREBASE_API_KEY: &str = "AIzaSyApGVHktXeekGyAt-G6dIeWHUkq2oXqcjg";

/// The Firebase project id.
pub const FIREBASE_PROJECT_ID: &str = "simpleintervals";

/// The Firebase app id.
pub const FIREBASE_APP_ID: &str = "1:219218185774:android:a3e215cc246b92b0";

/// Where a password is exchanged for a session.
pub const SIGN_IN_URL: &str =
    "https://identitytoolkit.googleapis.com/v1/accounts:signInWithPassword";

/// Where a refresh token is exchanged for a fresh session.
pub const REFRESH_URL: &str = "https://securetoken.googleapis.com/v1/token";

/// The Storage bucket holding the curated solids food database.
pub const CURATED_FOODS_BUCKET: &str = "simpleintervals.appspot.com";

/// The object inside that bucket.
pub const CURATED_FOODS_OBJECT: &str = "foods/fooddb.json";

/// The root of the Firestore REST surface for the default database.
#[must_use]
pub fn firestore_documents_url() -> String {
    format!(
        "https://firestore.googleapis.com/v1/projects/{FIREBASE_PROJECT_ID}/databases/(default)/documents"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_documents_url_names_the_default_database() {
        assert_eq!(
            firestore_documents_url(),
            "https://firestore.googleapis.com/v1/projects/simpleintervals/databases/(default)/documents"
        );
    }
}
