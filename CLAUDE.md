# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Sendme is a cross-platform Flutter application for peer-to-peer file transfer using the iroh networking library. It allows users to send files and directories over the internet with NAT hole punching and blake3 verified streaming. The application combines a Flutter frontend with Rust backend functionality using flutter_rust_bridge for FFI communication.

## Development Commands

### Building and Running
- `flutter run -d macos` - Run the app on macOS (most common for development)
- `flutter run -d ios` - Run on iOS simulator/device
- `flutter run -d android` - Run on Android emulator/device
- `flutter run -d windows` - Run on Windows
- `flutter run -d linux` - Run on Linux
- `flutter run -d web` - Run as web application

### Rust Backend Development
- `cd rust && cargo check` - Check Rust code for compilation errors
- `cd rust && cargo build` - Build the Rust library
- `cd rust && cargo run --bin sendme` - Run the original CLI version (reference)
- `flutter_rust_bridge_codegen generate` - Generate FFI bindings after Rust changes

### Code Quality
- `flutter analyze` - Analyze Dart code for issues
- `flutter test` - Run Flutter tests
- `flutter doctor` - Check development environment

### Dependencies
- `flutter pub get` - Install Flutter dependencies
- `cd rust && cargo update` - Update Rust dependencies

## Architecture Overview

### High-Level Architecture
This is a hybrid Flutter/Rust application with the following key components:

1. **Flutter Frontend** (`lib/`): Material Design 3 UI with Provider state management
2. **Rust Backend** (`rust/`): Core file transfer logic using iroh P2P networking
3. **FFI Bridge**: flutter_rust_bridge generates Dart bindings for Rust functions
4. **Global State Management**: SendmeProvider handles async operations and UI state

### Key Architectural Patterns

#### P2P File Transfer Flow
- **Sender**: Prepares files, generates hash, creates network endpoint, produces ticket
- **Receiver**: Connects to sender using ticket, downloads files with integrity verification
- **Data Transfer**: Uses iroh's NAT hole punching and blake3 hash verification

#### State Management
- `SendmeProvider` extends `ChangeNotifier` for reactive UI updates
- Progress tracking uses simulated timers aligned with actual Rust operations
- Error handling with user-friendly messages

#### FFI Integration
- Rust functions marked with `#[flutter_rust_bridge::frb]` are exposed to Dart
- Complex data structures (SendResult, ReceiveResult, ProgressInfo) are serialized
- Global state (`SENDME_STATE`) keeps network connections alive

### File Structure
- `lib/main.dart` - Main UI with SendTab and ReceiveTab
- `lib/src/sendme_provider.dart` - State management and async operations
- `lib/src/rust/` - Generated FFI bindings (auto-generated)
- `rust/src/sendme_core.rs` - Core file transfer logic
- `rust/src/api/sendme.rs` - Public API exposed to Flutter
- `flutter_rust_bridge.yaml` - Bridge configuration

### Important Implementation Details

#### Network Configuration
- Uses `iroh` library with default relay mode for NAT traversal
- Creates temporary storage directories for each transfer session
- Handles connection timeouts gracefully

#### Progress System
- Flutter simulates progress based on Rust operation stages
- Progress messages are in Chinese for better UX
- Timer-based updates every 300ms for smooth UI feedback

#### Error Handling
- Rust `anyhow::Result` types convert to Dart exceptions
- Provider catches and displays user-friendly error messages
- Network timeouts and connection issues are handled gracefully

### Development Notes

#### When Modifying Rust Code
1. Make changes to `rust/src/` files
2. Run `flutter_rust_bridge_codegen generate` to update bindings
3. Build Rust library with `cd rust && cargo build`
4. Restart Flutter app to load new bindings

#### Common Issues
- **Network Timeouts**: App may get stuck during endpoint initialization
- **macOS Build**: Requires SystemConfiguration framework linking
- **Hot Reload**: Rust changes require full app restart, not just hot reload

#### Testing Strategy
- Test file transfer with different file types and sizes
- Verify NAT traversal works across different networks
- Test both sending and receiving workflows
- Ensure progress bars update smoothly