import { useCallback, useEffect, useRef, useState } from "react";
import { subscribeToJobEvents, type JobEvent, type JobId } from "@/lib/jobs";

export type ConversionStatus =
  "queued" | "running" | "cancelling" | "cancelled" | "completed" | "failed";

export interface ConversionItem {
  id: JobId;
  source: string;
  outPath: string;
  /** 목록에 보여줄 이름 (경로 전체가 아니라 파일명) */
  name: string;
  status: ConversionStatus;
  progress: number;
  message: string | null;
  /** 변환을 막지는 않지만 결과물이 원본과 다를 수 있다는 안내 (배포용 문서 등). */
  note: string | null;
}

const PROGRESS_MAX = 100;

/** 경로 구분자는 OS 마다 다르다 — 둘 다 끊는다. */
function fileNameOf(path: string): string {
  const segments = path.split(/[\\/]/);

  return segments[segments.length - 1] || path;
}

function applyEvent(item: ConversionItem, event: JobEvent): ConversionItem {
  switch (event.kind) {
    case "progress":
      return { ...item, status: "running", progress: event.progress };
    case "completed":
      return { ...item, status: "completed", progress: PROGRESS_MAX };
    case "failed":
      return { ...item, status: "failed", message: event.message };
    // 안내는 통지일 뿐 — 상태·진행률을 건드리지 않고 완료 후에도 남는다.
    case "note":
      return { ...item, note: event.message };
    case "cancelling":
      return { ...item, status: "cancelling" };
    case "cancelled":
      return { ...item, status: "cancelled" };
    case "queued":
      return item;
  }
}

/**
 * 변환 작업 목록. Rust 코어가 보내는 작업 이벤트를 받아 상태를 갱신한다.
 *
 * 이벤트에는 저장 경로가 실려 오지 않으므로, 변환을 시작한 쪽이 [`track`] 으로
 * 먼저 등록해야 목록에 나타난다 (등록되지 않은 id 의 이벤트는 무시한다).
 */
export function useConversionQueue() {
  const [items, setItems] = useState<ConversionItem[]>([]);
  const isMountedRef = useRef(true);
  /// 커맨드가 id 를 돌려주기 전에 도착한 이벤트를 잠시 담아 둔다 —
  /// 즉시 실패하는 변환(암호 문서 등)은 등록보다 이벤트가 먼저 온다.
  const pendingRef = useRef<Map<JobId, JobEvent[]>>(new Map());

  const track = useCallback((id: JobId, source: string, outPath: string) => {
    const buffered = pendingRef.current.get(id) ?? [];
    pendingRef.current.delete(id);

    const initial: ConversionItem = {
      id,
      source,
      outPath,
      name: fileNameOf(source),
      status: "queued",
      progress: 0,
      message: null,
      note: null,
    };

    setItems((current) => [...current, buffered.reduce(applyEvent, initial)]);
  }, []);

  useEffect(() => {
    isMountedRef.current = true;
    let unlisten: (() => void) | undefined;

    void subscribeToJobEvents((event) => {
      setItems((current) => {
        if (!current.some((item) => item.id === event.id)) {
          const buffered = pendingRef.current.get(event.id) ?? [];
          pendingRef.current.set(event.id, [...buffered, event]);
          return current;
        }

        return current.map((item) =>
          item.id === event.id ? applyEvent(item, event) : item,
        );
      });
    }).then((dispose) => {
      if (isMountedRef.current) {
        unlisten = dispose;
      } else {
        dispose();
      }
    });

    return () => {
      isMountedRef.current = false;
      unlisten?.();
    };
  }, []);

  return { items, track };
}
