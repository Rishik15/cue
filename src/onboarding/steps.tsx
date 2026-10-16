/**
 * Purpose: The pages of the first-run walkthrough: a welcome, a short list of features, and the shared setup page every feature uses (its texts are in `pages.ts`). Nothing is downloaded unless the user presses Download.
 * Contents: Welcome — logo and greeting; Features — a few of the features with their shortcuts, then "and so much more"; Setup — a page with a shortcut and, for features that need one, a download card with a hand-drawn arrow pointing at its Download button (shown until the download starts or is installed); Page — the centered heading and column every step shares.
 */
import { For, Show, type JSX } from "solid-js";
import type { useModels } from "../useModels";
import { Doodle } from "./Doodle";
import type { PageDef } from "./pages";
import { KeyChip } from "../ui/KeyChip";
import { CueLogo, Mic, Scan, Snippet, Text } from "../ui/glyphs";
import { ModelCard } from "../ui/ModelCard";
import { Card, Row } from "../ui/kit";
import { NavIcon, type Glyph } from "../ui/NavIcon";
import { ShortcutInput } from "../ui/ShortcutInput";
import { knownShortcut } from "../ui/useShortcutRecorder";

/** A few features, shown with their shortcuts; every feature has its own setup page after this one. */
const FEATURES: { icon: Glyph; name: string; hint: string; shortcut: string }[] = [
  { icon: Mic, name: "Dictation", hint: "Speak instead of typing", shortcut: "dictation" },
  { icon: Text, name: "Selection Actions", hint: "Fix, convert and extract selected text", shortcut: "selection" },
  { icon: Scan, name: "Grab Text", hint: "Copy the text on any part of the screen", shortcut: "grab" },
  { icon: Snippet, name: "Snippets", hint: "Type a trigger, get your text", shortcut: "snippets" },
];

export function Welcome() {
  return (
    <div class="flex h-full flex-col items-center justify-center gap-6 text-center">
      <CueLogo size={96} />
      <div class="flex flex-col gap-2">
        <h1 class="t-display">Welcome to Cue</h1>
        <p class="t-label text-muted">Small jobs, done for you.</p>
      </div>
    </div>
  );
}

/** A page: a centered heading over a column of content. The heading always starts at the same height and its text reserves two lines, so moving between steps never shifts it; the page fades in over 120 ms. `below` is centred in the space left under the column. */
function Page(p: { tag: string; title: string; text: string; children: JSX.Element; below?: JSX.Element }) {
  return (
    <div class="flex min-h-full animate-[fade-in_120ms_var(--ease-out)] flex-col items-center gap-9 px-10 pb-4 pt-10">
      <div class="flex flex-col items-center gap-1.5 text-center">
        <span class="t-meta">{p.tag}</span>
        <h1 class="t-display">{p.title}</h1>
        <p class="t-lead min-h-11 max-w-[46ch] text-muted">{p.text}</p>
      </div>
      <div class="flex w-full max-w-[540px] flex-col gap-3">{p.children}</div>
      <Show when={p.below}><div class="flex flex-1 items-center">{p.below}</div></Show>
    </div>
  );
}

function FeatureRow(p: (typeof FEATURES)[number]) {
  return (
    <div class="flex items-center gap-3.5 px-4 py-2">
      <NavIcon icon={p.icon} size={30} />
      <div class="flex min-w-0 flex-1 flex-col">
        <span class="t-lead truncate [font-weight:var(--w-strong)]">{p.name}</span>
        <span class="t-hint truncate">{p.hint}</span>
      </div>
      <KeyChip parts={knownShortcut(p.shortcut)} />
    </div>
  );
}

export function Features() {
  return (
    <Page tag="Features" title="Out of the box" text="These work from any app." below={<p class="t-lead text-muted">And so much more…</p>}>
      <Card>
        <For each={FEATURES}>{(f) => <FeatureRow {...f} />}</For>
      </Card>
    </Page>
  );
}

type Models = ReturnType<typeof useModels>;

/** A setup page: the shortcut and, when the feature needs one, its download. The download is the user's to start. */
export function Setup(p: PageDef & { models: Models }) {
  const info = () => p.models.models().find((m) => m.id === p.model);
  // Pressing Download hides the arrow at once (the progress entry exists before the list says "downloading"); cancelling, a failure or deleting the download brings it back.
  const busy = () => info()?.downloading || p.models.progress()[p.model ?? ""] !== undefined;
  // The arrow stays in the layout (hidden) when there is nothing to point at, so the page does not shift.
  const pointing = () => !!info() && !info()?.installed && !busy();
  return (
    <Page tag={p.tag} title={p.title} text={p.text}>
      <Card>
        <Row label="Shortcut"><ShortcutInput name={p.shortcut} /></Row>
        <Show when={info()}>
          {(model) => (
            <ModelCard
              model={model()}
              selected={false}
              progress={p.models.progress()[model().id]}
              onDownload={() => p.models.download(model().id)}
              onCancel={() => p.models.cancel(model().id)}
              onDelete={() => p.models.remove(model().id)}
            />
          )}
        </Show>
      </Card>
      <Show when={p.model}><Doodle text="Download to get started" show={pointing()} /></Show>
    </Page>
  );
}
