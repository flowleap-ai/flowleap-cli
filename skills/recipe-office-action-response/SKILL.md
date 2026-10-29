---
name: recipe-office-action-response
description: Prosecution recipe for the office action analysis of one EP or US application — find the action type and posture, read the newest examiner action, list each objection with its legal basis, interpret the disputed claim terms, read each cited document at the passages the examiner relied on, run the novelty and inventive-step tests, and write an analysis with response options and deadlines for the attorney, with each statement about a cited document tied to its source passage. Trigger when the user asks to analyse or respond to an office action, an EPO communication, search opinion or search report, or a USPTO rejection, or when a routine instruction names an application and a new office action event.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-ops", "flowleap-legal", "flowleap-citation", "flowleap-uspto", "recipe-claim-analysis"]
---

# Recipe: Office Action Analysis

Turn the newest office action (OA) of one application into an analysis and a response plan. The attorney decides. You read, compare and propose.

In a chat client with the FlowLeap connector, call the tools named in the `flowleap-shared` connector table instead of these commands. The steps stay the same.

Read the two references before Step 1: [action types and deadlines](references/action-types-and-deadlines.md) and [legal tests](references/legal-tests.md). Write the report in the [analysis layout](references/analysis-layout.md).

## Rules

- **Nothing is final and nothing is sent.** The analysis and each argument or amendment in it are drafts for the attorney. Do not file, send, email or export anything to the office or the client. Do not call a document "final", "ready to file" or "the response".
- **Each statement about a cited document names its source passage.** Give the document, the passage (paragraph, column and line, claim, or figure) and the source anchor that the tool printed with the text, for example `US1111111A1 ¶[0059] [source anchor: …]`. Quote the words the argument turns on, verbatim, from a tool result of this run. A statement with no passage that you read is not a finding: write "not read" and the reason.
- **The examiner's words are evidence too.** Quote the OA for each objection, with its page or section.
- **Do not fill gaps from memory.** A passage you could not read, a law text you could not fetch, or a date you could not find is a gap. Write it down.
- **Law that you did not read is a check item.** When a rule, a period or a decision is not in a page that you fetched in this run, write it as "check: …" for the attorney, not as a statement of law.

## Step 1: Intake, action type and posture

Find the application, the office, the action and its type. Done when the field table of the layout is filled, including the action type, final or not, and the later filings.

**Started from a routine.** The instruction names the application and the new event, and no attorney answers questions. Take the application and the event date from the instruction. Analyse the newest office action on or after that date. Do not ask questions; write each assumption in the report.

**Action type and later filings.** Record the type (US: CTNF, CTFR, restriction, Ex parte Quayle, advisory action; EP: search report with or without search opinion, Art. 94(3) communication, Rule 71(3) communication, summons under Rule 115). Each type has different options: see [action types and deadlines](references/action-types-and-deadlines.md). Then check whether a reply, an RCE, an interview summary or a newer action was filed after it. If one exists, the selected action can be superseded: say so at the top of the report.

**US application.** The OA comes from the file wrapper, as text from OCR on the server:

```bash
flowleap --json uspto documents <application-number> --code CTNF   # non-final rejections
flowleap --json uspto documents <application-number> --code CTFR   # final rejections
flowleap --json uspto documents <application-number>               # all documents: later replies, RCE, interviews
flowleap uspto document-text <application-number> <documentIdentifier> > office-action.md
```

Take the newest action. Also list code `892` (references cited by the examiner) for the full numbers of the cited documents.

**EP application.** No tool serves the text of an EPO communication or of a search opinion. Use what the tools serve, and say what they do not:

```bash
flowleap --json ops biblio <EP-publication>      # citedReferences: category, claims, relevantPassages
flowleap --json timeline <EP-publication>        # register events: the communications and their dates
```

- When the user gave the communication or the search opinion as a file, read it (`flowleap ocr ./communication.pdf`; in the app, `read_pdf` on the workspace file). The objections then come from its text.
- When there is no file, you have only the search report. It is not an examination communication, and it does not raise objections: its categories are the searcher's view of the searched claims. Write at the top of the report: "SEARCH REPORT ONLY: no examination communication or search opinion text was read. The 'objections' below are inferred from search categories, not raised by the EPO." Read the categories with [legal tests](references/legal-tests.md), section "Search report categories". If a search opinion exists, a reply to it is due under Rule 70a: give that posture and the period as [action types and deadlines](references/action-types-and-deadlines.md) says, and ask the attorney for the opinion as a file.

From the OA, record for each objection: the ground (35 U.S.C. 101, 102, 103, 112, double patenting; EPC Art. 52 to 57, 82, 83, 84, 123(2), Rule 43), the claims it hits, the documents it applies (D1, D2, …), the passages it cites, and the examiner's reasoning. Keep formal objections (claim clarity, drawings, unity) as separate objections.

## Step 2: The claims as examined, and their meaning

The examined claims are not always the published claims. Done when you have the claim text that the OA examined (or, for a search report, searched), its source, and an interpretation of each disputed term.

- US: the claims listing (code `CLM`) dated nearest before the OA. The OA line "Responsive to communication(s) filed on <date>" names it.

  ```bash
  flowleap --json uspto documents <application-number> --code CLM
  flowleap uspto document-text <application-number> <documentIdentifier> > claims-as-examined.md
  ```

- EP: `flowleap --json ops claims <EP-publication>` gives the A1 claims, which the search report searched. For a Euro-PCT application, the searched claims can be the claims after Rule 161/162. A later communication can examine amended claims that no tool serves: say so.

Divide each independent claim that an objection hits into elements, verbatim (the method is in `recipe-claim-analysis`, Step 4). Then interpret each disputed term as [legal tests](references/legal-tests.md), section "Claim interpretation", says. Quote the definitions you rely on, and mark disputed readings for the attorney.

## Step 3: Read each cited document at the cited passages

Done when each passage that the examiner cites is read and quoted, or is written as a gap with what you tried, and the date of each document is checked.

```bash
flowleap --json summary <cited>              # dates, family, status
flowleap --json ops claims <cited>
flowleap --json ops description <cited>      # EP and WO only
```

- **Date check.** Use [legal tests](references/legal-tests.md), section "Date check". Never write that a date is a "complete answer": write "date issue for the attorney".
- **A refusal that names another tool.** When a text tool answers that another tool serves the text (for example a paid route for US claims), call that tool before you write a gap.
- **Text that the tools do not serve** (US description; CN, JP, KR and the other offices): use the family-member workaround in `flowleap-shared`, section "Office coverage of the text tools and the family-member workaround". Label the text of the member as the member's text. Its paragraph numbers can be different from the cited numbers.
- **Last step:** fetch the public page of the document (for example `https://patents.google.com/patent/<number>/en`) with your web tool. Quote only text that the page returned, and give the URL as the source.
- The comparison verb (`compare` / `compare_patents`) can show you where to look. It is not a source: quote the text of the document itself.
- US: `flowleap --json citation search <application-number> --examiner-cited-only` gives the office's record of how the examiner applied each document.
- **Family art.** Look at the other members of the family (`flowleap --json ops family <publication>`) for the same documents and the arguments made there. Record art from a counterpart that the attorney may need to cite (US duty of disclosure: check 37 CFR 1.56).

## Step 4: Compare, claim by claim

For each objection and each claim it hits, write one row per element: the claim element (verbatim), what the examiner says (quote and OA page), what the cited passage says (quote, passage, anchor), and your assessment: `disclosed`, `not disclosed`, `disputable` or `not read`. Done when each element of each independent claim that an objection hits has a row, the added feature of each dependent claim that the OA names is checked, and each 102/103 or Art. 54/56 objection has its test block.

- **Novelty (102 / Art. 54):** check that the elements are arranged as the claim requires, and any inherency or implicit disclosure ([legal tests](references/legal-tests.md), "Novelty").
- **Inventive step (103 / Art. 56):** write a separate block. EP: the problem-solution approach. US: the Graham findings and the KSR rationale ([legal tests](references/legal-tests.md), "Inventive step and obviousness").

The argument is where the mapping or the test breaks: a missing element, a different arrangement, a misread passage, a combination without a reason to combine, or a date issue.

## Step 5: Legal basis

Read the law from the office's own page with `flowleap-legal`, and quote it with its URL and edition. Done when each objection has its legal basis quoted, or the failed fetch is named.

- 102 / Art. 54: MPEP 2131, 2112; EPC Art. 54, Guidelines G-VI
- 103 / Art. 56: MPEP 2141 to 2145, 716; EPC Art. 56, Guidelines G-VII, 5
- 112 / Art. 84, 123(2): MPEP 2161 to 2174, 2181 (112(f)); EPC Art. 84, Art. 123(2), Guidelines F-IV and H-IV
- 101 / Art. 52(2), 53: MPEP 2106; Guidelines G-II
- Art. 83 (F-III), Art. 82 unity (F-V), Rule 43 claim form
- Double patenting: MPEP 804

## Step 6: Response options and deadlines for the attorney

For each objection, give the options and what speaks for and against each. Do not choose for the attorney. The options that apply depend on the action type ([action types and deadlines](references/action-types-and-deadlines.md)).

- **Argue**: the element, the arrangement or the test step that the argument rests on, with its quote. US: say that an argument can also narrow the claims (argument-based disclaimer).
- **Amend**: list candidate features from dependent claims and the description. Which feature to choose is the attorney's decision. For each feature, give its basis and test it as [legal tests](references/legal-tests.md), section "Amendments", says (EP: Rule 137(4) basis, "directly and unambiguously derivable", intermediate generalisation, Rule 137(5) unsearched subject-matter; US: written description, 35 U.S.C. 112(a)). Check the amended claim against each cited document. Say which scope it gives up (US: amendment-based and argument-based estoppel or disclaimer; EP: effect on scope, for the attorney).
- **Accept or cancel**: the claims, and what stays.
- **Interview** (US) or **informal consultation** with the examiner (EP).
- **After a final action** (US): a 37 CFR 1.116 amendment, an RCE (1.114), or an appeal.
- **Terminal disclaimer** (US double patenting) or **divisional application** (EP unity), where they apply.
- **Ask**: facts that only the applicant or the attorney has.

Then check reply completeness ([legal tests](references/legal-tests.md), "Reply completeness"): list in each objection block what a reply must address.

**Deadlines.** Give the action date, the stated period, the extension rule and the computed last dates, as [action types and deadlines](references/action-types-and-deadlines.md) says. Mark each computed date "computed by the agent; the attorney confirms".

## Step 7: Write the analysis

Write the report in the [analysis layout](references/analysis-layout.md): the summary, the action table, one block for each objection, the claim-by-claim comparison, the response plan, and the sources and gaps. Done when the report is saved and the chat answer gives its path and the open gaps.

- In the FlowLeap app (the Patent AI web app and the IDE), save it with `write_patent_results`, `template: "office-action-scaffold"`, `filePath: "office-actions/<application>/<OA date>-analysis.md"` and `matter` set to the application number. Put the reader link of the claim (`[US1111111A1 claim 1](flowleap://flowleap.patent-ai/patent?publication=US1111111A1&section=claims&claim=1)`) directly before each quotation from a claim: the writer then checks the quotation against the recorded claim text. The writer adds the provenance appendix, and the evidence ledger of the session records each tool result that the report cites. When the writer refuses a save, correct the content and save again. Do not edit the saved file directly.
- With the CLI only, write the same Markdown file yourself. No tool checks the quotes or the anchors there: mark the report "unvalidated: written by the agent, not checked by a tool".

End the report and the chat answer with: "AI-assisted analysis for review by a registered patent attorney. Not legal advice. Nothing was filed or sent. Confirm the response deadline independently."
