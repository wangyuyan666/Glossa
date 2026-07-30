import React from "react";
import ReactDOM from "react-dom/client";

import { installImeClickRecovery } from "./lib/imeClick";
import { Main } from "./main/Main";
import { Settings } from "./settings/Settings";
import "./global.css";

// 两个窗口都要装：设置页里「输入框打完字点保存」和主窗口的齿轮是同一个坑。
installImeClickRecovery();

// 两个窗口共用一份构建产物，靠 tauri.conf.json 里配的 `?w=` 参数分流。
const which = new URLSearchParams(window.location.search).get("w");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{which === "settings" ? <Settings /> : <Main />}</React.StrictMode>,
);
