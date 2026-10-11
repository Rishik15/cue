/**
 * Purpose: The ready-made rules offered on the Rules page, and a realistic sample text per action so the editor's Test panel is never empty.
 * Contents: TEMPLATES — starting points (all of them stay listed, however many rules exist); sampleFor — the sample for a rule's first action.
 */
import { newRule, type Rule } from "./rules";

const make = (name: string, part: Partial<Rule>): Rule => ({ ...newRule(), name, ...part });

export const TEMPLATES: Rule[] = [
  make("Tidy Dictation Spacing", { actions: ["clean_whitespace"] }),
  make("Fix Dictation Grammar", { actions: ["correct"] }),
  make("Clean Copied Links", { trigger: "selection", conditions: [{ type: "contains", value: "http" }], actions: ["clean_links"] }),
  make("Join PDF Lines", { trigger: "selection", actions: ["join_lines"] }),
  make("Extract Emails to Clipboard", { trigger: "selection", actions: ["extract_emails"], output: "copy" }),
  make("Extract Links to Clipboard", { trigger: "selection", actions: ["extract_urls"], output: "copy" }),
  make("Fix Selected Text", { trigger: "selection", actions: ["correct"], output: "preview" }),
  make("Make Selection Formal", { trigger: "selection", actions: ["make_formal"], output: "preview" }),
  make("Make Selection Friendly", { trigger: "selection", actions: ["make_friendly"], output: "preview" }),
  make("Shorten Selection", { trigger: "selection", actions: ["make_shorter"], output: "preview" }),
  make("Sentence Case Selection", { trigger: "selection", actions: ["sentence_case"] }),
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
};

export const sampleFor = (rule: Rule) => SAMPLES[rule.actions[0] ?? ""] ?? "";
