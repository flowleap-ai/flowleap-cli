---
name: recipe-office-action-response
description: Prosecution recipe for the office action analysis of one EP or US application — read the newest examiner action, list each objection with its legal basis, read each cited document at the passages the examiner relied on, compare the claims element by element, and write an analysis with response options for the attorney, with each statement about a cited document tied to its source passage. Trigger when the user asks to analyse or respond to an office action, an EPO communication or search opinion, or a USPTO rejection, or when a routine instruction names an application and a new office action event.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-ops", "flowleap-legal", "flowleap-citation", "flowleap-uspto", "recipe-claim-analysis"]
---

# Recipe: Office Action Analysis

Turn the newest office action (OA) of one application into an analysis and a response plan. The attorney decides. You read, compare and propose.

In a chat client with the FlowLeap connector, call the tools named in the `flowleap-shared` connector table instead of these commands. The steps stay the same.

## Rules

- **Nothing is final and nothing is sent.** The analysis and each argument or amendment in it are drafts for the attorney. Do not file, send, email or export anything to the office or the client. Do not call a document "final", "ready to file" or "the response".
- **Each statement about a cited document names its source passage.** Give the document, the passage (paragraph, column and line, claim, or figure) and the source anchor that the tool printed with the text, for example `US1111111A1 ¶[0059] [source anchor: …]`. Quote the words the argument turns on, verbatim, from a tool result of this run. A statement with no passage that you read is not a finding: write "not read" and the reason.
- **The examiner's words are evidence too.** Quote the OA for each objection, with its page or section.
- **Do not fill gaps from memory.** A passage you could not read, a law text you could not fetch, or a date you could not find is a gap. Write it down.

## Step 1: Intake

Find the application, the office and the action. Done when you have the OA text (or know that no tool serves it) and the field table of the [analysis layout](references/analysis-layout.md) is filled.

**Started from a routine.** The instruction names the application and the new event, and no attorney answers questions. Take the application and the event date from the instruction. Analyse the newest office action on or after that date. Do not ask questions; write each assumption in the report.

**US application.** The OA comes from the file wrapper, as text from OCR on the server:

```bash
flowleap --json uspto documents <application-number> --code CTNF   # non-final rejections
flowleap --json uspto documents <application-number> --code CTFR   # final rejections
flowleap uspto document-text <application-number> <documentIdentifier> > office-action.md
```

Take the newest. Also list code `892` (references cited by the examiner) for the full numbers of the cited documents.

**EP application.** No tool serves the text of an EPO communication (Art. 94(3) EPC) or of the search opinion. Use what the tools serve, and say what they do not:

```bash
flowleap --json ops biblio <EP-publication>      # citedReferences: category, claims, relevantPassages
flowleap --json timeline <EP-publication>        # register events: the communication and its date
```

- When the user gave the communication as a file, read it (`flowleap ocr ./communication.pdf`; in the app, `read_pdf` on the workspace file). The objections then come from its text.
- When there is no file, analyse the European search report. Each `citedReferences` row with `citedBy: "examiner"` gives the category, the claims it hits (`categoryClaims`) and the passages the examiner relied on (`relevantPassages`). Category X means "particularly relevant if taken alone": a probable Art. 54 or Art. 56 objection. Category Y means "particularly relevant if combined": a probable Art. 56 objection. Label each such objection "from the search report categories; the search opinion text was not read", and ask the attorney for the communication or the search opinion as a file.

From the OA, record for each objection: the ground (35 U.S.C. 101, 102, 103, 112, double patenting; EPC Art. 52 to 57, 82, 83, 84, 123(2)), the claims it hits, the documents it applies (D1, D2, …), the passages it cites, and the examiner's reasoning. Keep formal objections (claim clarity, drawings, unity) as separate objections.

## Step 2: The claims as examined

The examined claims are not always the published claims. Done when you have the claim text that the OA examined, and its source.

- US: the claims listing (code `CLM`) dated nearest before the OA. The OA line "Responsive to communication(s) filed on <date>" names it.

  ```bash
  flowleap --json uspto documents <application-number> --code CLM
  flowleap uspto document-text <application-number> <documentIdentifier> > claims-as-examined.md
  ```

- EP: `flowleap --json ops claims <EP-publication>` gives the A1 claims, which the search report examined. A later communication can examine amended claims that no tool serves: say so.

Divide each independent claim that an objection hits into elements, verbatim (the method is in `recipe-claim-analysis`, Step 4).

## Step 3: Read each cited document at the cited passages

Done when each passage that the examiner cites is read and quoted, or is written as a gap with what you tried.

```bash
flowleap --json summary <cited>              # dates, family, status
flowleap --json ops claims <cited>
flowleap --json ops description <cited>      # EP and WO only
```

- **Date check.** Compare the publication date of each document with the earliest priority date of the application. A document published after that date is a complete answer to a 102 or Art. 54(2) objection; at the EPO it can still be prior art under Art. 54(3).
- **Text that the tools do not serve** (US description; CN, JP, KR and the other offices): use the family-member workaround in `flowleap-shared`, section "Office coverage of the text tools and the family-member workaround". Label the text of the member as the member's text. Its paragraph numbers can be different from the cited numbers.
- **Last step:** fetch the public page of the document (for example `https://patents.google.com/patent/<number>/en`) with your web tool. Quote only text that the page returned, and give the URL as the source.
- The comparison verb (`compare` / `compare_patents`) can show you where to look. It is not a source: quote the text of the document itself.
- US: `flowleap --json citation search <application-number> --examiner-cited-only` gives the office's record of how the examiner applied each document.

## Step 4: Compare, claim by claim

For each objection and each claim it hits, write one row per element: the claim element (verbatim), what the examiner says (quote and OA page), what the cited passage says (quote, passage, anchor), and your assessment: `disclosed`, `not disclosed`, `disputable` or `not read`. Done when each element of each independent claim that an objection hits has a row, and the added feature of each dependent claim that the OA names is checked.

The argument is where the mapping breaks: a missing element, a misread passage, a combination without a reason to combine, or a date that disqualifies the document.

## Step 5: Legal basis

Read the law from the office's own page with `flowleap-legal`, and quote it with its URL and edition. Done when each objection has its legal basis quoted, or the failed fetch is named.

- 102 / Art. 54: MPEP 2131; EPC Art. 54, Guidelines G-VI
- 103 / Art. 56: MPEP 2141 to 2145; EPC Art. 56, Guidelines G-VII, 5 (problem-solution approach)
- 112 / Art. 84, 123(2): MPEP 2161 to 2174; EPC Art. 84, Art. 123(2), Guidelines F-IV and H-IV
- Double patenting: MPEP 804

## Step 6: Response options for the attorney

For each objection, give the options and what speaks for and against each. Do not choose for the attorney.

- **Argue**: the element or the passage that the argument rests on, with its quote.
- **Amend**: the narrowing feature and its support in the application as filed (paragraph and quote). Prefer features of dependent claims. Check the amended claim against each cited document. Say which scope it gives up (prosecution history estoppel).
- **Accept or cancel**: the claims, and what stays.
- **Terminal disclaimer** (US double patenting) or **divisional application** (EP unity), where they apply.
- **Ask**: facts that only the applicant or the attorney has.

Give the response deadline as the OA states it, and tell the attorney to confirm it.

## Step 7: Write the analysis

Write the report in the [analysis layout](references/analysis-layout.md): one block for each objection, then the claim-by-claim comparison, the response plan, and the sources and gaps. Done when the report is saved and the chat answer gives its path and the open gaps.

- In the FlowLeap app (the Patent AI web app and the IDE), save it with `write_patent_results`, `template: "office-action-scaffold"`, `filePath: "office-actions/<application>/<OA date>-analysis.md"` and `matter` set to the application number. The writer adds the provenance appendix, and the evidence ledger of the session records each tool result that the report cites. When the writer refuses a save, correct the content and save again. Do not edit the saved file directly.
- With the CLI only, write the same Markdown file yourself. No tool checks the quotes or the anchors there: mark the report "unvalidated: written by the agent, not checked by a tool".

End the report and the chat answer with: "AI-assisted analysis for review by a registered patent attorney. Not legal advice. Nothing was filed or sent. Confirm the response deadline independently."
