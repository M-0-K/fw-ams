import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import Flyout from "./Flyout";
import { applyCachedTheme } from "./theme";
import "./styles.css";

const view = new URLSearchParams(window.location.search).get("view");

applyCachedTheme();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>{view === "flyout" ? <Flyout /> : <App />}</React.StrictMode>,
);
