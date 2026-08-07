import { useCallback } from "react";
import { save } from "@tauri-apps/plugin-dialog";
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
  const { items, track } = useConversionQueue();

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
    <main className="mx-auto flex min-h-svh w-full max-w-2xl flex-col gap-6 p-8">
      <header className="flex flex-col gap-1">
        <h1 className="text-3xl font-semibold tracking-tight">파일 변환기</h1>
        <p className="text-muted-foreground text-sm">
          파일은 이 기기를 떠나지 않습니다. 모든 변환은 로컬에서 실행됩니다.
        </p>
      </header>

      <RuntimeStatus />
      <Dropzone onFiles={(paths) => void handleFiles(paths)} />
      <ConversionQueue items={items} />
    </main>
  );
}

export default App;
