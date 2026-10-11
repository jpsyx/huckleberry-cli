//! Shared diaper and potty collection, followed by one consuming recording operation.
use super::{
    diaper_form::{DiaperForm, DiaperInput, DiaperSubmission},
    prepared::{CommandProgress, PreparedCommand},
};
use crate::{
    cli::{Colour, Consistency, DiaperKind, PottyOutcome, actions::DiaperFlags},
    prompt::{
        self,
        flow::{FormModel, FormState, runner},
    },
    render::output,
    session::Context,
};
use anyhow::Result;
use huckleberry_api::{DiaperDetails, Huckleberry};

/// Records a diaper using the same retained form as the shell.
pub async fn diaper(
    context: &Context,
    mode: Option<DiaperKind>,
    flags: DiaperFlags,
    notes: Option<&str>,
    at: Option<&str>,
) -> Result<()> {
    run_input(
        context,
        DiaperInput {
            mode,
            flags,
            notes: notes.map(str::to_owned),
            ..DiaperInput::default()
        },
        at,
    )
    .await
}

/// Records a potty trip, which has an outcome and no rash question.
pub async fn potty(
    context: &Context,
    mode: Option<DiaperKind>,
    how: Option<PottyOutcome>,
    color: Option<Colour>,
    consistency: Option<Consistency>,
    notes: Option<&str>,
    at: Option<&str>,
) -> Result<()> {
    run_input(
        context,
        DiaperInput {
            mode,
            outcome: how,
            potty: true,
            flags: DiaperFlags {
                pee: None,
                poo: None,
                color,
                consistency,
                rash: false,
            },
            notes: notes.map(str::to_owned),
            at: None,
        },
        at,
    )
    .await
}

async fn run_input(context: &Context, mut input: DiaperInput, at: Option<&str>) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let interactive = prompt::available() || prompt::host::hosted();
    input.at = prompt::time::resolve_at(context, at, interactive)?;
    let model = DiaperForm::create_diaper(context.calendar()?, input);
    let submission = if interactive && !model.route().is_empty() {
        runner::collect(
            &mut FormState::create_form(model, huckleberry_api::client::now_seconds())?,
            context.theme,
        )?
    } else {
        model.complete()?
    };
    execute(context, &client, &cid, submission).await
}

/// Binds an owned draft to its prepared client without executing a write.
pub(crate) fn prepare_with_client(
    context: Context,
    client: Huckleberry,
    cid: String,
    input: DiaperInput,
    now: f64,
) -> Result<PreparedCommand> {
    let model = DiaperForm::create_diaper(context.calendar()?, input);
    PreparedCommand::create_prepared(model, now, move |submission| async move {
        execute(&context, &client, &cid, submission).await?;
        Ok(CommandProgress::Finished { home: true })
    })
}

/// Submits validated answers once and reports the existing receipt.
pub(crate) async fn execute(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    submission: DiaperSubmission,
) -> Result<()> {
    let DiaperSubmission {
        at,
        mode,
        details,
        potty,
    } = submission;
    if let Some(outcome) = potty {
        client
            .log_potty_at(cid, mode.to_api(), outcome.to_api(), &details, at)
            .await?;
    } else {
        client
            .log_diaper_at(cid, mode.to_api(), &details, at)
            .await?;
    }
    super::persist_session(context, client).await?;
    let mut fields = receipt_fields(mode, &details);
    fields.push((
        "When",
        crate::render::format::date_time(at, &context.calendar()?),
    ));
    if let Some(outcome) = potty {
        fields.push(("Outcome", output::words(outcome.to_api().as_str())));
    }
    context.receipt(
        if potty.is_some() {
            "🚽 Potty trip recorded"
        } else {
            "🧷 Diaper recorded"
        },
        &fields,
        &[],
    );
    Ok(())
}

/// The details a parent just recorded, using the words from the prompts.
fn receipt_fields(mode: DiaperKind, details: &DiaperDetails) -> output::Fields {
    let contents = match mode {
        DiaperKind::Pee => "Wet",
        DiaperKind::Poo => "Dirty",
        DiaperKind::Both => "Wet and dirty",
        DiaperKind::Dry => "Dry",
    };
    let mut fields = vec![("Contents", contents.into())];
    if let Some(value) = &details.pee_amount {
        fields.push(("Wet amount", output::words(&value.to_string())));
    }
    if let Some(value) = &details.poo_amount {
        fields.push(("Dirty amount", output::words(&value.to_string())));
    }
    if let Some(value) = &details.color {
        fields.push(("Colour", output::words(value.as_str())));
    }
    if let Some(value) = &details.consistency {
        fields.push(("Consistency", output::words(value.as_str())));
    }
    if details.rash {
        fields.push(("Rash", "Recorded".into()));
    }
    if let Some(notes) = &details.notes {
        fields.push(("Notes", notes.clone()));
    }
    fields
}
