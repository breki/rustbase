---
description: Restate the reply above, or answer an instruction, in under 40 words
---

Say it in what a reader gets through in ten seconds.

## Usage

- `/short` -- restate the reply immediately above, in under 40
  words.
- `/short <instruction>` -- carry out `$ARGUMENTS`, and answer in
  under 40 words.

## The budget is forty words

Ten seconds of silent reading comes to roughly forty words, and
fewer when the text carries paths and identifiers. "Be brief" is
a criterion nobody can fail; a word count is one a reader can
check.

Over budget, cut in this order: the evidence, then the
reasoning, then a qualification the reader has already accepted.
Never cut the answer.

## What survives

The outcome, and whatever the reader has to do next. Name that
next step in words rather than by quoting a path or a command:
the reply above is where the exact spelling belongs.

Drop tables, headings and code blocks. One or two sentences, or
two short lines. No preamble and no closing summary.

**If it may be read aloud** (a hook or notifier that speaks or
forwards the text), put no backticks, paths or symbols in it --
a synthesizer that strips backticked spans deletes them rather
than pronouncing them. Prefer whole words to symbols, digits to
spelled-out numbers, and end each sentence with a full stop.

## What does not bend

**A failure stays a failure.** When the reply above reported a
failing test, a skipped step or a claim nobody verified, the
short version reports it too. Compressing "nine gates pass,
coverage could not run" into "gates pass" produces a false
statement rather than a brief one. Cut the detail and keep the
bad news.

**This command re-derives nothing.** In the first form the reply
above is the whole input: read it, compress it, and stop. Do not
re-run a command to confirm it, and do not add a fact it did not
contain. When no reply precedes the call, say that in one line.

The **Narrate the work as it happens** rule under
**Collaboration** does not apply here. The command is its own
output, so a sentence announcing what is about to happen would
spend a quarter of the budget saying nothing.

## Tools

There is no `allowed-tools` line, deliberately. The first form
needs no tools at all. The second needs whatever its instruction
turns out to need, and that cannot be listed in advance.
