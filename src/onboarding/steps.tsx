/**
 * Purpose: The pages of the first-run walkthrough: a welcome, a short list of features, a short intro to the setup, and the shared setup page every feature uses (its texts are in `pages.ts`). Nothing is downloaded unless the user presses Download.
 * Contents: Welcome — logo and greeting; Overview — one line on what the next pages are for; Features — a few of the features with their shortcuts, then "and so much more"; Setup — a page with a shortcut (when the feature has one) and, for features that need one, a download card; Page — the centered heading and column every step shares.
 */
import { For, Show, type JSX } from "solid-js";
import type { useModels } from "../useModels";
import type { PageDef } from "./pages";
import { KeyChip } from "../ui/KeyChip";
import { CueLogo, Mic, Scan, Snippet, Text } from "../ui/glyphs";
import { ModelCard } from "../ui/ModelCard";
import { Card, Row } from "../ui/kit";
import { NavIcon, type Glyph } from "../ui/NavIcon";
import { ShortcutInput } from "../ui/ShortcutInput";
import { knownShortcut } from "../ui/useShortcutRecorder";

/** A few features, shown with their shortcuts; every feature has its own setup page after this one. */
export const FEATURES: { icon: Glyph; name: string; hint: string; shortcut?: string }[] = [
  { icon: Mic, name: "Dictation", hint: "Say it, and it gets typed", shortcut: "dictation" },
  { icon: Text, name: "Transform Text", hint: "Clean up or reshape selected text", shortcut: "selection" },
  { icon: Scan, name: "Grab Text", hint: "Copy text from anything on screen", shortcut: "grab" },
  { icon: Snippet, name: "Snippets", hint: "Short triggers that become your text" },
];

export function Welcome() {
  return (
    <div class="flex h-full flex-col items-center justify-center gap-6 text-center">
      <CueLogo size={96} />
      <div class="flex flex-col gap-2">
        <h1 class="t-display">Welcome to Cue</h1>
        <p class="t-label text-muted">Small, local, always at hand.</p>
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

function FeatureRow(p: { icon: Glyph; name: string; hint: string; shortcut?: string }) {
  return (
    <div class="flex items-center gap-3.5 px-4 py-2">
      <NavIcon icon={p.icon} size={30} />
      <div class="flex min-w-0 flex-1 flex-col">
        <span class="t-lead truncate [font-weight:var(--w-strong)]">{p.name}</span>
        <span class="t-hint truncate">{p.hint}</span>
      </div>
      <Show when={p.shortcut}>{(name) => <KeyChip parts={knownShortcut(name())} />}</Show>
    </div>
  );
}

export function Features() {
  return (
    <Page tag="Features" title="What Cue does" text="Each one works in any app, with its own shortcut. You can change them next." below={<p class="t-lead text-muted">And so much more…</p>}>
      <Card>
        <For each={FEATURES}>{(f) => <FeatureRow {...f} />}</For>
      </Card>
    </Page>
  );
}

export function Overview() {
  return (
    <div class="flex h-full flex-col items-center justify-center gap-2 px-10 text-center">
      <h1 class="t-display">Set your shortcuts</h1>
      <p class="t-lead max-w-[46ch] text-muted">The next few pages let you choose your shortcuts and download what each feature needs. You can change all of it later in Settings.</p>
    </div>
  );
}

type Models = ReturnType<typeof useModels>;

/** A setup page: the shortcut and, when the feature needs one, its download. The download is the user's to start. */
export function Setup(p: PageDef & { models: Models }) {
  const info = () => p.models.models().find((m) => m.id === p.model);
  return (
    <Page tag={p.tag} title={p.title} text={p.text}>
      <Card>
        <Show when={p.shortcut}>
          {(name) => <Row label="Shortcut" hint="Press the button to change it"><ShortcutInput name={name()} /></Row>}
        </Show>
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
    </Page>
  );
}
