import { JobProgressDemo } from "@/components/JobProgressDemo";

function App() {
  return (
    <main className="flex min-h-svh flex-col items-center justify-center gap-6 p-8">
      <h1 className="text-3xl font-semibold tracking-tight">파일 변환기</h1>
      <p className="text-muted-foreground text-sm">
        파일은 이 기기를 떠나지 않습니다. 모든 변환은 로컬에서 실행됩니다.
      </p>
      <JobProgressDemo />
    </main>
  );
}

export default App;
