import { useCallback, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { CategoryNav, type ConversionCategory } from "@/components/CategoryNav";
import { ConversionQueue } from "@/components/ConversionQueue";
import { Dropzone } from "@/components/Dropzone";
import { RuntimeStatus } from "@/components/RuntimeStatus";
import { useConversionQueue } from "@/hooks/useConversionQueue";
import { convertHwp, planOutputPath } from "@/lib/runtime";

/** `.hwp` / `.hwpx` 를 `.pdf` 로 바꾼 기본 저장 이름. */
function defaultOutputPath(source: string): string {
  return source.replace(/\.(hwp|hwpx)$/i, ".pdf");
}

function App() {
  const { items, track, clearFinished } = useConversionQueue();
  const [category, setCategory] = useState<ConversionCategory>("document");

  const startOne = useCallback(
    async (source: string, outPath: string) => {
      const id = await convertHwp(source, outPath);
      track(id, source, outPath);
    },
    [track],
  );

  /**
   * 한 건은 저장 위치를 묻고, 여러 건은 폴더를 **한 번만** 묻는다.
   *
   * 파일마다 대화상자를 띄우면 10개를 드롭한 사용자는 10번 답해야 한다.
   * 폴더 저장은 덮어쓰기 동의를 받은 적이 없으므로 이름은 코어가 겹치지 않게 정한다.
   */
  const handleFiles = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0) return;

      if (paths.length === 1) {
        const outPath = await save({
          title: "PDF 저장 위치",
          defaultPath: defaultOutputPath(paths[0]),
          filters: [{ name: "PDF 문서", extensions: ["pdf"] }],
        });

        // 사용자가 취소하면 변환하지 않는다.
        if (outPath === null) return;

        await startOne(paths[0], outPath);
        return;
      }

      const dir = await open({
        title: `PDF ${paths.length}개를 저장할 폴더`,
        directory: true,
        multiple: false,
      });

      if (dir === null) return;

      for (const source of paths) {
        await startOne(source, await planOutputPath(source, dir));
      }
    },
    [startOne],
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
