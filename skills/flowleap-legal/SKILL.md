---
name: flowleap-legal
description: Read patent law from the office's own page (the MPEP, 35 USC and 37 CFR, the EPC and the EPO Guidelines, the PCT rules) and quote it with its URL and edition. Trigger when an agent needs legal grounds, examination-guideline citations, statute text, or authoritative references for a patent-law question, or when the user names a section such as 35 USC 103 or EPC Art. 54.
---

# FlowLeap Legal (patent law from the official page)

Read patent law from the office's own page at the moment of the question, and
quote the text the page gives. No FlowLeap command and no FlowLeap key is part
of this method: the page is public, and you fetch it with your own web tool.

## Steps

1. **Name the source.** Decide which kind of text answers the question: a
   statute (35 USC, the EPC articles, the PCT articles), a rule (37 CFR, the EPC
   Implementing Regulations, the PCT Regulations) or office guidance (the MPEP,
   the EPO Guidelines, the PCT Applicant's Guide). A question about what the
   examiner does goes to office guidance. A question about what the law
   requires goes to the statute or the rule. Done when the source and the
   office are written down before the first fetch.

2. **Resolve the section.** When the user names a section ("MPEP 2143", "EPC
   Art. 123(2)", "Guidelines G-VII, 5", "PCT Rule 4.9"), build its URL from the
   table below. When the user asks in plain words, fetch the index page of the
   source and find the section from its titles. Read the edition year of a
   dated source (the EPO Guidelines) from its index page, never from memory.
   Done when you have one URL for each section you need.

3. **Fetch and quote.** Fetch the page. Quote only text that the page returned,
   word for word, in quotation marks. Do not quote from memory. Do not change
   the words of a quote. A paraphrase is not a quote: mark it as your summary.
   Done when each quote in the answer is text of a fetched page.

4. **State the edition and the URL.** Give the URL of the page with each quote.
   Give the edition or the revision the page shows (see the Edition column).
   When the page shows no edition, say so and give the date of the fetch. Done
   when each quote has a URL and an edition statement.

5. **Say what the page did not answer.** When the page does not hold the
   answer, say so. Name the page you read and the question that stays open. Do
   not fill the gap from memory. Do not report silence as "no such rule". When
   the fetch fails, say that the fetch failed and give the URL, so the user can
   open it. Done when each open question is named in the answer.

## Official sources

Each pattern was verified by a plain HTTPS fetch on 2026-10-20. A status of 200
alone proves nothing: uscode.house.gov and the human ecfr.gov page can answer
200 with a shell ("Page Not Found", a bot check) to a non-browser client. Fetch
the GPO and eCFR API URLs below instead, and never quote a shell page.

| Source | URL pattern | Edition | Fetch note |
|---|---|---|---|
| MPEP (USPTO guidance) | https://www.uspto.gov/web/offices/pac/mpep/s<NNNN>.html; chapter index mpep-<NNNN>.html | The revision marker at the end of the section heading, such as [R-01.2024] | A sub-section (2143.01) is a heading inside its parent page s2143.html; there is no page per sub-section |
| 35 USC (statute) | https://www.govinfo.gov/link/uscode/35/<N>?link-type=html (GPO link service) | The page header, such as "United States Code, 2024 Edition" | Without link-type=html the service returns a PDF. The main edition is annual: for a recent amendment, read the MPEP section the office applies |
| 37 CFR (rules) | Fetch https://www.ecfr.gov/api/renderer/v1/content/enhanced/current/title-37?section=<part>.<section>; cite https://www.ecfr.gov/current/title-37/section-<part>.<section> | The API body shows no date: say "eCFR current text" and give the fetch date | The human page is bot-checked for a non-browser client; the API URL works for every client |
| EPC articles and rules | https://www.epo.org/en/legal/epc/2020/a<N>.html (article), r<N>.html (rule) | No edition text on the page: the URL names the EPC 2020 text; give the fetch date | A paragraph (Art. 123(2)) is a part of its article page. Build the URL from the number; the index table of contents sits in a script block |
| EPO Guidelines | https://www.epo.org/en/legal/guidelines-epc/<YEAR>/<part>_<chapter>_<section>.html in lower case, chapter in Roman numerals (g_vii_5.html, h_v_3_1.html) | The index https://www.epo.org/en/legal/guidelines-epc names the edition in force (2026 on 2026-10-20) | A wrong year returns an old text with no error: read the year from the index, never from memory |
| PCT articles and rules (WIPO) | https://www.wipo.int/en/web/pct-system/texts/rules/r<N> (rule), texts/articles/a<N> (article) | The index https://www.wipo.int/en/web/pct-system/texts/index says "as in force from <date>"; the rule page shows none | A paragraph (Rule 4.9) is a part of its rule page |

Two traps: MPEP 2132 is the pre-AIA novelty section, so a first-inventor-to-file
application reads MPEP 2152. MPEP 2111 (broadest reasonable interpretation)
applies to a pending application; a granted patent in litigation is read under
the Phillips standard, which is case law, not the MPEP.
