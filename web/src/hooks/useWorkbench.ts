import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { compileAndRun } from "../trace/backend";
import { SAMPLES, readUrlState, type Sample } from "../trace/samples";
import type { Highlight } from "../lib/spans";
import { stepLabel } from "../lib/step";
import type { ExecutionTrace } from "../trace/types";

export type ViewTab = "memory" | "tokens" | "bytecode";

const initial = readUrlState();
/** Arrived with a bare URL (not a shared or reloaded state): the first run plays once, so a cold visitor sees memory move. */
const freshVisit = window.location.search === "";

const prefersReducedMotion = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/** All workbench state: source, the latest trace, and the step scrubber. */
export function useWorkbench() {
  const [sample, setSample] = useState<Sample>(initial.sample);
  const [source, setSourceState] = useState(initial.sample.source);
  const [highlight, setHighlight] = useState<Highlight | null>(null);
  const [hover, setHover] = useState<Highlight | null>(null);
  const [caret, setCaret] = useState<number | null>(null);
  const [nav, setNav] = useState<{ seq: number; start: number; end: number } | null>(null);
  const [trace, setTrace] = useState<ExecutionTrace | null>(null);
  const [note, setNote] = useState<string | null>(null);
  /** Completed runs: lets a re-run that changes nothing visible still be acknowledged. */
  const [runs, setRuns] = useState(0);
  const [busy, setBusy] = useState(false);
  const [step, setStepState] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [tab, setTab] = useState<ViewTab>(initial.view);
  const runId = useRef(0);
  /** The fresh-visit run plays briskly; any hand on the controls (or a new run) returns playback to its normal pace. */
  const brisk = useRef(false);
  /** Edits made to a sample, kept when you switch away so a click never discards work. */
  const drafts = useRef(new Map<string, string>());
  const latest = useRef({ sample: initial.sample, source: initial.sample.source });
  latest.current = { sample, source };

  /** Click-to-source: select the row, then ask the editor to scroll, place the caret and pulse. */
  const navigate = useCallback((target: Highlight) => {
    setHighlight(target);
    setNav((now) => ({ seq: (now?.seq ?? 0) + 1, start: target.start, end: target.end }));
  }, []);

  const setSource = useCallback((text: string) => {
    setSourceState(text);
    setHighlight(null);
  }, []);

  const stepCount = trace?.steps.length ?? 0;
  const lastStep = Math.max(0, stepCount - 1);

  const run = useCallback(
    async (text: string, fallback: Sample, autoplay: boolean, arrival = false) => {
      const id = ++runId.current;
      brisk.current = false;
      setBusy(true);
      setPlaying(false);
      const outcome = await compileAndRun(text, fallback);
      if (id !== runId.current) return;
      setTrace(outcome.trace);
      setHighlight(null);
      setNote(outcome.note);
      setRuns((n) => n + 1);
      setBusy(false);
      const steps = outcome.trace.steps.length;
      // A recorded trace standing in for edited code never plays itself: it would look like the edits ran.
      const ignoredEdits = outcome.trace.mode === "mock" && outcome.trace.diagnostics.some((d) => d.severity === "warning");
      if (autoplay && !ignoredEdits && steps > 1 && !prefersReducedMotion()) {
        // Arrival opens one step before the first swap (as the readout names it), so the first thing seen is memory moving.
        const swap = arrival ? outcome.trace.steps.findIndex((s) => stepLabel(s).startsWith("Swap")) : -1;
        brisk.current = arrival;
        setStepState(Math.max(0, swap - 1));
        setPlaying(true);
      } else {
        setStepState(Math.max(0, steps - 1));
      }
    },
    [],
  );

  // First load: a fresh visit plays the run once; a shared or reloaded URL shows the finished run, paused.
  useEffect(() => {
    void run(initial.sample.source, initial.sample, freshVisit, freshVisit);
  }, [run]);

  // Closing the tab with unsaved edits asks first.
  useEffect(() => {
    const guard = (event: BeforeUnloadEvent) => {
      const { sample: current, source: text } = latest.current;
      const dirty = text !== current.source || SAMPLES.some((s) => s.id !== current.id && (drafts.current.get(s.id) ?? s.source) !== s.source);
      if (dirty) event.preventDefault();
    };
    window.addEventListener("beforeunload", guard);
    return () => window.removeEventListener("beforeunload", guard);
  }, []);

  const compile = useCallback(() => void run(source, sample, true), [run, source, sample]);

  const chooseSample = useCallback(
    (next: Sample) => {
      if (next.id === latest.current.sample.id) return;
      drafts.current.set(latest.current.sample.id, latest.current.source);
      const text = drafts.current.get(next.id) ?? next.source;
      setSample(next);
      setSource(text);
      void run(text, next, false);
    },
    [run, setSource],
  );

  /** Discard edits to the current sample and run its original source again. */
  const revert = useCallback(() => {
    const { sample: current } = latest.current;
    drafts.current.delete(current.id);
    setSource(current.source);
    void run(current.source, current, false);
  }, [run, setSource]);

  const setStep = useCallback(
    (index: number) => {
      brisk.current = false;
      setStepState(Math.min(Math.max(0, index), lastStep));
    },
    [lastStep],
  );

  const pause = useCallback(() => {
    brisk.current = false;
    setPlaying(false);
  }, []);
  const togglePlay = useCallback(() => {
    if (stepCount < 2) return;
    brisk.current = false;
    setPlaying((now) => {
      if (!now && step >= lastStep) setStepState(0);
      return !now;
    });
  }, [stepCount, step, lastStep]);

  // Playback: one timer per step, so the scrubber and the timer never disagree.
  useEffect(() => {
    if (!playing) return;
    if (step >= lastStep) {
      setPlaying(false);
      return;
    }
    // Bubble Sort's 47 steps play in about 21s; the fresh-visit run, from the first swap, in about 9s.
    const delay = brisk.current ? 240 : stepCount > 60 ? 220 : 450;
    const timer = window.setTimeout(() => setStepState((s) => Math.min(s + 1, lastStep)), delay);
    return () => window.clearTimeout(timer);
  }, [playing, step, lastStep, stepCount]);

  const current = trace?.steps[Math.min(step, lastStep)] ?? null;
  const previous = step > 0 ? (trace?.steps[Math.min(step, lastStep) - 1] ?? null) : null;

  return useMemo(
    () => ({
      samples: SAMPLES,
      sample,
      source,
      edited: source !== sample.source,
      setSource,
      highlight,
      setHighlight,
      hover,
      setHover,
      caret,
      setCaret,
      nav,
      navigate,
      trace,
      note,
      runs,
      busy,
      step: Math.min(step, lastStep),
      stepCount,
      current,
      previous,
      playing,
      tab,
      setTab,
      setStep,
      pause,
      togglePlay,
      compile,
      chooseSample,
      revert,
    }),
    [sample, source, highlight, hover, caret, nav, navigate, setSource, trace, note, runs, busy, step, lastStep, stepCount, current, previous, playing, tab, setStep, pause, togglePlay, compile, chooseSample, revert],
  );
}

export type Workbench = ReturnType<typeof useWorkbench>;
