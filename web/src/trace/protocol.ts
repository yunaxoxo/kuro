/** Messages from the execution worker to the main thread. */
export type WorkerMessage =
  | { type: "print"; text: string }
  | { type: "snapshot"; label: string; cells: number[] }
  | { type: "done"; returned: number | null; trap: string | null; dropped: number };
