//! Read the picker and its original entry identities before opening a draft.
use super::{
    history, live,
    retained::{EditForm, EditSubmission, EditTarget},
};
use crate::{
    cli::{EditOptions, Units},
    commands::prepared::{CommandProgress, PreparedCommand},
    domain::log,
    edit,
    prompt::{self, flow::runner},
    render::format,
    session::Context,
};
use anyhow::{Context as _, Result, bail};
use huckleberry_api::{Huckleberry, client::now_seconds};
/// Prepares all read-only context without keeping a prompt worker alive.
pub async fn prepare_with_client(
    context: Context,
    client: Huckleberry,
    cid: String,
    options: EditOptions,
    now: f64,
) -> Result<PreparedCommand> {
    let model = create_model(&context, &client, &cid, &options, now).await?;
    prepare_model(context, client, cid, model, now)
}
/// Binds an owned draft to the existing sparse history or live timer save.
pub fn prepare_model(
    context: Context,
    client: Huckleberry,
    cid: String,
    model: EditForm,
    now: f64,
) -> Result<PreparedCommand> {
    PreparedCommand::create_prepared(model, now, move |answer| async move {
        execute(&context, &client, &cid, answer).await?;
        Ok(CommandProgress::Finished { home: false })
    })
}
pub(super) async fn run(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    options: &EditOptions,
) -> Result<()> {
    if options.id.is_none() && !prompt::available() {
        bail!("no entry to change: pass --id <ENTRY> (`edit --list` prints the list)");
    }
    if options.set.is_empty() && !prompt::available() {
        bail!("no changes supplied: pass --set at=<TIME> or --set <FIELD=VALUE>");
    }
    let now = now_seconds();
    let answer = runner::read(
        create_model(context, client, cid, options, now).await?,
        now,
        context.theme,
    )?;
    execute(context, client, cid, answer).await
}
async fn execute(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    answer: EditSubmission,
) -> Result<()> {
    if answer.target.timer.is_some() {
        live::execute(context, client, cid, answer).await
    } else {
        history::execute(context, client, cid, answer).await
    }
}
async fn create_model(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    options: &EditOptions,
    now: f64,
) -> Result<EditForm> {
    let calendar = context.calendar()?;
    if options.id.as_deref() == Some(live::TOKEN) {
        let target = live_target(client, cid, now).await?;
        return EditForm::create_edit(
            calendar,
            vec![],
            vec![target],
            options.id.clone(),
            options.set.clone(),
        );
    }
    let window = context.days(options.days);
    let dataset =
        crate::dataset::pull(client, cid, None, window, &context.config.timezone, now).await?;
    let mut entries = log::build(&dataset, |amount| {
        format::volume(amount, Units::from_setting(&context.config.units))
    });
    history::add_health(context, client, cid, window, &mut entries).await?;
    let mut targets = entries
        .iter()
        .filter_map(|entry| {
            Some(EditTarget {
                token: edit::token_for(entry.at.as_ref()?),
                started: entry.start,
                draft: edit::draft_for(
                    &dataset,
                    entry.at.as_ref()?,
                    Units::from_setting(&context.config.units),
                ),
                timer: None,
            })
        })
        .collect::<Vec<_>>();
    entries.truncate(options.limit);
    if let Some(entry) = live::entry(&dataset.live, now) {
        entries.insert(0, entry);
        targets.push(live_target(client, cid, now).await?);
    }
    if let Some(token) = &options.id
        && !targets.iter().any(|target| target.token == *token)
    {
        let at = edit::parse_token(token)?;
        targets.push(EditTarget {
            token: token.clone(),
            started: history::started_at(client, cid, &[], &at).await?,
            draft: None,
            timer: None,
        });
    }
    if targets.is_empty() {
        bail!("nothing logged in this window: try a longer --days");
    }
    let rows = crate::render::log::rows(&entries, &calendar, now, &|entry| {
        (!super::editable(entry)).then(|| "this entry has no stored location".into())
    });
    let known = if targets
        .iter()
        .any(|target| matches!(target.draft, Some(crate::edit::Draft::Solids(_))))
    {
        client.custom_foods(cid, false).await.unwrap_or_default()
    } else {
        vec![]
    };
    EditForm::create_edit(
        calendar,
        rows,
        targets,
        options.id.clone(),
        options.set.clone(),
    )
    .map(|model| model.with_foods(&known))
}
async fn live_target(client: &Huckleberry, cid: &str, now: f64) -> Result<EditTarget> {
    let timer = client
        .sleep_document(cid)
        .await?
        .and_then(|document| document.timer)
        .filter(|timer| timer.active)
        .context("no ongoing sleep to edit")?;
    Ok(EditTarget {
        token: live::TOKEN.into(),
        started: timer.started_at().unwrap_or(now),
        draft: None,
        timer: Some(timer),
    })
}
