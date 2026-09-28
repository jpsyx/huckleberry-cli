//! `users/{uid}`: the account, and the children on it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::common::Number;

/// One child, as the account lists them.
///
/// This is the cheap way to find a child's id: the full profile is a second
/// read against `childs/{cid}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserChildRef {
    /// The child id every other collection is keyed by.
    pub cid: String,
    /// What the account calls this child. The profile's `childsName` wins
    /// when there is one.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub nickname: Option<String>,
    /// A Firebase Storage filename.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub picture: Option<String>,
    /// The colour the app paints this child in.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub color: Option<String>,
}

/// A child reference under `hbChilds`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HbChildRef {
    /// When the child was added, as the app formats it.
    #[serde(rename = "addedAt")]
    pub added_at: String,
}

/// The subscription state on the account.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UserSubscription {
    /// Which plan.
    #[serde(
        rename = "type",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub kind: Option<Number>,
    /// The trial entitlement identifier.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub free_trial_entitlement: Option<String>,
    /// The trial plan identifier.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub free_trial_plan: Option<String>,
    /// Whether the app has shown the trial-expired dialog.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub trial_expired_modal: Option<bool>,
    /// When the trial started.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub free_trial_time: Option<Number>,
    /// When the subscription lapses.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub expiration: Option<Number>,
    /// When the trial lapses.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub free_trial_expiration: Option<Number>,
}

/// `users/{uid}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UserDocument {
    /// The account's email address.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub email: Option<String>,
    /// Given name.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub firstname: Option<String>,
    /// Family name.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub lastname: Option<String>,
    /// Every child on the account. This is the list `child list` prints.
    #[serde(rename = "childList", default)]
    pub child_list: Vec<UserChildRef>,
    /// The child the app last had open.
    #[serde(
        rename = "lastChild",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub last_child: Option<String>,
    /// When the child list last changed.
    #[serde(
        rename = "childrenUpdatedAt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub children_updated_at: Option<Number>,
    /// Children keyed by id, with when each was added.
    #[serde(
        rename = "hbChilds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub hb_childs: Option<BTreeMap<String, HbChildRef>>,
    /// Whether onboarding finished.
    #[serde(
        rename = "isOnboardingCompleted",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub is_onboarding_completed: Option<bool>,
    /// The timezone the app last reported.
    #[serde(
        rename = "latestTimezone",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub latest_timezone: Option<String>,
    /// Which platform the account signed up on.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub onboarding_platform: Option<String>,
    /// The subscription, when there is one.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub subscription: Option<UserSubscription>,
    /// Push tokens, keyed by device.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub tokens: Option<BTreeMap<String, String>>,
    /// Which tooltips have been dismissed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub tooltips: Option<BTreeMap<String, bool>>,
    /// Fields the app writes in more than one shape, kept untyped rather than
    /// modelled: `appsFlyerId` is a string or a list of them, `installedApps`
    /// a map or a list. Nothing in this crate reads them.
    #[serde(
        rename = "appsFlyerId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub apps_flyer_id: Option<serde_json::Value>,
    /// See [`Self::apps_flyer_id`].
    #[serde(
        rename = "installedApps",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub installed_apps: Option<serde_json::Value>,
}

impl UserDocument {
    /// The first child on the account, which is the one a single-child
    /// household means when it does not say.
    #[must_use]
    pub fn first_child(&self) -> Option<&UserChildRef> {
        self.child_list.first()
    }

    /// The child with this id.
    #[must_use]
    pub fn child(&self, cid: &str) -> Option<&UserChildRef> {
        self.child_list.iter().find(|child| child.cid == cid)
    }

    /// The account holder's name, as a person would write it.
    #[must_use]
    pub fn full_name(&self) -> String {
        [self.firstname.as_deref(), self.lastname.as_deref()]
            .into_iter()
            .flatten()
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn account() -> UserDocument {
        serde_json::from_value(json!({
            "email": "parent@example.com",
            "firstname": "Ada",
            "lastname": "Lovelace",
            "childList": [
                { "cid": "child-one", "nickname": "Bear" },
                { "cid": "child-two" },
            ],
            "somethingNobodyHasSeen": 42,
        }))
        .expect("an account document")
    }

    #[test]
    fn a_field_this_crate_does_not_model_is_ignored_rather_than_fatal() {
        assert_eq!(account().child_list.len(), 2);
    }

    #[test]
    fn the_first_child_is_the_one_a_single_child_household_means() {
        assert_eq!(account().first_child().expect("a child").cid, "child-one");
    }

    #[test]
    fn a_child_can_be_found_by_id() {
        assert!(account().child("child-two").is_some());
        assert!(account().child("child-three").is_none());
    }

    #[test]
    fn a_name_with_no_surname_has_no_trailing_space() {
        let user = UserDocument {
            firstname: Some("Ada".to_owned()),
            ..UserDocument::default()
        };
        assert_eq!(user.full_name(), "Ada");
    }

    #[test]
    fn a_subscription_expiry_is_a_timestamp_and_not_text() {
        let user: UserDocument = serde_json::from_value(json!({
            "childList": [{ "cid": "child-one" }],
            "subscription": { "free_trial_expiration": 1_789_657_330_i64 },
        }))
        .expect("an account with a trial on it");
        assert_eq!(
            user.subscription
                .and_then(|plan| plan.free_trial_expiration)
                .map(Number::as_i64),
            Some(1_789_657_330)
        );
    }

    #[test]
    fn a_field_of_a_surprising_type_does_not_cost_the_whole_account() {
        // Huckleberry's schema is not ours, and a field this crate reads
        // wrongly must not be the difference between seeing the account and
        // seeing nothing.
        let user: UserDocument = serde_json::from_value(json!({
            "firstname": "Ada",
            "childList": [{ "cid": "child-one" }],
            "latestTimezone": 12_345,
            "isOnboardingCompleted": "yes",
            "hbChilds": { "child-one": { "addedAt": 1_789_657_330_i64 } },
        }))
        .expect("an account with three surprises on it");
        assert_eq!(user.first_child().expect("a child").cid, "child-one");
        assert_eq!(user.firstname.as_deref(), Some("Ada"));
        assert_eq!(user.latest_timezone, None, "unreadable, so absent");
        assert_eq!(user.is_onboarding_completed, None);
        assert_eq!(user.hb_childs, None);
    }

    #[test]
    fn an_account_with_no_children_is_a_document_rather_than_a_failure() {
        let empty: UserDocument = serde_json::from_value(json!({})).expect("an empty document");
        assert!(empty.first_child().is_none());
    }
}
