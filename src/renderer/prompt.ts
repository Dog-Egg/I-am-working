import "./app.css";
import Prompt from "./Prompt.svelte";
import { mount } from "svelte";

const target = document.getElementById("prompt");

if (!target) {
  throw new Error("Missing prompt root.");
}

mount(Prompt, { target });
