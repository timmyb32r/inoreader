import { render } from "preact";
import { apiClient } from "./api/client";
import { App } from "./app/App";
import { startPerformanceDiagnostics } from "./performanceDiagnostics";
import "./styles.css";

startPerformanceDiagnostics();
render(<App client={apiClient} />, document.getElementById("app")!);
