# SmartBridge Setup

[Download SmartBridge Setup](https://github.com/claudioreyesdeveloper/smartbridge-setup/releases/latest)

Choose the download for your computer: Apple Silicon macOS 14 or newer (`.dmg`) or Windows x64 (`.exe`). Open Setup and start installation. Beta requires your email and serial; Demo provides the existing 30-day trial.

Setup installs SmartBridge, its library, the current Cubase template, and connections for supported music applications it detects.

## Local AI

- **AI Melodies:** Setup installs a private Python runtime, Composer’s Assistant and SkyTNT automatically. You do not need to install Python or run terminal commands.
- **AI Lyrics (Ollama): optional and off by default.** Select AI Lyrics in Setup if you want it. Only then does Setup install Ollama if missing and download the lyrics models. Leave it unselected to skip those downloads.

An internet connection is required for the initial downloads. Setup checks download integrity and loads both melody models before activating the new runtime.

Application packages and supporting files are published in [SmartBridge releases](https://github.com/claudioreyesdeveloper/smartbridge-releases/releases/latest).
