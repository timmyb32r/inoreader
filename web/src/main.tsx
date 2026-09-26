import { render } from "preact";
import { App } from "./app/App";
import { apiClient } from "./api/client";
import { startPerformanceDiagnostics } from "./performanceDiagnostics";
import "./styles.css";

startPerformanceDiagnostics();
render(<App client={apiClient} />, document.getElementById("app")!);
