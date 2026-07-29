import React from "react";
import ReactDOM from "react-dom/client";

import { Main } from "./main/Main";
import { Popup } from "./popup/Popup";
import { Settings } from "./settings/Settings";
import "./global.css";

// 三个窗口共用一份构建产物，靠 tauri.conf.json 里配的 `?w=` 参数分流。
const which = new URLSearchParams(window.location.search).get("w");

function Window() {
  switch (which) {
    case "settings":
      return <Settings />;
    case "popup":
      return <Popup />;
    default:
      return <Main />;
  }
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Window />
  </React.StrictMode>,
);
