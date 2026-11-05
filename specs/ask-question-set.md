---
id: AQS
status: accepted
adrs: [0042, 0047]
---

# Question set of `retrodoc ask`

## Intent

`retrodoc ask` answers from the generated docs, and may repeat their errors with confidence. A question set per
benchmark repository, with known answers, lets the measuring tools score its answers by rule and by an LLM judge,
with and without the tools that read the code, so a change of the agent or of the docs can be measured instead of
read by hand.

## Requirements

- **AQS-1** (deterministic) A question set (`benchmark/<repo>/questions.yaml`) must load when each question has an
  `id`, a `question`, an `expected` answer and exactly one of `feature` (a documented question: the reference
  feature that answers it) or `absent_terms` (an undocumented question: words of what the application does not do).
- **AQS-2** (deterministic) The check of a question set must report each documented question whose `feature` is not
  a feature of the reference, or is a `disputed` one.
- **AQS-3** (deterministic) The check must report each undocumented question one of whose `absent_terms` appears in a
  text file of the checkout that its ignore files keep (case ignored, whole words, `.git` and files over 2 MB left
  out), naming the term and the file.
- **AQS-4** (deterministic) The check must fail when the checkout is a Git checkout whose `HEAD` is not the `commit`
  of the reference, or when it reports a question.
- **AQS-5** (deterministic) Each documented question must be classed `covered` when the saved run pairs a generated
  feature with its reference feature, as the benchmark scoring pairs them (a hand pair of `matches.yaml`, a pair of
  the run's `judge.yaml` that `matches.yaml` does not reject, or the same name once normalized), else `uncovered`.
- **AQS-6** (deterministic) A blank answer must be judged `not documented` without the LLM judge.
- **AQS-7** (deterministic) Any other answer must take the verdict, `right`, `wrong` or `not documented`, of one call
  to the LLM judge given the question, the expected answer and the text of the answer; the judge's prompt says that
  an answer saying the application does not do something the expected answer says it does not do is right, and that
  `not documented` is an answer that says the docs do not cover the question without answering it.
- **AQS-8** (deterministic) The answer to a covered question must count as `source hit` when one of its sources is a
  generated feature paired with the expected reference feature, or a use case of one.
- **AQS-9** (deterministic) A question whose `ask` fails must be recorded as `failed` with its error, counted apart,
  and the run must go on with the next question.
- **AQS-10** (deterministic) The report of a run must give, per variant (with and without the code tools), the count
  of each judgement per group (covered, uncovered, undocumented), the source hits, the mean rounds, and the calls and
  tokens of the agent apart from the judge's.
- **AQS-11** (deterministic) A run must save each answer with its sources, its warnings, its rounds and its
  judgement (and the judge's reason), per variant.
- **AQS-12** (deterministic) The load of a question set must fail, naming the question, on a question with neither
  `feature` nor `absent_terms`, with both, with an empty field or an unknown key, or whose id repeats another's.

## Scenarios

- (AQS-12) Given a question with both `feature` and `absent_terms`, when the set loads, then it fails naming the
  question.
- (AQS-2) Given a documented question naming `Export to PDF`, which the reference lacks, when the set is checked,
  then the check reports the question and the name.
- (AQS-3) Given an undocumented question with `absent_terms: [totp]` and a checkout whose `settings.py` holds `TOTP`,
  when the set is checked, then the check reports `totp` in `settings.py`.
- (AQS-5) Given a saved `judge.yaml` pairing `Bookmark sharing` with the reference feature `Share bookmarks`, when the
  question on `Share bookmarks` is classed, then it is `covered`.
- (AQS-6) Given a blank answer, when it is judged, then it is `not documented` and the judge is not called.
- (AQS-7) Given the answer "Yes, from the search box; the sort order is not documented.", when it is judged, then the
  judge's verdict is taken, not a rule on the words "not documented".
- (AQS-9) Given a provider that fails on the second question, when the run goes on, then the second is `failed` and
  the third is judged.

## Non-goals

- Writing the questions: they are written by hand (by the agent) following the method of
  `issues/done/ask_question_set.md`, then checked by AQS-2 to AQS-4.
- Partial credit: an answer is right or wrong; the judge's reason is saved for the reader.
- Repositories other than the benchmark ones; private repositories never get a question set.
