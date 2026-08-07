import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  JOB_EVENT,
  cancelJob,
  subscribeToJobEvents,
  type JobEvent,
} from "./jobs";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

describe("jobs 브리지", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockReset();
  });

  it("코어와 같은 이벤트 이름을 구독한다", async () => {
    const unlisten = vi.fn(() => {});
    vi.mocked(listen).mockResolvedValue(unlisten);

    await subscribeToJobEvents(() => {});

    expect(JOB_EVENT).toBe("job://event");
    expect(vi.mocked(listen).mock.calls[0][0]).toBe(JOB_EVENT);
  });

  it("Tauri 이벤트 payload 만 꺼내 핸들러에 넘긴다", async () => {
    const handler = vi.fn();
    const received: JobEvent = { kind: "progress", id: 1, progress: 40 };
    vi.mocked(listen).mockImplementation(async (_name, callback) => {
      (callback as (event: { payload: JobEvent }) => void)({
        payload: received,
      });
      return () => {};
    });

    await subscribeToJobEvents(handler);

    expect(handler).toHaveBeenCalledWith(received);
  });

  it("구독 해제 함수를 그대로 돌려준다", async () => {
    const unlisten = vi.fn(() => {});
    vi.mocked(listen).mockResolvedValue(unlisten);

    const result = await subscribeToJobEvents(() => {});

    expect(result).toBe(unlisten);
  });

  it("취소는 id 를 인자로 cancel_job 커맨드를 호출한다", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await cancelJob(3);

    expect(invoke).toHaveBeenCalledWith("cancel_job", { id: 3 });
  });
});
