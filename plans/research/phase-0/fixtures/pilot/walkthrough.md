# Maintainer pilot walkthrough

Run this on your Omarchy machine, in a Hyprland session. You are the only Phase 0 early adopter. Offish has no application yet, so this is a rehearsal of the workflow on a disposable copy of the synthetic launch brief, in any text editor.

The workflow is: open a folder offline, edit one sentence, use `assumptions.md` as the only authorised context, review a date change, reject it, accept it, restore it, then keep an outside edit that arrives while the same proposal is pending.

Send back the notes listed at the end. Until those notes are in, the plan keeps this run pending.

## 1. Record the machine

This is the Phase 0 hardware baseline. Copy the results into your notes.

- Contents of `/etc/os-release`
- `uname -r`
- `hyprctl version`
- The `scale` line from `hyprctl monitors`
- `pacman -Q webkit2gtk-4.1` if that package is installed. If the command fails, write "not installed"
- The text editor you will use

Turn networking off for the rehearsal when you can do that without losing the session. If you cannot, leave it up and say so in the notes. The files need no network and no account.

## 2. Copy the fixtures out of the repository

From a checkout of the repository:

```sh
mkdir -p ~/offish-pilot
cp -a plans/research/phase-0/fixtures/pilot/. ~/offish-pilot/
cd ~/offish-pilot
cp brief.md brief.original.md
```

Leave the copies inside the repository untouched.

## 3. Open the folder

Open `~/offish-pilot` as an ordinary folder, then open `brief.md`. You should see:

- front matter with `status: draft`
- the sentence "Keep the first release on the current schedule."
- a checklist whose first item links to `assumptions.md`
- a table: Pilot seats 5, Budget (USD) 2,500
- an image link, Pilot timeline, pointing at `assets/timeline.svg`
- an HTML comment: Preserve this comment across visual edits.
- a notes line with café, 日本語, and العربية

Save without typing anything, then:

```sh
cmp brief.md brief.original.md
```

Pass: the command prints nothing. If it reports a difference, stop and note what the editor changed on a no-op save.

## 4. Make your own edit

Change only this sentence:

```text
Keep the first release on the current schedule.
```

to:

```text
Check the release date before publishing.
```

Save. Pass: that sentence is the only change. The table still says 5 and 2,500. The image link, the HTML comment, the front matter, and the café / 日本語 / العربية line are unchanged.

Keep this state. It is the copy you will restore to later:

```sh
cp brief.md brief.preimage.md
```

## 5. Use one authorised context file

Open `assumptions.md`. It is the only context for the proposal. It says the pilot date moves to 14 December, the budget rises to USD 3,200, and the seats stay at five. Do not open any other file as context for this proposal.

## 6. Reject the proposal

Select "Check the release date before publishing." The proposal, taken from the date in `assumptions.md`, replaces that selection with:

```text
Move the pilot launch to 14 December.
```

Leave `brief.md` as it is. That is the rejection. Pass:

```sh
cmp brief.md brief.preimage.md
```

The command prints nothing. The budget stays 2,500. The assumptions mention 3,200, and this proposal does not change the budget.

## 7. Accept the proposal

Make the same proposal again and accept it. Replace only the selected sentence with "Move the pilot launch to 14 December." Save.

Pass: that sentence is the only change from `brief.preimage.md`. Seats stay 5. Budget stays 2,500. The comment, image, front matter, and multilingual line stay as they were.

## 8. Restore

Put the sentence back to "Check the release date before publishing." so the file matches the preimage again.

```sh
cmp brief.md brief.preimage.md
```

Pass: the command prints nothing.

## 9. External save while a proposal is pending

Propose the date sentence again, and leave it unapplied.

In a second editor or another window, change a different line: on the notes line, add the word "local" after "Notes use", and save `brief.md` from that second editor.

The pending proposal is now stale. Discard it. Leave the schedule sentence as "Check the release date before publishing."

Pass: the notes line contains "local", and the schedule sentence was not replaced.

## 10. What to send back

- The hardware lines from step 1
- The editor you used
- Whether networking was off
- Whether each `cmp` passed
- The first place you were unsure what to do, or where you wanted to see the raw Markdown
