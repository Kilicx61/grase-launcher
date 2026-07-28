# Grase Launcher 🚀

Grase Launcher is a modern, ultra-fast, and premium Minecraft launcher built using **Tauri v2**, **Rust**, and glassmorphic **HTML5/CSS3/JS**.

Designed for ultimate performance, simplicity, and flexibility, Grase Launcher offers independent game instances, built-in mod/shaderpack management, automated backups, and instant launch speeds.

---

## Key Features 🌟

*   **Integrated Glassmorphism UI**: Beautiful, premium dark-mode theme with dynamic micro-animations and unified layout.
*   **Instant Start (0ms Caching)**: Built-in local caching for Mojang version manifests, version detail files, and Fabric profile files. Java lookup is optimized to under 1ms.
*   **Isolated Game Instances**: Different Minecraft versions or mod loaders (Vanilla vs. Fabric) are kept in fully isolated directories. Your worlds, mods, and options will never collide.
*   **Built-in Mod, Shader & Resource Pack Manager**: Drag and drop mod files, toggle them on/off dynamically, search and install mods from Modrinth in one click.
*   **Automatic & Manual Backups**: Automatic worlds backup on every game start, running asynchronously in the background so it doesn't delay your game loading.
*   **Responsive Sunucularım (Servers) Widget**: Centered launch form with persistent right-aligned server list widget and server pinger.
*   **Tauri v2 App Update Banner**: Receives updates automatically over GitHub Releases, notifying you with a top bar action button.

---

## Gelişmiş Özellikler (Türkçe) 🇹🇷

*   **Modern Cam Efektli (Glassmorphic) Tasarım**: Koyu tema ile uyumlu, dinamik animasyonlara sahip pürüzsüz arayüz.
*   **Anında Oyun Başlatma (0ms Önbellek)**: Sürüm listeleri ve detayları yerel diske önbelleğe alınır. Çevrimiçi ağ gecikmeleri veya Mojang sunucu yavaşlamaları oyuna girmenizi engellemez.
*   **İzole Profiller (Instance Sistemi)**: Her Minecraft sürümü ve yükleyicisi (Vanilla / Fabric) kendi bağımsız klasöründe çalışır. Modlarınız veya dünyalarınız asla birbirine karışmaz.
*   **Tümleşik İçerik Yöneticisi**: Mod, Shader ve Kaynak Paketlerini tek bir ekrandan yönetin. Modrinth üzerinden tek tıkla arama yapıp mod yükleyin.
*   **Otomatik ve Güvenli Yedekleme**: Her oyuna girişte dünyalarınız arka planda otomatik olarak yedeklenir.
*   **Yerleşik Sunucu Listesi**: Ana ekranın sağ tarafında konumlandırılan, sunucu durumu (Ping) göstergeli widget.

---

## Installation & Build Instructions 🛠️

### Prerequisites
1.  **Rust & Cargo**: Follow instructions at [rustup.rs](https://rustup.rs).
2.  **Node.js & npm**: Install Node.js from [nodejs.org](https://nodejs.org).

### Build Setup

1.  Clone this repository:
    ```bash
    git clone https://github.com/Kilicx61/grase-launcher.git
    cd grase-launcher
    ```

2.  Install frontend dependencies:
    ```bash
    npm install
    ```

3.  Run in Development Mode:
    ```bash
    npm run tauri dev
    ```

4.  Compile Production MSI Installer:
    ```bash
    npm run tauri build
    ```
    The compiled `.msi` file will be generated under `src-tauri/target/release/bundle/msi/`.

---

## License 📄

This project is open-source and available under the MIT License.
