import React from "react";
import ReactDOM from "react-dom/client";

import { Popup } from "./popup/Popup";
import { Settings } from "./settings/Settings";
import "./global.css";

// 两个窗口共用一份构建产物，靠 tauri.conf.json 里配的 `?w=` 参数分流。
const which = new URLSearchParams(window.location.search).get("w");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{which === "settings" ? <Settings /> : <Popup />}</React.StrictMode>,
);
