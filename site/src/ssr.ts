import render from "preact-render-to-string";
import { h } from "preact";
import { App } from "./App";
import * as fs from "fs";
import * as path from "path";

const indexHTMLPath = path.join(__dirname, "../dist/index.html");

if (fs.existsSync(indexHTMLPath)) {
  const renderedApp = render(h(App, {}));

  const htmlContent = fs.readFileSync(indexHTMLPath, "utf-8");

  const prerendered = htmlContent.replace(
    '<div id="root"></div>',
    `<div id="root">${renderedApp}</div>`
  );

  fs.writeFileSync(indexHTMLPath, prerendered);
  console.log("Prerendered <App/>!!");
} else {
  console.error(`${indexHTMLPath} doesn't exist`);
}
