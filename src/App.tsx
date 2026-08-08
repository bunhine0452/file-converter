import { useCallback, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { CategoryNav, type ConversionCategory } from "@/components/CategoryNav";
import { ConversionQueue } from "@/components/ConversionQueue";
import { Dropzone } from "@/components/Dropzone";
import { RuntimeStatus } from "@/components/RuntimeStatus";
import { useConversionQueue } from "@/hooks/useConversionQueue";
import { convertHwp } from "@/lib/runtime";

/** `.hwp` / `.hwpx` 를 `.pdf` 로 바꾼 기본 저장 이름. */
function defaultOutputPath(source: string): string {
  return source.replace(/\.(hwp|hwpx)$/i, ".pdf");
}

function App() {
  const { items, track, clearFinished } = useConversionQueue();
  const [category, setCategory] = useState<ConversionCategory>("document");

  const handleFiles = useCallback(
    async (paths: string[]) => {
      for (const source of paths) {
        const outPath = await save({
          title: "PDF 저장 위치",
          defaultPath: defaultOutputPath(source),
          filters: [{ name: "PDF 문서", extensions: ["pdf"] }],
        });

        // 사용자가 취소하면 이 파일은 건너뛴다.
        if (outPath === null) continue;

        const id = await convertHwp(source, outPath);
        track(id, source, outPath);
      }
    },
    [track],
  );

  return (
    <div className="grid min-h-svh grid-cols-1 sm:grid-cols-[13.5rem_1fr]">
      {/* 사이드바 — 좁은 창에서는 위쪽 가로 목록으로 접힌다. */}
      <aside className="bg-muted/40 flex flex-col gap-4 border-b p-4 sm:border-r sm:border-b-0 sm:p-5">
        <div className="flex flex-col gap-0.5">
          <h1 className="text-sm font-semibold tracking-tight">파일 변환기</h1>
          <span className="text-muted-foreground text-xs">
            기기 안에서만 변환합니다
          </span>
        </div>

        <CategoryNav active={category} onSelect={setCategory} />

        <div className="mt-auto">
          <RuntimeStatus />
        </div>
      </aside>

      <main className="flex flex-col gap-6 p-6 sm:p-8">
        <header className="flex flex-col gap-1">
          <h2 className="text-2xl font-semibold tracking-tight">
            한글 문서를 PDF 로
          </h2>
          <p className="text-muted-foreground text-sm">
            파일은 이 기기를 떠나지 않습니다. 변환은 전부 로컬에서 실행됩니다.
          </p>
        </header>

        <Dropzone onFiles={(paths) => void handleFiles(paths)} />

        <ConversionQueue items={items} onClearFinished={clearFinished} />
      </main>
    </div>
  );
}

export default App;
