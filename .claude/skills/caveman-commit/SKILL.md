---
name: caveman-commit
description: Write git commit messages in caveman speak. Use when the user asks for a "caveman commit", wants to commit in caveman/grunt style, or says things like "commit like caveman" / "ooga booga commit".
---

# Caveman Commit

Stage the relevant changes and create a git commit whose message is written in
caveman speak: short, blunt, primitive grammar. No articles, no helper verbs,
present tense, ALL CAPS optional but encouraged for the subject.

## How to write the message

1. Look at the diff (`git status` + `git diff --staged` or `git diff`) to
   understand what actually changed.
2. Compress the real change into caveman grammar:
   - Drop articles ("the", "a", "an") and most pronouns.
   - Drop helper verbs ("is", "are", "have", "will").
   - Use bare present-tense verbs: ADD, FIX, SMASH, MAKE, KILL, MOVE, CLEAN.
   - Keep it truthful: the grunt must describe the genuine change.
3. Subject line: one short caveman sentence, e.g. `ME ADD LOGIN BUTTON`.
4. Optional body: a few more grunts giving detail, one thought per line.

## Examples

```
FIX CRASH WHEN USER CLICK FAST

BUTTON SMASH TWICE. CODE NO LIKE. NOW CODE WAIT. NO MORE CRASH.
```

```
ADD DARK MODE

EYE HURT IN CAVE. NOW APP GO DARK. EYE HAPPY.
```

```
ME DELETE DEAD CODE. CODE NOT MOVE. CODE GONE NOW.
```

## Rules

- Never invent changes that are not in the diff. Grunt about real work only.
- Keep the subject under ~50 characters.
- Do not use em-dashes anywhere.
- Follow the repo's normal commit/push conventions (branch, co-author trailer,
  etc.) unless the user says otherwise. Only the wording is caveman.
