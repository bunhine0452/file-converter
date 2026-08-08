import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { Button } from "@/components/ui/button";
import type {
  ConversionItem,
  ConversionStatus,
} from "@/hooks/useConversionQueue";
import { cn } from "@/lib/utils";

const STATUS_LABEL: Record<ConversionStatus, string> = {
  queued: "대기 중",
  running: "변환 중",
  cancelling: "취소하는 중",
  cancelled: "취소됨",
  completed: "완료",
  failed: "실패",
};

export interface ConversionQueueProps {
  items: ConversionItem[];
}

/** 변환 목록. 완료된 항목은 결과 위치를 열어 볼 수 있다. */
export function ConversionQueue({ items }: ConversionQueueProps) {
  if (items.length === 0) return null;

  return (
    <ul aria-label="변환 목록" className="flex w-full flex-col gap-2">
      {items.map((item) => (
        <li
          key={item.id}
          className="flex flex-col gap-1 rounded-md border px-3 py-2"
        >
          <div className="flex items-baseline justify-between gap-3">
            <span className="truncate text-sm">{item.name}</span>
            <span
              className={cn(
                "shrink-0 text-xs tabular-nums",
                item.status === "failed"
                  ? "text-destructive"
                  : "text-muted-foreground",
              )}
            >
              {STATUS_LABEL[item.status]}
              {item.status === "running" && ` ${item.progress}%`}
            </span>
          </div>

          {/* 안내는 실패가 아니다 — 변환은 됐지만 결과가 원본과 다를 수 있다는 뜻이다. */}
          {item.note && (
            <p role="status" className="text-muted-foreground text-xs">
              {item.note}
            </p>
          )}

          {item.message && (
            <p className="text-destructive text-xs">{item.message}</p>
          )}

          {item.status === "completed" && (
            <Button
              variant="link"
              size="sm"
              className="h-auto self-start p-0"
              onClick={() => revealItemInDir(item.outPath)}
            >
              저장 위치 열기
            </Button>
          )}
        </li>
      ))}
    </ul>
  );
}
