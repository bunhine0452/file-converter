import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  cancelJob,
  startDemoJob,
  subscribeToJobEvents,
  type JobEvent,
  type JobId,
} from "@/lib/jobs";

type DemoStatus =
  "idle" | "running" | "cancelling" | "cancelled" | "completed" | "failed";

const STATUS_LABEL: Record<DemoStatus, string> = {
  idle: "대기",
  running: "변환 중",
  cancelling: "취소하는 중",
  cancelled: "취소됨",
  completed: "완료",
  failed: "실패",
};

/** Rust→프론트 진행 이벤트 브리지가 살아 있는지 눈으로 확인하는 데모 카운터. */
export function JobProgressDemo() {
  const [jobId, setJobId] = useState<JobId | null>(null);
  const [progress, setProgress] = useState(0);
  const [status, setStatus] = useState<DemoStatus>("idle");
  const [error, setError] = useState<string | null>(null);

  // 이벤트 핸들러는 구독 시점의 클로저를 쓰므로, 현재 작업 id 는 ref 로 읽는다.
  const jobIdRef = useRef<JobId | null>(null);

  const handleEvent = useCallback((event: JobEvent) => {
    if (jobIdRef.current !== null && event.id !== jobIdRef.current) {
      return;
    }

    switch (event.kind) {
      case "queued":
        setProgress(0);
        break;
      case "progress":
        setProgress(event.progress);
        break;
      case "completed":
        setProgress(100);
        setStatus("completed");
        break;
      case "failed":
        setStatus("failed");
        setError(event.message);
        break;
      case "cancelling":
        setStatus("cancelling");
        break;
      case "cancelled":
        setStatus("cancelled");
        break;
    }
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let subscribed = true;

    subscribeToJobEvents(handleEvent)
      .then((dispose) => {
        // 구독이 끝나기 전에 언마운트됐다면 곧바로 해제한다.
        if (subscribed) {
          unlisten = dispose;
        } else {
          dispose();
        }
      })
      .catch((cause) => setError(String(cause)));

    return () => {
      subscribed = false;
      unlisten?.();
    };
  }, [handleEvent]);

  async function start() {
    setError(null);
    setProgress(0);

    try {
      const id = await startDemoJob();
      jobIdRef.current = id;
      setJobId(id);
      setStatus("running");
    } catch (cause) {
      setStatus("failed");
      setError(String(cause));
    }
  }

  async function cancel() {
    if (jobId === null) return;

    try {
      await cancelJob(jobId);
    } catch (cause) {
      setError(String(cause));
    }
  }

  const isActive = status === "running" || status === "cancelling";

  return (
    <section className="flex w-full max-w-sm flex-col items-center gap-4">
      <div
        role="progressbar"
        aria-label="데모 작업 진행률"
        aria-valuenow={progress}
        aria-valuemin={0}
        aria-valuemax={100}
        className="bg-muted h-2 w-full overflow-hidden rounded-full"
      >
        <div
          className="bg-primary h-full transition-[width] duration-200 motion-reduce:transition-none"
          style={{ width: `${progress}%` }}
        />
      </div>

      <p className="text-muted-foreground flex items-baseline gap-2 text-sm">
        <span data-testid="demo-progress" className="tabular-nums">
          {progress}%
        </span>
        <span>{STATUS_LABEL[status]}</span>
      </p>

      <div className="flex gap-2">
        <Button onClick={start} disabled={isActive}>
          데모 작업 시작
        </Button>
        {isActive && (
          <Button variant="outline" onClick={cancel}>
            취소
          </Button>
        )}
      </div>

      {error && (
        <p role="alert" className="text-destructive text-sm">
          {error}
        </p>
      )}
    </section>
  );
}
