# Research methodology

How a number gets into this repository, and how it is torn apart before it
stays there.

This file is deliberately general. It was written while researching the
typical ranges in [`data/reference.toml`](../../data/reference.toml), but
nothing in it is specific to diapers or sleep. Use it for the next thing too,
whatever that is: introducing solids, growth, milestones, medication intervals,
anything where this tool would otherwise be quietly asserting a fact about
somebody's child.

## Why this is heavier than it looks

A parent opens this tool at 3am, holding a baby, frightened, with one hand
free. Whatever number is on the screen at that moment carries the authority of
having been printed by a computer, and it will be believed.

That cuts both ways, and both ways are bad:

- A range set too high tells a parent whose baby is fine that something is
  wrong. At 3am that is not a minor inconvenience.
- A range set too low tells a parent whose baby is not fine that everything is
  normal. That is worse, and it is the one nobody notices.

So the bar is not "this number appears on the internet". The bar is "a named
body that a clinician would accept said this, in these words, about this age,
and here is the sentence".

## Source tiers

Every claim is supported by sources, and every source is tiered before it is
used. The tier is a property of the *specific material cited*, not of the
organisation in general: a professional body's press release is not its
guideline.

| Tier | What it is | How it may be used |
| --- | --- | --- |
| 1 | Peer-reviewed research, or a formal guideline, position statement or consensus statement from a professional body | May support a band on its own |
| 2 | A patient-facing page published by a professional body or a major academic children's hospital | May support a band on its own |
| 3 | Advocacy organisations, commercial health-content sites, manufacturers who also publish research | Corroboration only, never the sole support, and the file must say so |
| 4 | Blogs, parenting magazines, forums, SEO content farms, marketing pages, opinion and editorial pieces, AI-generated summaries, anything that cites nothing checkable | Not usable at all |

Worked examples, so the tiers are not abstract:

- The AAP's clinical guidance, and `healthychildren.org` which is the AAP's own
  consumer site: tiers 1 and 2.
- A consensus statement in a society's peer-reviewed journal: tier 1. Check
  whether other bodies formally endorsed it, and say so if they did, because an
  endorsement is a second body putting its name to the same number.
- A breastfeeding advocacy organisation: tier 3. It may well be right, and it
  is frequently the most practical writing on a subject, but it is advocating.
  Find the professional body that says the same thing and cite that instead.
- A manufacturer's research pages: tier 3 at best, and the funding relationship
  gets stated out loud. Where a manufacturer funds good research, cite the
  research, by author and journal, not the manufacturer's page about it.
- A non-profit whose name sounds official: check before trusting the name.
  Establish who funds it and whether the material was peer-reviewed or written
  by a panel that the organisation itself convened.

**A claim resting only on tier 3 is not a claim, it is a rumour with a
footnote.** Either find tier 1 or 2 support, or do not publish the band.

## The review protocol

### 1. Write the claims down first, atomically

Before any searching, decompose what is already in the file, or what is
proposed for it, into one-line claims that can each be true or false on their
own. A band is at least three claims bundled together: the lower bound, the
upper bound, and the age range it applies to. Any of the three can be wrong
while the others are right.

Give each claim an identifier. The review record refers to claims by
identifier, so that a later reader can find exactly what was checked.

### 2. Take the adversarial stance explicitly

The instruction to a reviewer is *try to disprove this*, not *check this*.
Those produce different work. A reviewer told to check a number finds the page
that agrees and stops; a reviewer told to break it goes looking for the study
that disagrees.

Assume every claim is wrong until a qualifying source says otherwise. Name the
specific way each claim might be dangerous and make the reviewer answer it.

### 3. Use independent reviewers, and separate the numbers from the sources

Two rules, both learned the hard way:

- **One reviewer per metric, with no shared context.** A single reviewer
  checking everything starts rationalising: having decided the file is broadly
  sound, it reads the fourth metric more charitably than the first.
- **A separate reviewer audits source reputability, and is not told the
  verdicts on the numbers.** The reviewer who found a number is the worst
  possible judge of whether the source it came from is any good. Split the two
  jobs across different agents so neither can cover for the other.

Give every reviewer the same source-tier table and the same verdict vocabulary,
and give each one the claims verbatim rather than a summary, including the
attribution currently in the file, so it can catch an attribution that has
drifted from what the body actually said.

### 4. Demand a fixed verdict vocabulary

Every claim comes back as exactly one of:

| Verdict | Meaning |
| --- | --- |
| `CONFIRMED` | A qualifying source states this range, for this age |
| `WRONG` | A qualifying source states a materially different range. The correction is given |
| `UNSUPPORTED` | No qualifying source found. What was found instead is described |
| `OVERSTATED-SOURCE` | The number may be sound, but the attribution is not: the named body does not say this, or says it differently |

`OVERSTATED-SOURCE` is the one that earns its place. Most bad data in a file
like this is not an invented number, it is a real number with somebody else's
name on it, and that is invisible unless a verdict exists for it.

Every verdict carries the exact quoted sentence, the URL, the publishing body,
and the publication or review date. A verdict without a quote is an opinion.

### 5. Ask the questions that are easy not to ask

These recur across topics. Ask them every time:

- **Age boundaries.** Bands switch on a specific day. Sources almost never do.
  Where does each source's boundary actually fall, and what happens to a child
  in the gap between two sources' bands? Gaps are where wrong numbers hide.
- **Unit and denominator.** Is a sleep figure per 24 hours or per night? Is a
  feed count breast, bottle, or either? A correct number under the wrong
  denominator is a wrong number.
- **Population.** Does the figure describe breastfed infants, formula-fed
  infants, or both? If a single band misleads one group, say so, and either
  widen it until it is honest for both or decline to publish it.
- **Off-by-one on day counting.** Clinical guidance usually counts "day of
  life" starting at 1 on the day of birth. Code usually counts days since
  birth, starting at 0. These differ by one, forever, and the difference lands
  precisely in the newborn window where the numbers matter most. Check it
  explicitly, every time, with the arithmetic written out.
- **Is the measurement itself contested?** Sometimes the professional bodies
  argue that counting the thing is the wrong idea. That is a finding, and it
  outranks the number.

## Turning findings into bands

Research produces sources. Deciding what to publish is a separate act with its
own rules, and the rules lean one direction: toward saying less.

1. **A floor sits under every qualifying source, never above one.** Where
   sources disagree, the published floor goes beneath the lowest of them. A
   band that tells somebody they are short when a reputable body says they are
   not is worse than a band that is merely wide.
2. **Material disagreement widens the band or removes it.** Never split the
   difference between two sources and present the result as though it came from
   somewhere. That number has no source, and the file requires one.
3. **No band is a legitimate answer, and often the honest one.** Where the
   pattern stops being typical enough to name, publish nothing. Running off the
   end of a metric's bands is a designed outcome rather than a gap.
4. **Omit a band whose absence protects the reader.** The standing example is
   milk volume: see the header of `data/reference.toml`. A range that is
   technically defensible but would read as a daily accusation against a parent
   doing nothing wrong does not go in.
5. **Labels are declarative, never imperative.** State what is typical. Do not
   tell anybody what to do about it, and never name a threshold for calling
   somebody. The parent decides; this tool reports.
6. **Every band names its source in the data file**, so a future reader can
   check it instead of trusting it.
7. **Colour is never the only carrier.** Say the standing in words too, so it
   survives a pipe, a screenshot, and colour blindness. Yellow, never red: a
   figure outside a typical range is worth a second look and is not an
   emergency, and this tool is in no position to tell which it is.

## The pediatrician outranks this file

Everything here is general guidance about populations. A pediatrician has the
one thing this file structurally cannot have, which is the actual child.

So the tool says so, in the output, next to the ranges rather than buried in a
manual, and the README says so at greater length. When a range disagrees with
what a pediatrician told a parent, the pediatrician is right and this file is
the thing that is wrong.

This is the single exception to rule 5 above. The pointer to the pediatrician
is a statement about the authority of the whole table, not advice triggered by
any particular reading, which is why it is allowed to exist and why there is
exactly one of it.

## Recording the review

One file per review in this directory, named `YYYY-MM-DD-<topic>.md`. It
records:

- What was reviewed, on what date, and against which version of the data.
- Every claim, by identifier, with its verdict, the quoted source, and the URL.
  **Including the claims that came back `CONFIRMED`**: a review that records
  only the things that changed cannot be audited, because a later reader cannot
  tell what was checked and cleared from what was never checked at all.
- What changed in the data as a result, and what deliberately did not.
- Open questions and known gaps, stated plainly rather than left for somebody
  to rediscover.

## When to review again

- A professional body publishes or revises a relevant guideline.
- A source in use turns out to be commercially entangled, or is downgraded a
  tier for any other reason.
- Somebody reports a number that felt wrong in use. That is evidence, and the
  claim it rests on gets re-reviewed rather than defended.
- Before adding any new metric, because the new work is an opportunity to
  re-run the old claims at no extra cost.

## The honest limitation

This research is conducted by a language model reading sources on the web. The
protocol above exists to constrain that: the tiers, the quotes, the URLs, the
independent reviewers and the adversarial framing are all there so that a
human can check the work rather than trust it.

It does not make the work authoritative. It makes it checkable. Those are
different things, the difference matters, and the README says so to every user
in plain language.
