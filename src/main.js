// invoke — функція, якою JS викликає команду, оголошену в Rust (#[tauri::command])
import { invoke } from "@tauri-apps/api/core";

const form = document.querySelector("#greet-form");
const input = document.querySelector("#greet-input");
const message = document.querySelector("#greet-msg");

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  // Назва команди та імена аргументів мають збігатися з функцією greet у src-tauri/src/lib.rs
  message.textContent = await invoke("greet", { name: input.value });
});
