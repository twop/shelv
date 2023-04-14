import { h, hydrate } from "preact";

import "tailwindcss/dist/tailwind.css";
import "./index.css";
import "nord-highlightjs/dist/nord.css";

import { App } from "./App";

hydrate(<App />, document.getElementById("root")!);
