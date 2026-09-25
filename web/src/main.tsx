import { render } from "preact";
import { App } from "./app/App";
import { apiClient } from "./api/client";
import "./styles.css";

render(<App client={apiClient} />, document.getElementById("app")!);
