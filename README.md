# Power Interview AI - Privacy-First AI Interview Assistant

<div align="center">

**Your Personal AI-Powered Interview Coach**

🌐 **Website**: [https://www.powerinterviewai.com](https://www.powerinterviewai.com)

[![Version](https://img.shields.io/github/v/release/PowerInterviewAI/client-app-tauri?label=version&color=blue&cacheSeconds=3600)](https://github.com/PowerInterviewAI/client-app-tauri/releases/latest)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

📧 [Email](mailto:team@vectorleappulse.xyz) | 💬 [Telegram](https://t.me/power_interview_ai) | 💭 [Discord](https://discord.gg/TJJp5azK7Z) | 🐦 [X](https://x.com/power_interview)

</div>

## Overview

Power Interview is a privacy-first AI assistant designed to help you ace technical and behavioral interviews. With real-time transcription and intelligent suggestions, you'll have the confidence and support you need during live interviews, all while maintaining your privacy.

## Privacy First

**Your data stays with you.** Power Interview is built with privacy as a core principle:

- **Client-Side Application**: Native Tauri desktop client (Rust + React) for account management and UI
- **Secure Storage**: Credentials and personal info stored locally in a JSON config file managed by the Rust backend
- **AI Processing**: Handled by secure backend services
- **No Data Mining**: No selling or sharing personal data
- **Minimal Data Transfer**: Only necessary data sent for AI suggestions
- **Your Control**: CV, profile, and configs remain on your device

## Key Features

### Real-Time Transcription

Stay on top of the conversation with live ASR:

- Dual-channel transcription (you + interviewer)
- WebSocket streaming for low latency
- Speaker detection
- Full transcript history

### Intelligent AI Suggestions

#### Live Suggestions

- Personalized responses based on CV and job description
- Streaming responses in real time
- Context-aware outputs
- Natural language responses

#### Action Suggestions

- Screenshot-based problem understanding
- Multi-image support (up to 4)
- LLM-powered solutions
- Syntax-highlighted code output

### Smart Configuration

- Profile management (CV, job description, etc.)
- Audio device selection
- Language support (English)
- Persistent settings

## Platform Support

Power Interview desktop client is supported on:

- Windows 11+ (x64 installer build)
- macOS 14.4+ (Apple Silicon and Intel release artifacts)

Release binaries are published on the [GitHub Releases](https://github.com/PowerInterviewAI/client-app-tauri/releases) page.

## Architecture

Power Interview follows a **client-server architecture**.

### Desktop Client

- Tauri (Rust) + React + TypeScript
- Zustand + React Query
- Native Rust backend handles audio capture, OS integrations, and local storage; the React UI handles orchestration and rendering

### Backend Services

- AI/LLM Service for suggestions
- ASR Service for transcription
- Auth Service

### Communication

- WebSocket (real-time)
- REST API

## Getting Started

### Prerequisites

- Node.js 22.x
- pnpm
- Rust toolchain (for Tauri builds)

### Installation

```bash
git clone https://github.com/PowerInterviewAI/client-app-tauri.git
cd client-app-tauri
pnpm install
```

### Run

```bash
pnpm tauri:dev
```

### Configuration

- Set profile (CV, job description)
- Select microphone
- Start assistant

## Use Cases

### Technical Interviews

- Code suggestions
- Debugging assistance
- Live transcription

### Behavioral Interviews

- AI-generated responses
- Context-aware answers

### Practice Sessions

- Self-monitoring
- Feedback loops

## Security & Privacy

- Local, file-based config storage in the OS app data directory
- HTTPS + secure WebSockets
- No external transcript storage
- Full user control

## Technology Stack

### Frontend

- Tauri (WebView)
- React 19
- TypeScript
- Tailwind CSS

### Backend

- Rust (Tauri native backend)
- WebSocket

## Project Structure

```
client-app-tauri/
├── src/            # React frontend
├── src-tauri/      # Rust Tauri backend and native commands
├── public/
```

## Legal Disclaimer

Use for **ethical and legal interview preparation only**.

Users are responsible for complying with all applicable laws and platform policies.

## Contributing

Pull requests welcome.

## License

MIT License

## Support

- Email: [team@vectorleappulse.xyz](mailto:team@vectorleappulse.xyz)
- GitHub Issues for bugs/features

---

<div align="center">

**Built to help you succeed in interviews**

</div>

---
