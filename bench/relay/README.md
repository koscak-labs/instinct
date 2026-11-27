# Claude learned the job. Could Codex run the next event?

Yesterday the workshop desk had to reprint its badges. Several guests had bought seats for someone else. Claude received the old roster and the sheet the desk had accepted. It banked a way to choose which seats needed a badge and whose name to print. A fresh Codex session then got a different roster. All events and names are fictional.

| What Codex received | Usual desk rules | Desk explicitly changed its rules |
| --- | --- | --- |
| Empty bank | Asked for the desk note | 3 correct badges |
| Claude’s lesson available through Instinct | 4 correct badges | 3 correct badges |
| Same lesson copied directly into the request | 4 correct badges | 3 correct badges |

The usual-rules job requires a local fact. The prior event used yellow sleeves for pickup. On the changed brief the desk explicitly specified green sleeves for pickup and a new greeting for blank names. Codex used the bank only on the usual-rules job. Two different guests shared a badge name on each roster. Keeping each seat separate mattered.

## Check the work

From the repository root, the first command checks the saved outputs with Python’s standard library. The second also prints each sheet with Chromium, then checks the PDF page count, A4 size and printed words with Poppler. Supply your installed headless Chromium executable.

```sh
python3 bench/relay/verify.py
python3 bench/relay/verify.py --browser /path/to/chrome-headless-shell --pdf-dir /tmp/instinct-print
```

[Expected names by seat](expected.json) and both briefs were committed before comparison at `4ca8300`. The HTML check compares each visible name field and seat field to that list. The PDF check also detects missing text at print time. The offline demo embeds the two PDFs from the live arm. PDF bytes can differ between new renders because the browser adds a timestamp. [Saved outputs and tool-call receipts](results/)

## Repeat the agent handoff

1. Build the executable with `cargo build --locked --release --offline`. Start the two agent CLIs in separate empty working directories. Configure the executable as an MCP server with argument `mcp`, an absolute path for `INSTINCT_STORE`, and `INSTINCT_PROJECT=lantern-studio`. Use a new store for each independent experiment.
2. Give Claude only the banking tool and [the anonymous teaching prompt](teach-anonymous.txt). Let it call `instinct_bank`. Check the saved lesson for identifying details before using it in another session.
3. Give fresh Codex [the new event](new-event.txt) followed by [the output contract](format.txt). Give it only the recall tool and access to the same bank. Save its entire answer. The grader and expected names stay outside the agent’s working directory.
4. Repeat the job with a fresh empty bank. For a third run, disconnect the tool and append the banked lesson’s `when`, `found`, `how`, `evidence` and `reconsider` fields directly to the prompt. Start a new session for every run. Repeat all three with [the changed brief](changed-brief.txt).
5. Compare the six answers with the frozen expectations. To grade a new HTML answer independently, import `verify_sheet` from the verifier and pass the answer path and the matching dictionary from `expected.json`. Record feedback only after checking the deliverable.

The recorded experiment used Claude Code with `claude-haiku-5-5` and Codex with `gpt-6.2-astra`. Claude had one permitted banking tool. Codex ran with `--ephemeral --ignore-user-config`, read-only sandboxing and shell execution disabled. The measured comparison has one run per condition. After verification, the operator submitted one positive outcome through MCP. A new engine process returned one hit for the method and another project returned zero matches.

In an earlier teaching pilot Claude copied a fictional guest name into its evidence despite an instruction to omit names. The measured run started again with anonymized examples and a fresh bank. The original [pilot prompt](teach.txt) and the [lesson actually transferred](results/lesson.json) let you inspect the difference. This test demonstrates a handoff across two harnesses on one workflow. A general quality improvement over manually supplying the same information remains unmeasured.

## Native Helix DB runs

A fresh Claude Code session banked a lesson into a native `.hxdb` file. Fresh Codex retrieved its ID but answered `NEED DESK NOTE`. The stored method told it to reconfirm the usual rule. We counted this as an unfinished task.

We clarified how the skill distinguishes an established default from an observed exception. In a separate run both harnesses received the skill. Claude taught into a new native bank. Fresh Codex retrieved the new lesson and produced all four correct badges on one A4 page. Another fresh Codex followed the changed brief and produced all three correct badges without retrieving the lesson. These are individual runs with different learned entries. They establish a working handoff and preserve a genuine miss. They do not isolate the causal effect of changing the instructions. [Both lessons, model outputs and checksums](native/)
