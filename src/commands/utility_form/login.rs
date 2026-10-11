//! Credentials remain private owned values until the entire sign-in is accepted.
use crate::{
    domain::Calendar,
    prompt::flow::{FormModel, FormValue, View, ViewId, fields},
};
use anyhow::{Result, bail};

/// Explicit or previously resolved credentials suppress their questions.
#[derive(Default, Clone)]
pub struct LoginInput {
    /// Account email.
    pub email: Option<String>,
    /// Password, deliberately excluded from Debug.
    pub password: Option<String>,
    /// Explicit or configured timezone.
    pub timezone: Option<String>,
}
/// A fully collected sign-in, deliberately excluded from Debug.
pub struct LoginSubmission {
    /// Account email.
    pub email: String,
    /// Secret password.
    pub password: String,
    /// Validated IANA timezone.
    pub timezone: String,
}
/// Sign-in collection never authenticates or saves partial credentials.
pub struct LoginForm {
    given: LoginInput,
    draft: LoginInput,
}
impl LoginForm {
    /// Validates supplied values without revealing secrets.
    pub fn create_login(given: LoginInput) -> Result<Self> {
        if let Some(zone) = &given.timezone {
            Calendar::new(zone)?;
        }
        Ok(Self {
            draft: given.clone(),
            given,
        })
    }
}
impl FormModel for LoginForm {
    type Output = LoginSubmission;
    fn route(&self) -> Vec<ViewId> {
        [
            ("email", &self.given.email),
            ("password", &self.given.password),
            ("timezone", &self.given.timezone),
        ]
        .into_iter()
        .filter(|(_, value)| value.is_none())
        .map(|(field, _)| fields::create_id(field))
        .collect()
    }
    fn create_view(&self, id: &ViewId, _: f64) -> Result<View> {
        let (label, value, secret) = match id.field {
            "email" => (
                "Account email?",
                self.draft.email.as_deref().unwrap_or_default(),
                false,
            ),
            "password" => (
                "Password?",
                self.draft.password.as_deref().unwrap_or_default(),
                true,
            ),
            "timezone" => (
                "Which timezone does the family keep? (e.g. America/New_York)",
                self.draft.timezone.as_deref().unwrap_or("UTC"),
                false,
            ),
            _ => bail!("unknown sign-in field"),
        };
        Ok(View::create_text(label, value, secret))
    }
    fn accept(&mut self, id: &ViewId, value: FormValue, _: f64) -> Result<()> {
        let text = fields::text(value)?;
        if text.trim().is_empty() {
            bail!("enter {}", id.field);
        }
        match id.field {
            "email" => self.draft.email = Some(text),
            "password" => self.draft.password = Some(text),
            "timezone" => {
                Calendar::new(&text)?;
                self.draft.timezone = Some(text);
            }
            _ => bail!("unknown sign-in field"),
        }
        Ok(())
    }
    fn complete(&self) -> Result<LoginSubmission> {
        let required = |value: &Option<String>, flag: &str| match value {
            Some(value) if !value.trim().is_empty() => Ok(value.clone()),
            _ => bail!("provide --{flag}"),
        };
        Ok(LoginSubmission {
            email: required(&self.draft.email, "email <ADDRESS>")?,
            password: required(&self.draft.password, "password <PASSWORD>")?,
            timezone: self.draft.timezone.clone().unwrap_or_else(|| "UTC".into()),
        })
    }
}
