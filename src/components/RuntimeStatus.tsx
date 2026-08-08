import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  getRuntimeStatus,
  installRuntime,
  progressRatio,
  RUNTIME_STATE_MESSAGE,
  type InstallEvent,
  type RuntimeStatusView,
} from "@/lib/runtime";
import { cn } from "@/lib/utils";

/** 설치를 권할 수 있는 상태 — `unsupported` 는 사용자가 할 수 있는 게 없다. */
const INSTALLABLE = new Set([
  "needsLibreOffice",
  "needsJre",
  "needsExtension",
  "needsFonts",
]);

interface Progress {
  step: string;
  ratio: number | null;
}

/**
 * 변환 런타임(LibreOffice·JRE·한글 확장) 상태와 설치 진입점.
 *
 * 실패를 조용히 넘기지 않는다 — 상태를 못 읽었으면 그렇다고 말한다.
 */
export function RuntimeStatus() {
  const [status, setStatus] = useState<RuntimeStatusView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [isInstalling, setIsInstalling] = useState(false);

  // 최초 조회. setState 는 반드시 비동기 콜백 안에서 — 이펙트 본문에서 부르면
  // 렌더가 연쇄로 돈다. 언마운트 후 상태를 건드리지 않도록 플래그로 막는다.
  useEffect(() => {
    let active = true;

    getRuntimeStatus(false)
      .then((next) => {
        if (active) setStatus(next);
      })
      .catch((cause: unknown) => {
        if (active)
          setError(cause instanceof Error ? cause.message : String(cause));
      });

    return () => {
      active = false;
    };
  }, []);

  function handleEvent(event: InstallEvent) {
    switch (event.kind) {
      case "started":
      case "stepDone":
        setProgress({ step: event.step, ratio: null });
        break;
      case "progress":
        setProgress({
          step: event.step,
          ratio: progressRatio(event.received, event.total),
        });
        break;
      case "finished":
        setProgress(null);
        break;
      case "failed":
        setError(event.message);
        setProgress(null);
        break;
    }
  }

  async function handleInstall() {
    setIsInstalling(true);
    setError(null);
    try {
      setStatus(await installRuntime(handleEvent));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setIsInstalling(false);
      setProgress(null);
    }
  }

  const canInstall = status !== null && INSTALLABLE.has(status.state);

  return (
    <section
      aria-label="변환 런타임 상태"
      className="flex w-full flex-col gap-3 rounded-lg border p-4"
    >
      <p
        role="status"
        className={cn(
          "text-sm",
          error ? "text-destructive" : "text-muted-foreground",
        )}
      >
        {error
          ? `상태를 확인하지 못했습니다 — ${error}`
          : status
            ? RUNTIME_STATE_MESSAGE[status.state]
            : "상태를 확인하는 중…"}
      </p>

      {status?.version && (
        <p className="text-muted-foreground text-xs tabular-nums">
          LibreOffice {status.version}
          {status.managed ? " (앱이 설치)" : " (시스템에 설치된 LibreOffice)"}
        </p>
      )}

      {progress && (
        <div className="flex flex-col gap-1">
          <p className="text-muted-foreground text-xs">{progress.step}</p>
          <div
            role="progressbar"
            aria-label={progress.step}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={
              progress.ratio === null
                ? undefined
                : Math.round(progress.ratio * 100)
            }
            className="bg-muted h-1.5 w-full overflow-hidden rounded-full"
          >
            <div
              className="bg-primary h-full transition-[width] motion-reduce:transition-none"
              style={{
                width:
                  progress.ratio === null
                    ? "100%"
                    : `${Math.round(progress.ratio * 100)}%`,
              }}
            />
          </div>
        </div>
      )}

      {canInstall && (
        <Button onClick={handleInstall} disabled={isInstalling}>
          {isInstalling ? "설치 중…" : "지금 설치"}
        </Button>
      )}
    </section>
  );
}
