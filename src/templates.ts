/**
 * File: templates.ts
 * Purpose: The ready-made actions offered on the Dictation and Text pages, and a realistic sample text per action so the editor's Test panel is never empty.
 * Contents: TEMPLATES — a short starter set per trigger, each named for what it does in at most four words (all of them stay listed, however many rules exist); sampleFor — the sample for a rule's first action.
 */
import { newRule, type Rule } from "./rules";

/** A template rule. Input: its name and the fields that differ from a new rule. Return: the rule. */
const make = (name: string, part: Partial<Rule>): Rule => ({ ...newRule(), name, ...part });

export const TEMPLATES: Rule[] = [
  make("Clean Up Spacing", { actions: ["clean_whitespace"] }),
  make("Remove Filler Words", { actions: ["remove_fillers", "remove_repeats"] }),
  make("Fix Spelling and Grammar", { actions: ["correct"] }),
  make("Clean Copied Links", { trigger: "selection", conditions: [{ type: "contains", value: "http" }], actions: ["clean_links"] }),
  make("Join PDF Lines", { trigger: "selection", actions: ["join_lines"] }),
  make("Copy All Emails", { trigger: "selection", actions: ["extract_emails"], output: "copy" }),
  make("Copy All Links", { trigger: "selection", actions: ["extract_urls"], output: "copy" }),
  make("Fix Spelling and Grammar", { trigger: "selection", actions: ["correct"], output: "preview" }),
  make("Make It Formal", { trigger: "selection", actions: ["make_formal"], output: "preview" }),
  make("Summarize Text", { trigger: "selection", actions: ["summarize"], output: "preview" }),
  make("Make Bullet List", { trigger: "selection", actions: ["bullet_list"], output: "preview" }),
];

const SAMPLES: Record<string, string> = {
  clean_whitespace: "so   I was thinking   about the  launch\n\n\n   and what we   still need.  ",
  join_lines: "The quarterly report shows strong growth in all re-\ngions, with the largest gain in\nthe north.",
  upper_case: "meeting moved to thursday",
  lower_case: "MEETING MOVED TO THURSDAY",
  title_case: "the quick brown fox jumps over the lazy dog",
  sentence_case: "this IS the first sentence. and THIS is the second one",
  clean_links: "Worth a read: https://example.com/post?id=42&utm_source=newsletter&utm_medium=email&fbclid=abc123",
  extract_emails: "Contact ana@example.com or sales@example.org; cc bob.smith@mail.example.net for details.",
  extract_urls: "Docs at https://example.com/docs and the repo at https://github.com/example/app, see also http://example.org.",
  correct: "i dont think we has enough time to finish the reveiw before friday",
  make_shorter: "I just wanted to reach out and let you know that, if it is at all possible, I would really appreciate it if we could possibly find some time to meet.",
  make_formal: "hey can u send me that file asap thx",
  make_friendly: "Send me the report by noon. I need it for the meeting.",
  remove_fillers: "um so I I think we should uh ship it, you know, on on friday",
  remove_repeats: "we need to to move the the launch to friday",
  quote: "Thanks for the update.\n\nI will review it tonight.",
  unquote: "> Thanks for the update.\n>\n> I will review it tonight.",
  tidy_speech: "so um I I wanted to ask if we can uh move the the meeting to thursday",
  bullet_list: "We need to book the venue, send the invites by Friday and ask Dana about catering.",
  summarize: "Hi all, quick update: the release slipped from Monday to Wednesday because QA found a login bug. It is fixed now and tests are running. Nothing else changes.",
  simplify: "We will endeavour to ascertain the cause of the discrepancy prior to the deadline.",
};

/** The sample text for a rule's test panel. Input: the rule. Return: the sample of its first action, or "". */
export const sampleFor = (rule: Rule) => SAMPLES[rule.actions[0] ?? ""] ?? "";
