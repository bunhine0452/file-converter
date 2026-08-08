import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { Button } from "@/components/ui/button";
import type {
  ConversionItem,
  ConversionStatus,
} from "@/hooks/useConversionQueue";
import { cancelJob } from "@/lib/jobs";
import { cn } from "@/lib/utils";

const STATUS_LABEL: Record<ConversionStatus, string> = {
  queued: "대기 중",
  running: "변환 중",
  cancelling: "취소하는 중",
  cancelled: "취소됨",
  completed: "완료",
  failed: "실패",
};

/** 상태별 표시 색. 성공을 회색으로 두면 목록에서 결과를 훑을 수 없다. */
const STATUS_TONE: Record<ConversionStatus, string> = {
  queued: "text-muted-foreground",
  running: "text-accent-strong",
  cancelling: "text-muted-foreground",
  cancelled: "text-muted-foreground",
  completed: "text-success",
  failed: "text-destructive",
};

/** 아직 끝나지 않아 취소할 수 있는 상태. */
const CANCELLABLE: readonly ConversionStatus[] = ["queued", "running"];
/** 더 이상 변하지 않아 목록에서 치워도 되는 상태. */
const FINISHED: readonly ConversionStatus[] = [
  "completed",
  "failed",
  "cancelled",
];

export interface ConversionQueueProps {
  items: ConversionItem[];
  /** 끝난 항목을 목록에서 치운다. 넘기지 않으면 정리 버튼을 그리지 않는다. */
  onClearFinished?: () => void;
}

/** 변환 목록. 진행 중인 것은 멈출 수 있고, 끝난 것은 바로 열어 볼 수 있다. */
export function ConversionQueue({
  items,
  onClearFinished,
}: ConversionQueueProps) {
  if (items.length === 0) return null;

  const hasFinished = items.some((item) => FINISHED.includes(item.status));

  return (
    <section className="flex w-full flex-col gap-3">
      <header className="flex items-baseline justify-between gap-3">
        <h3 className="text-sm font-medium">변환 목록</h3>
        {onClearFinished && hasFinished && (
          <Button
            variant="ghost"
            size="sm"
            className="text-muted-foreground h-auto px-2 py-1 text-xs"
            onClick={onClearFinished}
          >
            끝난 항목 지우기
          </Button>
        )}
      </header>

      <ul aria-label="변환 목록" className="flex w-full flex-col gap-2">
        {items.map((item) => (
          <QueueRow key={item.id} item={item} />
        ))}
      </ul>
    </section>
  );
}

function QueueRow({ item }: { item: ConversionItem }) {
  const isRunning = item.status === "running";

  return (
    <li className="bg-card shadow-raised flex flex-col gap-2 rounded-lg border px-3 py-2.5">
      <div className="flex items-baseline justify-between gap-3">
        <span className="truncate text-sm" title={item.source}>
          {item.name}
        </span>
        <span
          className={cn(
            "shrink-0 text-xs tabular-nums",
            STATUS_TONE[item.status],
          )}
        >
          {STATUS_LABEL[item.status]}
          {isRunning && ` ${item.progress}%`}
        </span>
      </div>

      {/* 막대는 진행 중일 때만 — 끝난 줄에 남아 있으면 아직 도는 것처럼 보인다. */}
      {isRunning && (
        <div
          role="progressbar"
          aria-label={`${item.name} 변환 진행률`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={item.progress}
          className="bg-muted h-1 w-full overflow-hidden rounded-full"
        >
          <div
            className="bg-accent-strong h-full rounded-full transition-[width] duration-[var(--motion-normal)] ease-[var(--ease-out-soft)] motion-reduce:transition-none"
            style={{ width: `${item.progress}%` }}
          />
        </div>
      )}

      {/* 안내는 실패가 아니다 — 변환은 됐지만 결과가 원본과 다를 수 있다는 뜻이다. */}
      {item.note && (
        <p role="status" className="text-muted-foreground text-xs">
          {item.note}
        </p>
      )}

      {item.message && (
        <p className="text-destructive text-xs">{item.message}</p>
      )}

      <div className="flex flex-wrap items-center gap-1">
        {CANCELLABLE.includes(item.status) && (
          <RowAction onClick={() => void cancelJob(item.id)}>
            변환 취소
          </RowAction>
        )}

        {item.status === "completed" && (
          <>
            <RowAction onClick={() => void openPath(item.outPath)}>
              PDF 열기
            </RowAction>
            <RowAction onClick={() => void revealItemInDir(item.outPath)}>
              저장 위치 열기
            </RowAction>
          </>
        )}
      </div>
    </li>
  );
}

function RowAction({
  onClick,
  children,
}: {
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Button
      variant="ghost"
      size="sm"
      className="text-muted-foreground hover:text-foreground h-auto px-2 py-1 text-xs"
      onClick={onClick}
    >
      {children}
    </Button>
  );
}
