import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Rust 코어(`core::events::JOB_EVENT`)와 반드시 같은 값이어야 한다. */
export const JOB_EVENT = "job://event";

export type JobId = number;

/** Rust `JobEvent` 의 serde 표현(`kind` 태그 + camelCase)을 그대로 옮긴 타입. */
export type JobEvent =
  | { kind: "queued"; id: JobId; source: string }
  | { kind: "progress"; id: JobId; progress: number }
  | { kind: "completed"; id: JobId }
  | { kind: "failed"; id: JobId; message: string }
  | { kind: "cancelling"; id: JobId }
  | { kind: "cancelled"; id: JobId };

/** 작업 이벤트를 구독한다. 반환된 함수를 호출하면 구독이 해제된다. */
export function subscribeToJobEvents(
  handler: (event: JobEvent) => void,
): Promise<UnlistenFn> {
  return listen<JobEvent>(JOB_EVENT, (event) => handler(event.payload));
}

export function cancelJob(id: JobId): Promise<void> {
  return invoke("cancel_job", { id });
}
