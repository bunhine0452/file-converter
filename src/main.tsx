import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { applyTheme, prefersDark, resolveTheme } from "./lib/theme";
import "./index.css";

// 설정을 읽어 오기 전에 일단 OS 를 따른다 — 다크 사용자가 첫 프레임에 흰 화면을
// 맞는 것을 막는다. 저장된 설정이 도착하면 useSettings 가 바로잡는다.
applyTheme(resolveTheme("system", prefersDark()));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
