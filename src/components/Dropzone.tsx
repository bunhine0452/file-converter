import { useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { HWP_PATTERN, useFileDrop } from "@/hooks/useFileDrop";
import { cn } from "@/lib/utils";

export interface DropzoneProps {
  onFiles: (paths: string[]) => void;
  disabled?: boolean;
}

const HINT = {
  idle: "한글 문서(.hwp, .hwpx)를 여기에 놓으세요",
  valid: "놓으면 변환을 시작합니다",
  invalid: "한글 문서(.hwp, .hwpx)만 변환할 수 있습니다",
} as const;

/**
 * 드롭 영역. Tauri 네이티브 드롭만 받는다 — HTML5 drop 이벤트는 발화하지 않으므로
 * 하이라이트도 CSS 가 아니라 훅이 준 상태로 그린다.
 */
export function Dropzone({ onFiles, disabled = false }: DropzoneProps) {
  const handleFiles = useCallback(
    (paths: string[]) => {
      if (disabled) return;
      onFiles(paths);
    },
    [disabled, onFiles],
  );

  const { isOver, isInvalid } = useFileDrop({
    accept: HWP_PATTERN,
    onFiles: handleFiles,
  });

  async function pickFiles() {
    const picked = await open({
      title: "변환할 한글 문서 선택",
      multiple: true,
      directory: false,
      filters: [{ name: "한글 문서", extensions: ["hwp", "hwpx"] }],
    });

    // 사용자가 취소하면 null 이 온다.
    if (picked === null) return;

    onFiles(Array.isArray(picked) ? picked : [picked]);
  }

  const state = isInvalid ? "invalid" : isOver ? "valid" : "idle";

  return (
    <section
      aria-label="한글 문서 드롭 영역"
      data-state={state}
      className={cn(
        "flex w-full flex-col items-center justify-center gap-4 rounded-xl border-2 border-dashed p-12 transition-colors motion-reduce:transition-none",
        state === "idle" && "border-border",
        state === "valid" && "border-primary bg-primary/5",
        state === "invalid" && "border-destructive bg-destructive/5",
        disabled && "opacity-50",
      )}
    >
      <p
        role="status"
        className={cn(
          "text-sm",
          state === "invalid" ? "text-destructive" : "text-muted-foreground",
        )}
      >
        {HINT[state]}
      </p>

      <Button variant="outline" onClick={pickFiles} disabled={disabled}>
        파일 선택
      </Button>
    </section>
  );
}
