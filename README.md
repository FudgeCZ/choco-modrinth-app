# ChocoModrinth

A fork of the [Modrinth App](https://github.com/modrinth/code) — a fast, cross-platform Minecraft launcher — with a chocolate theme and extra power features.

Based on upstream release **v0.20.0**.

## Features

All Modrinth App features (Modrinth login, modpack browsing and installing, mod management, instances, skins, screenshots) plus:

### 🍫 ChocoModrinth branding

- Custom chocolate-bar logo and icons
- Cocoa-brown accent color by default
- **Accent color picker** in Settings → Appearance: switch between chocolate, green, blue, red, purple, orange and pink — the logo and all UI follow the chosen color

### 🗜️ Profile compression (7z)

- **Compress profile (7z)** action in the instance page menu and the library right-click menu
- Maximum LZMA2 compression via the 7z format (pure Rust, no external tools needed)
- **Screenshots stay visible while compressed** — the screenshots folder is kept outside the archive, so the in-app gallery keeps working
- Launching a compressed profile **auto-extracts it** through the normal launch flow, and the profile is **compressed again automatically when the game exits**

### 🖥️ Local server creator

- Create local Minecraft servers from inside the launcher: pick a name, Minecraft version, loader, RAM, port, MOTD, difficulty, game mode, max players and online mode
- **Two creation modes**: from scratch, or **from an existing profile** — inherits the profile's version/loader, lets you pick one of its singleplayer saves as the server world, copies mods (automatically excluding client-only mods via Modrinth metadata) and the config folder, reuses the profile icon, and stays linked for one-click "Sync from profile" updates
- Automatic downloads for **Vanilla, Fabric, Quilt, Forge, NeoForge, Paper and Purpur** (loader jars and installers, with the required Java runtime installed automatically)
- Generates `eula.txt` (explicit EULA acceptance checkbox), `server.properties` and start scripts
- **Run / stop servers from the launcher** with a live console view
- Find it under the **Local servers** entry in the sidebar

### 📥 Profile import

- Imports profiles from the **official Modrinth App** directly (shown automatically in the "Import from launcher" flow when it's installed), alongside MultiMC, Prism, ATLauncher, GDLauncher and CurseForge

## Development

Same toolchain as upstream:

- Node.js >= 24.15 (see `.nvmrc`) and pnpm 10 (`corepack enable`)
- Rust toolchain (see `rust-toolchain.toml`; rustup installs the pinned version automatically)

```bash
pnpm install
pnpm app:dev   # run the desktop app in dev mode
pnpm app:build # build the Windows NSIS installer
```

The Windows installer is produced at `apps/app/target/release/bundle/nsis/`.

## Credits & license

- Upstream: [Modrinth/code](https://github.com/modrinth/code) — all code is licensed **GPL-3.0**, as required by the upstream license. Modrinth branding assets have been replaced as required by the upstream [copying guidelines](COPYING.md).
- Some upstream artwork (e.g. Rinthbot mascots in a few empty states) may still appear; all primary branding (name, logo, icons, colors) has been replaced.
