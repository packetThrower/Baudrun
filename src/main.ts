import "./style.css";
import { mount } from "svelte";
import App from "./App.svelte";

// Svelte 5 replaced `new App({ target })` with mount(). The bang on
// getElementById is safe — index.html ships the #app div, and we'd
// have bigger problems if it was missing.
const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
