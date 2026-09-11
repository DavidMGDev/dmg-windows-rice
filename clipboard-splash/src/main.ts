import { mount } from "svelte";
import App from "./App.svelte";
import Tip from "./Tip.svelte";
import "./app.css";

// Two windows, one page: the Claude Mode label is the same bundle asked for a
// different root, which beats a second Vite entry for thirty lines of markup.
const tip = location.search.includes("tip");
if (tip) document.documentElement.classList.add("tip");
const Root = tip ? Tip : App;

export default mount(Root, { target: document.getElementById("app")! });
