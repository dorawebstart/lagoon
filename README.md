# Lagoon 🏝️

> 🇺🇦 [Українська](#українська) · 🇬🇧 [English](#english)

> ⚠️ **Стан: ранній розвиток.** Застосунку ще немає, репозиторій щойно створено. / **Status: early development**, nothing to install yet.

---

## Українська

**Lagoon** — це «Dynamic Island» для Windows: маленька чорна пігулка по центру верхнього краю екрана, яка розкривається при наведенні і показує корисне:

- 🕒 годинник;
- 🎵 що зараз грає (Spotify, YouTube у браузері та інші програми з медіапанеллю Windows) з кнопками ⏮ ⏯ ⏭;
- 🤖 статус **Claude Code** і кнопки «Дозволити / Відхилити» для запитів дозволу прямо з острова;
- 🌐 інтерфейс українською та англійською.

Зроблено на **Tauri 2** (Rust) + HTML/CSS/JavaScript.

### Скриншот

_З'явиться після першої робочої версії._

### Встановлення

Готові інсталятори (`.msi` / `.exe`) з'являться на сторінці [Releases](https://github.com/dorawebstart/lagoon/releases) починаючи з `v0.1.0`.

### Збірка з коду

Потрібно: [Rust](https://rustup.rs) (`stable-x86_64-pc-windows-msvc`), Microsoft C++ Build Tools («Desktop development with C++»), Node.js LTS, WebView2.

```bash
git clone https://github.com/dorawebstart/lagoon.git
cd lagoon
npm install
npm run tauri dev     # запуск у режимі розробки
npm run tauri build   # інсталятор
```

### Підключення Claude Code

_Інструкція з налаштування hooks з'явиться разом з кроком 9 плану._

### Подяка

Ідея навіяна концепцією «динамічного острова» та відео про подібні застосунки для Windows. Код, назва і графіка цього проєкту — власні.

---

## English

**Lagoon** is a "Dynamic Island" for Windows: a small black pill at the top center of the screen that expands on hover and shows:

- 🕒 a clock;
- 🎵 what's playing (Spotify, YouTube in a browser and other apps that integrate with Windows media controls) with ⏮ ⏯ ⏭ buttons;
- 🤖 **Claude Code** status and Allow / Deny buttons for permission requests, right from the island;
- 🌐 Ukrainian and English UI.

Built with **Tauri 2** (Rust) + HTML/CSS/JavaScript.

### Screenshot

_Coming with the first working version._

### Install

Installers (`.msi` / `.exe`) will be published on the [Releases](https://github.com/dorawebstart/lagoon/releases) page starting with `v0.1.0`.

### Build from source

Requirements: [Rust](https://rustup.rs) (`stable-x86_64-pc-windows-msvc`), Microsoft C++ Build Tools ("Desktop development with C++"), Node.js LTS, WebView2.

```bash
git clone https://github.com/dorawebstart/lagoon.git
cd lagoon
npm install
npm run tauri dev     # development mode
npm run tauri build   # installer
```

### Connecting Claude Code

_Hook setup instructions will be added in step 9 of the plan._

### Credits

Inspired by the "dynamic island" concept and videos about similar Windows apps. All code, naming and artwork in this project are original.

## License

[MIT](LICENSE)
