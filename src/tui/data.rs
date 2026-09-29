//! Reading the facts the shell draws, without narrating and without asking.
//!
//! Two things separate this from [`crate::commands::load`], and both come from
//! the shell owning the screen:
//!
//! - **Nothing is printed.** Narration goes to stderr, which is where the
//!   shell draws, so a progress line would scribble across the frame. The
//!   widget says `reading…` instead.
//! - **Nothing is asked.** A prompt would draw over the screen it is asking in
//!   front of. When the child is ambiguous this fails with the command that
//!   settles it, which is a row in the menu beside the message.

use anyhow::{Result, bail};
use huckleberry_api::Huckleberry;
use huckleberry_api::client::now_seconds;

use crate::cli::Units;
use crate::domain::Calendar;
use crate::domain::types::Dataset;
use crate::interactive::session::{SessionOptions, load_context};
use crate::theme::Theme;

/// One read, or the reason there is not one.
pub async fn read(globals: &SessionOptions, theme: Theme) -> Result<(Dataset, Calendar, Units)> {
    // Reopened every time, so a settings change made from the menu takes
    // effect on the next refresh rather than on the next session.
    let context = load_context(globals, theme)?;
    let units = Units::from_setting(&context.config.units);
    if let Some(path) = &context.offline {
        let dataset = crate::dataset::read_snapshot(path)?;
        let calendar = Calendar::new(&dataset.timezone)?;
        return Ok((dataset, calendar, units));
    }
    let client: Huckleberry = context.client()?;
    let cid = child_without_asking(&context, &client).await?;
    let dataset = crate::dataset::pull(
        &client,
        &cid,
        None,
        context.days(None),
        &context.config.timezone,
        now_seconds(),
    )
    .await?;
    crate::commands::persist_session(&context, &client).await?;
    let calendar = Calendar::new(&dataset.timezone)?;
    Ok((dataset, calendar, units))
}

/// Starts a read in the background and hands back the handle to collect it.
///
/// The shell must stay usable while this runs. A read that cannot reach
/// Huckleberry takes a full minute to give up, and a menu that accepts no keys
/// for a minute is a menu that is broken exactly when the wifi is: the widgets
/// say `reading…`, the rows still move, and the answer arrives when it
/// arrives. Abandoning one costs nothing, so nothing here warns about it.
#[must_use]
pub fn start(globals: &SessionOptions, theme: Theme) -> Reading {
    let globals = globals.clone();
    tokio::spawn(async move { read(&globals, theme).await })
}

/// A read in flight.
pub type Reading = tokio::task::JoinHandle<Result<(Dataset, Calendar, Units)>>;

/// The child to draw, resolved without a question.
async fn child_without_asking(
    context: &crate::session::Context,
    client: &Huckleberry,
) -> Result<String> {
    if let Some(cid) =
        crate::session::resolve_child(context.child_override.as_deref(), context.config.child())
    {
        return Ok(cid);
    }
    let user = client.user().await?;
    match user.child_list.as_slice() {
        [] => bail!("this account has no children on it"),
        [only] => Ok(only.cid.clone()),
        _ => bail!("several children on this account: choose one with `child use`"),
    }
}
