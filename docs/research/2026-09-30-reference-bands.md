# Review: the typical ranges, 2026-09-30

The first adversarial review of [`data/reference.toml`](../../data/reference.toml),
covering all four metrics and every band in the file as it stood at commit
`4b290c9`. Method: [`methodology.md`](methodology.md).

## How it was done

Five reviewers worked independently and in parallel, none seeing another's
findings: one per metric, plus one auditing the reputability of every cited
body without being told the verdicts on the numbers. Each was given the claims
verbatim, including the attribution already in the file, told to try to
disprove rather than confirm, and required to return a fixed verdict with an
exact quote, a URL and a date.

Four load-bearing quotations were then re-verified directly against the
publisher before anything was changed: the NHS newborn sleep range, the CDC
breastfeeding frequency figures, the AAP's wet-diaper and stool sentences, and
the Galland meta-analysis abstract.

## Outcome

**Sixteen bands were reviewed. Two came through with their numbers and their
ages intact**, both of them sleep bands from the AASM consensus statement, and
even those had the opening word of their label corrected. Five rested on
attributions the named body had never written, and two cited no body at all.
Three age ranges lost their band entirely, because no qualifying source
supports a range there: milk feeds past six months, wet diapers past six
months, and dirty diapers past three weeks. The file went from 16 bands to 12.

The failures were not random. Three patterns produced nearly all of them:

1. **Ceilings nobody publishes.** Every count metric carried an upper bound. No
   qualifying source names a high number of feeds or diapers as atypical, and
   several say the opposite, so those ceilings only ever flagged normal babies.
   All are gone; counts are floors now.
2. **Recommendations presented as observations.** The word "typical" was
   applied to figures that are advice, not description. Observed normal is
   consistently wider than the recommendation, so the label alone converted
   sound guidance into a false alarm.
3. **Real numbers under the wrong name.** Five attributions credited a body
   with words it never published. The numbers were often fine; the citations
   were not, and a citation nobody can check is the thing this file exists to
   avoid.

The worst single finding is recorded under D2 below: the floor we called
typical was the exact value the literature uses to flag inadequate intake.

---

## Sleep

| ID | Claim as it stood | Verdict | Outcome |
| --- | --- | --- | --- |
| S1 | 0-89 days: 14 to 17 hours, cited to the National Sleep Foundation | OVERSTATED-SOURCE, plus a wrong boundary | **Replaced**: 0-121 days, 8 to 18 hours, NHS |
| S2 | 90-364 days: 12 to 16 hours, AASM | CONFIRMED from 122 days, OVERSTATED-SOURCE below it | **Boundary moved** to 122, relabelled "recommended" |
| S3 | 365-1094 days: 11 to 14 hours, AASM | CONFIRMED | Relabelled "recommended" |
| S4 | 1095-2190 days: 10 to 13 hours, AASM | CONFIRMED | Relabelled "recommended" |
| S5 | No band past 6 years | Sound as a product choice | Unchanged |

**S1 is the one that mattered.** Three separate problems compounded:

- The 14 to 17 figure is a *recommendation*, not a description. Observed sleep
  is much lower and much wider. Galland's meta-analysis of diary and
  questionnaire data, which is the same kind of record this tool keeps, puts
  the infant mean at 12.8 hours with a range of 9.7 to 15.9. A floor of 14
  labelled "typical" tells more than half a healthy newborn population that
  they are short, every night.
- The AASM, which the AAP endorses, deliberately publishes no figure under four
  months: "Recommendations for infants younger than 4 months are not included
  due to the wide range of normal variation in duration and patterns of sleep,
  and insufficient evidence for associations with health outcomes." The one
  pediatric body that examined this age refused to name a number, and we named
  one anyway.
- The National Sleep Foundation is not a professional medical body. It has no
  clinician membership, accredits nothing, and is documented as substantially
  funded by the sleep-product and pharmaceutical industries. Its 2015
  recommendations were published in its own journal and endorsed by nine
  organisations, **none of them pediatric**. Tier 3.

Replaced with the NHS, which describes the normal range rather than
recommending a target: "Some newborns may sleep for a total of around 8 hours a
day, while others may sleep up to around 18 hours a day, either is perfectly
normal." Verified directly.

The 90-day boundary had no source behind it. NSF's newborn band runs to 3
months and AASM's infant band starts at 4, so ages 90 to 121 were being shown
an AASM band for an age AASM refuses to cover. The boundary is now 121/122.

S2 to S4 keep their numbers, which match the consensus statement word for word,
but now say "recommended" rather than "typical", because that is what they are.
Noted and not acted on: the NSF gives 12 to 15 where the AASM gives 12 to 16
for the same band. The AASM is used throughout, being newer, pediatric rather
than lifespan, and AAP-endorsed.

## Feeds

| ID | Claim as it stood | Verdict | Outcome |
| --- | --- | --- | --- |
| F1 | 0-28 days: 8 to 12 | OVERSTATED-SOURCE | **Floor kept, ceiling removed**, re-cited to CDC |
| F2 | 29-120 days: 6 to 10 | WRONG | **Replaced**: 29-180 days, 4 or more |
| F3 | 121-180 days: 5 to 8 | WRONG, and wrong in direction | **Merged into F2** |
| F4 | 181-365 days: 4 to 6 | OVERSTATED-SOURCE | **Deleted** |
| F5 | No band past 12 months | CONFIRMED | Now no band past 6 months |

**F2 was the dangerous one.** Kent's study of exclusively breastfed infants
aged 1 to 6 months found a mean of 7.9 feeding sessions a day with an observed
range of 4 to 13. Our band of 6 to 10 called roughly a quarter of a normally
feeding population atypical, every day. The floor now sits at 4, under the
observed range rather than through the middle of it.

**F3 asserted a decline that does not happen.** Kent's follow-up found feeding
frequency steady from 3 to 6 months: "These parameters remained constant
between 3 and 6 months." There is no separate 4-to-6-month band now.

**F1's attribution swapped populations.** The AAP's "8 times" is the *bottle-fed
minimum*; its breastfed figure is "10-12 sessions in 24 hours is the norm". The
CDC states the intended range directly and is now cited for it: "Your baby will
breastfeed about 8 to 12 times in 24 hours." Verified directly. The ceiling is
gone because the CDC also says "Some babies may feed as often as every hour at
times, often called cluster feeding."

**F4 applied a formula figure to breastfed babies for six months.** The AAP's "4
or 5 feedings in 24 hours" appears under bottle feeding and is stated at exactly
6 months. The CDC declines to give any number for this age: "Continue to follow
your baby's cues and breastfeed when you notice signs of hunger." Verified
directly. Deleted rather than guessed at.

**A finding worth keeping in view.** The sources that establish what is typical
also argue against measuring a parent against it. Kent's conclusion: breastfed
infants "should be encouraged to feed on demand, day and night, rather than
conform to an average that may not be appropriate for the mother-infant dyad."
The AAP: "No book, or website, can tell you precisely how much or how often they
need to be fed." The band stays, because a parent watching for a problem needs
a floor, but it is a floor and never a target, and it is the metric most open
to the argument that it should not exist.

Two citations were removed entirely. **Medela**, a breast pump manufacturer, was
Tier 4 as cited, and the "mean 6.6 a day" we took from it was not a 1-to-3-month
mean at all but the modelled value at 13 weeks. **La Leche League**, an advocacy
organisation, was Tier 3. Both are replaced by the underlying research and by
the CDC. Because that research is manufacturer-funded, the source string now
says so.

## Wet diapers

| ID | Claim as it stood | Verdict | Outcome |
| --- | --- | --- | --- |
| W1 | Day 1: 1 to 2 | OVERSTATED-SOURCE | **Folded into a 1-or-more floor for days 1-2** |
| W2 | Day 2: 2 to 3 | Floor confirmed, ceiling unsupported | **Folded in with W1** |
| W3 | Day 3: 3 to 4 | WRONG ceiling | **Replaced**: days 3-4, 2 or more |
| W4 | Days 4-5: 4 to 6 | WRONG floor | **Replaced**: from day 5, 5 or more |
| W5 | Day 6 on: 6 or more | Number confirmed, label off by one | **Corrected**, label now true |
| W6 | 6-24 months: 4 or more | UNSUPPORTED | **Deleted** |
| W7 | No band past 2 years | CONFIRMED | Unchanged |

**W3's ceiling flagged compliance as excess.** The CDC's newborn chart puts the
day 3 minimum at 5. Our band called 5 *above* typical, so a baby meeting CDC
guidance exactly was painted as out of range.

**W4's floor sat below every source it could have cited.** The AAP: "After the
first 4 to 5 days, a baby should have at least 5 to 6 wet diapers a day."
Verified directly. The ABM protocol: "at least five to six urinations per day by
Day 5." The CDC: 6. Our floor was 4, and the attribution was spliced from two
different AAP pages saying two different things.

**W5 was off by one, provably.** Ages are days since birth, so the band that
started at age 5 first fired on the sixth day of life while its label said "from
day 5". The band now starts at age 4 and the label is true. This is the failure
mode the file header now warns about explicitly.

**The AAP does not publish a day-by-day wet-diaper progression at all.** Three
attributions credited it with one. Its actual statement is "In the first few
days after birth, a baby should have 2 to 3 wet diapers each day". The
day-by-day numbers belong to the CDC's chart, and the AAP's own first-days
figures count wet and soiled diapers *together*, which is not the thing this
metric counts.

**W6 cited nothing, and nothing supports it.** No professional body publishes a
typical wet-diaper count for a 6-to-24-month-old. Every figure that exists is a
dehydration threshold, and the bodies disagree about it: the Canadian
Paediatric Society uses fewer than 4, the AAP and Cleveland Clinic use fewer
than 6, and others use 3. A threshold for calling a doctor is a different kind
of claim from a typical range. Deleted.

All ceilings removed: observed voiding runs to about twenty times a day in the
first month.

## Dirty diapers

| ID | Claim as it stood | Verdict | Outcome |
| --- | --- | --- | --- |
| D1 | Days 1-3: 1 or more, no body named | UNSUPPORTED, and the age span wrong | **Replaced**: days 1-2 at 1 or more, days 3-4 at 2 or more |
| D2 | Day 4 to 6 weeks: 3 or more | WRONG, both halves of the attribution false | **Replaced**: 1 or more, to 3 weeks |
| D3 | No band past 6 weeks | CONFIRMED as a decision, reasoning partly wrong | **Boundary moved** to 3 weeks |

**D2 is the worst thing this review found.** Three soiled diapers on day 4 is
the value the literature uses to *flag* possible inadequate intake: the ABM's
supplementation protocol lists "fewer than four stools on day 4 of life" among
signs of inadequate milk transfer, resting on Nommsen-Rivers 2008, which found
"the most efficient day 4 predictor of breastfeeding inadequacy was soiled
diaper output <= 3". Our band presented that number as the reassuring floor. A
parent whose day-4 reading is the one a clinician would act on was told it was
typical.

The same floor was also above the *observed mean* for formula-fed babies, 2.3 a
day in the first month and 1.6 in the second, so it called a normally fed baby
short every day for two months.

Neither half of its attribution was real. The AAP has never published "at least
4 stools a day by day 4"; its day-4 figure is two, and the wording has been
unchanged since at least 2016. La Leche League does not say "3 or more through
6 weeks"; it says three to five, after the first week, dropping to one or two in
weeks five and six. Both halves overstated their source in the direction that
made the band look better founded than it was.

**D1 traced to a blog.** "One stool per day of life" is real and widely
repeated, but no professional body publishes it for stools. Its most
authoritative-looking home is a PDF on a children's hospital domain, which
turns out to be republished KellyMom content carrying its original blog byline.
Anyone spot-checking it lands on a hospital URL and concludes wrongly. The
published shape is not a 1/2/3 ramp either: the AAP and UNICEF both give a two
step, 1 or more on days 1-2 and 2 or more on days 3-4, which is what the file
now says.

**D3's boundary moved from 6 weeks to 3.** Six weeks is well corroborated as the
point where infrequent stooling is unremarkable, but the AAP puts the start of
the change earlier: "By three to six weeks of age, some breastfed babies have
only one bowel movement a week and still are normal." A daily floor asserted to
day 43 would call that baby short for three weeks. The band now ends at 3 weeks,
where the earliest qualifying source says the pattern starts to break up.

**From day 5 the two feeding modes diverge past what one number can cover.** The
AAP puts a formula-fed baby at "at least one bowel movement a day" and a
breastfed one at "at least 3 to 4". This tool cannot tell which a baby is: a
bottle may hold expressed milk, and many babies are fed both ways. The floor is
therefore the one true for both, and it still catches the reading that matters
at that age, which is a day with none at all.

## The milk volume omission

The file's refusal to publish a milk-volume band was upheld, but the reasoning
written down for it was wrong and has been rewritten.

The old text said 150 to 200 ml per kilogram per day "describes established
feeding from roughly two weeks on, not the first week". Both halves fail.
Intake reaches that band around day 4 to 5, not two weeks: measured intake rises
from a mean of 13 g/kg on day 1 to 155 g/kg on day 5. The seven-day-old the
comment invokes is comfortably *inside* the band, so that baby was the wrong
example. "Roughly two weeks" is the birth-weight-regain milestone, borrowed from
a different axis.

The real argument is much stronger and is now the one in the file: **no
professional body publishes 150 to 200 as a range for a term baby at all.** The
AAP gives a single point, 75 ml per pound, which is 163. The 200 ceiling belongs
to preterm protocols. And measured intake in exclusively breastfed babies never
reaches even the 150 floor at any age, running 135 ml/kg/day at one month, 126
at three and 107 at six. A chart drawing that line would tell the parent of a
thriving breastfed baby they were underfeeding, every day, for a year.

## What was deliberately not done

- **No 6-to-12-year sleep band**, although the AASM publishes one. The tool
  stops at five because the product does, not because the evidence does.
- **No split by feeding method**, anywhere. It would materially improve the feed
  and stool bands, and the app does record bottles separately from nursing, but
  a bottle may hold expressed milk and mixed feeding is common, so the recorded
  data does not reliably answer the question. Revisit only with an explicit
  setting the parent sets, never inferred.
- **No meconium-to-yellow transition indicator**, although the sources agree on
  it more tightly than on any stool count, and delayed transition is a
  recognised red flag. It is a better measurement than the one this file makes,
  and it is a feature rather than a band. Worth building.
- **No dehydration thresholds.** Several were found while looking for typical
  ranges. They are a different kind of claim and this file does not make it.

## Open questions

- The Kent 2013 full text could not be reached to confirm its own funding
  disclosure. Its companion paper declares Medela funding, and the research
  group is described by its university as Medela-sponsored since 1996, so the
  source string says so. Confirm when the publisher is reachable.
- The exact phrasing of the NICE CG99 six-week exemption is corroborated but
  not read from the source PDF directly.
- The 8-to-18-hour newborn sleep band is wide enough that it will rarely say
  anything. That is honest, but if it proves useless in practice the answer is
  to drop the band, not to narrow it back toward a recommendation.
